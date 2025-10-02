# Task 7: Component Diagram - Подробное объяснение компонентов отказоустойчивости

## 🎯 Цель диаграммы

Component диаграмма Task 7 детализирует **внутреннюю структуру компонентов системы отказоустойчивости**, показывая конкретные компоненты внутри каждого слоя, их взаимодействие и специализированные функции для обеспечения enterprise-grade надежности GraphQL федерации Auto.ru.

## 🏗️ Детальная структура компонентов отказоустойчивости

### 1. Error Handling Components - Компоненты обработки ошибок

#### UgcError Enum - Типизированная система ошибок
```rust
// ugc-subgraph/src/error/types.rs
use async_graphql::ErrorExtensions;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use uuid::Uuid;

/// Централизованная система типизированных ошибок с полным контекстом
#[derive(Error, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum UgcError {
    // === CLIENT ERRORS (4xx) - Ошибки клиента ===
    
    /// Отзыв не найден
    #[error("Review not found: {id}")]
    ReviewNotFound { 
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        search_context: Option<SearchContext>,
    },
    
    /// Неавторизованный доступ
    #[error("Unauthorized: user {user_id} cannot access review {review_id}")]
    Unauthorized { 
        user_id: Uuid, 
        review_id: Uuid,
        required_permission: Permission,
        current_permissions: Vec<Permission>,
    },
    
    /// Ошибка валидации
    #[error("Validation error in field '{field}': {message}")]
    ValidationError { 
        message: String,
        field: String,
        code: ValidationErrorCode,
        constraints: ValidationConstraints,
    },
    
    /// Ошибка аутентификации
    #[error("Authentication failed: {reason}")]
    AuthenticationError { 
        reason: String,
        auth_method: AuthMethod,
        retry_after: Option<Duration>,
    },
    
    /// Превышение лимита запросов
    #[error("Rate limit exceeded for user {user_id}: {current}/{limit} requests in {window}s")]
    RateLimitExceeded { 
        user_id: Uuid,
        current: u32,
        limit: u32,
        window: u32,
        retry_after: u64,
        rate_limit_type: RateLimitType,
    },
    
    /// Запрещенная операция
    #[error("Forbidden operation '{operation}' requires '{required_role}' role")]
    Forbidden {
        operation: String,
        required_role: UserRole,
        current_role: UserRole,
    },

    // === SERVER ERRORS (5xx) - Серверные ошибки ===
    
    /// Ошибка базы данных
    #[error("Database error in {operation}: {message}")]
    DatabaseError {
        message: String,
        operation: DatabaseOperation,
        #[serde(skip)]
        source_error: Option<sqlx::Error>,
        query_info: Option<QueryInfo>,
        connection_info: ConnectionInfo,
    },
    
    /// Ошибка внешнего сервиса
    #[error("External service '{service}' error: {message}")]
    ExternalServiceError { 
        service: String, 
        message: String,
        status_code: Option<u16>,
        retry_after: Option<u64>,
        endpoint: String,
        request_id: Option<String>,
    },
    
    /// Circuit Breaker открыт
    #[error("Circuit breaker open for service '{service}' (opened at {opened_at})")]
    CircuitBreakerOpen { 
        service: String,
        opened_at: chrono::DateTime<chrono::Utc>,
        estimated_recovery: chrono::DateTime<chrono::Utc>,
        failure_count: usize,
        last_failure_reason: String,
    },
    
    /// Таймаут сервиса
    #[error("Service '{service}' timeout after {timeout_ms}ms (attempt {attempt}/{max_attempts})")]
    ServiceTimeout { 
        service: String,
        timeout_ms: u64,
        attempt: u32,
        max_attempts: u32,
        operation: String,
    },
    
    /// Ошибка кеша
    #[error("Cache error in {operation} for key '{key}': {message}")]
    CacheError {
        operation: CacheOperation,
        key: String,
        message: String,
        cache_level: CacheLevel,
    },
    
    /// Исчерпание пула соединений
    #[error("Connection pool '{pool_name}' exhausted: {active}/{max} connections")]
    ConnectionPoolExhausted {
        pool_name: String,
        active_connections: u32,
        max_connections: u32,
        wait_time_ms: u64,
    },
    
    /// Ошибка конфигурации
    #[error("Configuration error for '{config_key}': {message}")]
    ConfigError {
        config_key: String,
        message: String,
        config_source: ConfigSource,
    },
    
    /// Внутренняя ошибка
    #[error("Internal error [{error_id}]: {message}")]
    InternalError {
        message: String,
        error_id: Uuid,
        component: String,
        severity: ErrorSeverity,
    },
}

// Вспомогательные типы для обогащения ошибок
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchContext {
    pub filters: Vec<String>,
    pub sort_order: Option<String>,
    pub pagination: Option<PaginationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    ReadReview,
    WriteReview,
    DeleteReview,
    ModerateReview,
    AdminAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationErrorCode {
    Required,
    TooShort,
    TooLong,
    InvalidFormat,
    InvalidRange,
    Duplicate,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub allowed_values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthMethod {
    JWT,
    OAuth2,
    ApiKey,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RateLimitType {
    PerUser,
    PerIP,
    PerEndpoint,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Guest,
    User,
    Premium,
    Moderator,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseOperation {
    Select,
    Insert,
    Update,
    Delete,
    Transaction,
    Migration,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryInfo {
    pub query: String,
    pub parameters: Vec<String>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConnectionInfo {
    pub pool_name: String,
    pub database_name: String,
    pub host: String,
    pub active_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheOperation {
    Get,
    Set,
    Delete,
    Invalidate,
    Expire,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheLevel {
    L1Memory,
    L2Redis,
    L3Static,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigSource {
    Environment,
    File,
    Database,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl UgcError {
    /// Определяет, можно ли повторить операцию при этой ошибке
    pub fn is_retryable(&self) -> bool {
        match self {
            // Клиентские ошибки обычно не повторяемы
            UgcError::ReviewNotFound { .. } => false,
            UgcError::Unauthorized { .. } => false,
            UgcError::ValidationError { .. } => false,
            UgcError::Forbidden { .. } => false,
            
            // Аутентификация может быть повторяема при временных проблемах
            UgcError::AuthenticationError { retry_after, .. } => retry_after.is_some(),
            
            // Rate limiting - повторяемо после задержки
            UgcError::RateLimitExceeded { .. } => true,
            
            // Серверные ошибки обычно повторяемы
            UgcError::DatabaseError { .. } => true,
            UgcError::ExternalServiceError { .. } => true,
            UgcError::ServiceTimeout { .. } => true,
            UgcError::ConnectionPoolExhausted { .. } => true,
            
            // Cache errors - повторяемы, но не критичны
            UgcError::CacheError { .. } => true,
            
            // Circuit breaker - не повторяемо (должен сработать fallback)
            UgcError::CircuitBreakerOpen { .. } => false,
            
            // Конфигурационные ошибки не повторяемы
            UgcError::ConfigError { .. } => false,
            
            // Внутренние ошибки - зависит от severity
            UgcError::InternalError { severity, .. } => {
                matches!(severity, ErrorSeverity::Low | ErrorSeverity::Medium)
            }
        }
    }
    
    /// Возвращает категорию ошибки для метрик и логирования
    pub fn category(&self) -> &'static str {
        match self {
            UgcError::ReviewNotFound { .. }
            | UgcError::Unauthorized { .. }
            | UgcError::ValidationError { .. }
            | UgcError::AuthenticationError { .. }
            | UgcError::RateLimitExceeded { .. }
            | UgcError::Forbidden { .. } => "CLIENT_ERROR",

            UgcError::DatabaseError { .. }
            | UgcError::ExternalServiceError { .. }
            | UgcError::CircuitBreakerOpen { .. }
            | UgcError::ServiceTimeout { .. }
            | UgcError::CacheError { .. }
            | UgcError::ConnectionPoolExhausted { .. }
            | UgcError::ConfigError { .. }
            | UgcError::InternalError { .. } => "SERVER_ERROR",
        }
    }
    
    /// Возвращает HTTP статус код
    pub fn status_code(&self) -> u16 {
        match self {
            UgcError::ReviewNotFound { .. } => 404,
            UgcError::Unauthorized { .. } => 401,
            UgcError::ValidationError { .. } => 400,
            UgcError::AuthenticationError { .. } => 401,
            UgcError::RateLimitExceeded { .. } => 429,
            UgcError::Forbidden { .. } => 403,
            
            UgcError::DatabaseError { .. } => 500,
            UgcError::ExternalServiceError { status_code, .. } => {
                status_code.unwrap_or(502)
            }
            UgcError::CircuitBreakerOpen { .. } => 503,
            UgcError::ServiceTimeout { .. } => 504,
            UgcError::CacheError { .. } => 500,
            UgcError::ConnectionPoolExhausted { .. } => 503,
            UgcError::ConfigError { .. } => 500,
            UgcError::InternalError { .. } => 500,
        }
    }
    
    /// Возвращает приоритет ошибки для алертов
    pub fn alert_priority(&self) -> AlertPriority {
        match self {
            UgcError::ReviewNotFound { .. } => AlertPriority::None,
            UgcError::ValidationError { .. } => AlertPriority::None,
            
            UgcError::Unauthorized { .. } => AlertPriority::Low,
            UgcError::Forbidden { .. } => AlertPriority::Low,
            UgcError::RateLimitExceeded { .. } => AlertPriority::Medium,
            
            UgcError::AuthenticationError { .. } => AlertPriority::Medium,
            UgcError::CacheError { .. } => AlertPriority::Medium,
            
            UgcError::ExternalServiceError { .. } => AlertPriority::High,
            UgcError::ServiceTimeout { .. } => AlertPriority::High,
            UgcError::CircuitBreakerOpen { .. } => AlertPriority::High,
            
            UgcError::DatabaseError { .. } => AlertPriority::Critical,
            UgcError::ConnectionPoolExhausted { .. } => AlertPriority::Critical,
            UgcError::ConfigError { .. } => AlertPriority::Critical,
            
            UgcError::InternalError { severity, .. } => match severity {
                ErrorSeverity::Low => AlertPriority::Low,
                ErrorSeverity::Medium => AlertPriority::Medium,
                ErrorSeverity::High => AlertPriority::High,
                ErrorSeverity::Critical => AlertPriority::Critical,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertPriority {
    None,
    Low,
    Medium,
    High,
    Critical,
}
```

#### Error Extensions - GraphQL расширения ошибок
```rust
// ugc-subgraph/src/error/extensions.rs
use async_graphql::{ErrorExtensions, Error as GraphQLError};
use serde_json::json;

impl ErrorExtensions for UgcError {
    fn extend(&self) -> GraphQLError {
        // Логируем ошибку перед обработкой
        self.log_structured_error();
        
        let mut error = GraphQLError::new(self.to_string());
        
        // Базовые расширения для всех ошибок
        error = error.extend_with(|_, e| {
            e.set("category", self.category());
            e.set("retryable", self.is_retryable());
            e.set("statusCode", self.status_code());
            e.set("alertPriority", format!("{:?}", self.alert_priority()));
            e.set("timestamp", chrono::Utc::now().to_rfc3339());
        });
        
        // Специфичные расширения для каждого типа ошибки
        match self {
            UgcError::ReviewNotFound { id, search_context } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "REVIEW_NOT_FOUND");
                    e.set("reviewId", id.to_string());
                    if let Some(context) = search_context {
                        e.set("searchContext", json!(context));
                    }
                    e.set("suggestions", json!([
                        "Check if the review ID is correct",
                        "Verify that the review exists and is not deleted",
                        "Try searching with different filters"
                    ]));
                });
            }
            
            UgcError::Unauthorized { user_id, review_id, required_permission, current_permissions } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "UNAUTHORIZED");
                    e.set("userId", user_id.to_string());
                    e.set("reviewId", review_id.to_string());
                    e.set("requiredPermission", format!("{:?}", required_permission));
                    e.set("currentPermissions", json!(current_permissions));
                    e.set("suggestions", json!([
                        "Login with appropriate credentials",
                        "Contact administrator for permission upgrade",
                        "Use a different account with required permissions"
                    ]));
                });
            }
            
            UgcError::ValidationError { message, field, code, constraints } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "VALIDATION_ERROR");
                    e.set("field", field);
                    e.set("validationCode", format!("{:?}", code));
                    e.set("constraints", json!(constraints));
                    e.set("suggestions", json!([
                        format!("Fix the '{}' field according to constraints", field),
                        "Check the API documentation for field requirements",
                        "Validate input data before sending"
                    ]));
                });
            }
            
            UgcError::RateLimitExceeded { user_id, current, limit, window, retry_after, rate_limit_type } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "RATE_LIMIT_EXCEEDED");
                    e.set("userId", user_id.to_string());
                    e.set("current", *current);
                    e.set("limit", *limit);
                    e.set("window", *window);
                    e.set("retryAfter", *retry_after);
                    e.set("rateLimitType", format!("{:?}", rate_limit_type));
                    e.set("suggestions", json!([
                        format!("Wait {} seconds before retrying", retry_after),
                        "Implement exponential backoff in your client",
                        "Consider upgrading to a higher rate limit tier"
                    ]));
                });
            }
            
            UgcError::CircuitBreakerOpen { service, opened_at, estimated_recovery, failure_count, last_failure_reason } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CIRCUIT_BREAKER_OPEN");
                    e.set("service", service);
                    e.set("openedAt", opened_at.to_rfc3339());
                    e.set("estimatedRecovery", estimated_recovery.to_rfc3339());
                    e.set("failureCount", *failure_count);
                    e.set("lastFailureReason", last_failure_reason);
                    e.set("fallbackAvailable", true);
                    e.set("suggestions", json!([
                        "Use cached data if available",
                        "Enable offline mode in your application",
                        format!("Retry after {}", estimated_recovery.to_rfc3339()),
                        "Check service status page for updates"
                    ]));
                });
            }
            
            UgcError::ExternalServiceError { service, message, status_code, retry_after, endpoint, request_id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "EXTERNAL_SERVICE_ERROR");
                    e.set("service", service);
                    e.set("endpoint", endpoint);
                    if let Some(status) = status_code {
                        e.set("upstreamStatusCode", *status);
                    }
                    if let Some(retry) = retry_after {
                        e.set("retryAfter", *retry);
                    }
                    if let Some(req_id) = request_id {
                        e.set("upstreamRequestId", req_id);
                    }
                    e.set("suggestions", json!([
                        "Retry the request with exponential backoff",
                        "Check if the external service is operational",
                        "Use fallback data if available",
                        "Contact support if the issue persists"
                    ]));
                });
            }
            
            UgcError::DatabaseError { message, operation, query_info, connection_info } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "DATABASE_ERROR");
                    e.set("operation", format!("{:?}", operation));
                    e.set("connectionInfo", json!(connection_info));
                    if let Some(query) = query_info {
                        e.set("queryInfo", json!({
                            "executionTimeMs": query.execution_time_ms,
                            "parameterCount": query.parameters.len()
                        }));
                    }
                    e.set("suggestions", json!([
                        "Retry the operation after a short delay",
                        "Check database connectivity",
                        "Verify that the database schema is up to date",
                        "Contact database administrator if issue persists"
                    ]));
                });
            }
            
            UgcError::ServiceTimeout { service, timeout_ms, attempt, max_attempts, operation } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "SERVICE_TIMEOUT");
                    e.set("service", service);
                    e.set("timeoutMs", *timeout_ms);
                    e.set("attempt", *attempt);
                    e.set("maxAttempts", *max_attempts);
                    e.set("operation", operation);
                    e.set("suggestions", json!([
                        "Retry with exponential backoff",
                        "Increase timeout if appropriate",
                        "Check network connectivity",
                        "Use cached data if available"
                    ]));
                });
            }
            
            UgcError::CacheError { operation, key, cache_level, .. } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CACHE_ERROR");
                    e.set("operation", format!("{:?}", operation));
                    e.set("cacheLevel", format!("{:?}", cache_level));
                    e.set("keyPrefix", key.split(':').next().unwrap_or("unknown"));
                    e.set("suggestions", json!([
                        "Operation will continue without cache",
                        "Cache will be automatically repaired",
                        "Performance may be temporarily degraded"
                    ]));
                });
            }
            
            UgcError::ConnectionPoolExhausted { pool_name, active_connections, max_connections, wait_time_ms } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CONNECTION_POOL_EXHAUSTED");
                    e.set("poolName", pool_name);
                    e.set("activeConnections", *active_connections);
                    e.set("maxConnections", *max_connections);
                    e.set("waitTimeMs", *wait_time_ms);
                    e.set("suggestions", json!([
                        "Retry after a short delay",
                        "Reduce concurrent requests",
                        "Consider increasing connection pool size",
                        "Check for connection leaks in the application"
                    ]));
                });
            }
            
            UgcError::InternalError { error_id, component, severity, .. } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "INTERNAL_ERROR");
                    e.set("errorId", error_id.to_string());
                    e.set("component", component);
                    e.set("severity", format!("{:?}", severity));
                    e.set("suggestions", json!([
                        format!("Reference error ID {} when contacting support", error_id),
                        "Retry the operation if appropriate",
                        "Check system status page for known issues"
                    ]));
                });
            }
            
            _ => {
                error = error.extend_with(|_, e| {
                    e.set("code", "UNKNOWN_ERROR");
                    e.set("suggestions", json!([
                        "Retry the operation",
                        "Contact support if the issue persists"
                    ]));
                });
            }
        }
        
        error
    }
}

impl UgcError {
    /// Структурированное логирование ошибки
    pub fn log_structured_error(&self) {
        use tracing::{error, warn, info, debug};
        
        let error_context = json!({
            "error_type": std::any::type_name::<Self>(),
            "category": self.category(),
            "status_code": self.status_code(),
            "retryable": self.is_retryable(),
            "alert_priority": format!("{:?}", self.alert_priority()),
        });
        
        match self.alert_priority() {
            AlertPriority::Critical => {
                error!(
                    error = %self,
                    context = %error_context,
                    "Critical error occurred"
                );
            }
            AlertPriority::High => {
                error!(
                    error = %self,
                    context = %error_context,
                    "High priority error occurred"
                );
            }
            AlertPriority::Medium => {
                warn!(
                    error = %self,
                    context = %error_context,
                    "Medium priority error occurred"
                );
            }
            AlertPriority::Low => {
                info!(
                    error = %self,
                    context = %error_context,
                    "Low priority error occurred"
                );
            }
            AlertPriority::None => {
                debug!(
                    error = %self,
                    context = %error_context,
                    "Informational error occurred"
                );
            }
        }
    }
}
```

#### Error Converter - Конвертация внешних ошибок
```rust
// ugc-subgraph/src/error/converter.rs
use sqlx::Error as SqlxError;
use reqwest::Error as ReqwestError;
use redis::RedisError;
use serde_json::Error as JsonError;

/// Автоматическая конвертация из sqlx::Error
impl From<SqlxError> for UgcError {
    fn from(error: SqlxError) -> Self {
        let connection_info = ConnectionInfo {
            pool_name: "main_pool".to_string(),
            database_name: "ugc_db".to_string(),
            host: "localhost".to_string(),
            active_connections: 0, // Будет заполнено из контекста
        };
        
        match error {
            SqlxError::RowNotFound => {
                // Не конвертируем в DatabaseError, так как это ожидаемое поведение
                UgcError::InternalError {
                    message: "Row not found in database".to_string(),
                    error_id: Uuid::new_v4(),
                    component: "database".to_string(),
                    severity: ErrorSeverity::Low,
                }
            }
            
            SqlxError::Database(db_err) => {
                let operation = Self::determine_db_operation_from_error(&db_err);
                
                UgcError::DatabaseError {
                    message: db_err.message().to_string(),
                    operation,
                    source_error: Some(SqlxError::Database(db_err)),
                    query_info: None, // Будет заполнено из контекста
                    connection_info,
                }
            }
            
            SqlxError::Io(io_err) => {
                UgcError::DatabaseError {
                    message: format!("Database I/O error: {}", io_err),
                    operation: DatabaseOperation::Select, // По умолчанию
                    source_error: Some(error),
                    query_info: None,
                    connection_info,
                }
            }
            
            SqlxError::PoolTimedOut => {
                UgcError::ConnectionPoolExhausted {
                    pool_name: "main_pool".to_string(),
                    active_connections: 0, // Будет заполнено из контекста
                    max_connections: 10,   // Будет заполнено из контекста
                    wait_time_ms: 5000,    // Стандартный таймаут
                }
            }
            
            _ => {
                UgcError::DatabaseError {
                    message: error.to_string(),
                    operation: DatabaseOperation::Select,
                    source_error: Some(error),
                    query_info: None,
                    connection_info,
                }
            }
        }
    }
}

/// Конвертация из reqwest::Error
impl From<ReqwestError> for UgcError {
    fn from(error: ReqwestError) -> Self {
        let service_name = error.url()
            .and_then(|url| url.host_str())
            .unwrap_or("unknown")
            .to_string();
            
        let endpoint = error.url()
            .map(|url| url.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        if error.is_timeout() {
            UgcError::ServiceTimeout {
                service: service_name,
                timeout_ms: 5000, // Стандартный таймаут
                attempt: 1,
                max_attempts: 3,
                operation: "http_request".to_string(),
            }
        } else if error.is_connect() {
            UgcError::ExternalServiceError {
                service: service_name,
                message: format!("Connection error: {}", error),
                status_code: None,
                retry_after: Some(30),
                endpoint,
                request_id: None,
            }
        } else if let Some(status) = error.status() {
            UgcError::ExternalServiceError {
                service: service_name,
                message: format!("HTTP error: {}", status),
                status_code: Some(status.as_u16()),
                retry_after: if status.is_server_error() { Some(60) } else { None },
                endpoint,
                request_id: None,
            }
        } else {
            UgcError::ExternalServiceError {
                service: service_name,
                message: error.to_string(),
                status_code: None,
                retry_after: Some(30),
                endpoint,
                request_id: None,
            }
        }
    }
}

/// Конвертация из Redis ошибок
impl From<RedisError> for UgcError {
    fn from(error: RedisError) -> Self {
        match error.kind() {
            redis::ErrorKind::IoError => {
                UgcError::CacheError {
                    operation: CacheOperation::Get,
                    key: "unknown".to_string(),
                    message: format!("Redis I/O error: {}", error),
                    cache_level: CacheLevel::L2Redis,
                }
            }
            
            redis::ErrorKind::AuthenticationFailed => {
                UgcError::CacheError {
                    operation: CacheOperation::Get,
                    key: "auth".to_string(),
                    message: "Redis authentication failed".to_string(),
                    cache_level: CacheLevel::L2Redis,
                }
            }
            
            redis::ErrorKind::TypeError => {
                UgcError::CacheError {
                    operation: CacheOperation::Get,
                    key: "unknown".to_string(),
                    message: format!("Redis type error: {}", error),
                    cache_level: CacheLevel::L2Redis,
                }
            }
            
            _ => {
                UgcError::CacheError {
                    operation: CacheOperation::Get,
                    key: "unknown".to_string(),
                    message: error.to_string(),
                    cache_level: CacheLevel::L2Redis,
                }
            }
        }
    }
}

/// Конвертация из JSON ошибок
impl From<JsonError> for UgcError {
    fn from(error: JsonError) -> Self {
        UgcError::ValidationError {
            message: format!("JSON parsing error: {}", error),
            field: "json_body".to_string(),
            code: ValidationErrorCode::InvalidFormat,
            constraints: ValidationConstraints {
                min_length: None,
                max_length: None,
                pattern: Some("valid JSON".to_string()),
                allowed_values: None,
            },
        }
    }
}

impl UgcError {
    /// Определение типа операции БД из ошибки
    fn determine_db_operation_from_error(db_err: &sqlx::database::DatabaseError) -> DatabaseOperation {
        let message = db_err.message().to_lowercase();
        
        if message.contains("insert") || message.contains("duplicate") {
            DatabaseOperation::Insert
        } else if message.contains("update") {
            DatabaseOperation::Update
        } else if message.contains("delete") {
            DatabaseOperation::Delete
        } else if message.contains("transaction") {
            DatabaseOperation::Transaction
        } else {
            DatabaseOperation::Select
        }
    }
    
    /// Обогащение ошибки контекстом из текущего запроса
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        match &mut self {
            UgcError::DatabaseError { query_info, connection_info, .. } => {
                if let Some(query) = context.query_info {
                    *query_info = Some(query);
                }
                if let Some(conn) = context.connection_info {
                    *connection_info = conn;
                }
            }
            
            UgcError::ExternalServiceError { request_id, .. } => {
                *request_id = context.request_id;
            }
            
            UgcError::CacheError { key, .. } => {
                if let Some(cache_key) = context.cache_key {
                    *key = cache_key;
                }
            }
            
            _ => {}
        }
        
        self
    }
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub query_info: Option<QueryInfo>,
    pub connection_info: Option<ConnectionInfo>,
    pub request_id: Option<String>,
    pub cache_key: Option<String>,
    pub user_id: Option<Uuid>,
    pub correlation_id: Option<String>,
}
```

### 2. Circuit Breaker Components - Компоненты Circuit Breaker

#### Circuit State - Управление состояниями
```rust
// ugc-subgraph/src/resilience/circuit_breaker/state.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Нормальная работа - все запросы проходят
    Closed,
    /// Сервис недоступен - все запросы отклоняются
    Open,
    /// Тестирование восстановления - ограниченные запросы
    HalfOpen,
}

impl CircuitState {
    /// Возвращает числовое представление состояния для метрик
    pub fn as_metric_value(&self) -> i64 {
        match self {
            CircuitState::Closed => 0,
            CircuitState::Open => 1,
            CircuitState::HalfOpen => 2,
        }
    }
    
    /// Проверяет, можно ли выполнить запрос в данном состоянии
    pub fn allows_requests(&self) -> bool {
        match self {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // Ограниченно
        }
    }
}

/// Управление состоянием Circuit Breaker с атомарными операциями
#[derive(Debug)]
pub struct CircuitStateManager {
    /// Текущее состояние Circuit Breaker
    current_state: Arc<RwLock<CircuitState>>,
    
    /// Атомарные счетчики для thread-safe операций
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    consecutive_failures: AtomicUsize,
    consecutive_successes: AtomicUsize,
    
    /// Счетчики для HalfOpen состояния
    half_open_calls: AtomicUsize,
    half_open_successes: AtomicUsize,
    half_open_failures: AtomicUsize,
    
    /// Временные метки
    last_failure_time: AtomicU64,
    last_success_time: AtomicU64,
    state_changed_at: AtomicU64,
    last_half_open_attempt: AtomicU64,
    
    /// Конфигурация
    config: CircuitBreakerConfig,
    
    /// Метрики
    metrics: Arc<CircuitStateMetrics>,
}

impl CircuitStateManager {
    pub fn new(config: CircuitBreakerConfig, service_name: String) -> Self {
        let now = Self::current_time_nanos();
        
        Self {
            current_state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            consecutive_failures: AtomicUsize::new(0),
            consecutive_successes: AtomicUsize::new(0),
            half_open_calls: AtomicUsize::new(0),
            half_open_successes: AtomicUsize::new(0),
            half_open_failures: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            last_success_time: AtomicU64::new(now),
            state_changed_at: AtomicU64::new(now),
            last_half_open_attempt: AtomicU64::new(0),
            config,
            metrics: Arc::new(CircuitStateMetrics::new(service_name)),
        }
    }

    /// Проверка возможности выполнения запроса
    pub async fn can_execute(&self) -> bool {
        let state = self.current_state.read().await;
        
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Проверяем, не пора ли перейти в HalfOpen
                let now = Self::current_time_nanos();
                let state_changed = self.state_changed_at.load(Ordering::Relaxed);
                
                if now - state_changed >= self.config.timeout.as_nanos() as u64 {
                    drop(state);
                    self.try_transition_to_half_open().await
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // В HalfOpen разрешаем ограниченное количество запросов
                let current_calls = self.half_open_calls.load(Ordering::Relaxed);
                current_calls < self.config.half_open_max_calls
            }
        }
    }

    /// Регистрация успешного выполнения
    pub async fn record_success(&self) {
        let now = Self::current_time_nanos();
        self.last_success_time.store(now, Ordering::Relaxed);
        
        // Увеличиваем общие счетчики
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        
        let state = self.current_state.read().await;
        match *state {
            CircuitState::Closed => {
                // В Closed состоянии сбрасываем счетчик ошибок
                self.failure_count.store(0, Ordering::Relaxed);
                self.metrics.record_success_in_closed().await;
            }
            
            CircuitState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::Relaxed) + 1;
                
                info!(
                    successes = successes,
                    threshold = self.config.success_threshold,
                    "Success in HalfOpen state"
                );
                
                self.metrics.record_success_in_half_open().await;
                
                // Проверяем, достаточно ли успехов для закрытия
                if successes >= self.config.success_threshold {
                    drop(state);
                    self.transition_to_closed().await;
                }
            }
            
            CircuitState::Open => {
                // Не должно происходить, но логируем
                warn!("Unexpected success in Open state");
                self.metrics.record_unexpected_success_in_open().await;
            }
        }
    }

    /// Регистрация неудачного выполнения
    pub async fn record_failure(&self, error: &UgcError) {
        let now = Self::current_time_nanos();
        self.last_failure_time.store(now, Ordering::Relaxed);
        
        // Увеличиваем общие счетчики
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        
        let state = self.current_state.read().await;
        match *state {
            CircuitState::Closed => {
                let failures = self.consecutive_failures.load(Ordering::Relaxed);
                
                warn!(
                    failures = failures,
                    threshold = self.config.failure_threshold,
                    error = %error,
                    "Failure in Closed state"
                );
                
                self.metrics.record_failure_in_closed(error).await;
                
                // Проверяем, достигли ли порога для открытия
                if failures >= self.config.failure_threshold {
                    drop(state);
                    self.transition_to_open(error).await;
                }
            }
            
            CircuitState::HalfOpen => {
                self.half_open_failures.fetch_add(1, Ordering::Relaxed);
                
                warn!(
                    error = %error,
                    "Failure in HalfOpen state, returning to Open"
                );
                
                self.metrics.record_failure_in_half_open(error).await;
                
                // Любая ошибка в HalfOpen возвращает в Open
                drop(state);
                self.transition_to_open(error).await;
            }
            
            CircuitState::Open => {
                // Увеличиваем счетчик отклоненных запросов
                self.metrics.record_rejected_call().await;
            }
        }
    }

    /// Переход в состояние Open
    async fn transition_to_open(&self, last_error: &UgcError) {
        let mut state = self.current_state.write().await;
        
        if *state != CircuitState::Open {
            let previous_state = *state;
            *state = CircuitState::Open;
            
            let now = Self::current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            
            // Сбрасываем счетчики HalfOpen
            self.half_open_calls.store(0, Ordering::Relaxed);
            self.half_open_successes.store(0, Ordering::Relaxed);
            self.half_open_failures.store(0, Ordering::Relaxed);
            
            let failure_count = self.consecutive_failures.load(Ordering::Relaxed);
            
            error!(
                previous_state = ?previous_state,
                failure_count = failure_count,
                last_error = %last_error,
                timeout_seconds = self.config.timeout.as_secs(),
                "Circuit breaker opened"
            );
            
            self.metrics.record_state_transition(previous_state, CircuitState::Open).await;
        }
    }

    /// Попытка перехода в состояние HalfOpen
    async fn try_transition_to_half_open(&self) -> bool {
        let mut state = self.current_state.write().await;
        
        if *state == CircuitState::Open {
            *state = CircuitState::HalfOpen;
            
            let now = Self::current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            self.last_half_open_attempt.store(now, Ordering::Relaxed);
            
            // Сбрасываем счетчики HalfOpen
            self.half_open_calls.store(0, Ordering::Relaxed);
            self.half_open_successes.store(0, Ordering::Relaxed);
            self.half_open_failures.store(0, Ordering::Relaxed);
            
            info!(
                timeout_duration = ?self.config.timeout,
                max_calls = self.config.half_open_max_calls,
                success_threshold = self.config.success_threshold,
                "Circuit breaker transitioned to HalfOpen"
            );
            
            self.metrics.record_state_transition(CircuitState::Open, CircuitState::HalfOpen).await;
            
            true
        } else {
            false
        }
    }

    /// Переход в состояние Closed
    async fn transition_to_closed(&self) {
        let mut state = self.current_state.write().await;
        
        if *state != CircuitState::Closed {
            let previous_state = *state;
            *state = CircuitState::Closed;
            
            let now = Self::current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            
            // Сбрасываем все счетчики
            self.failure_count.store(0, Ordering::Relaxed);
            self.consecutive_failures.store(0, Ordering::Relaxed);
            self.half_open_calls.store(0, Ordering::Relaxed);
            self.half_open_successes.store(0, Ordering::Relaxed);
            self.half_open_failures.store(0, Ordering::Relaxed);
            
            let success_count = self.half_open_successes.load(Ordering::Relaxed);
            
            info!(
                previous_state = ?previous_state,
                success_count = success_count,
                "Circuit breaker closed - service recovered"
            );
            
            self.metrics.record_state_transition(previous_state, CircuitState::Closed).await;
        }
    }

    /// Принудительное открытие (для maintenance)
    pub async fn force_open(&self) {
        let mut state = self.current_state.write().await;
        let previous_state = *state;
        *state = CircuitState::Open;
        
        let now = Self::current_time_nanos();
        self.state_changed_at.store(now, Ordering::Relaxed);
        
        warn!(
            previous_state = ?previous_state,
            "Circuit breaker forcefully opened for maintenance"
        );
        
        self.metrics.record_forced_state_change(previous_state, CircuitState::Open).await;
    }

    /// Принудительное закрытие
    pub async fn force_close(&self) {
        let mut state = self.current_state.write().await;
        let previous_state = *state;
        *state = CircuitState::Closed;
        
        let now = Self::current_time_nanos();
        self.state_changed_at.store(now, Ordering::Relaxed);
        
        // Сбрасываем счетчики
        self.failure_count.store(0, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        
        info!(
            previous_state = ?previous_state,
            "Circuit breaker forcefully closed"
        );
        
        self.metrics.record_forced_state_change(previous_state, CircuitState::Closed).await;
    }

    /// Получение текущего состояния
    pub async fn get_current_state(&self) -> CircuitState {
        *self.current_state.read().await
    }

    /// Получение статистики
    pub async fn get_statistics(&self) -> CircuitStateStatistics {
        let state = *self.current_state.read().await;
        
        CircuitStateStatistics {
            current_state: state,
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            consecutive_successes: self.consecutive_successes.load(Ordering::Relaxed),
            half_open_calls: self.half_open_calls.load(Ordering::Relaxed),
            half_open_successes: self.half_open_successes.load(Ordering::Relaxed),
            half_open_failures: self.half_open_failures.load(Ordering::Relaxed),
            last_failure_time: self.get_timestamp_from_nanos(self.last_failure_time.load(Ordering::Relaxed)),
            last_success_time: self.get_timestamp_from_nanos(self.last_success_time.load(Ordering::Relaxed)),
            state_changed_at: self.get_timestamp_from_nanos(self.state_changed_at.load(Ordering::Relaxed)),
            config: self.config.clone(),
        }
    }

    /// Регистрация вызова в HalfOpen состоянии
    pub async fn record_half_open_call(&self) {
        self.half_open_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Получение текущего времени в наносекундах
    fn current_time_nanos() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    /// Конвертация наносекунд в DateTime
    fn get_timestamp_from_nanos(&self, nanos: u64) -> chrono::DateTime<chrono::Utc> {
        if nanos == 0 {
            return chrono::DateTime::from_timestamp(0, 0).unwrap_or_else(chrono::Utc::now);
        }
        
        let secs = nanos / 1_000_000_000;
        let nsecs = (nanos % 1_000_000_000) as u32;
        
        chrono::DateTime::from_timestamp(secs as i64, nsecs)
            .unwrap_or_else(chrono::Utc::now)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitStateStatistics {
    pub current_state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub consecutive_failures: usize,
    pub consecutive_successes: usize,
    pub half_open_calls: usize,
    pub half_open_successes: usize,
    pub half_open_failures: usize,
    pub last_failure_time: chrono::DateTime<chrono::Utc>,
    pub last_success_time: chrono::DateTime<chrono::Utc>,
    pub state_changed_at: chrono::DateTime<chrono::Utc>,
    pub config: CircuitBreakerConfig,
}

/// Метрики для состояния Circuit Breaker
#[derive(Debug)]
pub struct CircuitStateMetrics {
    service_name: String,
    // Здесь будут Prometheus метрики
}

impl CircuitStateMetrics {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }

    pub async fn record_success_in_closed(&self) {
        debug!(service = %self.service_name, "Success recorded in Closed state");
    }

    pub async fn record_success_in_half_open(&self) {
        info!(service = %self.service_name, "Success recorded in HalfOpen state");
    }

    pub async fn record_unexpected_success_in_open(&self) {
        warn!(service = %self.service_name, "Unexpected success in Open state");
    }

    pub async fn record_failure_in_closed(&self, error: &UgcError) {
        warn!(
            service = %self.service_name,
            error = %error,
            "Failure recorded in Closed state"
        );
    }

    pub async fn record_failure_in_half_open(&self, error: &UgcError) {
        warn!(
            service = %self.service_name,
            error = %error,
            "Failure recorded in HalfOpen state"
        );
    }

    pub async fn record_rejected_call(&self) {
        debug!(service = %self.service_name, "Call rejected in Open state");
    }

    pub async fn record_state_transition(&self, from: CircuitState, to: CircuitState) {
        info!(
            service = %self.service_name,
            from_state = ?from,
            to_state = ?to,
            "Circuit breaker state transition"
        );
    }

    pub async fn record_forced_state_change(&self, from: CircuitState, to: CircuitState) {
        warn!(
            service = %self.service_name,
            from_state = ?from,
            to_state = ?to,
            "Forced circuit breaker state change"
        );
    }
}
```

Эта Component диаграмма демонстрирует детальную внутреннюю структуру каждого компонента системы отказоустойчивости, показывая как типизированные ошибки, Circuit Breaker состояния, retry механизмы и fallback компоненты работают вместе для обеспечения enterprise-grade надежности с полным мониторингом и наблюдаемостью.