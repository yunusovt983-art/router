# Task 7: Container Diagram - Подробное объяснение контейнерной архитектуры отказоустойчивости

## 🎯 Цель диаграммы

Container диаграмма Task 7 детализирует **внутреннюю архитектуру системы обработки ошибок и отказоустойчивости** на уровне контейнеров, показывая как различные слои resilience взаимодействуют друг с другом, их технологические стеки и паттерны интеграции для обеспечения enterprise-grade надежности.

## 🏗️ Архитектурные слои отказоустойчивости

### 1. Error Handling Layer - Слой обработки ошибок

#### Error Processor - Централизованный процессор ошибок
```rust
// ugc-subgraph/src/error/processor.rs
use std::sync::Arc;
use tracing::{error, warn, info, instrument};
use uuid::Uuid;

#[derive(Clone)]
pub struct ErrorProcessor {
    logger: Arc<StructuredLogger>,
    metrics_collector: Arc<ErrorMetricsCollector>,
    error_enricher: Arc<ErrorEnricher>,
    notification_service: Arc<ErrorNotificationService>,
}

impl ErrorProcessor {
    pub fn new() -> Result<Self, ProcessorError> {
        Ok(Self {
            logger: Arc::new(StructuredLogger::new()?),
            metrics_collector: Arc::new(ErrorMetricsCollector::new()?),
            error_enricher: Arc::new(ErrorEnricher::new()),
            notification_service: Arc::new(ErrorNotificationService::new()?),
        })
    }

    /// Централизованная обработка всех ошибок в системе
    #[instrument(skip(self))]
    pub async fn process_error(&self, error: UgcError) -> ProcessedError {
        let error_id = Uuid::new_v4();
        let processing_start = std::time::Instant::now();
        
        // 1. Обогащение ошибки контекстом
        let enriched_error = self.error_enricher.enrich(error, error_id).await;
        
        // 2. Структурированное логирование
        self.logger.log_error(&enriched_error).await;
        
        // 3. Сбор метрик
        self.metrics_collector.record_error(&enriched_error).await;
        
        // 4. Уведомления для критических ошибок
        if enriched_error.is_critical() {
            self.notification_service.send_alert(&enriched_error).await;
        }
        
        // 5. Создание обработанной ошибки для клиента
        let processed_error = ProcessedError {
            id: error_id,
            original_error: enriched_error,
            processing_time: processing_start.elapsed(),
            recommendations: self.generate_recommendations(&enriched_error).await,
        };
        
        info!(
            error_id = %error_id,
            processing_time_ms = processing_start.elapsed().as_millis(),
            error_category = %processed_error.original_error.category(),
            "Error processed successfully"
        );
        
        processed_error
    }

    /// Генерация рекомендаций для клиента на основе типа ошибки
    async fn generate_recommendations(&self, error: &EnrichedUgcError) -> Vec<ErrorRecommendation> {
        let mut recommendations = Vec::new();
        
        match &error.error_type {
            UgcError::CircuitBreakerOpen { service, estimated_recovery, .. } => {
                recommendations.push(ErrorRecommendation {
                    action: "use_cached_data".to_string(),
                    description: "Use cached data while service recovers".to_string(),
                    retry_after: Some(estimated_recovery.clone()),
                });
                
                recommendations.push(ErrorRecommendation {
                    action: "enable_offline_mode".to_string(),
                    description: "Enable offline mode for better user experience".to_string(),
                    retry_after: None,
                });
            }
            
            UgcError::RateLimitExceeded { retry_after, .. } => {
                recommendations.push(ErrorRecommendation {
                    action: "implement_backoff".to_string(),
                    description: format!("Implement exponential backoff, retry after {}s", retry_after),
                    retry_after: Some(chrono::Utc::now() + chrono::Duration::seconds(*retry_after as i64)),
                });
            }
            
            UgcError::ValidationError { field, .. } => {
                if let Some(field_name) = field {
                    recommendations.push(ErrorRecommendation {
                        action: "fix_field_validation".to_string(),
                        description: format!("Fix validation for field: {}", field_name),
                        retry_after: None,
                    });
                }
            }
            
            _ => {
                recommendations.push(ErrorRecommendation {
                    action: "retry_with_backoff".to_string(),
                    description: "Retry the operation with exponential backoff".to_string(),
                    retry_after: Some(chrono::Utc::now() + chrono::Duration::seconds(30)),
                });
            }
        }
        
        recommendations
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedError {
    pub id: Uuid,
    pub original_error: EnrichedUgcError,
    pub processing_time: std::time::Duration,
    pub recommendations: Vec<ErrorRecommendation>,
}

#[derive(Debug, Clone)]
pub struct ErrorRecommendation {
    pub action: String,
    pub description: String,
    pub retry_after: Option<chrono::DateTime<chrono::Utc>>,
}

/// Обогащение ошибок дополнительным контекстом
#[derive(Debug)]
pub struct ErrorEnricher {
    system_info: SystemInfo,
    correlation_tracker: Arc<CorrelationTracker>,
}

impl ErrorEnricher {
    pub async fn enrich(&self, error: UgcError, error_id: Uuid) -> EnrichedUgcError {
        let correlation_id = self.correlation_tracker.get_current_correlation_id();
        let system_context = self.system_info.get_current_context().await;
        
        EnrichedUgcError {
            id: error_id,
            error_type: error,
            correlation_id,
            timestamp: chrono::Utc::now(),
            system_context,
            stack_trace: self.capture_stack_trace(),
            user_context: self.get_user_context().await,
        }
    }
    
    fn capture_stack_trace(&self) -> Option<String> {
        // Захват stack trace для debugging
        std::backtrace::Backtrace::capture().to_string().into()
    }
}

#[derive(Debug, Clone)]
pub struct EnrichedUgcError {
    pub id: Uuid,
    pub error_type: UgcError,
    pub correlation_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub system_context: SystemContext,
    pub stack_trace: Option<String>,
    pub user_context: Option<UserContext>,
}

impl EnrichedUgcError {
    pub fn is_critical(&self) -> bool {
        match &self.error_type {
            UgcError::DatabaseError { .. } => true,
            UgcError::ConnectionPoolExhausted { .. } => true,
            UgcError::InternalError { .. } => true,
            UgcError::ConfigError { .. } => true,
            _ => false,
        }
    }
    
    pub fn category(&self) -> &'static str {
        self.error_type.category()
    }
}
```

#### Error Logger - Структурированное логирование
```rust
// ugc-subgraph/src/error/logger.rs
use tracing::{error, warn, info, debug};
use serde_json::json;

#[derive(Debug)]
pub struct StructuredLogger {
    log_level_config: LogLevelConfig,
    output_formatters: Vec<Box<dyn LogFormatter>>,
}

impl StructuredLogger {
    pub async fn log_error(&self, error: &EnrichedUgcError) {
        let log_entry = self.create_log_entry(error).await;
        
        match error.error_type.category() {
            "CLIENT_ERROR" => {
                match &error.error_type {
                    UgcError::Unauthorized { .. } | UgcError::Forbidden { .. } => {
                        // Security-related errors - warn level
                        warn!(
                            error_id = %error.id,
                            correlation_id = ?error.correlation_id,
                            error_type = ?error.error_type,
                            user_context = ?error.user_context,
                            system_context = ?error.system_context,
                            "Security-related client error"
                        );
                    }
                    _ => {
                        // Regular client errors - info level
                        info!(
                            error_id = %error.id,
                            correlation_id = ?error.correlation_id,
                            error_type = ?error.error_type,
                            "Client error occurred"
                        );
                    }
                }
            }
            
            "SERVER_ERROR" => {
                if error.is_critical() {
                    error!(
                        error_id = %error.id,
                        correlation_id = ?error.correlation_id,
                        error_type = ?error.error_type,
                        system_context = ?error.system_context,
                        stack_trace = ?error.stack_trace,
                        "Critical server error occurred"
                    );
                } else {
                    warn!(
                        error_id = %error.id,
                        correlation_id = ?error.correlation_id,
                        error_type = ?error.error_type,
                        system_context = ?error.system_context,
                        "Server error occurred"
                    );
                }
            }
            
            _ => {
                debug!(
                    error_id = %error.id,
                    error_type = ?error.error_type,
                    "Unknown error category"
                );
            }
        }
        
        // Отправляем в различные форматтеры (JSON, ELK, etc.)
        for formatter in &self.output_formatters {
            formatter.format_and_send(&log_entry).await;
        }
    }
    
    async fn create_log_entry(&self, error: &EnrichedUgcError) -> LogEntry {
        LogEntry {
            timestamp: error.timestamp,
            level: self.determine_log_level(&error.error_type),
            error_id: error.id,
            correlation_id: error.correlation_id.clone(),
            error_category: error.category().to_string(),
            error_code: self.extract_error_code(&error.error_type),
            message: error.error_type.to_string(),
            system_context: error.system_context.clone(),
            user_context: error.user_context.clone(),
            stack_trace: error.stack_trace.clone(),
            additional_fields: self.extract_additional_fields(&error.error_type),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub level: LogLevel,
    pub error_id: Uuid,
    pub correlation_id: Option<String>,
    pub error_category: String,
    pub error_code: String,
    pub message: String,
    pub system_context: SystemContext,
    pub user_context: Option<UserContext>,
    pub stack_trace: Option<String>,
    pub additional_fields: serde_json::Value,
}
```

#### Error Metrics Collector - Сбор метрик ошибок
```rust
// ugc-subgraph/src/error/metrics.rs
use prometheus::{Counter, Histogram, Gauge, IntCounter, IntGauge};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ErrorMetricsCollector {
    // Счетчики ошибок по категориям
    errors_total: IntCounter,
    errors_by_category: Counter,
    errors_by_code: Counter,
    
    // Гистограммы времени обработки
    error_processing_duration: Histogram,
    error_recovery_duration: Histogram,
    
    // Gauge метрики для текущего состояния
    active_errors: IntGauge,
    error_rate_per_minute: Gauge,
    
    // Специфичные метрики для resilience
    circuit_breaker_errors: Counter,
    retry_attempts: Counter,
    fallback_activations: Counter,
}

impl ErrorMetricsCollector {
    pub fn new() -> Result<Self, MetricsError> {
        Ok(Self {
            errors_total: IntCounter::new(
                "ugc_errors_total",
                "Total number of errors in UGC service"
            )?,
            
            errors_by_category: Counter::new(
                "ugc_errors_by_category_total",
                "Errors grouped by category (CLIENT_ERROR/SERVER_ERROR)"
            )?,
            
            errors_by_code: Counter::new(
                "ugc_errors_by_code_total", 
                "Errors grouped by specific error code"
            )?,
            
            error_processing_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "ugc_error_processing_duration_seconds",
                    "Time spent processing errors"
                ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0])
            )?,
            
            error_recovery_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "ugc_error_recovery_duration_seconds",
                    "Time from error occurrence to recovery"
                ).buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0])
            )?,
            
            active_errors: IntGauge::new(
                "ugc_active_errors",
                "Number of currently active/unresolved errors"
            )?,
            
            error_rate_per_minute: Gauge::new(
                "ugc_error_rate_per_minute",
                "Error rate per minute"
            )?,
            
            circuit_breaker_errors: Counter::new(
                "ugc_circuit_breaker_errors_total",
                "Errors caused by circuit breaker being open"
            )?,
            
            retry_attempts: Counter::new(
                "ugc_retry_attempts_total",
                "Total number of retry attempts"
            )?,
            
            fallback_activations: Counter::new(
                "ugc_fallback_activations_total",
                "Number of times fallback mechanisms were activated"
            )?,
        })
    }

    pub async fn record_error(&self, error: &EnrichedUgcError) {
        // Общий счетчик ошибок
        self.errors_total.inc();
        
        // Ошибки по категориям
        self.errors_by_category
            .with_label_values(&[error.category()])
            .inc();
        
        // Ошибки по кодам
        let error_code = self.extract_error_code(&error.error_type);
        self.errors_by_code
            .with_label_values(&[&error_code])
            .inc();
        
        // Специфичные метрики для resilience паттернов
        match &error.error_type {
            UgcError::CircuitBreakerOpen { service, .. } => {
                self.circuit_breaker_errors
                    .with_label_values(&[service])
                    .inc();
            }
            
            UgcError::ExternalServiceError { service, .. } => {
                // Это может привести к retry
                self.retry_attempts
                    .with_label_values(&[service, "external_service_error"])
                    .inc();
            }
            
            _ => {}
        }
        
        // Обновляем активные ошибки
        self.active_errors.inc();
        
        // Записываем время обработки (если доступно)
        if let Some(processing_time) = self.get_processing_time(error) {
            self.error_processing_duration.observe(processing_time.as_secs_f64());
        }
    }

    pub async fn record_error_recovery(&self, error_id: Uuid, recovery_duration: std::time::Duration) {
        self.active_errors.dec();
        self.error_recovery_duration.observe(recovery_duration.as_secs_f64());
        
        info!(
            error_id = %error_id,
            recovery_duration_ms = recovery_duration.as_millis(),
            "Error recovered successfully"
        );
    }

    pub async fn record_fallback_activation(&self, service: &str, fallback_type: &str) {
        self.fallback_activations
            .with_label_values(&[service, fallback_type])
            .inc();
        
        info!(
            service = service,
            fallback_type = fallback_type,
            "Fallback mechanism activated"
        );
    }

    /// Вычисление error rate в реальном времени
    pub async fn update_error_rate(&self, errors_in_last_minute: f64) {
        self.error_rate_per_minute.set(errors_in_last_minute);
    }
}
```

### 2. Circuit Breaker Layer - Слой защиты от каскадных сбоев

#### Circuit Breaker Manager - Управление Circuit Breaker'ами
```rust
// ugc-subgraph/src/resilience/circuit_breaker_manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct CircuitBreakerManager {
    circuit_breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
    service_configs: HashMap<String, CircuitBreakerConfig>,
    health_monitor: Arc<ServiceHealthMonitor>,
    metrics: Arc<CircuitBreakerManagerMetrics>,
}

impl CircuitBreakerManager {
    pub fn new() -> Result<Self, ManagerError> {
        Ok(Self {
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            default_config: CircuitBreakerConfig::default(),
            service_configs: Self::load_service_configs()?,
            health_monitor: Arc::new(ServiceHealthMonitor::new()),
            metrics: Arc::new(CircuitBreakerManagerMetrics::new()?),
        })
    }

    /// Получение или создание Circuit Breaker для сервиса
    pub async fn get_circuit_breaker(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let circuit_breakers = self.circuit_breakers.read().await;
        
        if let Some(cb) = circuit_breakers.get(service_name) {
            return cb.clone();
        }
        
        drop(circuit_breakers);
        
        // Создаем новый Circuit Breaker
        let config = self.service_configs
            .get(service_name)
            .cloned()
            .unwrap_or_else(|| self.default_config.clone());
            
        let circuit_breaker = Arc::new(CircuitBreaker::new(service_name.to_string(), config));
        
        let mut circuit_breakers = self.circuit_breakers.write().await;
        circuit_breakers.insert(service_name.to_string(), circuit_breaker.clone());
        
        info!(
            service = service_name,
            "Created new circuit breaker"
        );
        
        circuit_breaker
    }

    /// Выполнение операции через соответствующий Circuit Breaker
    pub async fn execute<F, Fut, T>(&self, service_name: &str, operation: F) -> Result<T, UgcError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        let circuit_breaker = self.get_circuit_breaker(service_name).await;
        
        let start_time = std::time::Instant::now();
        let result = circuit_breaker.call(operation).await;
        let duration = start_time.elapsed();
        
        // Записываем метрики выполнения
        match &result {
            Ok(_) => {
                self.metrics.record_successful_execution(service_name, duration);
                self.health_monitor.record_success(service_name).await;
            }
            Err(error) => {
                self.metrics.record_failed_execution(service_name, duration, error);
                self.health_monitor.record_failure(service_name, &error.to_string()).await;
            }
        }
        
        result
    }

    /// Получение статуса всех Circuit Breaker'ов
    pub async fn get_all_statuses(&self) -> HashMap<String, CircuitBreakerStatus> {
        let circuit_breakers = self.circuit_breakers.read().await;
        let mut statuses = HashMap::new();
        
        for (service_name, cb) in circuit_breakers.iter() {
            let stats = cb.get_stats().await;
            let health = self.health_monitor.get_service_health(service_name).await;
            
            statuses.insert(service_name.clone(), CircuitBreakerStatus {
                service_name: service_name.clone(),
                state: stats.state,
                failure_count: stats.failure_count,
                success_count: stats.success_count,
                last_state_change: stats.last_failure_time,
                health_status: health,
                config: stats.config,
            });
        }
        
        statuses
    }

    /// Принудительное открытие Circuit Breaker (для maintenance)
    pub async fn force_open(&self, service_name: &str) -> Result<(), ManagerError> {
        let circuit_breaker = self.get_circuit_breaker(service_name).await;
        circuit_breaker.force_open().await;
        
        warn!(
            service = service_name,
            "Circuit breaker forcefully opened for maintenance"
        );
        
        Ok(())
    }

    /// Принудительное закрытие Circuit Breaker
    pub async fn force_close(&self, service_name: &str) -> Result<(), ManagerError> {
        let circuit_breaker = self.get_circuit_breaker(service_name).await;
        circuit_breaker.force_close().await;
        
        info!(
            service = service_name,
            "Circuit breaker forcefully closed"
        );
        
        Ok(())
    }

    /// Загрузка конфигураций для различных сервисов
    fn load_service_configs() -> Result<HashMap<String, CircuitBreakerConfig>, ManagerError> {
        let mut configs = HashMap::new();
        
        // Конфигурация для Users Service (более чувствительная)
        configs.insert("users".to_string(), CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(30),
            success_threshold: 2,
            failure_window: Duration::from_secs(60),
            half_open_max_calls: 2,
        });
        
        // Конфигурация для Offers Service (менее чувствительная)
        configs.insert("offers".to_string(), CircuitBreakerConfig {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_window: Duration::from_secs(120),
            half_open_max_calls: 3,
        });
        
        // Конфигурация для Payment Service (критически важный)
        configs.insert("payment".to_string(), CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(120),
            success_threshold: 5,
            failure_window: Duration::from_secs(30),
            half_open_max_calls: 1,
        });
        
        Ok(configs)
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub service_name: String,
    pub state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub last_state_change: chrono::DateTime<chrono::Utc>,
    pub health_status: ServiceHealth,
    pub config: CircuitBreakerConfig,
}
```

#### Retry Engine - Механизм повторных попыток
```rust
// ugc-subgraph/src/resilience/retry_engine.rs
use std::time::Duration;
use rand::Rng;
use tracing::{warn, info, debug};

#[derive(Debug, Clone)]
pub struct RetryEngine {
    config: RetryConfig,
    jitter_generator: Arc<JitterGenerator>,
    metrics: Arc<RetryMetrics>,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter_enabled: bool,
    pub jitter_max_percentage: f64, // 0.0 - 1.0
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_enabled: true,
            jitter_max_percentage: 0.1, // 10% jitter
        }
    }
}

impl RetryEngine {
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            jitter_generator: Arc::new(JitterGenerator::new()),
            metrics: Arc::new(RetryMetrics::new()),
        }
    }

    /// Выполнение операции с retry логикой
    pub async fn execute<F, Fut, T>(&self, mut operation: F) -> Result<T, UgcError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        let mut attempt = 0;
        let mut delay = self.config.initial_delay;
        let mut last_error = None;
        
        let operation_start = std::time::Instant::now();
        
        loop {
            attempt += 1;
            
            debug!(
                attempt = attempt,
                max_attempts = self.config.max_attempts,
                "Executing retry attempt"
            );
            
            let attempt_start = std::time::Instant::now();
            
            match operation().await {
                Ok(result) => {
                    let total_duration = operation_start.elapsed();
                    
                    if attempt > 1 {
                        info!(
                            attempt = attempt,
                            total_duration_ms = total_duration.as_millis(),
                            "Operation succeeded after retry"
                        );
                        
                        self.metrics.record_successful_retry(attempt, total_duration);
                    }
                    
                    return Ok(result);
                }
                Err(error) => {
                    let attempt_duration = attempt_start.elapsed();
                    last_error = Some(error.clone());
                    
                    // Проверяем, можно ли повторить эту ошибку
                    if !self.is_retryable_error(&error) {
                        warn!(
                            attempt = attempt,
                            error = %error,
                            "Non-retryable error encountered, stopping retries"
                        );
                        
                        self.metrics.record_non_retryable_error(&error);
                        return Err(error);
                    }
                    
                    // Проверяем, достигли ли максимального количества попыток
                    if attempt >= self.config.max_attempts {
                        let total_duration = operation_start.elapsed();
                        
                        error!(
                            attempt = attempt,
                            max_attempts = self.config.max_attempts,
                            total_duration_ms = total_duration.as_millis(),
                            final_error = %error,
                            "All retry attempts exhausted"
                        );
                        
                        self.metrics.record_exhausted_retries(attempt, total_duration, &error);
                        return Err(error);
                    }
                    
                    // Вычисляем задержку для следующей попытки
                    let actual_delay = if self.config.jitter_enabled {
                        self.jitter_generator.add_jitter(delay, self.config.jitter_max_percentage)
                    } else {
                        delay
                    };
                    
                    warn!(
                        attempt = attempt,
                        max_attempts = self.config.max_attempts,
                        delay_ms = actual_delay.as_millis(),
                        attempt_duration_ms = attempt_duration.as_millis(),
                        error = %error,
                        "Retry attempt failed, waiting before next attempt"
                    );
                    
                    self.metrics.record_failed_attempt(attempt, attempt_duration, &error);
                    
                    // Ждем перед следующей попыткой
                    tokio::time::sleep(actual_delay).await;
                    
                    // Увеличиваем задержку для следующей попытки (exponential backoff)
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64
                        ),
                        self.config.max_delay,
                    );
                }
            }
        }
    }

    /// Определяет, можно ли повторить операцию при данной ошибке
    fn is_retryable_error(&self, error: &UgcError) -> bool {
        match error {
            // Клиентские ошибки обычно не повторяемы
            UgcError::ReviewNotFound { .. } => false,
            UgcError::Unauthorized { .. } => false,
            UgcError::ValidationError { .. } => false,
            UgcError::Forbidden { .. } => false,
            
            // Rate limiting - повторяемо с задержкой
            UgcError::RateLimitExceeded { .. } => true,
            
            // Серверные ошибки обычно повторяемы
            UgcError::DatabaseError { .. } => true,
            UgcError::ExternalServiceError { .. } => true,
            UgcError::ServiceTimeout { .. } => true,
            UgcError::ConnectionPoolExhausted { .. } => true,
            UgcError::CacheError { .. } => true,
            
            // Circuit breaker - не повторяемо (fallback должен сработать)
            UgcError::CircuitBreakerOpen { .. } => false,
            
            // Конфигурационные ошибки не повторяемы
            UgcError::ConfigError { .. } => false,
            
            // Внутренние ошибки - зависит от контекста, по умолчанию не повторяемы
            UgcError::InternalError { .. } => false,
            
            // Authentication errors - могут быть повторяемы в случае временных проблем
            UgcError::AuthenticationError { .. } => true,
        }
    }

    /// Создание специализированного retry engine для конкретного сервиса
    pub fn for_service(service_name: &str) -> Self {
        let config = match service_name {
            "users" => RetryConfig {
                max_attempts: 3,
                initial_delay: Duration::from_millis(100),
                max_delay: Duration::from_secs(5),
                backoff_multiplier: 2.0,
                jitter_enabled: true,
                jitter_max_percentage: 0.1,
            },
            
            "offers" => RetryConfig {
                max_attempts: 4,
                initial_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(10),
                backoff_multiplier: 1.5,
                jitter_enabled: true,
                jitter_max_percentage: 0.15,
            },
            
            "payment" => RetryConfig {
                max_attempts: 5,
                initial_delay: Duration::from_millis(50),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.5,
                jitter_enabled: true,
                jitter_max_percentage: 0.05, // Меньше jitter для критичного сервиса
            },
            
            _ => RetryConfig::default(),
        };
        
        Self::new(config)
    }
}

/// Генератор jitter для избежания thundering herd эффекта
#[derive(Debug)]
pub struct JitterGenerator {
    rng: Arc<tokio::sync::Mutex<rand::rngs::ThreadRng>>,
}

impl JitterGenerator {
    pub fn new() -> Self {
        Self {
            rng: Arc::new(tokio::sync::Mutex::new(rand::thread_rng())),
        }
    }

    /// Добавляет jitter к задержке
    pub async fn add_jitter(&self, base_delay: Duration, max_jitter_percentage: f64) -> Duration {
        let mut rng = self.rng.lock().await;
        
        let jitter_range = (base_delay.as_millis() as f64 * max_jitter_percentage) as u64;
        let jitter = rng.gen_range(0..=jitter_range);
        
        // Добавляем или вычитаем jitter случайным образом
        if rng.gen_bool(0.5) {
            base_delay + Duration::from_millis(jitter)
        } else {
            base_delay.saturating_sub(Duration::from_millis(jitter))
        }
    }
}

/// Метрики для retry механизма
#[derive(Debug)]
pub struct RetryMetrics {
    successful_retries: Counter,
    failed_attempts: Counter,
    exhausted_retries: Counter,
    non_retryable_errors: Counter,
    retry_duration: Histogram,
}

impl RetryMetrics {
    pub fn new() -> Self {
        Self {
            successful_retries: Counter::new(
                "ugc_retry_successful_total",
                "Number of operations that succeeded after retry"
            ).unwrap(),
            
            failed_attempts: Counter::new(
                "ugc_retry_failed_attempts_total",
                "Number of individual retry attempts that failed"
            ).unwrap(),
            
            exhausted_retries: Counter::new(
                "ugc_retry_exhausted_total",
                "Number of operations that failed after all retries"
            ).unwrap(),
            
            non_retryable_errors: Counter::new(
                "ugc_retry_non_retryable_total",
                "Number of non-retryable errors encountered"
            ).unwrap(),
            
            retry_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "ugc_retry_duration_seconds",
                    "Total time spent on retry operations"
                ).buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0])
            ).unwrap(),
        }
    }

    pub fn record_successful_retry(&self, attempts: usize, total_duration: Duration) {
        self.successful_retries
            .with_label_values(&[&attempts.to_string()])
            .inc();
        
        self.retry_duration.observe(total_duration.as_secs_f64());
    }

    pub fn record_failed_attempt(&self, attempt: usize, duration: Duration, error: &UgcError) {
        self.failed_attempts
            .with_label_values(&[&attempt.to_string(), error.category()])
            .inc();
    }

    pub fn record_exhausted_retries(&self, attempts: usize, total_duration: Duration, final_error: &UgcError) {
        self.exhausted_retries
            .with_label_values(&[&attempts.to_string(), final_error.category()])
            .inc();
        
        self.retry_duration.observe(total_duration.as_secs_f64());
    }

    pub fn record_non_retryable_error(&self, error: &UgcError) {
        self.non_retryable_errors
            .with_label_values(&[error.category()])
            .inc();
    }
}
```

### 3. Graceful Degradation Layer - Слой graceful degradation

#### Fallback Data Provider - Провайдер fallback данных
```rust
// ugc-subgraph/src/resilience/fallback_provider.rs
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FallbackDataProvider {
    // Многоуровневое кеширование
    l1_cache: Arc<InMemoryCache<String>>, // Горячие данные
    l2_cache: Arc<RedisCache>,            // Персистентный кеш
    l3_cache: Arc<StaticFallbackCache>,   // Статические fallback данные
    
    // Конфигурация fallback стратегий
    fallback_strategies: HashMap<String, FallbackStrategy>,
    
    // Метрики
    metrics: Arc<FallbackMetrics>,
    
    // Управление качеством данных
    data_quality_manager: Arc<DataQualityManager>,
}

impl FallbackDataProvider {
    pub fn new() -> Result<Self, FallbackError> {
        let fallback_strategies = Self::initialize_fallback_strategies();
        
        Ok(Self {
            l1_cache: Arc::new(InMemoryCache::new(Duration::from_secs(300))), // 5 минут TTL
            l2_cache: Arc::new(RedisCache::new("redis://localhost:6379")?),
            l3_cache: Arc::new(StaticFallbackCache::new()),
            fallback_strategies,
            metrics: Arc::new(FallbackMetrics::new()?),
            data_quality_manager: Arc::new(DataQualityManager::new()),
        })
    }

    /// Получение пользователя с многоуровневым fallback
    pub async fn get_user_fallback(&self, user_id: Uuid) -> ExternalUser {
        let cache_key = format!("user:{}", user_id);
        
        // Уровень 1: Горячий кеш в памяти
        if let Some(user_data) = self.l1_cache.get(&cache_key).await {
            if let Ok(user) = serde_json::from_str::<ExternalUser>(&user_data) {
                self.metrics.record_cache_hit("l1", "user").await;
                
                info!(
                    user_id = %user_id,
                    cache_level = "L1",
                    "Retrieved user from L1 cache"
                );
                
                return self.data_quality_manager.ensure_quality(user).await;
            }
        }
        
        // Уровень 2: Redis кеш
        if let Some(user_data) = self.l2_cache.get(&cache_key).await {
            if let Ok(user) = serde_json::from_str::<ExternalUser>(&user_data) {
                self.metrics.record_cache_hit("l2", "user").await;
                
                // Прогреваем L1 кеш
                self.l1_cache.set(cache_key.clone(), user_data).await;
                
                info!(
                    user_id = %user_id,
                    cache_level = "L2",
                    "Retrieved user from L2 cache, warmed L1"
                );
                
                return self.data_quality_manager.ensure_quality(user).await;
            }
        }
        
        // Уровень 3: Статические fallback данные
        if let Some(user) = self.l3_cache.get_user_fallback(user_id).await {
            self.metrics.record_cache_hit("l3", "user").await;
            
            warn!(
                user_id = %user_id,
                cache_level = "L3",
                "Using static fallback data for user"
            );
            
            return user;
        }
        
        // Уровень 4: Минимальные данные-заглушки
        self.metrics.record_cache_miss("all_levels", "user").await;
        
        error!(
            user_id = %user_id,
            "No fallback data available, creating minimal user stub"
        );
        
        self.create_minimal_user_stub(user_id).await
    }

    /// Кеширование пользователя на всех уровнях
    pub async fn cache_user(&self, user: &ExternalUser) {
        let cache_key = format!("user:{}", user.id);
        let user_json = serde_json::to_string(user).unwrap_or_default();
        
        // Кешируем на всех уровнях
        self.l1_cache.set(cache_key.clone(), user_json.clone()).await;
        self.l2_cache.set(&cache_key, &user_json, Duration::from_secs(3600)).await; // 1 час в Redis
        
        // Обновляем статический кеш для критически важных пользователей
        if self.is_critical_user(user).await {
            self.l3_cache.cache_critical_user(user.clone()).await;
        }
        
        info!(
            user_id = %user.id,
            "Cached user data on all levels"
        );
    }

    /// Получение предложения с fallback
    pub async fn get_offer_fallback(&self, offer_id: Uuid) -> ExternalOffer {
        let cache_key = format!("offer:{}", offer_id);
        
        // Аналогичная логика многоуровневого кеширования для предложений
        if let Some(offer_data) = self.l1_cache.get(&cache_key).await {
            if let Ok(offer) = serde_json::from_str::<ExternalOffer>(&offer_data) {
                self.metrics.record_cache_hit("l1", "offer").await;
                return offer;
            }
        }
        
        if let Some(offer_data) = self.l2_cache.get(&cache_key).await {
            if let Ok(offer) = serde_json::from_str::<ExternalOffer>(&offer_data) {
                self.metrics.record_cache_hit("l2", "offer").await;
                self.l1_cache.set(cache_key.clone(), offer_data).await;
                return offer;
            }
        }
        
        if let Some(offer) = self.l3_cache.get_offer_fallback(offer_id).await {
            self.metrics.record_cache_hit("l3", "offer").await;
            return offer;
        }
        
        self.metrics.record_cache_miss("all_levels", "offer").await;
        self.create_minimal_offer_stub(offer_id).await
    }

    /// Создание минимальных данных-заглушек для пользователя
    async fn create_minimal_user_stub(&self, user_id: Uuid) -> ExternalUser {
        let strategy = self.fallback_strategies
            .get("user")
            .cloned()
            .unwrap_or_default();
        
        let stub_user = ExternalUser {
            id: user_id,
            name: strategy.default_name.unwrap_or_else(|| "Unknown User".to_string()),
            email: None,
            avatar_url: strategy.default_avatar_url,
            created_at: chrono::Utc::now() - chrono::Duration::days(30), // Примерная дата
            is_verified: false,
            reputation_score: 0,
        };
        
        warn!(
            user_id = %user_id,
            "Created minimal user stub"
        );
        
        stub_user
    }

    /// Создание минимальных данных-заглушек для предложения
    async fn create_minimal_offer_stub(&self, offer_id: Uuid) -> ExternalOffer {
        let strategy = self.fallback_strategies
            .get("offer")
            .cloned()
            .unwrap_or_default();
        
        let stub_offer = ExternalOffer {
            id: offer_id,
            title: strategy.default_title.unwrap_or_else(|| "Offer Unavailable".to_string()),
            description: Some("This offer is temporarily unavailable".to_string()),
            price: None,
            currency: "RUB".to_string(),
            status: OfferStatus::Unavailable,
            created_at: chrono::Utc::now() - chrono::Duration::days(7),
            updated_at: chrono::Utc::now(),
        };
        
        warn!(
            offer_id = %offer_id,
            "Created minimal offer stub"
        );
        
        stub_offer
    }

    /// Проверка, является ли пользователь критически важным
    async fn is_critical_user(&self, user: &ExternalUser) -> bool {
        // Критически важные пользователи: верифицированные, с высокой репутацией
        user.is_verified || user.reputation_score > 1000
    }

    /// Инициализация стратегий fallback для различных типов данных
    fn initialize_fallback_strategies() -> HashMap<String, FallbackStrategy> {
        let mut strategies = HashMap::new();
        
        strategies.insert("user".to_string(), FallbackStrategy {
            default_name: Some("Anonymous User".to_string()),
            default_avatar_url: Some("https://cdn.auto.ru/default-avatar.png".to_string()),
            default_title: None,
            cache_ttl: Duration::from_secs(1800), // 30 минут
            quality_threshold: 0.7,
        });
        
        strategies.insert("offer".to_string(), FallbackStrategy {
            default_name: None,
            default_avatar_url: None,
            default_title: Some("Offer Temporarily Unavailable".to_string()),
            cache_ttl: Duration::from_secs(600), // 10 минут
            quality_threshold: 0.8,
        });
        
        strategies
    }
}

#[derive(Debug, Clone)]
pub struct FallbackStrategy {
    pub default_name: Option<String>,
    pub default_avatar_url: Option<String>,
    pub default_title: Option<String>,
    pub cache_ttl: Duration,
    pub quality_threshold: f64, // 0.0 - 1.0
}

impl Default for FallbackStrategy {
    fn default() -> Self {
        Self {
            default_name: Some("Unknown".to_string()),
            default_avatar_url: None,
            default_title: Some("Unavailable".to_string()),
            cache_ttl: Duration::from_secs(300),
            quality_threshold: 0.5,
        }
    }
}

/// Управление качеством fallback данных
#[derive(Debug)]
pub struct DataQualityManager {
    quality_rules: HashMap<String, Vec<QualityRule>>,
}

impl DataQualityManager {
    pub fn new() -> Self {
        let mut quality_rules = HashMap::new();
        
        // Правила качества для пользователей
        quality_rules.insert("user".to_string(), vec![
            QualityRule {
                field: "name".to_string(),
                rule_type: QualityRuleType::NotEmpty,
                fallback_value: Some("Anonymous User".to_string()),
            },
            QualityRule {
                field: "email".to_string(),
                rule_type: QualityRuleType::ValidEmail,
                fallback_value: None,
            },
        ]);
        
        Self { quality_rules }
    }

    /// Обеспечение качества данных пользователя
    pub async fn ensure_quality(&self, mut user: ExternalUser) -> ExternalUser {
        if let Some(rules) = self.quality_rules.get("user") {
            for rule in rules {
                match rule.field.as_str() {
                    "name" => {
                        if user.name.trim().is_empty() {
                            user.name = rule.fallback_value
                                .clone()
                                .unwrap_or_else(|| "Anonymous User".to_string());
                        }
                    }
                    "email" => {
                        if let Some(email) = &user.email {
                            if !email.contains('@') {
                                user.email = None; // Удаляем невалидный email
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        
        user
    }
}

#[derive(Debug, Clone)]
pub struct QualityRule {
    pub field: String,
    pub rule_type: QualityRuleType,
    pub fallback_value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum QualityRuleType {
    NotEmpty,
    ValidEmail,
    MinLength(usize),
    MaxLength(usize),
}
```

## 🔗 Интеграция слоев отказоустойчивости

### UGC Application Layer - Слой приложения с интегрированной отказоустойчивостью

#### UGC GraphQL Server с resilience
```rust
// ugc-subgraph/src/graphql/mod.rs
use async_graphql::{Context, FieldResult, Object, Schema, EmptySubscription};
use std::sync::Arc;

pub struct UgcQuery;
pub struct UgcMutation;

#[Object]
impl UgcMutation {
    /// Создание отзыва с полной отказоустойчивостью
    async fn create_review_resilient(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> FieldResult<Review> {
        // Получаем все необходимые сервисы из контекста
        let error_processor = ctx.data::<Arc<ErrorProcessor>>()?;
        let circuit_breaker_manager = ctx.data::<Arc<CircuitBreakerManager>>()?;
        let fallback_provider = ctx.data::<Arc<FallbackDataProvider>>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        // Выполняем операцию с полной защитой
        let result = self.execute_create_review_with_resilience(
            input,
            user_context,
            circuit_breaker_manager,
            fallback_provider,
        ).await;
        
        // Обрабатываем результат через Error Processor
        match result {
            Ok(review) => Ok(review),
            Err(error) => {
                let processed_error = error_processor.process_error(error).await;
                Err(processed_error.original_error.error_type.extend())
            }
        }
    }
    
    async fn execute_create_review_with_resilience(
        &self,
        input: CreateReviewInput,
        user_context: &UserContext,
        cb_manager: &Arc<CircuitBreakerManager>,
        fallback_provider: &Arc<FallbackDataProvider>,
    ) -> Result<Review, UgcError> {
        // 1. Получаем пользователя через Circuit Breaker
        let user = cb_manager
            .execute("users", || async {
                self.fetch_user_from_external_service(user_context.user_id).await
            })
            .await
            .unwrap_or_else(|_| {
                // Fallback: используем кешированные данные
                fallback_provider.get_user_fallback(user_context.user_id)
            })
            .await;
        
        // 2. Получаем предложение через Circuit Breaker
        let offer = cb_manager
            .execute("offers", || async {
                self.fetch_offer_from_external_service(input.offer_id).await
            })
            .await
            .unwrap_or_else(|_| {
                // Fallback: используем кешированные данные
                fallback_provider.get_offer_fallback(input.offer_id)
            })
            .await;
        
        // 3. Создаем отзыв с обработкой ошибок
        let review = self.create_review_in_database(input, &user, &offer).await?;
        
        // 4. Кешируем данные для будущих fallback
        fallback_provider.cache_user(&user).await;
        fallback_provider.cache_offer(&offer).await;
        
        Ok(review)
    }
}

/// Создание схемы с интегрированной отказоустойчивостью
pub fn create_resilient_schema(
    error_processor: Arc<ErrorProcessor>,
    circuit_breaker_manager: Arc<CircuitBreakerManager>,
    fallback_provider: Arc<FallbackDataProvider>,
) -> Schema<UgcQuery, UgcMutation, EmptySubscription> {
    Schema::build(UgcQuery, UgcMutation, EmptySubscription)
        .data(error_processor)
        .data(circuit_breaker_manager)
        .data(fallback_provider)
        .enable_federation()
        .finish()
}
```

Эта Container диаграмма демонстрирует детальную архитектуру отказоустойчивости на уровне контейнеров, показывая как различные слои (Error Handling, Circuit Breaker, Graceful Degradation) работают вместе для обеспечения enterprise-grade надежности системы, с полной интеграцией мониторинга, логирования и fallback механизмов.