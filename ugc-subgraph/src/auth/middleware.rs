use crate::auth::{AuthError, AuthService, UserContext};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, instrument, warn};

/// Authentication middleware for Axum
/// 
/// This middleware extracts and validates JWT tokens from the Authorization header,
/// and adds the user context to the request extensions.
#[instrument(skip(auth_service, headers, request, next))]
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Processing authentication middleware");
    
    let user_context = match extract_user_context(&auth_service, &headers).await {
        Ok(context) => context,
        Err(auth_error) => {
            warn!("Authentication failed: {}", auth_error);
            
            // For some endpoints, we might want to allow anonymous access
            // In that case, we add an anonymous user context
            if should_allow_anonymous(&request) {
                debug!("Allowing anonymous access");
                UserContext::anonymous()
            } else {
                return Err(StatusCode::from(auth_error));
            }
        }
    };
    
    debug!(
        "User context established: {} (authenticated: {})",
        user_context.name, user_context.is_authenticated
    );
    
    // Add user context to request extensions
    request.extensions_mut().insert(user_context);
    
    Ok(next.run(request).await)
}

/// Extract user context from request headers
async fn extract_user_context(
    auth_service: &AuthService,
    headers: &HeaderMap,
) -> Result<UserContext, AuthError> {
    // Extract Authorization header
    let auth_header = headers
        .get("authorization")
        .ok_or(AuthError::MissingAuthHeader)?
        .to_str()
        .map_err(|_| AuthError::InvalidAuthHeaderFormat)?;
    
    // Validate token and get user context
    auth_service.validate_auth_header(auth_header)
}

/// Check if the request should allow anonymous access
/// 
/// This function determines whether a request can proceed without authentication.
/// Typically used for public endpoints like health checks, introspection, etc.
fn should_allow_anonymous(request: &Request) -> bool {
    let path = request.uri().path();
    
    // Allow anonymous access to certain paths
    matches!(path, 
        "/health" | 
        "/ready" | 
        "/metrics" |
        "/graphql" // GraphQL endpoint allows anonymous access, but individual resolvers may require auth
    )
}

/// Optional authentication middleware
/// 
/// This middleware extracts user context if a valid token is provided,
/// but doesn't fail if no token is present. Useful for endpoints that
/// have different behavior for authenticated vs anonymous users.
#[instrument(skip(auth_service, headers, request, next))]
pub async fn optional_auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    debug!("Processing optional authentication middleware");
    
    let user_context = match extract_user_context(&auth_service, &headers).await {
        Ok(context) => {
            debug!("Authentication successful: {}", context.name);
            context
        }
        Err(auth_error) => {
            debug!("Authentication failed, using anonymous context: {}", auth_error);
            UserContext::anonymous()
        }
    };
    
    // Add user context to request extensions
    request.extensions_mut().insert(user_context);
    
    next.run(request).await
}

/// Strict authentication middleware
/// 
/// This middleware requires valid authentication for all requests.
/// Returns 401 Unauthorized if no valid token is provided.
#[instrument(skip(auth_service, headers, request, next))]
pub async fn strict_auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    debug!("Processing strict authentication middleware");
    
    let user_context = extract_user_context(&auth_service, &headers)
        .await
        .map_err(|auth_error| {
            warn!("Strict authentication failed: {}", auth_error);
            StatusCode::from(auth_error)
        })?;
    
    debug!("Strict authentication successful: {}", user_context.name);
    
    // Add user context to request extensions
    request.extensions_mut().insert(user_context);
    
    Ok(next.run(request).await)
}

/// Role-based authorization middleware
/// 
/// This middleware requires the user to have specific roles.
/// Must be used after an authentication middleware.
pub fn require_role(required_role: String) -> impl Fn(Request, Next) -> Result<Response, StatusCode> + Clone {
    move |mut request: Request, next: Next| {
        let required_role = required_role.clone();
        async move {
            let user_context = request
                .extensions()
                .get::<UserContext>()
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if !user_context.is_authenticated {
                warn!("Unauthenticated user attempted to access role-protected resource");
                return Err(StatusCode::UNAUTHORIZED);
            }
            
            if !user_context.has_role(&required_role) {
                warn!(
                    "User {} lacks required role '{}' (has roles: {:?})",
                    user_context.user_id, required_role, user_context.roles
                );
                return Err(StatusCode::FORBIDDEN);
            }
            
            debug!(
                "Role authorization successful: user {} has role '{}'",
                user_context.user_id, required_role
            );
            
            Ok(next.run(request).await)
        }
    }
}

/// Multiple roles authorization middleware
/// 
/// This middleware requires the user to have at least one of the specified roles.
pub fn require_any_role(required_roles: Vec<String>) -> impl Fn(Request, Next) -> Result<Response, StatusCode> + Clone {
    move |request: Request, next: Next| {
        let required_roles = required_roles.clone();
        async move {
            let user_context = request
                .extensions()
                .get::<UserContext>()
                .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            
            if !user_context.is_authenticated {
                warn!("Unauthenticated user attempted to access role-protected resource");
                return Err(StatusCode::UNAUTHORIZED);
            }
            
            let required_roles_str: Vec<&str> = required_roles.iter().map(|s| s.as_str()).collect();
            if !user_context.has_any_role(&required_roles_str) {
                warn!(
                    "User {} lacks any of required roles {:?} (has roles: {:?})",
                    user_context.user_id, required_roles, user_context.roles
                );
                return Err(StatusCode::FORBIDDEN);
            }
            
            debug!(
                "Role authorization successful: user {} has one of roles {:?}",
                user_context.user_id, required_roles
            );
            
            Ok(next.run(request).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{HeaderValue, Method, Request},
    };
    use jsonwebtoken::{encode, EncodingKey, Header};
    use uuid::Uuid;
    
    fn create_test_auth_service() -> Arc<AuthService> {
        Arc::new(AuthService::new_with_secret("test-secret"))
    }
    
    fn create_test_token(roles: Vec<String>) -> String {
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = crate::auth::Claims {
            sub: Uuid::new_v4().to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            roles,
            iat: now,
            exp: now + 3600,
            iss: None,
            aud: None,
        };
        
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("test-secret".as_ref()),
        ).unwrap()
    }
    
    #[tokio::test]
    async fn test_should_allow_anonymous() {
        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        
        assert!(should_allow_anonymous(&request));
        
        let request = Request::builder()
            .uri("/private")
            .body(Body::empty())
            .unwrap();
        
        assert!(!should_allow_anonymous(&request));
    }
    
    #[tokio::test]
    async fn test_extract_user_context_success() {
        let auth_service = create_test_auth_service();
        let token = create_test_token(vec!["user".to_string()]);
        
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
        );
        
        let result = extract_user_context(&auth_service, &headers).await;
        assert!(result.is_ok());
        
        let user_context = result.unwrap();
        assert!(user_context.is_authenticated);
        assert_eq!(user_context.name, "Test User");
    }
    
    #[tokio::test]
    async fn test_extract_user_context_missing_header() {
        let auth_service = create_test_auth_service();
        let headers = HeaderMap::new();
        
        let result = extract_user_context(&auth_service, &headers).await;
        assert!(matches!(result, Err(AuthError::MissingAuthHeader)));
    }
}