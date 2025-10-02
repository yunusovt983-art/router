use async_graphql::ErrorExtensions;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use tracing::{error, warn, info};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum UgcError {
    // Client errors (4xx)
    #[error("Review not found: {id}")]
    ReviewNotFound { id: Uuid },

    #[error("Unauthorized: user {user_id} cannot access review {review_id}")]
    Unauthorized { user_id: Uuid, review_id: Uuid },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Rate limit exceeded for user: {user_id}")]
    RateLimitExceeded { user_id: Uuid },

    #[error("Forbidden: insufficient permissions")]
    Forbidden,

    // Server errors (5xx)
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("External service error: {service} - {message}")]
    ExternalServiceError { service: String, message: String },

    #[error("Circuit breaker open for service: {service}")]
    CircuitBreakerOpen { service: String },

    #[error("Service timeout: {service}")]
    ServiceTimeout { service: String },

    #[error("Configuration error: {0}")]
    ConfigError(#[from] anyhow::Error),

    #[error("Internal server error: {0}")]
    InternalError(String),

    // Cache errors
    #[error("Cache error: {0}")]
    CacheError(String),

    // Connection pool errors
    #[error("Connection pool exhausted")]
    ConnectionPoolExhausted,
}

impl ErrorExtensions for UgcError {
    fn extend(&self) -> async_graphql::Error {
        // Centralized logging based on error severity
        self.log_error();

        let mut error = async_graphql::Error::new(self.to_string());

        match self {
            // Client errors (4xx)
            UgcError::ReviewNotFound { id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "REVIEW_NOT_FOUND");
                    e.set("reviewId", id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }
            UgcError::Unauthorized { user_id, review_id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "UNAUTHORIZED");
                    e.set("userId", user_id.to_string());
                    e.set("reviewId", review_id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }
            UgcError::ValidationError { message } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "VALIDATION_ERROR");
                    e.set("message", message.clone());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }
            UgcError::AuthenticationError(_) => {
                error = error.extend_with(|_, e| {
                    e.set("code", "AUTHENTICATION_ERROR");
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }
            UgcError::RateLimitExceeded { user_id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "RATE_LIMIT_EXCEEDED");
                    e.set("userId", user_id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::Forbidden => {
                error = error.extend_with(|_, e| {
                    e.set("code", "FORBIDDEN");
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }

            // Server errors (5xx)
            UgcError::DatabaseError(_) => {
                error = error.extend_with(|_, e| {
                    e.set("code", "DATABASE_ERROR");
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::ExternalServiceError { service, message } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "EXTERNAL_SERVICE_ERROR");
                    e.set("service", service.clone());
                    e.set("message", message.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::CircuitBreakerOpen { service } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CIRCUIT_BREAKER_OPEN");
                    e.set("service", service.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::ServiceTimeout { service } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "SERVICE_TIMEOUT");
                    e.set("service", service.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::CacheError(_) => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CACHE_ERROR");
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            UgcError::ConnectionPoolExhausted => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CONNECTION_POOL_EXHAUSTED");
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            _ => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INTERNAL_ERROR");
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", false);
                });
            }
        }

        error
    }
}

impl UgcError {
    /// Centralized error logging based on severity
    pub fn log_error(&self) {
        match self {
            // Info level for client errors that are expected
            UgcError::ReviewNotFound { id } => {
                info!(
                    error = %self,
                    review_id = %id,
                    error_code = "REVIEW_NOT_FOUND",
                    "Review not found"
                );
            }
            UgcError::ValidationError { message } => {
                info!(
                    error = %self,
                    validation_message = %message,
                    error_code = "VALIDATION_ERROR",
                    "Validation error"
                );
            }

            // Warn level for authentication/authorization issues
            UgcError::Unauthorized { user_id, review_id } => {
                warn!(
                    error = %self,
                    user_id = %user_id,
                    review_id = %review_id,
                    error_code = "UNAUTHORIZED",
                    "Unauthorized access attempt"
                );
            }
            UgcError::AuthenticationError(msg) => {
                warn!(
                    error = %self,
                    auth_error = %msg,
                    error_code = "AUTHENTICATION_ERROR",
                    "Authentication failed"
                );
            }
            UgcError::RateLimitExceeded { user_id } => {
                warn!(
                    error = %self,
                    user_id = %user_id,
                    error_code = "RATE_LIMIT_EXCEEDED",
                    "Rate limit exceeded"
                );
            }
            UgcError::Forbidden => {
                warn!(
                    error = %self,
                    error_code = "FORBIDDEN",
                    "Forbidden access"
                );
            }

            // Error level for server errors that need attention
            UgcError::DatabaseError(db_err) => {
                error!(
                    error = %self,
                    db_error = %db_err,
                    error_code = "DATABASE_ERROR",
                    "Database operation failed"
                );
            }
            UgcError::ExternalServiceError { service, message } => {
                error!(
                    error = %self,
                    service = %service,
                    service_message = %message,
                    error_code = "EXTERNAL_SERVICE_ERROR",
                    "External service error"
                );
            }
            UgcError::CircuitBreakerOpen { service } => {
                warn!(
                    error = %self,
                    service = %service,
                    error_code = "CIRCUIT_BREAKER_OPEN",
                    "Circuit breaker is open"
                );
            }
            UgcError::ServiceTimeout { service } => {
                warn!(
                    error = %self,
                    service = %service,
                    error_code = "SERVICE_TIMEOUT",
                    "Service timeout"
                );
            }
            UgcError::ConnectionPoolExhausted => {
                error!(
                    error = %self,
                    error_code = "CONNECTION_POOL_EXHAUSTED",
                    "Database connection pool exhausted"
                );
            }
            UgcError::CacheError(cache_err) => {
                warn!(
                    error = %self,
                    cache_error = %cache_err,
                    error_code = "CACHE_ERROR",
                    "Cache operation failed"
                );
            }

            // Error level for unexpected internal errors
            UgcError::ConfigError(config_err) => {
                error!(
                    error = %self,
                    config_error = %config_err,
                    error_code = "CONFIG_ERROR",
                    "Configuration error"
                );
            }
            UgcError::InternalError(msg) => {
                error!(
                    error = %self,
                    internal_message = %msg,
                    error_code = "INTERNAL_ERROR",
                    "Internal server error"
                );
            }
        }
    }

    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            UgcError::DatabaseError(_)
                | UgcError::ExternalServiceError { .. }
                | UgcError::ServiceTimeout { .. }
                | UgcError::CacheError(_)
                | UgcError::ConnectionPoolExhausted
                | UgcError::RateLimitExceeded { .. }
        )
    }

    /// Get error category for metrics and monitoring
    pub fn category(&self) -> &'static str {
        match self {
            UgcError::ReviewNotFound { .. }
            | UgcError::Unauthorized { .. }
            | UgcError::ValidationError { .. }
            | UgcError::AuthenticationError(_)
            | UgcError::RateLimitExceeded { .. }
            | UgcError::Forbidden => "CLIENT_ERROR",

            UgcError::DatabaseError(_)
            | UgcError::ExternalServiceError { .. }
            | UgcError::CircuitBreakerOpen { .. }
            | UgcError::ServiceTimeout { .. }
            | UgcError::CacheError(_)
            | UgcError::ConnectionPoolExhausted
            | UgcError::ConfigError(_)
            | UgcError::InternalError(_) => "SERVER_ERROR",
        }
    }
}

impl IntoResponse for UgcError {
    fn into_response(self) -> Response {
        // Log the error before converting to response
        self.log_error();

        let (status, error_message) = match self {
            // Client errors (4xx)
            UgcError::ReviewNotFound { .. } => (StatusCode::NOT_FOUND, self.to_string()),
            UgcError::Unauthorized { .. } => (StatusCode::UNAUTHORIZED, self.to_string()),
            UgcError::ValidationError { .. } => (StatusCode::BAD_REQUEST, self.to_string()),
            UgcError::AuthenticationError(_) => (StatusCode::UNAUTHORIZED, self.to_string()),
            UgcError::RateLimitExceeded { .. } => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            UgcError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),

            // Server errors (5xx)
            UgcError::ExternalServiceError { .. } => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            UgcError::CircuitBreakerOpen { .. } => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            UgcError::ServiceTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            UgcError::ConnectionPoolExhausted => (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable".to_string()),

            // Generic server errors
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16(),
            "retryable": self.is_retryable(),
            "category": self.category()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, UgcError>;

#[cfg(test)]
mod tests;