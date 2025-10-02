# Task 7: Context Diagram - Подробное объяснение системы обработки ошибок и отказоустойчивости

## 🎯 Цель диаграммы

Context диаграмма Task 7 демонстрирует **комплексную enterprise-grade систему обработки ошибок и отказоустойчивости** для федеративной GraphQL платформы Auto.ru, показывая как система защищается от каскадных сбоев, обеспечивает graceful degradation и поддерживает высокую доступность через Circuit Breaker паттерн и fallback механизмы.

## 🏗️ Архитектурная эволюция: от хрупкой к отказоустойчивой системе

### От хрупкой архитектуры к resilient системе

#### Было: Хрупкая система без защиты
```rust
// Простой вызов без защиты
async fn get_user_review(user_id: Uuid) -> Result<Review, Error> {
    // Прямой вызов внешнего сервиса - точка отказа
    let user = external_service.get_user(user_id).await?;
    
    // Если сервис недоступен - вся операция падает
    let review = create_review_for_user(user).await?;
    Ok(review)
}

// Проблемы:
// - Каскадные сбои при недоступности внешних сервисов
// - Отсутствие retry логики
// - Нет fallback механизмов
// - Плохая наблюдаемость ошибок
```

#### Стало: Отказоустойчивая система с защитой
```rust
// Защищенный вызов с полной отказоустойчивостью
async fn get_user_review_resilient(user_id: Uuid) -> Result<Review, UgcError> {
    // 1. Circuit Breaker защищает от каскадных сбоев
    let user = circuit_breaker
        .call(|| {
            // 2. Retry механизм с exponential backoff
            retry_mechanism.call(|| {
                external_service.get_user(user_id)
            })
        })
        .await
        // 3. Graceful degradation с fallback данными
        .unwrap_or_else(|_| fallback_provider.get_user_fallback(user_id).await);
    
    // 4. Типизированные ошибки с полным контекстом
    let review = create_review_for_user(user).await
        .map_err(|e| UgcError::ReviewCreationFailed { 
            user_id, 
            reason: e.to_string() 
        })?;
    
    Ok(review)
}

// Преимущества:
// ✅ Защита от каскадных сбоев через Circuit Breaker
// ✅ Автоматическое восстановление через retry логику
// ✅ Graceful degradation с fallback данными
// ✅ Полная наблюдаемость и мониторинг
// ✅ Типизированные ошибки с контекстом
```

**Объяснение**: Отказоустойчивая архитектура превращает хрупкую систему в resilient платформу, которая продолжает работать даже при сбоях внешних зависимостей, обеспечивая высокую доступность и качество пользовательского опыта.

## 🔧 Ключевые компоненты и их реализация

### 1. Resilient Auto.ru Federation System - Основная отказоустойчивая система

#### UGC Subgraph (Resilient) - Отказоустойчивый подграф
```rust
// ugc-subgraph/src/main.rs
use std::sync::Arc;
use axum::{routing::post, Router, Extension};
use tower::ServiceBuilder;

#[derive(Clone)]
pub struct ResilientUgcService {
    // Система обработки ошибок
    error_handler: Arc<ErrorHandler>,
    
    // Circuit Breaker для внешних сервисов
    users_circuit_breaker: Arc<CircuitBreaker>,
    offers_circuit_breaker: Arc<CircuitBreaker>,
    
    // Retry механизм
    retry_engine: Arc<RetryEngine>,
    
    // Fallback провайдер
    fallback_provider: Arc<FallbackDataProvider>,
    
    // Мониторинг здоровья
    health_monitor: Arc<ServiceHealthMonitor>,
    
    // Метрики отказоустойчивости
    resilience_metrics: Arc<ResilienceMetrics>,
}

impl ResilientUgcService {
    pub fn new() -> Result<Self, ServiceError> {
        // Конфигурация Circuit Breaker для разных сервисов
        let users_cb_config = CircuitBreakerConfig {
            failure_threshold: 5,           // 5 ошибок для открытия
            timeout: Duration::from_secs(30), // 30 сек до HalfOpen
            success_threshold: 3,           // 3 успеха для закрытия
            failure_window: Duration::from_secs(60), // Окно подсчета ошибок
        };
        
        let offers_cb_config = CircuitBreakerConfig {
            failure_threshold: 3,           // Более чувствительный для offers
            timeout: Duration::from_secs(60),
            success_threshold: 2,
            failure_window: Duration::from_secs(120),
        };
        
        // Конфигурация retry с exponential backoff
        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: true, // Добавляем jitter для избежания thundering herd
        };
        
        Ok(Self {
            error_handler: Arc::new(ErrorHandler::new()),
            users_circuit_breaker: Arc::new(CircuitBreaker::new("users", users_cb_config)),
            offers_circuit_breaker: Arc::new(CircuitBreaker::new("offers", offers_cb_config)),
            retry_engine: Arc::new(RetryEngine::new(retry_config)),
            fallback_provider: Arc::new(FallbackDataProvider::new()),
            health_monitor: Arc::new(ServiceHealthMonitor::new()),
            resilience_metrics: Arc::new(ResilienceMetrics::new()?),
        })
    }

    /// Создание веб-сервера с resilience middleware
    pub fn create_server(&self) -> Result<Router, ServiceError> {
        let schema = self.create_graphql_schema();
        
        let app = Router::new()
            .route("/graphql", post(graphql_handler))
            .route("/health", get(health_check))
            .route("/metrics", get(metrics_handler))
            .layer(Extension(schema))
            .layer(Extension(self.clone()))
            .layer(
                ServiceBuilder::new()
                    // Resilience middleware stack
                    .layer(self.create_error_handling_middleware())
                    .layer(self.create_circuit_breaker_middleware())
                    .layer(self.create_retry_middleware())
                    .layer(self.create_fallback_middleware())
                    .layer(self.create_metrics_middleware())
            );

        Ok(app)
    }

    /// Создание middleware для обработки ошибок
    fn create_error_handling_middleware(&self) -> impl Layer<Router> {
        let error_handler = self.error_handler.clone();
        let metrics = self.resilience_metrics.clone();
        
        tower::layer::layer_fn(move |service| {
            let error_handler = error_handler.clone();
            let metrics = metrics.clone();
            
            tower::service_fn(move |request| {
                let error_handler = error_handler.clone();
                let metrics = metrics.clone();
                let service = service.clone();
                
                async move {
                    let start_time = std::time::Instant::now();
                    
                    match service.call(request).await {
                        Ok(response) => {
                            // Записываем метрики успешного запроса
                            metrics.record_request_success(start_time.elapsed());
                            Ok(response)
                        }
                        Err(error) => {
                            // Обрабатываем ошибку через централизованный handler
                            let processed_error = error_handler.process_error(error).await;
                            
                            // Записываем метрики ошибки
                            metrics.record_request_error(
                                &processed_error.category(),
                                start_time.elapsed()
                            );
                            
                            Err(processed_error)
                        }
                    }
                }
            })
        })
    }
}

/// Централизованный обработчик ошибок
#[derive(Debug)]
pub struct ErrorHandler {
    logger: Arc<StructuredLogger>,
    metrics_collector: Arc<ErrorMetricsCollector>,
}

impl ErrorHandler {
    pub async fn process_error(&self, error: UgcError) -> UgcError {
        // 1. Структурированное логирование
        self.logger.log_error(&error).await;
        
        // 2. Сбор метрик по категориям
        self.metrics_collector.record_error(&error).await;
        
        // 3. Обогащение ошибки контекстом
        self.enrich_error_context(error).await
    }
    
    async fn enrich_error_context(&self, mut error: UgcError) -> UgcError {
        match &mut error {
            UgcError::ExternalServiceError { service, message } => {
                // Добавляем информацию о состоянии Circuit Breaker
                let cb_state = self.get_circuit_breaker_state(service).await;
                *message = format!("{} (CB State: {:?})", message, cb_state);
            }
            UgcError::CircuitBreakerOpen { service } => {
                // Добавляем информацию о времени восстановления
                let recovery_time = self.get_estimated_recovery_time(service).await;
                error = UgcError::CircuitBreakerOpen { 
                    service: format!("{} (Recovery in: {}s)", service, recovery_time.as_secs())
                };
            }
            _ => {}
        }
        error
    }
}
```

### 2. Error Handling System - Централизованная система обработки ошибок

#### Типизированные ошибки с полным контекстом
```rust
// ugc-subgraph/src/error.rs
use async_graphql::ErrorExtensions;
use thiserror::Error;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum UgcError {
    // === CLIENT ERRORS (4xx) ===
    #[error("Review not found: {id}")]
    ReviewNotFound { 
        id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<String>,
    },
    
    #[error("Unauthorized: user {user_id} cannot access review {review_id}")]
    Unauthorized { 
        user_id: Uuid, 
        review_id: Uuid,
        reason: String,
    },
    
    #[error("Validation error: {message}")]
    ValidationError { 
        message: String,
        field: Option<String>,
        code: String,
    },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationError { 
        reason: String,
        retry_after: Option<u64>, // seconds
    },
    
    #[error("Rate limit exceeded for user {user_id}")]
    RateLimitExceeded { 
        user_id: Uuid,
        retry_after: u64, // seconds
        limit: u32,
        window: u32, // seconds
    },
    
    #[error("Forbidden operation")]
    Forbidden {
        operation: String,
        required_permission: String,
    },

    // === SERVER ERRORS (5xx) ===
    #[error("Database error: {message}")]
    DatabaseError {
        message: String,
        #[serde(skip)]
        source: Option<sqlx::Error>,
        query: Option<String>,
    },
    
    #[error("External service error: {service} - {message}")]
    ExternalServiceError { 
        service: String, 
        message: String,
        status_code: Option<u16>,
        retry_after: Option<u64>,
    },
    
    #[error("Circuit breaker open for service: {service}")]
    CircuitBreakerOpen { 
        service: String,
        opened_at: chrono::DateTime<chrono::Utc>,
        estimated_recovery: chrono::DateTime<chrono::Utc>,
    },
    
    #[error("Service timeout: {service} after {timeout_ms}ms")]
    ServiceTimeout { 
        service: String,
        timeout_ms: u64,
        attempt: u32,
    },
    
    #[error("Cache error: {operation} failed - {message}")]
    CacheError {
        operation: String, // get, set, delete, etc.
        message: String,
        key: Option<String>,
    },
    
    #[error("Connection pool exhausted")]
    ConnectionPoolExhausted {
        pool_name: String,
        max_connections: u32,
        active_connections: u32,
    },
    
    #[error("Configuration error: {message}")]
    ConfigError {
        message: String,
        config_key: Option<String>,
    },
    
    #[error("Internal error: {message}")]
    InternalError {
        message: String,
        error_id: Uuid, // Для трассировки
    },
}

impl UgcError {
    /// Определяет, можно ли повторить операцию при этой ошибке
    pub fn is_retryable(&self) -> bool {
        match self {
            // Клиентские ошибки обычно не повторяемы
            UgcError::ReviewNotFound { .. } => false,
            UgcError::Unauthorized { .. } => false,
            UgcError::ValidationError { .. } => false,
            UgcError::AuthenticationError { .. } => false,
            UgcError::Forbidden { .. } => false,
            
            // Rate limiting - повторяемо после задержки
            UgcError::RateLimitExceeded { .. } => true,
            
            // Серверные ошибки обычно повторяемы
            UgcError::DatabaseError { .. } => true,
            UgcError::ExternalServiceError { .. } => true,
            UgcError::ServiceTimeout { .. } => true,
            UgcError::CacheError { .. } => true,
            UgcError::ConnectionPoolExhausted { .. } => true,
            
            // Circuit breaker - не повторяемо (будет fallback)
            UgcError::CircuitBreakerOpen { .. } => false,
            
            // Конфигурационные ошибки не повторяемы
            UgcError::ConfigError { .. } => false,
            
            // Внутренние ошибки - зависит от контекста
            UgcError::InternalError { .. } => false,
        }
    }
    
    /// Возвращает категорию ошибки для метрик
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
            UgcError::ExternalServiceError { .. } => 502,
            UgcError::CircuitBreakerOpen { .. } => 503,
            UgcError::ServiceTimeout { .. } => 504,
            UgcError::CacheError { .. } => 500,
            UgcError::ConnectionPoolExhausted { .. } => 503,
            UgcError::ConfigError { .. } => 500,
            UgcError::InternalError { .. } => 500,
        }
    }
    
    /// Структурированное логирование ошибки
    pub fn log_error(&self) {
        use tracing::{error, warn, info};
        
        match self {
            // Info level для ожидаемых клиентских ошибок
            UgcError::ReviewNotFound { id, context } => {
                info!(
                    error = %self,
                    review_id = %id,
                    context = ?context,
                    error_code = "REVIEW_NOT_FOUND",
                    category = "CLIENT_ERROR",
                    "Review not found"
                );
            }
            
            // Warn level для проблем безопасности
            UgcError::Unauthorized { user_id, review_id, reason } => {
                warn!(
                    error = %self,
                    user_id = %user_id,
                    review_id = %review_id,
                    reason = reason,
                    error_code = "UNAUTHORIZED",
                    category = "CLIENT_ERROR",
                    "Unauthorized access attempt"
                );
            }
            
            UgcError::RateLimitExceeded { user_id, retry_after, limit, window } => {
                warn!(
                    error = %self,
                    user_id = %user_id,
                    retry_after = retry_after,
                    limit = limit,
                    window = window,
                    error_code = "RATE_LIMIT_EXCEEDED",
                    category = "CLIENT_ERROR",
                    "Rate limit exceeded"
                );
            }
            
            // Error level для серверных ошибок
            UgcError::DatabaseError { message, query, .. } => {
                error!(
                    error = %self,
                    message = message,
                    query = ?query,
                    error_code = "DATABASE_ERROR",
                    category = "SERVER_ERROR",
                    "Database operation failed"
                );
            }
            
            UgcError::ExternalServiceError { service, message, status_code, .. } => {
                error!(
                    error = %self,
                    service = service,
                    message = message,
                    status_code = ?status_code,
                    error_code = "EXTERNAL_SERVICE_ERROR",
                    category = "SERVER_ERROR",
                    "External service call failed"
                );
            }
            
            UgcError::CircuitBreakerOpen { service, opened_at, estimated_recovery } => {
                warn!(
                    error = %self,
                    service = service,
                    opened_at = %opened_at,
                    estimated_recovery = %estimated_recovery,
                    error_code = "CIRCUIT_BREAKER_OPEN",
                    category = "SERVER_ERROR",
                    "Circuit breaker is open"
                );
            }
            
            // Остальные ошибки...
            _ => {
                error!(
                    error = %self,
                    error_code = "UNKNOWN_ERROR",
                    category = self.category(),
                    "Unhandled error occurred"
                );
            }
        }
    }
}

/// Реализация GraphQL Extensions для богатых ошибок
impl ErrorExtensions for UgcError {
    fn extend(&self) -> async_graphql::Error {
        // Логируем ошибку
        self.log_error();
        
        let mut error = async_graphql::Error::new(self.to_string());
        
        match self {
            UgcError::ReviewNotFound { id, context } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "REVIEW_NOT_FOUND");
                    e.set("reviewId", id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                    e.set("statusCode", 404);
                    if let Some(ctx) = context {
                        e.set("context", ctx.clone());
                    }
                });
            }
            
            UgcError::CircuitBreakerOpen { service, opened_at, estimated_recovery } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CIRCUIT_BREAKER_OPEN");
                    e.set("service", service.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", false); // Клиент должен использовать fallback
                    e.set("statusCode", 503);
                    e.set("openedAt", opened_at.to_rfc3339());
                    e.set("estimatedRecovery", estimated_recovery.to_rfc3339());
                    e.set("fallbackAvailable", true);
                });
            }
            
            UgcError::RateLimitExceeded { user_id, retry_after, limit, window } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "RATE_LIMIT_EXCEEDED");
                    e.set("userId", user_id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", true);
                    e.set("statusCode", 429);
                    e.set("retryAfter", *retry_after);
                    e.set("limit", *limit);
                    e.set("window", *window);
                });
            }
            
            UgcError::ExternalServiceError { service, message, status_code, retry_after } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "EXTERNAL_SERVICE_ERROR");
                    e.set("service", service.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", self.is_retryable());
                    e.set("statusCode", status_code.unwrap_or(502));
                    if let Some(retry) = retry_after {
                        e.set("retryAfter", *retry);
                    }
                });
            }
            
            // Добавляем extensions для всех остальных типов ошибок...
            _ => {
                error = error.extend_with(|_, e| {
                    e.set("code", "GENERIC_ERROR");
                    e.set("category", self.category());
                    e.set("retryable", self.is_retryable());
                    e.set("statusCode", self.status_code());
                });
            }
        }
        
        error
    }
}
```

### 3. Circuit Breaker System - Система защиты от каскадных сбоев

#### Реализация Circuit Breaker с автоматическими переходами состояний
```rust
// ugc-subgraph/src/resilience/circuit_breaker.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Нормальная работа - все запросы проходят
    Open,     // Сервис недоступен - все запросы отклоняются
    HalfOpen, // Тестирование восстановления - ограниченные запросы
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Количество ошибок для перехода из Closed в Open
    pub failure_threshold: usize,
    
    /// Время ожидания перед переходом из Open в HalfOpen
    pub timeout: Duration,
    
    /// Количество успешных запросов для перехода из HalfOpen в Closed
    pub success_threshold: usize,
    
    /// Окно времени для подсчета ошибок
    pub failure_window: Duration,
    
    /// Максимальное количество запросов в HalfOpen состоянии
    pub half_open_max_calls: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    service_name: String,
    config: CircuitBreakerConfig,
    
    // Состояние Circuit Breaker
    state: Arc<RwLock<CircuitState>>,
    
    // Атомарные счетчики для thread-safe операций
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    half_open_calls: AtomicUsize,
    
    // Временные метки
    last_failure_time: AtomicU64,
    last_success_time: AtomicU64,
    state_changed_at: AtomicU64,
    
    // Метрики для мониторинга
    metrics: Arc<CircuitBreakerMetrics>,
}

impl CircuitBreaker {
    pub fn new(service_name: String, config: CircuitBreakerConfig) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
            
        Self {
            service_name: service_name.clone(),
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            half_open_calls: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            last_success_time: AtomicU64::new(now),
            state_changed_at: AtomicU64::new(now),
            metrics: Arc::new(CircuitBreakerMetrics::new(service_name)),
        }
    }

    /// Основной метод для выполнения операции через Circuit Breaker
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, UgcError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        // 1. Проверяем текущее состояние и возможность выполнения
        if !self.can_execute().await {
            self.metrics.record_rejected_call();
            return Err(UgcError::CircuitBreakerOpen {
                service: self.service_name.clone(),
                opened_at: self.get_state_changed_time(),
                estimated_recovery: self.get_estimated_recovery_time(),
            });
        }

        // 2. Выполняем операцию
        let start_time = std::time::Instant::now();
        let result = f().await;
        let duration = start_time.elapsed();

        // 3. Обрабатываем результат
        match &result {
            Ok(_) => {
                self.on_success().await;
                self.metrics.record_successful_call(duration);
            }
            Err(error) => {
                // Проверяем, является ли ошибка причиной для Circuit Breaker
                if self.should_count_as_failure(error) {
                    self.on_failure().await;
                    self.metrics.record_failed_call(duration);
                } else {
                    // Ошибки типа "не найдено" не должны открывать Circuit Breaker
                    self.metrics.record_ignored_error();
                }
            }
        }

        result
    }

    /// Проверяет, можно ли выполнить запрос в текущем состоянии
    async fn can_execute(&self) -> bool {
        let state = self.state.read().await;
        
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Проверяем, не пора ли перейти в HalfOpen
                let now = self.current_time_nanos();
                let state_changed = self.state_changed_at.load(Ordering::Relaxed);
                
                if now - state_changed >= self.config.timeout.as_nanos() as u64 {
                    drop(state);
                    self.transition_to_half_open().await;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // В HalfOpen состоянии разрешаем ограниченное количество запросов
                let current_calls = self.half_open_calls.load(Ordering::Relaxed);
                current_calls < self.config.half_open_max_calls
            }
        }
    }

    /// Обработка успешного выполнения
    async fn on_success(&self) {
        let now = self.current_time_nanos();
        self.last_success_time.store(now, Ordering::Relaxed);

        let state = self.state.read().await;
        match *state {
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                
                info!(
                    service = %self.service_name,
                    success_count = success_count,
                    threshold = self.config.success_threshold,
                    "Circuit breaker success in HalfOpen state"
                );
                
                if success_count >= self.config.success_threshold {
                    drop(state);
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Closed => {
                // В Closed состоянии сбрасываем счетчик ошибок при успехе
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Не должно происходить, но на всякий случай
                warn!(
                    service = %self.service_name,
                    "Unexpected success in Open state"
                );
            }
        }
    }

    /// Обработка неудачного выполнения
    async fn on_failure(&self) {
        let now = self.current_time_nanos();
        self.last_failure_time.store(now, Ordering::Relaxed);

        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                
                warn!(
                    service = %self.service_name,
                    failure_count = failure_count,
                    threshold = self.config.failure_threshold,
                    "Circuit breaker failure in Closed state"
                );
                
                if failure_count >= self.config.failure_threshold {
                    drop(state);
                    self.transition_to_open().await;
                }
            }
            CircuitState::HalfOpen => {
                // Любая ошибка в HalfOpen переводит обратно в Open
                warn!(
                    service = %self.service_name,
                    "Circuit breaker failure in HalfOpen state, returning to Open"
                );
                
                drop(state);
                self.transition_to_open().await;
            }
            CircuitState::Open => {
                // Увеличиваем счетчик отклоненных запросов
                self.metrics.record_rejected_call();
            }
        }
    }

    /// Переход в состояние Open (сервис недоступен)
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        if *state != CircuitState::Open {
            *state = CircuitState::Open;
            
            let now = self.current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            
            // Сбрасываем счетчики
            self.success_count.store(0, Ordering::Relaxed);
            self.half_open_calls.store(0, Ordering::Relaxed);
            
            error!(
                service = %self.service_name,
                failure_count = self.failure_count.load(Ordering::Relaxed),
                "Circuit breaker opened - service marked as unavailable"
            );
            
            self.metrics.record_state_change(CircuitState::Open);
        }
    }

    /// Переход в состояние HalfOpen (тестирование восстановления)
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        if *state != CircuitState::HalfOpen {
            *state = CircuitState::HalfOpen;
            
            let now = self.current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            
            // Сбрасываем счетчики для тестирования
            self.success_count.store(0, Ordering::Relaxed);
            self.half_open_calls.store(0, Ordering::Relaxed);
            
            info!(
                service = %self.service_name,
                timeout_duration = ?self.config.timeout,
                "Circuit breaker transitioned to HalfOpen - testing service recovery"
            );
            
            self.metrics.record_state_change(CircuitState::HalfOpen);
        }
    }

    /// Переход в состояние Closed (нормальная работа)
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        if *state != CircuitState::Closed {
            *state = CircuitState::Closed;
            
            let now = self.current_time_nanos();
            self.state_changed_at.store(now, Ordering::Relaxed);
            
            // Сбрасываем все счетчики
            self.failure_count.store(0, Ordering::Relaxed);
            self.success_count.store(0, Ordering::Relaxed);
            self.half_open_calls.store(0, Ordering::Relaxed);
            
            info!(
                service = %self.service_name,
                "Circuit breaker closed - service recovered and available"
            );
            
            self.metrics.record_state_change(CircuitState::Closed);
        }
    }

    /// Определяет, должна ли ошибка учитываться для Circuit Breaker
    fn should_count_as_failure(&self, error: &UgcError) -> bool {
        match error {
            // Клиентские ошибки не должны открывать Circuit Breaker
            UgcError::ReviewNotFound { .. } => false,
            UgcError::Unauthorized { .. } => false,
            UgcError::ValidationError { .. } => false,
            UgcError::Forbidden { .. } => false,
            
            // Серверные ошибки и проблемы с внешними сервисами должны
            UgcError::DatabaseError { .. } => true,
            UgcError::ExternalServiceError { .. } => true,
            UgcError::ServiceTimeout { .. } => true,
            UgcError::ConnectionPoolExhausted { .. } => true,
            UgcError::CacheError { .. } => false, // Кеш не критичен
            
            // Rate limiting - спорный случай, зависит от контекста
            UgcError::RateLimitExceeded { .. } => false,
            
            // Остальные ошибки
            _ => true,
        }
    }

    /// Получение текущего времени в наносекундах
    fn current_time_nanos(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    /// Получение времени изменения состояния
    fn get_state_changed_time(&self) -> chrono::DateTime<chrono::Utc> {
        let nanos = self.state_changed_at.load(Ordering::Relaxed);
        let secs = nanos / 1_000_000_000;
        let nsecs = (nanos % 1_000_000_000) as u32;
        
        chrono::DateTime::from_timestamp(secs as i64, nsecs)
            .unwrap_or_else(chrono::Utc::now)
    }

    /// Получение предполагаемого времени восстановления
    fn get_estimated_recovery_time(&self) -> chrono::DateTime<chrono::Utc> {
        let state_changed = self.get_state_changed_time();
        state_changed + chrono::Duration::from_std(self.config.timeout).unwrap()
    }

    /// Получение текущего состояния (для мониторинга)
    pub async fn get_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Получение статистики (для мониторинга)
    pub async fn get_stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            service_name: self.service_name.clone(),
            state: self.get_state().await,
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            last_failure_time: self.get_state_changed_time(),
            last_success_time: self.get_state_changed_time(),
            config: self.config.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub service_name: String,
    pub state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub last_failure_time: chrono::DateTime<chrono::Utc>,
    pub last_success_time: chrono::DateTime<chrono::Utc>,
    pub config: CircuitBreakerConfig,
}

/// Метрики Circuit Breaker для Prometheus
#[derive(Debug)]
pub struct CircuitBreakerMetrics {
    service_name: String,
    // Prometheus метрики будут добавлены здесь
}

impl CircuitBreakerMetrics {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }

    pub fn record_successful_call(&self, duration: Duration) {
        // Записываем метрики успешного вызова
        tracing::info!(
            service = %self.service_name,
            duration_ms = duration.as_millis(),
            "Circuit breaker successful call"
        );
    }

    pub fn record_failed_call(&self, duration: Duration) {
        // Записываем метрики неудачного вызова
        tracing::warn!(
            service = %self.service_name,
            duration_ms = duration.as_millis(),
            "Circuit breaker failed call"
        );
    }

    pub fn record_rejected_call(&self) {
        // Записываем метрики отклоненного вызова
        tracing::warn!(
            service = %self.service_name,
            "Circuit breaker rejected call"
        );
    }

    pub fn record_ignored_error(&self) {
        // Записываем метрики игнорируемой ошибки
        tracing::debug!(
            service = %self.service_name,
            "Circuit breaker ignored error"
        );
    }

    pub fn record_state_change(&self, new_state: CircuitState) {
        // Записываем метрики изменения состояния
        tracing::info!(
            service = %self.service_name,
            new_state = ?new_state,
            "Circuit breaker state changed"
        );
    }
}
```

## 🔗 Интеграция с внешними системами

### External Services - Нестабильные внешние сервисы

#### Симуляция реальных проблем внешних сервисов
```rust
// Пример интеграции с нестабильным Users Service
impl ExternalServiceClient {
    /// Вызов Users Service с полной защитой
    #[instrument(skip(self))]
    pub async fn get_user_protected(&self, user_id: Uuid) -> Result<ExternalUser, UgcError> {
        let client = self.client.clone();
        let url = format!("{}/users/{}", self.users_service_url, user_id);
        
        // Полная цепочка защиты: Circuit Breaker → Retry → Fallback
        self.users_circuit_breaker
            .call(|| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    self.retry_mechanism
                        .call(|| {
                            let client = client.clone();
                            let url = url.clone();
                            async move {
                                // Реальный HTTP вызов с таймаутом
                                let response = client
                                    .get(&url)
                                    .timeout(Duration::from_secs(5))
                                    .send()
                                    .await
                                    .map_err(|e| UgcError::ExternalServiceError {
                                        service: "users".to_string(),
                                        message: e.to_string(),
                                        status_code: None,
                                        retry_after: Some(30),
                                    })?;

                                if response.status().is_success() {
                                    let user: ExternalUser = response
                                        .json()
                                        .await
                                        .map_err(|e| UgcError::ExternalServiceError {
                                            service: "users".to_string(),
                                            message: format!("JSON parsing error: {}", e),
                                            status_code: Some(response.status().as_u16()),
                                            retry_after: None,
                                        })?;
                                    
                                    // Кешируем успешный результат для fallback
                                    self.fallback_provider.cache_user(&user).await;
                                    
                                    Ok(user)
                                } else {
                                    Err(UgcError::ExternalServiceError {
                                        service: "users".to_string(),
                                        message: format!("HTTP error: {}", response.status()),
                                        status_code: Some(response.status().as_u16()),
                                        retry_after: response
                                            .headers()
                                            .get("retry-after")
                                            .and_then(|h| h.to_str().ok())
                                            .and_then(|s| s.parse().ok()),
                                    })
                                }
                            }
                        })
                        .await
                }
            })
            .await
    }

    /// Метод с гарантированным возвратом результата (graceful degradation)
    pub async fn get_user_with_graceful_degradation(&self, user_id: Uuid) -> ExternalUser {
        match self.get_user_protected(user_id).await {
            Ok(user) => {
                info!(
                    user_id = %user_id,
                    "Successfully retrieved user from external service"
                );
                user
            }
            Err(UgcError::CircuitBreakerOpen { service, .. }) => {
                warn!(
                    user_id = %user_id,
                    service = %service,
                    "Circuit breaker open, using fallback data"
                );
                self.fallback_provider.get_user_fallback(user_id).await
            }
            Err(error) => {
                error!(
                    user_id = %user_id,
                    error = %error,
                    "Failed to retrieve user, using fallback data"
                );
                self.fallback_provider.get_user_fallback(user_id).await
            }
        }
    }
}
```

## 📊 Мониторинг и наблюдаемость

### Prometheus Metrics - Метрики отказоустойчивости
```rust
// Ключевые метрики для мониторинга отказоустойчивости
use prometheus::{Counter, Histogram, Gauge, IntGauge};

pub struct ResilienceMetrics {
    // Circuit Breaker метрики
    circuit_breaker_state: IntGauge,
    circuit_breaker_transitions: Counter,
    circuit_breaker_calls_total: Counter,
    circuit_breaker_failures_total: Counter,
    
    // Retry метрики
    retry_attempts_total: Counter,
    retry_success_rate: Gauge,
    
    // Fallback метрики
    fallback_cache_hits: Counter,
    fallback_cache_misses: Counter,
    fallback_usage_total: Counter,
    
    // Error метрики
    errors_by_category: Counter,
    error_recovery_time: Histogram,
}

impl ResilienceMetrics {
    pub fn record_circuit_breaker_state(&self, service: &str, state: CircuitState) {
        let state_value = match state {
            CircuitState::Closed => 0,
            CircuitState::Open => 1,
            CircuitState::HalfOpen => 2,
        };
        
        self.circuit_breaker_state
            .with_label_values(&[service])
            .set(state_value);
    }
    
    pub fn record_fallback_usage(&self, service: &str, fallback_type: &str) {
        self.fallback_usage_total
            .with_label_values(&[service, fallback_type])
            .inc();
    }
}
```

### Grafana Dashboards - Визуализация отказоустойчивости
```json
{
  "dashboard": {
    "title": "Auto.ru UGC Resilience Dashboard",
    "panels": [
      {
        "title": "Circuit Breaker States",
        "type": "stat",
        "targets": [
          {
            "expr": "circuit_breaker_state",
            "legendFormat": "{{ service }}"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "mappings": [
              {"options": {"0": {"text": "Closed ✅", "color": "green"}}},
              {"options": {"1": {"text": "Open ❌", "color": "red"}}},
              {"options": {"2": {"text": "Half-Open ⚠️", "color": "yellow"}}}
            ]
          }
        }
      },
      {
        "title": "Error Rate by Category",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(errors_by_category[5m])",
            "legendFormat": "{{ category }}"
          }
        ]
      },
      {
        "title": "Fallback Usage",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(fallback_usage_total[5m])",
            "legendFormat": "{{ service }} - {{ fallback_type }}"
          }
        ]
      }
    ]
  }
}
```

## 🚀 Практическое применение

### Полный пример использования отказоустойчивой системы
```rust
// Пример GraphQL резолвера с полной отказоустойчивостью
impl Mutation {
    async fn create_review_resilient(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> FieldResult<Review> {
        let user_context = ctx.data::<UserContext>()?;
        let external_service = ctx.data::<Arc<ExternalServiceClient>>()?;
        let review_service = ctx.data::<Arc<ReviewService>>()?;
        
        // 1. Получаем пользователя с graceful degradation
        let user = external_service
            .get_user_with_graceful_degradation(user_context.user_id)
            .await;
        
        // 2. Получаем предложение с fallback
        let offer = external_service
            .get_offer_with_graceful_degradation(input.offer_id)
            .await;
        
        // 3. Создаем отзыв с обработкой ошибок
        match review_service.create_review(input, &user_context).await {
            Ok(review) => {
                info!(
                    user_id = %user_context.user_id,
                    offer_id = %input.offer_id,
                    review_id = %review.id,
                    "Review created successfully"
                );
                Ok(review)
            }
            
            Err(UgcError::ValidationError { message, field, code }) => {
                // Клиентская ошибка - возвращаем как есть
                Err(async_graphql::Error::new(message)
                    .extend_with(|_, e| {
                        e.set("code", code);
                        if let Some(f) = field {
                            e.set("field", f);
                        }
                    }))
            }
            
            Err(UgcError::ExternalServiceError { service, message, .. }) => {
                warn!(
                    service = %service,
                    message = %message,
                    "External service failed, attempting fallback creation"
                );
                
                // Пытаемся создать отзыв с минимальными данными
                self.create_review_with_fallback(input, &user_context).await
            }
            
            Err(UgcError::CircuitBreakerOpen { service, estimated_recovery, .. }) => {
                warn!(
                    service = %service,
                    estimated_recovery = %estimated_recovery,
                    "Circuit breaker open, using cached operation"
                );
                
                // Возвращаем кешированный результат или откладываем операцию
                self.handle_circuit_breaker_open(input, &user_context).await
            }
            
            Err(error) => {
                error!(
                    error = %error,
                    user_id = %user_context.user_id,
                    "Failed to create review"
                );
                
                Err(error.extend())
            }
        }
    }
    
    /// Создание отзыва с fallback данными
    async fn create_review_with_fallback(
        &self,
        input: CreateReviewInput,
        user_context: &UserContext,
    ) -> FieldResult<Review> {
        // Создаем отзыв с минимальными данными
        let fallback_review = Review {
            id: Uuid::new_v4(),
            content: input.content,
            rating: input.rating,
            user_id: user_context.user_id,
            offer_id: input.offer_id,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            status: ReviewStatus::Pending, // Будет обработан позже
        };
        
        // Сохраняем в очередь для последующей обработки
        self.queue_review_for_processing(fallback_review.clone()).await?;
        
        Ok(fallback_review)
    }
}
```

Эта Context диаграмма демонстрирует комплексную enterprise-grade систему отказоустойчивости, которая превращает хрупкую архитектуру в resilient платформу, способную работать в условиях нестабильности внешних сервисов, обеспечивая высокую доступность и качество пользовательского опыта через Circuit Breaker паттерн, retry механизмы и graceful degradation.