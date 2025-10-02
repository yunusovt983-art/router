# Task 8: Code Diagram - Подробное объяснение реализации телеметрии

## 🎯 Цель диаграммы

Code диаграмма Task 8 демонстрирует **конкретную реализацию кода** для системы телеметрии и мониторинга, показывая как архитектурные компоненты превращаются в реальные Rust структуры, функции и макросы, обеспечивающие полную наблюдаемость GraphQL федерации Auto.ru.

## 🏗️ Архитектурная реализация: от дизайна к коду

### Distributed Tracing Implementation - Реализация распределенной трассировки

#### TracingConfig - Конфигурация трассировки
```rust
// src/telemetry/tracing_config.rs
pub struct TracingConfig {
    pub service_name: String,
    pub service_version: String,
    pub jaeger_endpoint: Option<String>,
    pub sample_rate: f64,
    pub enable_console: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "ugc-subgraph".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            sample_rate: 1.0,
            enable_console: true,
        }
    }
}

impl TracingConfig {
    /// Создание конфигурации из переменных окружения
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();
        
        if let Ok(service_name) = std::env::var("SERVICE_NAME") {
            config.service_name = service_name;
        }
        
        if let Ok(sample_rate_str) = std::env::var("TRACE_SAMPLE_RATE") {
            config.sample_rate = sample_rate_str.parse()
                .map_err(|_| ConfigError::InvalidSampleRate)?;
        }
        
        config.enable_console = std::env::var("ENABLE_CONSOLE_LOGS")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);
            
        Ok(config)
    }
    
    /// Валидация конфигурации
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.service_name.is_empty() {
            return Err(ConfigError::EmptyServiceName);
        }
        
        if !(0.0..=1.0).contains(&self.sample_rate) {
            return Err(ConfigError::InvalidSampleRate);
        }
        
        Ok(())
    }
}
```

**Объяснение**: `TracingConfig` - это центральная структура конфигурации, которая инкапсулирует все настройки трассировки. Она поддерживает загрузку из переменных окружения и валидацию, что критично для production развертывания.

#### init_tracing - Инициализация трассировки
```rust
// src/telemetry/tracing_init.rs
use opentelemetry::{
    global, runtime::TokioCurrentThread, sdk::trace::TracerProvider, KeyValue,
};
use opentelemetry_jaeger::JaegerPipeline;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub fn init_tracing(config: TracingConfig) -> Result<(), TelemetryError> {
    // Валидация конфигурации
    config.validate()?;
    
    // Создание фильтра логов
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            "ugc_subgraph=debug,tower_http=debug,sqlx=info,async_graphql=debug".into()
        });

    // Создание базового registry
    let registry = Registry::default().with(env_filter);

    // Инициализация OpenTelemetry tracer если настроен Jaeger
    if let Some(jaeger_endpoint) = &config.jaeger_endpoint {
        let tracer = init_jaeger_tracer(&config, jaeger_endpoint)?;
        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        
        // Добавление console layer если включен
        if config.enable_console {
            registry
                .with(telemetry_layer)
                .with(tracing_subscriber::fmt::layer().json())
                .try_init()?;
        } else {
            registry
                .with(telemetry_layer)
                .try_init()?;
        }
    } else {
        // Только локальное логирование без OpenTelemetry
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init()?;
    }

    Ok(())
}

/// Инициализация Jaeger tracer
fn init_jaeger_tracer(
    config: &TracingConfig,
    jaeger_endpoint: &str,
) -> Result<opentelemetry::sdk::trace::Tracer, TelemetryError> {
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(jaeger_endpoint)
        .with_service_name(&config.service_name)
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_sampler(opentelemetry::sdk::trace::Sampler::TraceIdRatioBased(
                    config.sample_rate,
                ))
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    KeyValue::new("service.name", config.service_name.clone()),
                    KeyValue::new("service.version", config.service_version.clone()),
                    KeyValue::new("deployment.environment", 
                        std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
                    ),
                ])),
        )
        .install_batch(TokioCurrentThread)
        .map_err(TelemetryError::TracerInitialization)?;

    Ok(tracer)
}
```

**Объяснение**: `init_tracing` - это функция инициализации, которая настраивает полный стек трассировки с поддержкой как локального логирования, так и отправки в Jaeger. Она использует layered архитектуру tracing-subscriber для композиции различных обработчиков.

#### CorrelationId - Управление correlation ID
```rust
// src/telemetry/correlation.rs
use uuid::Uuid;
use axum::http::HeaderMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Создание нового correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Создание из строки с валидацией
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// Преобразование в строку
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
    
    /// Получение внутреннего UUID
    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Извлечение correlation ID из HTTP заголовков
pub fn extract_correlation_id(headers: &HeaderMap) -> CorrelationId {
    headers
        .get("x-correlation-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| CorrelationId::from_string(s).ok())
        .unwrap_or_else(CorrelationId::new)
}

/// Извлечение или генерация correlation ID с логированием
pub fn extract_or_generate_correlation_id(headers: &HeaderMap) -> CorrelationId {
    match headers.get("x-correlation-id") {
        Some(header_value) => {
            match header_value.to_str() {
                Ok(id_str) => {
                    match CorrelationId::from_string(id_str) {
                        Ok(correlation_id) => {
                            tracing::debug!(
                                correlation_id = %correlation_id,
                                "Extracted existing correlation ID from headers"
                            );
                            correlation_id
                        }
                        Err(e) => {
                            let new_id = CorrelationId::new();
                            tracing::warn!(
                                invalid_correlation_id = id_str,
                                new_correlation_id = %new_id,
                                error = %e,
                                "Invalid correlation ID in headers, generated new one"
                            );
                            new_id
                        }
                    }
                }
                Err(e) => {
                    let new_id = CorrelationId::new();
                    tracing::warn!(
                        new_correlation_id = %new_id,
                        error = %e,
                        "Failed to parse correlation ID header, generated new one"
                    );
                    new_id
                }
            }
        }
        None => {
            let new_id = CorrelationId::new();
            tracing::debug!(
                correlation_id = %new_id,
                "No correlation ID in headers, generated new one"
            );
            new_id
        }
    }
}
```

**Объяснение**: `CorrelationId` - это type-safe обертка вокруг UUID, которая обеспечивает корреляцию запросов через всю систему. Функции извлечения поддерживают graceful fallback и подробное логирование для debugging.

### Prometheus Metrics Implementation - Реализация метрик Prometheus

#### Metrics - Центральная структура метрик
```rust
// src/telemetry/metrics.rs
use prometheus::{
    Counter, Gauge, Histogram, IntCounter, IntGauge, Registry, 
    HistogramOpts, Opts, Result as PrometheusResult,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    
    // HTTP метрики
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: IntGauge,
    pub http_requests_success_total: IntCounter,
    pub http_requests_error_total: IntCounter,
    
    // GraphQL метрики
    pub graphql_requests_total: IntCounter,
    pub graphql_request_duration: Histogram,
    pub graphql_errors_total: IntCounter,
    pub graphql_query_complexity: Histogram,
    pub graphql_resolver_duration: Histogram,
    
    // Бизнес-метрики
    pub reviews_created_total: IntCounter,
    pub reviews_updated_total: IntCounter,
    pub reviews_deleted_total: IntCounter,
    pub active_reviews_gauge: IntGauge,
    pub average_rating_gauge: Gauge,
    pub rating_distribution: Histogram,
    
    // Инфраструктурные метрики
    pub database_connections_active: IntGauge,
    pub database_connections_idle: IntGauge,
    pub database_query_duration: Histogram,
    pub external_service_calls_total: IntCounter,
    pub external_service_errors_total: IntCounter,
    pub external_service_duration: Histogram,
}

impl Metrics {
    pub fn new() -> PrometheusResult<Self> {
        let registry = Arc::new(Registry::new());
        
        // HTTP метрики
        let http_requests_total = IntCounter::new(
            "http_requests_total",
            "Total number of HTTP requests"
        )?;
        
        let http_request_duration = Histogram::with_opts(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
        )?;
        
        let http_requests_in_flight = IntGauge::new(
            "http_requests_in_flight",
            "Number of HTTP requests currently being processed"
        )?;
        
        let http_requests_success_total = IntCounter::new(
            "http_requests_success_total",
            "Total number of successful HTTP requests"
        )?;
        
        let http_requests_error_total = IntCounter::new(
            "http_requests_error_total",
            "Total number of failed HTTP requests"
        )?;
        
        // GraphQL метрики
        let graphql_requests_total = IntCounter::new(
            "graphql_requests_total",
            "Total number of GraphQL requests"
        )?;
        
        let graphql_request_duration = Histogram::with_opts(
            HistogramOpts::new(
                "graphql_request_duration_seconds",
                "GraphQL request duration in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
        )?;
        
        let graphql_errors_total = IntCounter::new(
            "graphql_errors_total",
            "Total number of GraphQL errors"
        )?;
        
        let graphql_query_complexity = Histogram::with_opts(
            HistogramOpts::new(
                "graphql_query_complexity",
                "GraphQL query complexity score"
            ).buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0])
        )?;
        
        let graphql_resolver_duration = Histogram::with_opts(
            HistogramOpts::new(
                "graphql_resolver_duration_seconds",
                "GraphQL resolver execution duration in seconds"
            ).buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5])
        )?;
        
        // Бизнес-метрики
        let reviews_created_total = IntCounter::new(
            "reviews_created_total",
            "Total number of reviews created"
        )?;
        
        let reviews_updated_total = IntCounter::new(
            "reviews_updated_total",
            "Total number of reviews updated"
        )?;
        
        let reviews_deleted_total = IntCounter::new(
            "reviews_deleted_total",
            "Total number of reviews deleted"
        )?;
        
        let active_reviews_gauge = IntGauge::new(
            "active_reviews_total",
            "Current number of active reviews"
        )?;
        
        let average_rating_gauge = Gauge::new(
            "average_rating",
            "Current average rating across all reviews"
        )?;
        
        let rating_distribution = Histogram::with_opts(
            HistogramOpts::new(
                "rating_distribution",
                "Distribution of review ratings"
            ).buckets(vec![1.0, 2.0, 3.0, 4.0, 5.0])
        )?;
        
        // Инфраструктурные метрики
        let database_connections_active = IntGauge::new(
            "database_connections_active",
            "Number of active database connections"
        )?;
        
        let database_connections_idle = IntGauge::new(
            "database_connections_idle",
            "Number of idle database connections"
        )?;
        
        let database_query_duration = Histogram::with_opts(
            HistogramOpts::new(
                "database_query_duration_seconds",
                "Database query duration in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
        )?;
        
        let external_service_calls_total = IntCounter::new(
            "external_service_calls_total",
            "Total number of external service calls"
        )?;
        
        let external_service_errors_total = IntCounter::new(
            "external_service_errors_total",
            "Total number of external service errors"
        )?;
        
        let external_service_duration = Histogram::with_opts(
            HistogramOpts::new(
                "external_service_duration_seconds",
                "External service call duration in seconds"
            ).buckets(vec![0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0])
        )?;
        
        // Регистрация всех метрик
        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(http_request_duration.clone()))?;
        registry.register(Box::new(http_requests_in_flight.clone()))?;
        registry.register(Box::new(http_requests_success_total.clone()))?;
        registry.register(Box::new(http_requests_error_total.clone()))?;
        
        registry.register(Box::new(graphql_requests_total.clone()))?;
        registry.register(Box::new(graphql_request_duration.clone()))?;
        registry.register(Box::new(graphql_errors_total.clone()))?;
        registry.register(Box::new(graphql_query_complexity.clone()))?;
        registry.register(Box::new(graphql_resolver_duration.clone()))?;
        
        registry.register(Box::new(reviews_created_total.clone()))?;
        registry.register(Box::new(reviews_updated_total.clone()))?;
        registry.register(Box::new(reviews_deleted_total.clone()))?;
        registry.register(Box::new(active_reviews_gauge.clone()))?;
        registry.register(Box::new(average_rating_gauge.clone()))?;
        registry.register(Box::new(rating_distribution.clone()))?;
        
        registry.register(Box::new(database_connections_active.clone()))?;
        registry.register(Box::new(database_connections_idle.clone()))?;
        registry.register(Box::new(database_query_duration.clone()))?;
        registry.register(Box::new(external_service_calls_total.clone()))?;
        registry.register(Box::new(external_service_errors_total.clone()))?;
        registry.register(Box::new(external_service_duration.clone()))?;
        
        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration,
            http_requests_in_flight,
            http_requests_success_total,
            http_requests_error_total,
            graphql_requests_total,
            graphql_request_duration,
            graphql_errors_total,
            graphql_query_complexity,
            graphql_resolver_duration,
            reviews_created_total,
            reviews_updated_total,
            reviews_deleted_total,
            active_reviews_gauge,
            average_rating_gauge,
            rating_distribution,
            database_connections_active,
            database_connections_idle,
            database_query_duration,
            external_service_calls_total,
            external_service_errors_total,
            external_service_duration,
        })
    }
}
```

**Объяснение**: `Metrics` - это центральная структура, которая инкапсулирует все метрики системы. Она использует Prometheus типы метрик (Counter, Gauge, Histogram) и автоматически регистрирует их в registry для экспорта.

### Structured Logging Implementation - Реализация структурированного логирования

#### JsonFormatter - JSON форматтер для логов
```rust
// src/telemetry/json_formatter.rs
use serde_json::{json, Value};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};
use std::fmt;

pub struct JsonFormatter;

impl<S, N> FormatEvent<S, N> for JsonFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        
        // Извлечение correlation ID из текущего span
        let correlation_id = ctx
            .lookup_current()
            .and_then(|span| span.extensions().get::<CorrelationId>().map(|id| id.to_string()))
            .unwrap_or_else(|| "unknown".to_string());
        
        // Извлечение полей события
        let mut fields = std::collections::HashMap::new();
        let mut visitor = JsonVisitor::new(&mut fields);
        event.record(&mut visitor);
        
        // Создание JSON объекта лога
        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "module": metadata.module_path().unwrap_or("unknown"),
            "file": metadata.file().unwrap_or("unknown"),
            "line": metadata.line().unwrap_or(0),
            "correlation_id": correlation_id,
            "service": "ugc-subgraph",
            "version": env!("CARGO_PKG_VERSION"),
            "environment": std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            "fields": fields,
            "span_context": extract_span_context(ctx),
        });
        
        writeln!(writer, "{}", log_entry)?;
        Ok(())
    }
}

/// Visitor для извлечения полей события в JSON формат
struct JsonVisitor<'a> {
    fields: &'a mut std::collections::HashMap<String, Value>,
}

impl<'a> JsonVisitor<'a> {
    fn new(fields: &'a mut std::collections::HashMap<String, Value>) -> Self {
        Self { fields }
    }
}

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            json!(format!("{:?}", value))
        );
    }
    
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(
            field.name().to_string(),
            json!(value)
        );
    }
    
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            json!(value)
        );
    }
    
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            json!(value)
        );
    }
    
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields.insert(
            field.name().to_string(),
            json!(value)
        );
    }
    
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(
            field.name().to_string(),
            json!(value)
        );
    }
}

/// Извлечение контекста span для логирования
fn extract_span_context<S, N>(ctx: &FmtContext<'_, S, N>) -> Value
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    if let Some(span) = ctx.lookup_current() {
        json!({
            "span_id": span.id().into_u64(),
            "span_name": span.name(),
            "span_target": span.metadata().target(),
        })
    } else {
        json!(null)
    }
}
```

**Объяснение**: `JsonFormatter` - это кастомный форматтер, который преобразует события tracing в структурированные JSON логи. Он автоматически извлекает correlation ID, span контекст и все поля события для создания богатых логов.

### Business Metrics Implementation - Реализация бизнес-метрик

#### BusinessMetricsService - Сервис бизнес-метрик
```rust
// src/telemetry/business_metrics.rs
use crate::telemetry::Metrics;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use rust_decimal::Decimal;

pub struct BusinessMetricsService {
    metrics: Arc<Metrics>,
    db_pool: PgPool,
    update_interval: Duration,
}

impl BusinessMetricsService {
    pub fn new(metrics: Arc<Metrics>, db_pool: PgPool) -> Self {
        Self {
            metrics,
            db_pool,
            update_interval: Duration::from_secs(30), // Обновление каждые 30 секунд
        }
    }
    
    /// Запуск периодического сбора бизнес-метрик
    pub async fn start_metrics_collection(&self) {
        let mut interval = time::interval(self.update_interval);
        
        tracing::info!(
            interval_seconds = self.update_interval.as_secs(),
            "Starting business metrics collection"
        );
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_business_metrics().await {
                tracing::error!(
                    error = %e,
                    "Failed to update business metrics"
                );
            }
        }
    }
    
    /// Обновление всех бизнес-метрик
    async fn update_business_metrics(&self) -> Result<(), BusinessMetricsError> {
        let start_time = std::time::Instant::now();
        
        // Обновление метрик отзывов
        self.update_review_metrics().await?;
        
        // Обновление метрик рейтингов
        self.update_rating_metrics().await?;
        
        // Обновление метрик активности пользователей
        self.update_user_activity_metrics().await?;
        
        let duration = start_time.elapsed();
        tracing::debug!(
            duration_ms = duration.as_millis(),
            "Business metrics updated successfully"
        );
        
        Ok(())
    }
    
    /// Обновление метрик отзывов
    async fn update_review_metrics(&self) -> Result<(), BusinessMetricsError> {
        // Количество активных отзывов
        let active_reviews_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reviews WHERE is_moderated = true AND deleted_at IS NULL"
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        self.metrics.active_reviews_gauge.set(active_reviews_count);
        
        // Количество отзывов за последний час
        let recent_reviews_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM reviews WHERE created_at > NOW() - INTERVAL '1 hour'"
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        tracing::debug!(
            active_reviews = active_reviews_count,
            recent_reviews = recent_reviews_count,
            "Updated review metrics"
        );
        
        Ok(())
    }
    
    /// Обновление метрик рейтингов
    async fn update_rating_metrics(&self) -> Result<(), BusinessMetricsError> {
        // Средний рейтинг
        let average_rating = sqlx::query_scalar::<_, Option<Decimal>>(
            "SELECT AVG(rating) FROM reviews WHERE is_moderated = true AND deleted_at IS NULL"
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        if let Some(rating) = average_rating {
            let rating_f64 = rating.to_f64().unwrap_or(0.0);
            self.metrics.average_rating_gauge.set(rating_f64);
        }
        
        // Распределение рейтингов
        let rating_distribution = sqlx::query(
            "SELECT rating, COUNT(*) as count 
             FROM reviews 
             WHERE is_moderated = true AND deleted_at IS NULL 
             GROUP BY rating 
             ORDER BY rating"
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        for row in rating_distribution {
            let rating: i32 = row.get("rating");
            let count: i64 = row.get("count");
            
            // Обновляем гистограмму рейтингов
            for _ in 0..count {
                self.metrics.rating_distribution.observe(rating as f64);
            }
        }
        
        tracing::debug!(
            average_rating = ?average_rating,
            "Updated rating metrics"
        );
        
        Ok(())
    }
    
    /// Обновление метрик активности пользователей
    async fn update_user_activity_metrics(&self) -> Result<(), BusinessMetricsError> {
        // Количество активных пользователей за последние 24 часа
        let active_users_24h = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT author_id) 
             FROM reviews 
             WHERE created_at > NOW() - INTERVAL '24 hours'"
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        // Количество новых пользователей за последние 24 часа
        let new_users_24h = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT author_id) 
             FROM reviews r1
             WHERE r1.created_at > NOW() - INTERVAL '24 hours'
             AND NOT EXISTS (
                 SELECT 1 FROM reviews r2 
                 WHERE r2.author_id = r1.author_id 
                 AND r2.created_at <= NOW() - INTERVAL '24 hours'
             )"
        )
        .fetch_one(&self.db_pool)
        .await
        .map_err(BusinessMetricsError::DatabaseError)?;
        
        tracing::debug!(
            active_users_24h = active_users_24h,
            new_users_24h = new_users_24h,
            "Updated user activity metrics"
        );
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BusinessMetricsError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Metrics error: {0}")]
    MetricsError(String),
}
```

**Объяснение**: `BusinessMetricsService` - это сервис, который периодически собирает бизнес-метрики из базы данных и обновляет Prometheus метрики. Он работает в фоновом режиме и обеспечивает актуальность бизнес-показателей.

### Application Integration Implementation - Интеграция в приложение

#### main.rs Integration - Интеграция в main функцию
```rust
// src/main.rs
use crate::telemetry::{
    TracingConfig, Metrics, BusinessMetricsService, 
    init_tracing, correlation_middleware, http_metrics_middleware
};
use axum::{
    routing::{get, post},
    Router, Extension,
    middleware,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<Metrics>,
    pub db_pool: PgPool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация трассировки
    let tracing_config = TracingConfig::from_env()?;
    init_tracing(tracing_config)?;

    tracing::info!(
        service = "ugc-subgraph",
        version = env!("CARGO_PKG_VERSION"),
        "Starting UGC Subgraph service"
    );

    // Создание метрик
    let metrics = Arc::new(Metrics::new()?);
    
    // Создание пула подключений к БД
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let db_pool = PgPool::connect(&database_url).await?;
    
    // Запуск сервиса бизнес-метрик
    let business_metrics = BusinessMetricsService::new(metrics.clone(), db_pool.clone());
    let business_metrics_handle = tokio::spawn(async move {
        business_metrics.start_metrics_collection().await;
    });

    // Создание состояния приложения
    let app_state = AppState {
        metrics: metrics.clone(),
        db_pool: db_pool.clone(),
    };

    // Создание GraphQL схемы с телеметрией
    let schema = create_instrumented_schema(app_state.clone()).await?;

    // Создание приложения с полной телеметрией
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_check))
        .route("/health/ready", get(readiness_check))
        .route("/health/live", get(liveness_check))
        .merge(create_metrics_router(metrics.clone()))
        .layer(Extension(schema))
        .layer(Extension(app_state))
        .layer(
            ServiceBuilder::new()
                // Порядок middleware важен - они выполняются в обратном порядке!
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn_with_state(
                    metrics.clone(),
                    http_metrics_middleware,
                ))
                .layer(middleware::from_fn(correlation_middleware))
                .layer(tower_http::timeout::TimeoutLayer::new(
                    Duration::from_secs(30)
                ))
                .layer(tower_http::limit::RequestBodyLimitLayer::new(
                    1024 * 1024 // 1MB limit
                ))
        );

    tracing::info!("UGC Subgraph server starting on port 4001");
    
    // Запуск сервера
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001").await?;
    
    // Graceful shutdown
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Ожидание сигнала завершения
    tokio::select! {
        _ = server_handle => {
            tracing::info!("Server stopped");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal");
        }
        _ = business_metrics_handle => {
            tracing::warn!("Business metrics service stopped unexpectedly");
        }
    }

    tracing::info!("UGC Subgraph service stopped");
    Ok(())
}

/// Создание роутера для метрик
fn create_metrics_router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(move || {
            let metrics = metrics.clone();
            async move {
                metrics_handler(metrics).await
            }
        }))
}

/// Handler для Prometheus метрик
async fn metrics_handler(metrics: Arc<Metrics>) -> impl axum::response::IntoResponse {
    use prometheus::TextEncoder;
    
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => {
            (
                axum::http::StatusCode::OK,
                [(
                    "content-type", 
                    "text/plain; version=0.0.4; charset=utf-8"
                )],
                output,
            )
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                "Failed to encode metrics"
            );
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                [("content-type", "text/plain")],
                "Failed to encode metrics".to_string(),
            )
        }
    }
}
```

**Объяснение**: Интеграция в `main.rs` показывает, как все компоненты телеметрии собираются вместе в реальном приложении. Это включает инициализацию трассировки, создание метрик, настройку middleware и graceful shutdown.

## 🔧 Ключевые паттерны реализации

### 1. Layered Middleware Architecture
Middleware выстроены в правильном порядке для корректной работы телеметрии:
1. **TraceLayer** - создает HTTP spans
2. **http_metrics_middleware** - собирает HTTP метрики
3. **correlation_middleware** - управляет correlation ID
4. **TimeoutLayer** - защита от зависших запросов
5. **RequestBodyLimitLayer** - защита от больших запросов

### 2. Error Handling with Telemetry
Все ошибки логируются с полным контекстом и обновляют соответствующие метрики:
```rust
match operation_result {
    Ok(result) => {
        metrics.operation_success_total.inc();
        tracing::info!(operation = "success", result = ?result);
        result
    }
    Err(error) => {
        metrics.operation_error_total.inc();
        tracing::error!(operation = "failed", error = %error);
        return Err(error);
    }
}
```

### 3. Async Background Tasks
Бизнес-метрики собираются в фоновых задачах, не блокируя основной поток обработки запросов.

### 4. Type Safety
Все компоненты телеметрии используют строгую типизацию Rust для предотвращения ошибок во время выполнения.

## 🎯 Заключение

Code диаграмма демонстрирует, как архитектурные решения Task 8 превращаются в конкретный, работающий код. Каждый компонент имеет четкую ответственность, правильно интегрируется с другими частями системы и обеспечивает полную наблюдаемость GraphQL федерации Auto.ru.