use crate::auth::{AuthError, Claims, UserContext};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::sync::Arc;
use tracing::{debug, error, instrument, warn};

/// JWT Authentication Service
#[derive(Clone)]
pub struct AuthService {
    decoding_key: DecodingKey,
    validation: Validation,
    issuer: Option<String>,
    audience: Option<String>,
}

impl AuthService {
    /// Create a new AuthService with HMAC secret
    pub fn new_with_secret(secret: &str) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            validation,
            issuer: None,
            audience: None,
        }
    }
    
    /// Create a new AuthService with RSA public key
    pub fn new_with_rsa_key(public_key: &str) -> Result<Self, AuthError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        
        let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
            .map_err(|e| AuthError::InternalError {
                message: format!("Failed to create RSA decoding key: {}", e),
            })?;
        
        Ok(Self {
            decoding_key,
            validation,
            issuer: None,
            audience: None,
        })
    }
    
    /// Set expected issuer for token validation
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.validation.validate_iss = true;
        self.validation.iss = Some(issuer.clone().into());
        self.issuer = Some(issuer);
        self
    }
    
    /// Set expected audience for token validation
    pub fn with_audience(mut self, audience: String) -> Self {
        self.validation.validate_aud = true;
        self.validation.aud = Some(audience.clone().into());
        self.audience = Some(audience);
        self
    }
    
    /// Validate JWT token and extract user context
    #[instrument(skip(self, token), fields(token_length = token.len()))]
    pub fn validate_token(&self, token: &str) -> Result<UserContext, AuthError> {
        debug!("Validating JWT token");
        
        // Decode and validate the token
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| {
                warn!("JWT validation failed: {}", e);
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                    jsonwebtoken::errors::ErrorKind::InvalidToken => AuthError::InvalidToken {
                        reason: "Token format is invalid".to_string(),
                    },
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => AuthError::InvalidToken {
                        reason: "Token signature is invalid".to_string(),
                    },
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => AuthError::InvalidToken {
                        reason: "Token issuer is invalid".to_string(),
                    },
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => AuthError::InvalidToken {
                        reason: "Token audience is invalid".to_string(),
                    },
                    _ => AuthError::JwtDecodeError(e),
                }
            })?;
        
        let claims = token_data.claims;
        
        // Additional validation
        if claims.is_expired() {
            return Err(AuthError::TokenExpired);
        }
        
        // Convert claims to user context
        let user_context = claims.to_user_context()?;
        
        debug!(
            "Token validated successfully for user: {} ({})",
            user_context.name, user_context.user_id
        );
        
        Ok(user_context)
    }
    
    /// Extract token from Authorization header
    pub fn extract_token_from_header(&self, auth_header: &str) -> Result<&str, AuthError> {
        if !auth_header.starts_with("Bearer ") {
            return Err(AuthError::InvalidAuthHeaderFormat);
        }
        
        let token = auth_header.strip_prefix("Bearer ").unwrap();
        
        if token.is_empty() {
            return Err(AuthError::InvalidToken {
                reason: "Token is empty".to_string(),
            });
        }
        
        Ok(token)
    }
    
    /// Validate authorization header and return user context
    #[instrument(skip(self, auth_header))]
    pub fn validate_auth_header(&self, auth_header: &str) -> Result<UserContext, AuthError> {
        let token = self.extract_token_from_header(auth_header)?;
        self.validate_token(token)
    }
}

/// Builder for AuthService configuration
pub struct AuthServiceBuilder {
    secret: Option<String>,
    rsa_key: Option<String>,
    issuer: Option<String>,
    audience: Option<String>,
    algorithm: Algorithm,
}

impl AuthServiceBuilder {
    pub fn new() -> Self {
        Self {
            secret: None,
            rsa_key: None,
            issuer: None,
            audience: None,
            algorithm: Algorithm::HS256,
        }
    }
    
    pub fn with_secret(mut self, secret: String) -> Self {
        self.secret = Some(secret);
        self.algorithm = Algorithm::HS256;
        self
    }
    
    pub fn with_rsa_key(mut self, rsa_key: String) -> Self {
        self.rsa_key = Some(rsa_key);
        self.algorithm = Algorithm::RS256;
        self
    }
    
    pub fn with_issuer(mut self, issuer: String) -> Self {
        self.issuer = Some(issuer);
        self
    }
    
    pub fn with_audience(mut self, audience: String) -> Self {
        self.audience = Some(audience);
        self
    }
    
    pub fn build(self) -> Result<AuthService, AuthError> {
        let mut service = match (self.secret, self.rsa_key) {
            (Some(secret), None) => AuthService::new_with_secret(&secret),
            (None, Some(rsa_key)) => AuthService::new_with_rsa_key(&rsa_key)?,
            (Some(_), Some(_)) => {
                return Err(AuthError::InternalError {
                    message: "Cannot specify both secret and RSA key".to_string(),
                });
            }
            (None, None) => {
                return Err(AuthError::InternalError {
                    message: "Must specify either secret or RSA key".to_string(),
                });
            }
        };
        
        if let Some(issuer) = self.issuer {
            service = service.with_issuer(issuer);
        }
        
        if let Some(audience) = self.audience {
            service = service.with_audience(audience);
        }
        
        Ok(service)
    }
}

impl Default for AuthServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use uuid::Uuid;
    
    fn create_test_claims() -> Claims {
        let now = chrono::Utc::now().timestamp() as usize;
        Claims {
            sub: Uuid::new_v4().to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            roles: vec!["user".to_string()],
            iat: now,
            exp: now + 3600, // 1 hour from now
            iss: Some("test-issuer".to_string()),
            aud: Some("test-audience".to_string()),
        }
    }
    
    #[test]
    fn test_validate_valid_token() {
        let secret = "test-secret";
        let auth_service = AuthService::new_with_secret(secret);
        
        let claims = create_test_claims();
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        ).unwrap();
        
        let user_context = auth_service.validate_token(&token).unwrap();
        
        assert_eq!(user_context.name, "Test User");
        assert_eq!(user_context.email, "test@example.com");
        assert!(user_context.is_authenticated);
        assert!(user_context.has_role("user"));
    }
    
    #[test]
    fn test_validate_expired_token() {
        let secret = "test-secret";
        let auth_service = AuthService::new_with_secret(secret);
        
        let mut claims = create_test_claims();
        claims.exp = chrono::Utc::now().timestamp() as usize - 3600; // 1 hour ago
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_ref()),
        ).unwrap();
        
        let result = auth_service.validate_token(&token);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }
    
    #[test]
    fn test_extract_token_from_header() {
        let auth_service = AuthService::new_with_secret("test-secret");
        
        // Valid header
        let token = auth_service
            .extract_token_from_header("Bearer abc123")
            .unwrap();
        assert_eq!(token, "abc123");
        
        // Invalid format
        let result = auth_service.extract_token_from_header("Invalid abc123");
        assert!(matches!(result, Err(AuthError::InvalidAuthHeaderFormat)));
        
        // Empty token
        let result = auth_service.extract_token_from_header("Bearer ");
        assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
    }
    
    #[test]
    fn test_user_context_roles() {
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
    }
}