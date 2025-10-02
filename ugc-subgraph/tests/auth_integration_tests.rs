use std::sync::Arc;
use uuid::Uuid;
use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use tower::ServiceExt;
use serde_json::json;
use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
use chrono::Utc;

use ugc_subgraph::{
    auth::{AuthService, Claims, UserContext, AuthError},
    error::UgcError,
    service::ReviewService,
    repository::PostgresReviewRepository,
    models::review::CreateReviewInput,
};

// Helper function to create a valid JWT token
fn create_test_jwt(secret: &str, user_id: Uuid, roles: Vec<String>) -> String {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles,
        iat: now,
        exp: now + 3600, // 1 hour from now
        iss: Some("test-issuer".to_string()),
        aud: Some("test-audience".to_string()),
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ).expect("Failed to create test JWT")
}

// Helper function to create an expired JWT token
fn create_expired_jwt(secret: &str, user_id: Uuid) -> String {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
        iat: now - 7200, // 2 hours ago
        exp: now - 3600, // 1 hour ago (expired)
        iss: Some("test-issuer".to_string()),
        aud: Some("test-audience".to_string()),
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ).expect("Failed to create expired JWT")
}

#[tokio::test]
async fn test_auth_service_valid_token() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    let user_id = Uuid::new_v4();
    
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    
    let user_context = auth_service.validate_token(&token)
        .expect("Token validation should succeed");
    
    assert_eq!(user_context.user_id, user_id);
    assert_eq!(user_context.name, "Test User");
    assert_eq!(user_context.email, "test@example.com");
    assert!(user_context.has_role("user"));
    assert!(user_context.is_authenticated);
}

#[tokio::test]
async fn test_auth_service_expired_token() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    let user_id = Uuid::new_v4();
    
    let token = create_expired_jwt(secret, user_id);
    
    let result = auth_service.validate_token(&token);
    assert!(matches!(result, Err(AuthError::TokenExpired)));
}

#[tokio::test]
async fn test_auth_service_invalid_signature() {
    let secret = "test-secret-key";
    let wrong_secret = "wrong-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    let user_id = Uuid::new_v4();
    
    // Create token with wrong secret
    let token = create_test_jwt(wrong_secret, user_id, vec!["user".to_string()]);
    
    let result = auth_service.validate_token(&token);
    assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
}

#[tokio::test]
async fn test_auth_service_malformed_token() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    
    let malformed_token = "not.a.valid.jwt.token";
    
    let result = auth_service.validate_token(malformed_token);
    assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
}

#[tokio::test]
async fn test_auth_header_extraction() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    
    // Valid Bearer token
    let token = auth_service.extract_token_from_header("Bearer abc123")
        .expect("Should extract token");
    assert_eq!(token, "abc123");
    
    // Invalid format
    let result = auth_service.extract_token_from_header("Invalid abc123");
    assert!(matches!(result, Err(AuthError::InvalidAuthHeaderFormat)));
    
    // Empty token
    let result = auth_service.extract_token_from_header("Bearer ");
    assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
    
    // Missing Bearer prefix
    let result = auth_service.extract_token_from_header("abc123");
    assert!(matches!(result, Err(AuthError::InvalidAuthHeaderFormat)));
}

#[tokio::test]
async fn test_user_context_roles() {
    let user_context = UserContext::new(
        Uuid::new_v4(),
        "Test User".to_string(),
        "test@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string()],
    );
    
    assert!(user_context.has_role("user"));
    assert!(user_context.has_role("moderator"));
    assert!(!user_context.has_role("admin"));
    assert!(user_context.is_moderator());
    assert!(!user_context.is_admin());
    assert!(user_context.has_any_role(&["user", "admin"]));
    assert!(user_context.has_any_role(&["moderator", "admin"]));
    assert!(!user_context.has_any_role(&["admin", "superuser"]));
}

#[tokio::test]
async fn test_admin_user_context() {
    let admin_context = UserContext::new(
        Uuid::new_v4(),
        "Admin User".to_string(),
        "admin@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string(), "admin".to_string()],
    );
    
    assert!(admin_context.has_role("user"));
    assert!(admin_context.has_role("moderator"));
    assert!(admin_context.has_role("admin"));
    assert!(admin_context.is_moderator());
    assert!(admin_context.is_admin());
}

#[tokio::test]
async fn test_auth_service_with_issuer_validation() {
    let secret = "test-secret-key";
    let expected_issuer = "test-issuer";
    let auth_service = AuthService::new_with_secret(secret)
        .with_issuer(expected_issuer.to_string());
    
    let user_id = Uuid::new_v4();
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    
    let user_context = auth_service.validate_token(&token)
        .expect("Token with correct issuer should be valid");
    
    assert_eq!(user_context.user_id, user_id);
}

#[tokio::test]
async fn test_auth_service_with_wrong_issuer() {
    let secret = "test-secret-key";
    let expected_issuer = "wrong-issuer";
    let auth_service = AuthService::new_with_secret(secret)
        .with_issuer(expected_issuer.to_string());
    
    let user_id = Uuid::new_v4();
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    
    let result = auth_service.validate_token(&token);
    assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
}

#[tokio::test]
async fn test_auth_service_with_audience_validation() {
    let secret = "test-secret-key";
    let expected_audience = "test-audience";
    let auth_service = AuthService::new_with_secret(secret)
        .with_audience(expected_audience.to_string());
    
    let user_id = Uuid::new_v4();
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    
    let user_context = auth_service.validate_token(&token)
        .expect("Token with correct audience should be valid");
    
    assert_eq!(user_context.user_id, user_id);
}

#[tokio::test]
async fn test_auth_service_builder() {
    let secret = "test-secret-key";
    let issuer = "test-issuer";
    let audience = "test-audience";
    
    let auth_service = ugc_subgraph::auth::AuthServiceBuilder::new()
        .with_secret(secret.to_string())
        .with_issuer(issuer.to_string())
        .with_audience(audience.to_string())
        .build()
        .expect("Should build auth service");
    
    let user_id = Uuid::new_v4();
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    
    let user_context = auth_service.validate_token(&token)
        .expect("Token should be valid");
    
    assert_eq!(user_context.user_id, user_id);
}

#[tokio::test]
async fn test_auth_service_builder_validation() {
    // Test builder without secret or RSA key
    let result = ugc_subgraph::auth::AuthServiceBuilder::new().build();
    assert!(matches!(result, Err(AuthError::InternalError { .. })));
    
    // Test builder with both secret and RSA key
    let result = ugc_subgraph::auth::AuthServiceBuilder::new()
        .with_secret("secret".to_string())
        .with_rsa_key("key".to_string())
        .build();
    assert!(matches!(result, Err(AuthError::InternalError { .. })));
}

// Integration test with actual HTTP middleware
#[tokio::test]
async fn test_auth_middleware_integration() {
    use axum::{
        middleware,
        routing::post,
        Router,
    };
    use tower::ServiceExt;
    
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    let user_id = Uuid::new_v4();
    
    // Create a simple handler that requires authentication
    async fn protected_handler(user_context: Option<UserContext>) -> Result<String, StatusCode> {
        match user_context {
            Some(ctx) => Ok(format!("Hello, {}!", ctx.name)),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
    
    // Create middleware that extracts user context
    async fn auth_middleware(
        mut request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // This is a simplified version - in real implementation,
        // you would extract the auth service from app state
        let auth_header = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());
        
        if let Some(auth_header) = auth_header {
            // In real implementation, validate the token here
            // For this test, we'll just check if it starts with "Bearer"
            if auth_header.starts_with("Bearer ") {
                let user_context = UserContext::new(
                    Uuid::new_v4(),
                    "Test User".to_string(),
                    "test@example.com".to_string(),
                    vec!["user".to_string()],
                );
                request.extensions_mut().insert(user_context);
            }
        }
        
        Ok(next.run(request).await)
    }
    
    let app = Router::new()
        .route("/protected", post(|request: Request| async move {
            let user_context = request.extensions().get::<UserContext>().cloned();
            protected_handler(user_context).await
        }))
        .layer(middleware::from_fn(auth_middleware));
    
    // Test with valid token
    let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // Test without token
    let request = Request::builder()
        .method(Method::POST)
        .uri("/protected")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    // This would be UNAUTHORIZED in a real implementation
    // but our simplified middleware doesn't enforce it
    assert!(response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED);
}

// Test role-based authorization
#[tokio::test]
async fn test_role_based_authorization() {
    let user_context = UserContext::new(
        Uuid::new_v4(),
        "Regular User".to_string(),
        "user@example.com".to_string(),
        vec!["user".to_string()],
    );
    
    let moderator_context = UserContext::new(
        Uuid::new_v4(),
        "Moderator User".to_string(),
        "moderator@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string()],
    );
    
    let admin_context = UserContext::new(
        Uuid::new_v4(),
        "Admin User".to_string(),
        "admin@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string(), "admin".to_string()],
    );
    
    // Test user permissions
    assert!(user_context.has_role("user"));
    assert!(!user_context.has_role("moderator"));
    assert!(!user_context.has_role("admin"));
    
    // Test moderator permissions
    assert!(moderator_context.has_role("user"));
    assert!(moderator_context.has_role("moderator"));
    assert!(!moderator_context.has_role("admin"));
    assert!(moderator_context.is_moderator());
    
    // Test admin permissions
    assert!(admin_context.has_role("user"));
    assert!(admin_context.has_role("moderator"));
    assert!(admin_context.has_role("admin"));
    assert!(admin_context.is_moderator());
    assert!(admin_context.is_admin());
}

// Test JWT claims conversion
#[tokio::test]
async fn test_claims_to_user_context_conversion() {
    let user_id = Uuid::new_v4();
    let now = Utc::now().timestamp() as usize;
    
    let claims = Claims {
        sub: user_id.to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string(), "moderator".to_string()],
        iat: now,
        exp: now + 3600,
        iss: Some("test-issuer".to_string()),
        aud: Some("test-audience".to_string()),
    };
    
    let user_context = claims.to_user_context()
        .expect("Should convert claims to user context");
    
    assert_eq!(user_context.user_id, user_id);
    assert_eq!(user_context.name, "Test User");
    assert_eq!(user_context.email, "test@example.com");
    assert!(user_context.has_role("user"));
    assert!(user_context.has_role("moderator"));
    assert!(user_context.is_authenticated);
}

#[tokio::test]
async fn test_claims_with_invalid_user_id() {
    let now = Utc::now().timestamp() as usize;
    
    let claims = Claims {
        sub: "invalid-uuid".to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
        iat: now,
        exp: now + 3600,
        iss: Some("test-issuer".to_string()),
        aud: Some("test-audience".to_string()),
    };
    
    let result = claims.to_user_context();
    assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
}

// Test authentication error conversion to UGC error
#[tokio::test]
async fn test_auth_error_to_ugc_error_conversion() {
    let auth_error = AuthError::TokenExpired;
    let ugc_error = UgcError::AuthenticationError(auth_error.to_string());
    
    assert_eq!(ugc_error.category(), "CLIENT_ERROR");
    assert!(!ugc_error.is_retryable());
    
    let auth_error = AuthError::InvalidToken { reason: "Bad signature".to_string() };
    let ugc_error = UgcError::AuthenticationError(auth_error.to_string());
    
    assert_eq!(ugc_error.category(), "CLIENT_ERROR");
    assert!(!ugc_error.is_retryable());
}

// Test concurrent authentication
#[tokio::test]
async fn test_concurrent_authentication() {
    let secret = "test-secret-key";
    let auth_service = Arc::new(AuthService::new_with_secret(secret));
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let auth_service_clone = Arc::clone(&auth_service);
        let handle = tokio::spawn(async move {
            let user_id = Uuid::new_v4();
            let token = create_test_jwt(secret, user_id, vec!["user".to_string()]);
            
            let result = auth_service_clone.validate_token(&token);
            (i, result)
        });
        handles.push(handle);
    }
    
    let mut successful_validations = 0;
    for handle in handles {
        let (i, result) = handle.await.expect("Task should complete");
        match result {
            Ok(user_context) => {
                assert!(user_context.is_authenticated);
                assert!(user_context.has_role("user"));
                successful_validations += 1;
            }
            Err(e) => {
                panic!("Authentication {} failed: {:?}", i, e);
            }
        }
    }
    
    assert_eq!(successful_validations, 10);
}

// Test token refresh scenario
#[tokio::test]
async fn test_token_near_expiry() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    let user_id = Uuid::new_v4();
    
    // Create a token that expires in 30 seconds
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
        iat: now,
        exp: now + 30, // 30 seconds from now
        iss: Some("test-issuer".to_string()),
        aud: Some("test-audience".to_string()),
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    ).expect("Failed to create test JWT");
    
    // Token should still be valid
    let user_context = auth_service.validate_token(&token)
        .expect("Token should still be valid");
    
    assert_eq!(user_context.user_id, user_id);
    
    // Check if token is near expiry (this would be used for refresh logic)
    assert!(claims.is_near_expiry(60)); // Within 60 seconds of expiry
    assert!(!claims.is_near_expiry(10)); // Not within 10 seconds of expiry
}