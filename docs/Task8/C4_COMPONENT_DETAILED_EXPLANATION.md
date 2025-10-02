# Task 8: Component Diagram - Подробное объяснение компонентов телеметрии

## 🎯 Цель диаграммы

Component диаграмма Task 8 детализирует **внутреннюю структуру компонентов системы телеметрии**, показывая конкретные компоненты внутри каждого слоя, их взаимодействие и специализированные функции для обеспечения comprehensive наблюдаемости GraphQL федерации Auto.ru.

## 🏗️ Детальная структура компонентов телеметрии

### 1. Distributed Tracing Components - Компоненты распределенной трассировки

#### OpenTelemetry Integration - Интеграция OpenTelemetry
```rust
// ugc-subgraph/src/telemetry/tracing/tracer_provider.rs
use opentelemetry::{
    global,
    sdk::{
        trace::{self, RandomIdGenerator, Sampler, BatchConfig},
        Resource,
    },
    KeyValue,
};

/// Провайдер трассировщика с полной конфигурацией
#[derive(Debug)]
pub struct TracerProvider {
    service_name: String,
    service_version: String,
    resource_attributes: Vec<KeyValue>,
    sampling_config: SamplingConfig,
    exporter_config: ExporterConfig,
}

impl TracerProvider {
    pub fn new(service_name: String, service_version: String) -> Self {
        Self {
            service_name: service_name.clone(),
            service_version: service_version.clone(),
            resource_attributes: Self::create_default_attributes(&service_name, &service_version),
            sampling_config: SamplingConfig::default(),
            exporter_config: ExporterConfig::default(),
        }
    }

    /// Создание resource attributes для идентификации сервиса
    fn create_default_attributes(service_name: &str, service_version: &str) -> Vec<KeyValue> {
        vec![
            KeyValue::new("service.name", service_name.to_string()),
            KeyValue::new("service.version", service_version.to_string()),
            KeyValue::new("service.namespace", "auto-ru-federation"),
            KeyValue::new("service.instance.id", uuid::Uuid::new_v4().to_string()),
            KeyValue::new("deployment.environment", 
                std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
            ),
            KeyValue::new("k8s.cluster.name", 
                std::env::var("K8S_CLUSTER_NAME").unwrap_or_else(|_| "local".to_string())
            ),
            KeyValue::new("k8s.namespace.name", 
                std::env::var("K8S_NAMESPACE").unwrap_or_else(|_| "default".to_string())
            ),
            KeyValue::new("k8s.pod.name", 
                std::env::var("K8S_POD_NAME").unwrap_or_else(|_| "unknown".to_string())
            ),
            KeyValue::new("telemetry.sdk.name", "opentelemetry"),
            KeyValue::new("telemetry.sdk.language", "rust"),
            KeyValue::new("telemetry.sdk.version", "0.20.0"),
        ]
    }

    /// Создание tracer provider с оптимизированной конфигурацией
    pub async fn build(&self) -> Result<opentelemetry::sdk::trace::TracerProvider, TracingError> {
        let resource = Resource::new(self.resource_attributes.clone());
        
        let tracer_provider = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(self.create_exporter())
            .with_trace_config(
                trace::config()
                    .with_sampler(self.create_sampler())
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(resource)
                    .with_max_events_per_span(self.sampling_config.max_events_per_span)
                    .with_max_attributes_per_span(self.sampling_config.max_attributes_per_span)
                    .with_max_links_per_span(self.sampling_config.max_links_per_span)
            )
            .with_batch_config(self.create_batch_config())
            .install_batch(opentelemetry::runtime::Tokio)?;

        Ok(tracer_provider)
    }

    /// Создание OTLP exporter
    fn create_exporter(&self) -> opentelemetry_otlp::SpanExporter {
        let mut exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(&self.exporter_config.endpoint)
            .with_timeout(self.exporter_config.timeout);

        // Добавляем authentication headers если настроены
        if let Some(ref headers) = self.exporter_config.headers {
            exporter = exporter.with_headers(headers.clone());
        }

        exporter
    }

    /// Создание sampler на основе конфигурации
    fn create_sampler(&self) -> Sampler {
        match self.sampling_config.strategy {
            SamplingStrategy::AlwaysOn => Sampler::AlwaysOn,
            SamplingStrategy::AlwaysOff => Sampler::AlwaysOff,
            SamplingStrategy::TraceIdRatio => {
                Sampler::TraceIdRatioBased(self.sampling_config.sample_rate)
            }
            SamplingStrategy::ParentBased => {
                Sampler::ParentBased(Box::new(
                    Sampler::TraceIdRatioBased(self.sampling_config.sample_rate)
                ))
            }
        }
    }

    /// Создание batch configuration для оптимизации производительности
    fn create_batch_config(&self) -> BatchConfig {
        BatchConfig::default()
            .with_max_export_batch_size(self.exporter_config.batch_size)
            .with_max_export_timeout(self.exporter_config.batch_timeout)
            .with_max_queue_size(self.exporter_config.queue_size)
            .with_scheduled_delay(self.exporter_config.scheduled_delay)
    }
}

#[derive(Debug, Clone)]
pub struct SamplingConfig {
    pub strategy: SamplingStrategy,
    pub sample_rate: f64,
    pub max_events_per_span: usize,
    pub max_attributes_per_span: usize,
    pub max_links_per_span: usize,
}

#[derive(Debug, Clone)]
pub enum SamplingStrategy {
    AlwaysOn,
    AlwaysOff,
    TraceIdRatio,
    ParentBased,
}

#[derive(Debug, Clone)]
pub struct ExporterConfig {
    pub endpoint: String,
    pub timeout: Duration,
    pub headers: Option<HashMap<String, String>>,
    pub batch_size: usize,
    pub batch_timeout: Duration,
    pub queue_size: usize,
    pub scheduled_delay: Duration,
}
```

#### Span Processor - Обработчик спанов
```rust
// ugc-subgraph/src/telemetry/tracing/span_processor.rs
use opentelemetry::sdk::trace::{SpanProcessor as OtelSpanProcessor, SpanData};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Кастомный span processor для дополнительной обработки
#[derive(Debug)]
pub struct SpanProcessor {
    batch_processor: Arc<dyn OtelSpanProcessor>,
    business_processor: Arc<BusinessSpanProcessor>,
    metrics_processor: Arc<MetricsSpanProcessor>,
    sender: mpsc::UnboundedSender<SpanData>,
}

impl SpanProcessor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let business_processor = Arc::new(BusinessSpanProcessor::new());
        let metrics_processor = Arc::new(MetricsSpanProcessor::new());
        
        // Запускаем фоновую задачу обработки спанов
        let business_proc = business_processor.clone();
        let metrics_proc = metrics_processor.clone();
        tokio::spawn(async move {
            Self::process_spans(receiver, business_proc, metrics_proc).await;
        });

        Self {
            batch_processor: Arc::new(
                opentelemetry::sdk::trace::BatchSpanProcessor::builder(
                    opentelemetry_otlp::new_exporter().http(),
                    opentelemetry::runtime::Tokio
                ).build()
            ),
            business_processor,
            metrics_processor,
            sender,
        }
    }

    /// Фоновая обработка спанов для бизнес-аналитики
    async fn process_spans(
        mut receiver: mpsc::UnboundedReceiver<SpanData>,
        business_processor: Arc<BusinessSpanProcessor>,
        metrics_processor: Arc<MetricsSpanProcessor>,
    ) {
        while let Some(span_data) = receiver.recv().await {
            // Обработка для бизнес-аналитики
            if let Err(e) = business_processor.process_span(&span_data).await {
                error!("Failed to process span for business analytics: {}", e);
            }

            // Обработка для метрик
            if let Err(e) = metrics_processor.process_span(&span_data).await {
                error!("Failed to process span for metrics: {}", e);
            }
        }
    }
}

impl OtelSpanProcessor for SpanProcessor {
    fn on_start(&self, span: &mut opentelemetry::sdk::trace::Span, cx: &opentelemetry::Context) {
        self.batch_processor.on_start(span, cx);
    }

    fn on_end(&self, span: SpanData) {
        // Отправляем в основной processor
        self.batch_processor.on_end(span.clone());
        
        // Отправляем в наш кастомный processor
        if let Err(e) = self.sender.send(span) {
            error!("Failed to send span for custom processing: {}", e);
        }
    }

    fn force_flush(&self) -> opentelemetry::sdk::trace::TraceResult<()> {
        self.batch_processor.force_flush()
    }

    fn shutdown(&mut self) -> opentelemetry::sdk::trace::TraceResult<()> {
        self.batch_processor.shutdown()
    }
}

/// Processor для бизнес-аналитики спанов
#[derive(Debug)]
pub struct BusinessSpanProcessor {
    business_metrics: Arc<BusinessMetrics>,
}

impl BusinessSpanProcessor {
    pub fn new() -> Self {
        Self {
            business_metrics: Arc::new(BusinessMetrics::new()),
        }
    }

    pub async fn process_span(&self, span_data: &SpanData) -> Result<(), ProcessingError> {
        let span_name = &span_data.name;
        let attributes = &span_data.attributes;

        // Анализируем бизнес-операции
        match span_name.as_ref() {
            "create_review" => {
                self.process_review_creation_span(span_data).await?;
            }
            "update_review" => {
                self.process_review_update_span(span_data).await?;
            }
            "fetch_user_data" => {
                self.process_user_fetch_span(span_data).await?;
            }
            "external_service_call" => {
                self.process_external_service_span(span_data).await?;
            }
            _ => {
                // Общая обработка для других спанов
                self.process_generic_span(span_data).await?;
            }
        }

        Ok(())
    }

    async fn process_review_creation_span(&self, span_data: &SpanData) -> Result<(), ProcessingError> {
        let duration = span_data.end_time - span_data.start_time;
        
        // Извлекаем бизнес-атрибуты
        let user_id = span_data.attributes.get(&Key::new("user_id"))
            .and_then(|v| v.as_str());
        let offer_id = span_data.attributes.get(&Key::new("offer_id"))
            .and_then(|v| v.as_str());
        let rating = span_data.attributes.get(&Key::new("rating"))
            .and_then(|v| v.as_i64());

        // Обновляем бизнес-метрики
        self.business_metrics.record_review_creation(
            duration,
            user_id,
            offer_id,
            rating,
        ).await;

        // Проверяем на аномалии
        if duration > Duration::from_secs(5) {
            warn!(
                span_name = %span_data.name,
                duration_ms = duration.as_millis(),
                user_id = ?user_id,
                "Slow review creation detected"
            );
        }

        Ok(())
    }
}
```#### 
Correlation Management - Управление корреляцией
```rust
// ugc-subgraph/src/telemetry/tracing/correlation.rs
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::http::HeaderMap;

/// Генератор correlation ID с различными стратегиями
#[derive(Debug)]
pub struct CorrelationGenerator {
    strategy: CorrelationStrategy,
    prefix: Option<String>,
    counter: Arc<RwLock<u64>>,
}

impl CorrelationGenerator {
    pub fn new(strategy: CorrelationStrategy) -> Self {
        Self {
            strategy,
            prefix: std::env::var("CORRELATION_ID_PREFIX").ok(),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Генерация нового correlation ID
    pub async fn generate(&self) -> String {
        match self.strategy {
            CorrelationStrategy::UUID => {
                let uuid = Uuid::new_v4().to_string();
                self.add_prefix(uuid)
            }
            CorrelationStrategy::Timestamp => {
                let timestamp = chrono::Utc::now().timestamp_millis();
                let counter = {
                    let mut c = self.counter.write().await;
                    *c += 1;
                    *c
                };
                let id = format!("{}-{}", timestamp, counter);
                self.add_prefix(id)
            }
            CorrelationStrategy::Sequential => {
                let counter = {
                    let mut c = self.counter.write().await;
                    *c += 1;
                    *c
                };
                let id = format!("req-{:08}", counter);
                self.add_prefix(id)
            }
        }
    }

    fn add_prefix(&self, id: String) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}-{}", prefix, id),
            None => id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CorrelationStrategy {
    UUID,
    Timestamp,
    Sequential,
}

/// Propagator для W3C Trace Context
#[derive(Debug)]
pub struct ContextPropagator {
    trace_context_propagator: opentelemetry::sdk::propagation::TraceContextPropagator,
    baggage_propagator: opentelemetry::sdk::propagation::BaggagePropagator,
}

impl ContextPropagator {
    pub fn new() -> Self {
        Self {
            trace_context_propagator: opentelemetry::sdk::propagation::TraceContextPropagator::new(),
            baggage_propagator: opentelemetry::sdk::propagation::BaggagePropagator::new(),
        }
    }

    /// Извлечение контекста из HTTP headers
    pub fn extract_context(&self, headers: &HeaderMap) -> opentelemetry::Context {
        let extractor = HeaderExtractor::new(headers);
        
        // Извлекаем trace context
        let mut context = self.trace_context_propagator.extract(&extractor);
        
        // Извлекаем baggage
        context = self.baggage_propagator.extract_with_context(&context, &extractor);
        
        context
    }

    /// Внедрение контекста в HTTP headers
    pub fn inject_context(&self, context: &opentelemetry::Context, headers: &mut HeaderMap) {
        let mut injector = HeaderInjector::new(headers);
        
        // Внедряем trace context
        self.trace_context_propagator.inject_context(context, &mut injector);
        
        // Внедряем baggage
        self.baggage_propagator.inject_context(context, &mut injector);
    }
}

/// Extractor для извлечения headers
struct HeaderExtractor<'a> {
    headers: &'a HeaderMap,
}

impl<'a> HeaderExtractor<'a> {
    fn new(headers: &'a HeaderMap) -> Self {
        Self { headers }
    }
}

impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key)?.to_str().ok()
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|k| k.as_str()).collect()
    }
}

/// Injector для внедрения headers
struct HeaderInjector<'a> {
    headers: &'a mut HeaderMap,
}

impl<'a> HeaderInjector<'a> {
    fn new(headers: &'a mut HeaderMap) -> Self {
        Self { headers }
    }
}

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_value) = value.parse() {
            self.headers.insert(
                key.parse().unwrap_or_else(|_| axum::http::HeaderName::from_static("x-unknown")),
                header_value,
            );
        }
    }
}
```

### 2. Prometheus Metrics Components - Компоненты метрик Prometheus

#### Metrics Collection - Сбор метрик
```rust
// ugc-subgraph/src/telemetry/metrics/collection.rs
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Registry, 
    register_counter_with_registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
    HistogramOpts, Opts,
};

/// Реестр метрик с автоматической регистрацией
#[derive(Debug)]
pub struct MetricsRegistry {
    registry: Arc<Registry>,
    namespace: String,
    default_labels: HashMap<String, String>,
}

impl MetricsRegistry {
    pub fn new(namespace: String) -> Result<Self, MetricsError> {
        let registry = Arc::new(Registry::new());
        let default_labels = Self::create_default_labels();
        
        Ok(Self {
            registry,
            namespace,
            default_labels,
        })
    }

    fn create_default_labels() -> HashMap<String, String> {
        let mut labels = HashMap::new();
        labels.insert("service".to_string(), "ugc-subgraph".to_string());
        labels.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        labels.insert("environment".to_string(), 
            std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
        );
        labels
    }

    /// Создание counter с автоматическими labels
    pub fn create_counter(&self, name: &str, help: &str) -> Result<IntCounter, MetricsError> {
        let full_name = format!("{}_{}", self.namespace, name);
        let opts = Opts::new(full_name, help)
            .const_labels(prometheus::labels! {
                "service" => &self.default_labels["service"],
                "version" => &self.default_labels["version"],
                "environment" => &self.default_labels["environment"],
            });
        
        register_int_counter_with_registry!(opts, self.registry.clone())
            .map_err(|e| MetricsError::RegistrationError(e.to_string()))
    }

    /// Создание histogram с оптимизированными buckets
    pub fn create_histogram(&self, name: &str, help: &str, buckets: Vec<f64>) -> Result<Histogram, MetricsError> {
        let full_name = format!("{}_{}", self.namespace, name);
        let opts = HistogramOpts::new(full_name, help)
            .const_labels(prometheus::labels! {
                "service" => &self.default_labels["service"],
                "version" => &self.default_labels["version"],
                "environment" => &self.default_labels["environment"],
            })
            .buckets(buckets);
        
        register_histogram_with_registry!(opts, self.registry.clone())
            .map_err(|e| MetricsError::RegistrationError(e.to_string()))
    }

    /// Создание gauge
    pub fn create_gauge(&self, name: &str, help: &str) -> Result<IntGauge, MetricsError> {
        let full_name = format!("{}_{}", self.namespace, name);
        let opts = Opts::new(full_name, help)
            .const_labels(prometheus::labels! {
                "service" => &self.default_labels["service"],
                "version" => &self.default_labels["version"],
                "environment" => &self.default_labels["environment"],
            });
        
        register_int_gauge_with_registry!(opts, self.registry.clone())
            .map_err(|e| MetricsError::RegistrationError(e.to_string()))
    }
}

/// HTTP метрики с детальной категоризацией
#[derive(Debug)]
pub struct HttpMetrics {
    requests_total: Counter,
    request_duration: Histogram,
    requests_in_flight: IntGauge,
    response_size: Histogram,
    request_size: Histogram,
}

impl HttpMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self, MetricsError> {
        Ok(Self {
            requests_total: registry.create_counter(
                "http_requests_total",
                "Total number of HTTP requests"
            )?,
            
            request_duration: registry.create_histogram(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
            )?,
            
            requests_in_flight: registry.create_gauge(
                "http_requests_in_flight",
                "Number of HTTP requests currently being processed"
            )?,
            
            response_size: registry.create_histogram(
                "http_response_size_bytes",
                "HTTP response size in bytes",
                prometheus::exponential_buckets(100.0, 2.0, 10).unwrap()
            )?,
            
            request_size: registry.create_histogram(
                "http_request_size_bytes", 
                "HTTP request size in bytes",
                prometheus::exponential_buckets(100.0, 2.0, 10).unwrap()
            )?,
        })
    }

    /// Запись HTTP метрик
    pub fn record_request(
        &self,
        method: &str,
        path: &str,
        status_code: u16,
        duration: Duration,
        request_size: u64,
        response_size: u64,
    ) {
        // Увеличиваем счетчик запросов
        self.requests_total
            .with_label_values(&[method, path, &status_code.to_string()])
            .inc();
        
        // Записываем время выполнения
        self.request_duration
            .with_label_values(&[method, path])
            .observe(duration.as_secs_f64());
        
        // Записываем размеры
        self.request_size
            .with_label_values(&[method])
            .observe(request_size as f64);
        
        self.response_size
            .with_label_values(&[&status_code.to_string()])
            .observe(response_size as f64);
    }

    pub fn increment_in_flight(&self) {
        self.requests_in_flight.inc();
    }

    pub fn decrement_in_flight(&self) {
        self.requests_in_flight.dec();
    }
}

/// GraphQL метрики с анализом сложности
#[derive(Debug)]
pub struct GraphQLMetrics {
    requests_total: Counter,
    request_duration: Histogram,
    errors_total: Counter,
    query_complexity: Histogram,
    query_depth: Histogram,
    field_usage: Counter,
}

impl GraphQLMetrics {
    pub fn new(registry: &MetricsRegistry) -> Result<Self, MetricsError> {
        Ok(Self {
            requests_total: registry.create_counter(
                "graphql_requests_total",
                "Total number of GraphQL requests"
            )?,
            
            request_duration: registry.create_histogram(
                "graphql_request_duration_seconds",
                "GraphQL request duration in seconds",
                vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
            )?,
            
            errors_total: registry.create_counter(
                "graphql_errors_total",
                "Total number of GraphQL errors"
            )?,
            
            query_complexity: registry.create_histogram(
                "graphql_query_complexity",
                "GraphQL query complexity score",
                prometheus::linear_buckets(0.0, 10.0, 20).unwrap()
            )?,
            
            query_depth: registry.create_histogram(
                "graphql_query_depth",
                "GraphQL query depth",
                vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 15.0, 20.0]
            )?,
            
            field_usage: registry.create_counter(
                "graphql_field_usage_total",
                "Total usage count of GraphQL fields"
            )?,
        })
    }

    /// Запись GraphQL метрик
    pub fn record_request(
        &self,
        operation_name: Option<&str>,
        operation_type: &str,
        duration: Duration,
        complexity: f64,
        depth: u32,
        field_count: u32,
        error_count: u32,
    ) {
        let op_name = operation_name.unwrap_or("anonymous");
        
        // Основные метрики
        self.requests_total
            .with_label_values(&[operation_type, op_name])
            .inc();
        
        self.request_duration
            .with_label_values(&[operation_type, op_name])
            .observe(duration.as_secs_f64());
        
        // Метрики сложности
        self.query_complexity.observe(complexity);
        self.query_depth.observe(depth as f64);
        
        // Ошибки
        if error_count > 0 {
            self.errors_total
                .with_label_values(&[operation_type, "execution_error"])
                .inc_by(error_count as u64);
        }
    }

    /// Запись использования поля
    pub fn record_field_usage(&self, type_name: &str, field_name: &str) {
        self.field_usage
            .with_label_values(&[type_name, field_name])
            .inc();
    }
}
```

### 3. Structured Logging Components - Компоненты структурированного логирования

#### Log Formatting - Форматирование логов
```rust
// ugc-subgraph/src/telemetry/logging/formatter.rs
use serde_json::{json, Value, Map};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};

/// JSON форматтер с расширенными возможностями
pub struct JsonFormatter {
    config: FormatterConfig,
    field_filter: FieldFilter,
    enricher: LogEnricher,
}

impl JsonFormatter {
    pub fn new(config: FormatterConfig) -> Self {
        Self {
            config,
            field_filter: FieldFilter::new(),
            enricher: LogEnricher::new(),
        }
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
        
        // Извлекаем поля события
        let mut fields = Map::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);
        
        // Фильтруем поля
        let filtered_fields = self.field_filter.filter_fields(fields);
        
        // Получаем контекст из span
        let span_context = self.extract_span_context(ctx);
        
        // Создаем базовую структуру лога
        let mut log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "module": metadata.module_path(),
            "file": metadata.file(),
            "line": metadata.line(),
            "service": "ugc-subgraph",
            "version": env!("CARGO_PKG_VERSION"),
        });
        
        // Добавляем span context
        if let Some(context) = span_context {
            log_entry.as_object_mut().unwrap().extend(context);
        }
        
        // Добавляем поля события
        log_entry.as_object_mut().unwrap().insert("fields".to_string(), Value::Object(filtered_fields));
        
        // Обогащаем лог дополнительной информацией
        log_entry = self.enricher.enrich_log(log_entry, metadata);
        
        // Форматируем в зависимости от конфигурации
        let output = if self.config.pretty_print {
            serde_json::to_string_pretty(&log_entry)
        } else {
            serde_json::to_string(&log_entry)
        }.map_err(|_| std::fmt::Error)?;
        
        writeln!(writer, "{}", output)?;
        Ok(())
    }
}

impl JsonFormatter {
    /// Извлечение контекста из span
    fn extract_span_context<S>(&self, ctx: &FmtContext<'_, S, impl FormatFields<S>>) -> Option<Map<String, Value>>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut context = Map::new();
        
        if let Some(span) = ctx.lookup_current() {
            // Извлекаем correlation ID
            if let Some(correlation_id) = span.extensions().get::<CorrelationId>() {
                context.insert("correlation_id".to_string(), json!(correlation_id.to_string()));
            }
            
            // Извлекаем trace context
            if let Some(trace_context) = span.extensions().get::<TraceContext>() {
                context.insert("trace_id".to_string(), json!(trace_context.trace_id()));
                context.insert("span_id".to_string(), json!(trace_context.span_id()));
            }
            
            // Извлекаем пользовательский контекст
            if let Some(user_context) = span.extensions().get::<UserContext>() {
                context.insert("user_id".to_string(), json!(user_context.user_id));
                context.insert("session_id".to_string(), json!(user_context.session_id));
            }
            
            // Извлекаем span name и attributes
            context.insert("span_name".to_string(), json!(span.name()));
            
            // Собираем span fields
            let mut span_fields = Map::new();
            span.extensions().get::<FormattedFields<N>>()
                .map(|fields| {
                    if !fields.fields.is_empty() {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&fields.fields) {
                            if let Value::Object(obj) = parsed {
                                span_fields.extend(obj);
                            }
                        }
                    }
                });
            
            if !span_fields.is_empty() {
                context.insert("span_fields".to_string(), Value::Object(span_fields));
            }
        }
        
        if context.is_empty() {
            None
        } else {
            Some(context)
        }
    }
}

/// Фильтр полей для безопасности и производительности
#[derive(Debug)]
pub struct FieldFilter {
    sensitive_fields: HashSet<String>,
    max_field_length: usize,
    max_fields_count: usize,
}

impl FieldFilter {
    pub fn new() -> Self {
        let mut sensitive_fields = HashSet::new();
        sensitive_fields.insert("password".to_string());
        sensitive_fields.insert("token".to_string());
        sensitive_fields.insert("secret".to_string());
        sensitive_fields.insert("key".to_string());
        sensitive_fields.insert("authorization".to_string());
        
        Self {
            sensitive_fields,
            max_field_length: 1000,
            max_fields_count: 50,
        }
    }

    pub fn filter_fields(&self, mut fields: Map<String, Value>) -> Map<String, Value> {
        // Удаляем чувствительные поля
        fields.retain(|key, _| !self.is_sensitive_field(key));
        
        // Ограничиваем количество полей
        if fields.len() > self.max_fields_count {
            fields = fields.into_iter().take(self.max_fields_count).collect();
            fields.insert("_truncated".to_string(), json!(true));
        }
        
        // Ограничиваем длину значений
        for (_, value) in fields.iter_mut() {
            self.truncate_value(value);
        }
        
        fields
    }

    fn is_sensitive_field(&self, field_name: &str) -> bool {
        let lower_name = field_name.to_lowercase();
        self.sensitive_fields.iter().any(|sensitive| lower_name.contains(sensitive))
    }

    fn truncate_value(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                if s.len() > self.max_field_length {
                    s.truncate(self.max_field_length);
                    s.push_str("...[truncated]");
                }
            }
            Value::Object(obj) => {
                for (_, v) in obj.iter_mut() {
                    self.truncate_value(v);
                }
            }
            Value::Array(arr) => {
                for v in arr.iter_mut() {
                    self.truncate_value(v);
                }
            }
            _ => {}
        }
    }
}

/// Обогащение логов дополнительной информацией
#[derive(Debug)]
pub struct LogEnricher {
    hostname: String,
    process_id: u32,
}

impl LogEnricher {
    pub fn new() -> Self {
        Self {
            hostname: hostname::get()
                .unwrap_or_else(|_| "unknown".into())
                .to_string_lossy()
                .to_string(),
            process_id: std::process::id(),
        }
    }

    pub fn enrich_log(&self, mut log_entry: Value, metadata: &tracing::Metadata) -> Value {
        if let Some(obj) = log_entry.as_object_mut() {
            // Добавляем системную информацию
            obj.insert("hostname".to_string(), json!(self.hostname));
            obj.insert("process_id".to_string(), json!(self.process_id));
            obj.insert("thread_id".to_string(), json!(format!("{:?}", std::thread::current().id())));
            
            // Добавляем информацию о среде выполнения
            obj.insert("environment".to_string(), json!(
                std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
            ));
            
            // Добавляем категоризацию логов
            obj.insert("log_category".to_string(), json!(self.categorize_log(metadata)));
            
            // Добавляем severity mapping
            obj.insert("severity".to_string(), json!(self.map_severity(metadata.level())));
        }
        
        log_entry
    }

    fn categorize_log(&self, metadata: &tracing::Metadata) -> &'static str {
        let target = metadata.target();
        
        if target.contains("business") || target.contains("event") {
            "business"
        } else if target.contains("security") || target.contains("auth") {
            "security"
        } else if target.contains("performance") || target.contains("metrics") {
            "performance"
        } else if target.contains("error") || metadata.level() == &tracing::Level::ERROR {
            "error"
        } else {
            "application"
        }
    }

    fn map_severity(&self, level: &tracing::Level) -> u8 {
        match *level {
            tracing::Level::ERROR => 3,
            tracing::Level::WARN => 4,
            tracing::Level::INFO => 6,
            tracing::Level::DEBUG => 7,
            tracing::Level::TRACE => 7,
        }
    }
}
```

Эта Component диаграмма демонстрирует детальную внутреннюю структуру каждого компонента системы телеметрии, показывая как OpenTelemetry integration, Prometheus metrics collection и structured logging работают вместе для обеспечения comprehensive наблюдаемости с полным мониторингом и business intelligence.