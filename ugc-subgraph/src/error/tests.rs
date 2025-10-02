use uuid::Uuid;
use async_graphql::ErrorExtensions;
use axum::{http::StatusCode, response::IntoResponse};
use serde_json::Value;

use super::UgcError;

#[test]
fn test_review_not_found_error() {
    let id = Uuid::new_v4();
    let error = UgcError::ReviewNotFound { id };
    
    assert_eq!(error.to_string(), format!("Review not found: {}", id));
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_unauthorized_error() {
    let user_id = Uuid::new_v4();
    let review_id = Uuid::new_v4();
    let error = UgcError::Unauthorized { user_id, review_id };
    
    assert_eq!(
        error.to_string(),
        format!("Unauthorized: user {} cannot access review {}", user_id, review_id)
    );
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_validation_error() {
    let message = "Rating must be between 1 and 5".to_string();
    let error = UgcError::ValidationError { message: message.clone() };
    
    assert_eq!(error.to_string(), format!("Validation error: {}", message));
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_authentication_error() {
    let message = "Invalid token".to_string();
    let error = UgcError::AuthenticationError(message.clone());
    
    assert_eq!(error.to_string(), format!("Authentication error: {}", message));
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_rate_limit_exceeded_error() {
    let user_id = Uuid::new_v4();
    let error = UgcError::RateLimitExceeded { user_id };
    
    assert_eq!(
        error.to_string(),
        format!("Rate limit exceeded for user: {}", user_id)
    );
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(error.is_retryable());
}

#[test]
fn test_forbidden_error() {
    let error = UgcError::Forbidden;
    
    assert_eq!(error.to_string(), "Forbidden: insufficient permissions");
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_external_service_error() {
    let service = "users".to_string();
    let message = "Connection timeout".to_string();
    let error = UgcError::ExternalServiceError { 
        service: service.clone(), 
        message: message.clone() 
    };
    
    assert_eq!(
        error.to_string(),
        format!("External service error: {} - {}", service, message)
    );
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(error.is_retryable());
}

#[test]
fn test_circuit_breaker_open_error() {
    let service = "offers".to_string();
    let error = UgcError::CircuitBreakerOpen { service: service.clone() };
    
    assert_eq!(
        error.to_string(),
        format!("Circuit breaker open for service: {}", service)
    );
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(!error.is_retryable()); // Circuit breaker errors are not retryable by default
}

#[test]
fn test_service_timeout_error() {
    let service = "database".to_string();
    let error = UgcError::ServiceTimeout { service: service.clone() };
    
    assert_eq!(
        error.to_string(),
        format!("Service timeout: {}", service)
    );
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(error.is_retryable());
}

#[test]
fn test_cache_error() {
    let message = "Redis connection failed".to_string();
    let error = UgcError::CacheError(message.clone());
    
    assert_eq!(error.to_string(), format!("Cache error: {}", message));
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(error.is_retryable());
}

#[test]
fn test_connection_pool_exhausted_error() {
    let error = UgcError::ConnectionPoolExhausted;
    
    assert_eq!(error.to_string(), "Connection pool exhausted");
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(error.is_retryable());
}

#[test]
fn test_internal_error() {
    let message = "Unexpected error occurred".to_string();
    let error = UgcError::InternalError(message.clone());
    
    assert_eq!(error.to_string(), format!("Internal server error: {}", message));
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_graphql_error_extensions() {
    let id = Uuid::new_v4();
    let error = UgcError::ReviewNotFound { id };
    let graphql_error = error.extend();
    
    assert!(graphql_error.message.contains(&id.to_string()));
    
    // Check extensions
    let extensions = &graphql_error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("REVIEW_NOT_FOUND".to_string())));
    assert_eq!(extensions.get("reviewId"), Some(&Value::String(id.to_string())));
    assert_eq!(extensions.get("category"), Some(&Value::String("CLIENT_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(false)));
}

#[test]
fn test_unauthorized_graphql_extensions() {
    let user_id = Uuid::new_v4();
    let review_id = Uuid::new_v4();
    let error = UgcError::Unauthorized { user_id, review_id };
    let graphql_error = error.extend();
    
    let extensions = &graphql_error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("UNAUTHORIZED".to_string())));
    assert_eq!(extensions.get("userId"), Some(&Value::String(user_id.to_string())));
    assert_eq!(extensions.get("reviewId"), Some(&Value::String(review_id.to_string())));
    assert_eq!(extensions.get("category"), Some(&Value::String("CLIENT_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(false)));
}

#[test]
fn test_validation_error_graphql_extensions() {
    let message = "Invalid rating".to_string();
    let error = UgcError::ValidationError { message: message.clone() };
    let graphql_error = error.extend();
    
    let extensions = &graphql_error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("VALIDATION_ERROR".to_string())));
    assert_eq!(extensions.get("message"), Some(&Value::String(message)));
    assert_eq!(extensions.get("category"), Some(&Value::String("CLIENT_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(false)));
}

#[test]
fn test_external_service_error_graphql_extensions() {
    let service = "users".to_string();
    let message = "Connection failed".to_string();
    let error = UgcError::ExternalServiceError { 
        service: service.clone(), 
        message: message.clone() 
    };
    let graphql_error = error.extend();
    
    let extensions = &graphql_error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("EXTERNAL_SERVICE_ERROR".to_string())));
    assert_eq!(extensions.get("service"), Some(&Value::String(service)));
    assert_eq!(extensions.get("message"), Some(&Value::String(message)));
    assert_eq!(extensions.get("category"), Some(&Value::String("SERVER_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(true)));
}

#[tokio::test]
async fn test_http_response_conversion() {
    // Test client error
    let id = Uuid::new_v4();
    let error = UgcError::ReviewNotFound { id };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // Test server error
    let error = UgcError::ConnectionPoolExhausted;
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    
    // Test unauthorized error
    let user_id = Uuid::new_v4();
    let review_id = Uuid::new_v4();
    let error = UgcError::Unauthorized { user_id, review_id };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    
    // Test validation error
    let error = UgcError::ValidationError { 
        message: "Invalid input".to_string() 
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    // Test rate limit error
    let user_id = Uuid::new_v4();
    let error = UgcError::RateLimitExceeded { user_id };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    
    // Test forbidden error
    let error = UgcError::Forbidden;
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    
    // Test service timeout
    let error = UgcError::ServiceTimeout { 
        service: "database".to_string() 
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    
    // Test circuit breaker
    let error = UgcError::CircuitBreakerOpen { 
        service: "users".to_string() 
    };
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn test_error_categorization() {
    // Client errors
    assert_eq!(UgcError::ReviewNotFound { id: Uuid::new_v4() }.category(), "CLIENT_ERROR");
    assert_eq!(UgcError::Unauthorized { user_id: Uuid::new_v4(), review_id: Uuid::new_v4() }.category(), "CLIENT_ERROR");
    assert_eq!(UgcError::ValidationError { message: "test".to_string() }.category(), "CLIENT_ERROR");
    assert_eq!(UgcError::AuthenticationError("test".to_string()).category(), "CLIENT_ERROR");
    assert_eq!(UgcError::RateLimitExceeded { user_id: Uuid::new_v4() }.category(), "CLIENT_ERROR");
    assert_eq!(UgcError::Forbidden.category(), "CLIENT_ERROR");
    
    // Server errors
    assert_eq!(UgcError::ExternalServiceError { service: "test".to_string(), message: "test".to_string() }.category(), "SERVER_ERROR");
    assert_eq!(UgcError::CircuitBreakerOpen { service: "test".to_string() }.category(), "SERVER_ERROR");
    assert_eq!(UgcError::ServiceTimeout { service: "test".to_string() }.category(), "SERVER_ERROR");
    assert_eq!(UgcError::CacheError("test".to_string()).category(), "SERVER_ERROR");
    assert_eq!(UgcError::ConnectionPoolExhausted.category(), "SERVER_ERROR");
    assert_eq!(UgcError::InternalError("test".to_string()).category(), "SERVER_ERROR");
}

#[test]
fn test_retryable_classification() {
    // Retryable errors
    assert!(UgcError::ExternalServiceError { service: "test".to_string(), message: "test".to_string() }.is_retryable());
    assert!(UgcError::ServiceTimeout { service: "test".to_string() }.is_retryable());
    assert!(UgcError::CacheError("test".to_string()).is_retryable());
    assert!(UgcError::ConnectionPoolExhausted.is_retryable());
    assert!(UgcError::RateLimitExceeded { user_id: Uuid::new_v4() }.is_retryable());
    
    // Non-retryable errors
    assert!(!UgcError::ReviewNotFound { id: Uuid::new_v4() }.is_retryable());
    assert!(!UgcError::Unauthorized { user_id: Uuid::new_v4(), review_id: Uuid::new_v4() }.is_retryable());
    assert!(!UgcError::ValidationError { message: "test".to_string() }.is_retryable());
    assert!(!UgcError::AuthenticationError("test".to_string()).is_retryable());
    assert!(!UgcError::Forbidden.is_retryable());
    assert!(!UgcError::InternalError("test".to_string()).is_retryable());
}

// Test error chaining with sqlx errors
#[test]
fn test_database_error_from_sqlx() {
    let sqlx_error = sqlx::Error::RowNotFound;
    let ugc_error = UgcError::from(sqlx_error);
    
    assert!(matches!(ugc_error, UgcError::DatabaseError(_)));
    assert_eq!(ugc_error.category(), "SERVER_ERROR");
    assert!(ugc_error.is_retryable());
}

// Test error chaining with anyhow errors
#[test]
fn test_config_error_from_anyhow() {
    let anyhow_error = anyhow::anyhow!("Configuration file not found");
    let ugc_error = UgcError::from(anyhow_error);
    
    assert!(matches!(ugc_error, UgcError::ConfigError(_)));
    assert_eq!(ugc_error.category(), "SERVER_ERROR");
    assert!(!ugc_error.is_retryable());
}

// Property-based tests for error consistency
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_client_errors_are_not_retryable(
            id in any::<Uuid>(),
            user_id in any::<Uuid>(),
            review_id in any::<Uuid>(),
            message in ".*"
        ) {
            let errors = vec![
                UgcError::ReviewNotFound { id },
                UgcError::Unauthorized { user_id, review_id },
                UgcError::ValidationError { message: message.clone() },
                UgcError::AuthenticationError(message.clone()),
                UgcError::Forbidden,
            ];
            
            for error in errors {
                if !matches!(error, UgcError::RateLimitExceeded { .. }) {
                    prop_assert!(!error.is_retryable());
                    prop_assert_eq!(error.category(), "CLIENT_ERROR");
                }
            }
        }

        #[test]
        fn test_server_errors_categorization(
            service in ".*",
            message in ".*"
        ) {
            let errors = vec![
                UgcError::ExternalServiceError { service: service.clone(), message: message.clone() },
                UgcError::CircuitBreakerOpen { service: service.clone() },
                UgcError::ServiceTimeout { service: service.clone() },
                UgcError::CacheError(message.clone()),
                UgcError::ConnectionPoolExhausted,
                UgcError::InternalError(message.clone()),
            ];
            
            for error in errors {
                prop_assert_eq!(error.category(), "SERVER_ERROR");
            }
        }
    }
}