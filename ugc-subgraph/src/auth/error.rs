use async_graphql::ErrorExtensions;
use thiserror::Error;

/// Authentication and authorization errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid token: {reason}")]
    InvalidToken { reason: String },
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Missing authorization header")]
    MissingAuthHeader,
    
    #[error("Invalid authorization header format")]
    InvalidAuthHeaderFormat,
    
    #[error("Insufficient permissions: required role '{required_role}'")]
    InsufficientPermissions { required_role: String },
    
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },
    
    #[error("JWT decode error: {0}")]
    JwtDecodeError(#[from] jsonwebtoken::errors::Error),
    
    #[error("Internal authentication error: {message}")]
    InternalError { message: String },
}

impl From<AuthError> for async_graphql::Error {
    fn from(err: AuthError) -> Self {
        let mut error = async_graphql::Error::new(err.to_string());
        
        match err {
            AuthError::InvalidToken { reason } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INVALID_TOKEN");
                    e.set("reason", reason);
                });
            }
            AuthError::TokenExpired => {
                error = error.extend_with(|_, e| {
                    e.set("code", "TOKEN_EXPIRED");
                });
            }
            AuthError::MissingAuthHeader => {
                error = error.extend_with(|_, e| {
                    e.set("code", "MISSING_AUTH_HEADER");
                });
            }
            AuthError::InvalidAuthHeaderFormat => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INVALID_AUTH_HEADER_FORMAT");
                });
            }
            AuthError::InsufficientPermissions { required_role } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INSUFFICIENT_PERMISSIONS");
                    e.set("requiredRole", required_role);
                });
            }
            AuthError::UserNotFound { user_id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "USER_NOT_FOUND");
                    e.set("userId", user_id);
                });
            }
            AuthError::JwtDecodeError(_) => {
                error = error.extend_with(|_, e| {
                    e.set("code", "JWT_DECODE_ERROR");
                });
            }
            AuthError::InternalError { message } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INTERNAL_AUTH_ERROR");
                    e.set("message", message);
                });
            }
        }
        
        error
    }
}

impl From<AuthError> for axum::http::StatusCode {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::InvalidToken { .. } 
            | AuthError::TokenExpired 
            | AuthError::MissingAuthHeader 
            | AuthError::InvalidAuthHeaderFormat 
            | AuthError::JwtDecodeError(_) => axum::http::StatusCode::UNAUTHORIZED,
            
            AuthError::InsufficientPermissions { .. } => axum::http::StatusCode::FORBIDDEN,
            
            AuthError::UserNotFound { .. } => axum::http::StatusCode::NOT_FOUND,
            
            AuthError::InternalError { .. } => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}