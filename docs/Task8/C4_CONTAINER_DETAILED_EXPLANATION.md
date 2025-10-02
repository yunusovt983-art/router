# Task 8: Container Diagram - Подробное объяснение контейнерной архитектуры телеметрии

## 🎯 Цель диаграммы

Container диаграмма Task 8 детализирует **внутреннюю архитектуру системы телеметрии и мониторинга** на уровне контейнеров, показывая как различные слои наблюдаемости взаимодействуют друг с другом, их технологические стеки и паттерны интеграции для обеспечения полной видимости системы.

## 🏗️ Архитектурные слои телеметрии

### 1. Telemetry Layer - Слой телеметрии

#### Tracing Service - Сервис distributed tracing
```rust
// ugc-subgraph/src/telemetry/tracing.rs
use opentelemetry::{
    global,
    sdk::{
        trace::{self, RandomIdGenerator, Sampler, BatchConfig},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

#[derive(Clone)]
pub struct TracingService {
    config: TracingConfig,
    tracer_provider: Option<opentelemetry::sdk::trace::TracerProvider>,
    correlation_tracker: Arc<CorrelationTracker>,
    span_processor: Arc<SpanProcessor>,
}

impl TracingService {
    pub fn new(config: TracingConfig) -> Result<Self, TracingError> {
        Ok(Self {
            config,
            tracer_provider: None,
            correlation_tracker: Arc::new(CorrelationTracker::new()),
            span_processor: Arc::new(SpanProcessor::new()),
        })
    }

    /// Инициализация distributed tracing с OpenTelemetry
    pub async fn initialize(&mut self) -> Result<(), TracingError> {
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,sqlx=info,async_graphql=debug",
                    self.config.service_name
                ).into()
            });

        let registry = Registry::default().with(env_filter);

        // Console layer для development
        let registry = if self.config.enable_console {
            registry.with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
            )
        } else {
            registry
        };

        // OpenTelemetry layer для production tracing
        if let Some(jaeger_endpoint) = &self.config.jaeger_endpoint {
            info!("Initializing OpenTelemetry with Jaeger endpoint: {}", jaeger_endpoint);
            
            let tracer_provider = self.init_tracer_provider().await?;
            let tracer = tracer_provider.tracer("ugc-subgraph");
            let telemetry_layer = OpenTelemetryLayer::new(tracer);
            
            self.tracer_provider = Some(tracer_provider);
            registry.with(telemetry_layer).try_init()?;
        } else {
            warn!("No Jaeger endpoint configured, skipping OpenTelemetry initialization");
            registry.try_init()?;
        }

        info!("Tracing service initialized successfully");
        Ok(())
    }

    /// Создание tracer provider с оптимизированной конфигурацией
    async fn init_tracer_provider(&self) -> Result<opentelemetry::sdk::trace::TracerProvider, TracingError> {
        let jaeger_endpoint = self.config.jaeger_endpoint.as_ref()
            .ok_or_else(|| TracingError::ConfigError("Jaeger endpoint not configured".to_string()))?;

        // Создание OTLP exporter
        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(jaeger_endpoint)
            .with_timeout(Duration::from_secs(10))
            .with_headers(self.create_exporter_headers());

        // Конфигурация batch processor для оптимизации производительности
        let batch_config = BatchConfig::default()
            .with_max_export_batch_size(512)
            .with_max_export_timeout(Duration::from_secs(30))
            .with_max_queue_size(2048)
            .with_scheduled_delay(Duration::from_secs(5));

        // Создание tracer provider
        let tracer_provider = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(
                trace::config()
                    .with_sampler(self.create_sampler())
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(self.create_resource())
                    .with_max_events_per_span(128)
                    .with_max_attributes_per_span(128)
                    .with_max_links_per_span(128)
            )
            .with_batch_config(batch_config)
            .install_batch(opentelemetry::runtime::Tokio)?;

        Ok(tracer_provider)
    }

    /// Создание sampler на основе конфигурации
    fn create_sampler(&self) -> Sampler {
        match self.config.sampling_strategy.as_str() {
            "always_on" => Sampler::AlwaysOn,
            "always_off" => Sampler::AlwaysOff,
            "trace_id_ratio" => Sampler::TraceIdRatioBased(self.config.sample_rate),
            "parent_based" => Sampler::ParentBased(Box::new(
                Sampler::TraceIdRatioBased(self.config.sample_rate)
            )),
            _ => Sampler::TraceIdRatioBased(self.config.sample_rate),
        }
    }

    /// Создание resource с метаданными сервиса
    fn create_resource(&self) -> Resource {
        Resource::new(vec![
            KeyValue::new("service.name", self.config.service_name.clone()),
            KeyValue::new("service.version", self.config.service_version.clone()),
            KeyValue::new("service.namespace", "auto-ru-federation"),
            KeyValue::new("service.instance.id", uuid::Uuid::new_v4().to_string()),
            KeyValue::new("deployment.environment", 
                std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
            ),
            KeyValue::new("telemetry.sdk.name", "opentelemetry"),
            KeyValue::new("telemetry.sdk.language", "rust"),
            KeyValue::new("telemetry.sdk.version", "0.20.0"),
        ])
    }

    /// Создание headers для exporter
    fn create_exporter_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/x-protobuf".to_string());
        
        if let Ok(auth_token) = std::env::var("JAEGER_AUTH_TOKEN") {
            headers.insert("Authorization".to_string(), format!("Bearer {}", auth_token));
        }
        
        headers
    }

    /// Создание span с контекстом
    pub fn create_span(&self, name: &str, attributes: Vec<(&str, String)>) -> tracing::Span {
        let span = tracing::info_span!(
            name,
            correlation_id = %self.correlation_tracker.get_or_create_correlation_id(),
            service.name = %self.config.service_name,
            service.version = %self.config.service_version
        );
        
        // Добавляем custom attributes
        for (key, value) in attributes {
            span.record(key, &tracing::field::display(&value));
        }
        
        span
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) {
        if let Some(provider) = &self.tracer_provider {
            if let Err(e) = provider.shutdown() {
                error!("Failed to shutdown tracer provider: {}", e);
            } else {
                info!("Tracer provider shutdown successfully");
            }
        }
        
        global::shutdown_tracer_provider();
    }
}

/// Конфигурация трассировки
#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub service_name: String,
    pub service_version: String,
    pub jaeger_endpoint: Option<String>,
    pub sample_rate: f64,
    pub sampling_strategy: String,
    pub enable_console: bool,
    pub max_spans_per_trace: usize,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "ugc-subgraph".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            sample_rate: std::env::var("TRACE_SAMPLE_RATE")
                .unwrap_or_else(|_| "1.0".to_string())
                .parse()
                .unwrap_or(1.0),
            sampling_strategy: std::env::var("SAMPLING_STRATEGY")
                .unwrap_or_else(|_| "parent_based".to_string()),
            enable_console: std::env::var("ENABLE_CONSOLE_LOGS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            max_spans_per_trace: 1000,
        }
    }
}

/// Трекер correlation ID
#[derive(Debug)]
pub struct CorrelationTracker {
    current_id: Arc<RwLock<Option<String>>>,
}

impl CorrelationTracker {
    pub fn new() -> Self {
        Self {
            current_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_or_create_correlation_id(&self) -> String {
        let current = self.current_id.read().unwrap();
        if let Some(id) = current.as_ref() {
            id.clone()
        } else {
            drop(current);
            let new_id = uuid::Uuid::new_v4().to_string();
            *self.current_id.write().unwrap() = Some(new_id.clone());
            new_id
        }
    }

    pub fn set_correlation_id(&self, id: String) {
        *self.current_id.write().unwrap() = Some(id);
    }
}
```#### Metri
cs Collector - Сборщик метрик Prometheus
```rust
// ugc-subgraph/src/telemetry/metrics.rs
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Registry, TextEncoder,
    register_counter_with_registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
    HistogramOpts, Opts,
};

#[derive(Clone)]
pub struct MetricsCollector {
    pub registry: Arc<Registry>,
    
    // HTTP метрики
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: IntGauge,
    pub http_responses_by_status: Counter,
    
    // GraphQL метрики
    pub graphql_requests_total: IntCounter,
    pub graphql_request_duration: Histogram,
    pub graphql_errors_total: Counter,
    pub graphql_query_complexity: Histogram,
    pub graphql_requests_successful: IntCounter,
    
    // Business метрики
    pub reviews_created_total: IntCounter,
    pub reviews_updated_total: IntCounter,
    pub reviews_deleted_total: IntCounter,
    pub active_reviews_gauge: IntGauge,
    pub average_rating_gauge: prometheus::Gauge,
    
    // Infrastructure метрики
    pub db_connections_active: IntGauge,
    pub db_query_duration: Histogram,
    pub db_queries_total: IntCounter,
    pub db_errors_total: IntCounter,
    pub external_requests_total: IntCounter,
    pub external_request_duration: Histogram,
}

impl MetricsCollector {
    pub fn new() -> Result<Self, MetricsError> {
        let registry = Arc::new(Registry::new());
        
        // Создание всех метрик с правильными labels
        let metrics = Self {
            registry: registry.clone(),
            
            http_requests_total: register_int_counter_with_registry!(
                Opts::new("http_requests_total", "Total HTTP requests")
                    .const_labels(prometheus::labels! {
                        "service" => "ugc-subgraph",
                        "version" => env!("CARGO_PKG_VERSION")
                    }),
                registry.clone()
            )?,
            
            // ... остальные метрики
        };
        
        Ok(metrics)
    }

    /// Экспорт метрик в формате Prometheus
    pub async fn export_metrics(&self) -> Result<String, MetricsError> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        
        encoder.encode_to_string(&metric_families)
            .map_err(|e| MetricsError::ExportError(e.to_string()))
    }
}
```

#### Logging Service - Сервис структурированного логирования
```rust
// ugc-subgraph/src/telemetry/logging.rs
use serde_json::{json, Value};
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};

#[derive(Clone)]
pub struct LoggingService {
    config: LoggingConfig,
    business_logger: Arc<BusinessEventLogger>,
    security_logger: Arc<SecurityEventLogger>,
    audit_logger: Arc<AuditLogger>,
}

impl LoggingService {
    pub fn new(config: LoggingConfig) -> Self {
        Self {
            config,
            business_logger: Arc::new(BusinessEventLogger::new()),
            security_logger: Arc::new(SecurityEventLogger::new()),
            audit_logger: Arc::new(AuditLogger::new()),
        }
    }

    /// Инициализация structured logging
    pub fn initialize(&self) -> Result<(), LoggingError> {
        let formatter = JsonFormatter::new(self.config.clone());
        
        let subscriber = tracing_subscriber::fmt()
            .event_format(formatter)
            .with_max_level(self.config.log_level)
            .with_target(false)
            .with_thread_ids(true)
            .finish();
            
        tracing::subscriber::set_global_default(subscriber)?;
        
        info!("Structured logging initialized");
        Ok(())
    }
}

/// JSON форматтер для structured logs
pub struct JsonFormatter {
    config: LoggingConfig,
}

impl JsonFormatter {
    pub fn new(config: LoggingConfig) -> Self {
        Self { config }
    }
}

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
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        let mut fields = HashMap::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        // Извлекаем correlation ID из span context
        let correlation_id = ctx
            .lookup_current()
            .and_then(|span| {
                span.extensions()
                    .get::<CorrelationId>()
                    .map(|id| id.0.to_string())
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "module": metadata.module_path(),
            "file": metadata.file(),
            "line": metadata.line(),
            "correlation_id": correlation_id,
            "service": "ugc-subgraph",
            "version": env!("CARGO_PKG_VERSION"),
            "environment": std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            "fields": fields,
        });

        writeln!(writer, "{}", log_entry)?;
        Ok(())
    }
}
```

### 2. Instrumented Application Layer - Слой инструментированного приложения

#### UGC GraphQL Server - GraphQL сервер с инструментацией
```rust
// ugc-subgraph/src/graphql/mod.rs
use async_graphql::{
    Context, FieldResult, Object, Schema, EmptySubscription,
    extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute},
};

/// GraphQL сервер с полной инструментацией
pub struct InstrumentedGraphQLServer {
    schema: Schema<Query, Mutation, EmptySubscription>,
    metrics: Arc<MetricsCollector>,
    tracing_service: Arc<TracingService>,
}

impl InstrumentedGraphQLServer {
    pub fn new(
        metrics: Arc<MetricsCollector>,
        tracing_service: Arc<TracingService>,
    ) -> Self {
        let schema = Schema::build(Query, Mutation, EmptySubscription)
            .data(metrics.clone())
            .data(tracing_service.clone())
            .extension(MetricsExtension::new(metrics.clone()))
            .extension(TracingExtension::new(tracing_service.clone()))
            .extension(QueryComplexityExtension::new(1000))
            .enable_federation()
            .finish();

        Self {
            schema,
            metrics,
            tracing_service,
        }
    }
}

/// Extension для сбора GraphQL метрик
pub struct MetricsExtension {
    metrics: Arc<MetricsCollector>,
}

impl MetricsExtension {
    pub fn new(metrics: Arc<MetricsCollector>) -> Self {
        Self { metrics }
    }
}

impl ExtensionFactory for MetricsExtension {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(MetricsExtensionImpl {
            metrics: self.metrics.clone(),
        })
    }
}

struct MetricsExtensionImpl {
    metrics: Arc<MetricsCollector>,
}

#[async_trait::async_trait]
impl Extension for MetricsExtensionImpl {
    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> async_graphql::ServerResult<async_graphql::Response> {
        let start_time = std::time::Instant::now();
        
        // Увеличиваем счетчик запросов
        self.metrics.graphql_requests_total.inc();
        
        // Анализируем сложность запроса
        if let Some(query) = ctx.query_env.query.as_ref() {
            let complexity = calculate_query_complexity(query);
            self.metrics.graphql_query_complexity.observe(complexity);
        }
        
        let result = next.run(ctx, operation_name).await;
        
        // Записываем время выполнения
        let duration = start_time.elapsed().as_secs_f64();
        self.metrics.graphql_request_duration.observe(duration);
        
        // Записываем результат
        match &result {
            Ok(response) => {
                if response.errors.is_empty() {
                    self.metrics.graphql_requests_successful.inc();
                } else {
                    self.metrics.graphql_errors_total
                        .with_label_values(&["graphql_error"])
                        .inc();
                }
            }
            Err(_) => {
                self.metrics.graphql_errors_total
                    .with_label_values(&["execution_error"])
                    .inc();
            }
        }
        
        result
    }
}
```

### 3. Telemetry Infrastructure - Инфраструктура телеметрии

#### OpenTelemetry Collector - Сборщик и обработчик телеметрии
```yaml
# otel-collector-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318
        cors:
          allowed_origins:
            - "http://localhost:3000"
            - "https://auto.ru"

  prometheus:
    config:
      scrape_configs:
        - job_name: 'ugc-subgraph'
          static_configs:
            - targets: ['ugc-subgraph:4001']
          scrape_interval: 15s
          metrics_path: /metrics

processors:
  batch:
    timeout: 1s
    send_batch_size: 1024
    send_batch_max_size: 2048

  memory_limiter:
    limit_mib: 512

  resource:
    attributes:
      - key: service.namespace
        value: auto-ru-federation
        action: upsert
      - key: deployment.environment
        from_attribute: environment
        action: insert

  attributes:
    actions:
      - key: correlation_id
        action: insert
        from_attribute: correlation_id
      - key: user_id
        action: insert
        from_attribute: user_id

exporters:
  jaeger:
    endpoint: jaeger-collector:14250
    tls:
      insecure: true

  prometheus:
    endpoint: "0.0.0.0:8889"
    namespace: auto_ru
    const_labels:
      environment: production

  elasticsearch:
    endpoints: ["http://elasticsearch:9200"]
    logs_index: "otel-logs"
    traces_index: "otel-traces"

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, resource, attributes, batch]
      exporters: [jaeger]

    metrics:
      receivers: [otlp, prometheus]
      processors: [memory_limiter, resource, batch]
      exporters: [prometheus]

    logs:
      receivers: [otlp]
      processors: [memory_limiter, resource, attributes, batch]
      exporters: [elasticsearch]

  extensions: [health_check, pprof, zpages]
```

Эта Container диаграмма демонстрирует детальную архитектуру телеметрии на уровне контейнеров, показывая как различные слои (Telemetry, Application, Infrastructure) работают вместе для обеспечения comprehensive наблюдаемости с полной интеграцией трассировки, метрик и логирования.