# Task 8: AI Commands Collection - Настройка телеметрии и мониторинга

## 📋 Обзор Task 8

Task 8 "Настройка телеметрии и мониторинга" включает в себя:
- **8.1** Реализовать distributed tracing (OpenTelemetry с Jaeger)
- **8.2** Добавить сбор метрик (Prometheus метрики для производительности)
- **8.3** Настроить логирование (структурированное логирование с контекстом)

## 🤖 Команды AI для реализации Task 8

### 📁 Этап 1: Создание структуры телеметрии (Task 8.1)

#### Команда 1: Создание модуля телеметрии
```bash
# Создание структуры для телеметрии
mkdir -p ugc-subgraph/src/telemetry
touch ugc-subgraph/src/telemetry/mod.rs
touch ugc-subgraph/src/telemetry/tracing.rs
touch ugc-subgraph/src/telemetry/metrics.rs
touch ugc-subgraph/src/telemetry/logging.rs
```

**Объяснение**: Создаем модульную структуру для телеметрии, разделяя функциональность на трассировку, метрики и логирование для лучшей организации кода.

#### Команда 2: Настройка зависимостей для телеметрии
```toml
# Добавление в Cargo.toml
[dependencies]
# OpenTelemetry и трассировка
opentelemetry = "0.20"
opentelemetry-otlp = "0.13"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tracing-opentelemetry = "0.21"

# Prometheus метрики
prometheus = "0.13"

# JSON логирование
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

**Объяснение**: Добавляем необходимые зависимости для OpenTelemetry (distributed tracing), Prometheus (метрики) и структурированного JSON логирования.

#### Команда 3: Реализация distributed tracing с OpenTelemetry
```rust
// Файл: ugc-subgraph/src/telemetry/tracing.rs
use anyhow::Result;
use opentelemetry::{
    global,
    sdk::{
        trace::{self, RandomIdGenerator, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

pub fn init_tracing(config: TracingConfig) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "ugc_subgraph=debug,tower_http=debug,sqlx=info".into());

    let registry = Registry::default().with(env_filter);

    if let Some(jaeger_endpoint) = &config.jaeger_endpoint {
        let tracer = init_tracer(&config)?;
        let telemetry_layer = OpenTelemetryLayer::new(tracer);
        registry.with(telemetry_layer).try_init()?;
    } else {
        registry.try_init()?;
    }

    Ok(())
}
```

**Объяснение**: Инициализируем OpenTelemetry с Jaeger для distributed tracing. Настраиваем sampling rate, service name и экспорт трассировок в Jaeger через OTLP протокол.

### ⚡ Этап 2: Реализация сбора метрик (Task 8.2)

#### Команда 4: Создание системы метрик Prometheus
```rust
// Файл: ugc-subgraph/src/telemetry/metrics.rs
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Registry, TextEncoder,
    register_counter_with_registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};

#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    
    // HTTP метрики
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: IntGauge,
    
    // GraphQL метрики
    pub graphql_requests_total: IntCounter,
    pub graphql_request_duration: Histogram,
    pub graphql_errors_total: IntCounter,
    pub graphql_query_complexity: Histogram,
    
    // Бизнес-метрики
    pub reviews_created_total: IntCounter,
    pub reviews_updated_total: IntCounter,
    pub reviews_deleted_total: IntCounter,
    pub active_reviews_gauge: IntGauge,
    pub average_rating_gauge: prometheus::Gauge,
    
    // Метрики базы данных
    pub db_connections_active: IntGauge,
    pub db_query_duration: Histogram,
    pub db_queries_total: IntCounter,
    pub db_errors_total: IntCounter,
}
```

**Объяснение**: Создаем comprehensive систему метрик с различными типами: Counter (счетчики), Histogram (распределения), Gauge (текущие значения) для HTTP, GraphQL, бизнес-логики и базы данных.

#### Команда 5: Реализация middleware для сбора HTTP метрик
```rust
pub async fn http_metrics_middleware<B>(
    State(metrics): State<Arc<Metrics>>,
    request: axum::extract::Request<B>,
    next: axum::middleware::Next<B>,
) -> Response {
    let start = std::time::Instant::now();
    metrics.http_requests_in_flight.inc();
    metrics.http_requests_total.inc();
    
    let response = next.run(request).await;
    
    let duration = start.elapsed().as_secs_f64();
    metrics.http_request_duration.observe(duration);
    metrics.http_requests_in_flight.dec();
    
    response
}
```

**Объяснение**: Создаем middleware для автоматического сбора HTTP метрик: количество запросов, время выполнения, количество активных запросов. Используем Axum middleware для интеграции.

#### Команда 6: Создание endpoint для метрик
```rust
pub async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => {
            (
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                output,
            ).into_response()
        }
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode metrics").into_response()
        }
    }
}
```

**Объяснение**: Создаем HTTP endpoint `/metrics` для экспорта метрик в формате Prometheus. Используем TextEncoder для сериализации метрик в стандартный формат Prometheus.

### 📊 Этап 3: Настройка структурированного логирования (Task 8.3)

#### Команда 7: Реализация JSON форматтера для логов
```rust
// Файл: ugc-subgraph/src/telemetry/logging.rs
use serde_json::{json, Value};
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};

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
    ) -> std::fmt::Result {
        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "correlation_id": correlation_id,
            "service": "ugc-subgraph",
            "version": env!("CARGO_PKG_VERSION"),
            "fields": fields,
        });

        writeln!(writer, "{}", log_entry)?;
        Ok(())
    }
}
```

**Объяснение**: Создаем кастомный JSON форматтер для структурированного логирования. Каждый лог содержит timestamp, уровень, correlation ID, service name и все поля события в JSON формате.

#### Команда 8: Реализация correlation ID middleware
```rust
pub async fn correlation_middleware<B>(
    mut request: axum::extract::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    let correlation_id = extract_correlation_id(request.headers());
    
    // Добавляем correlation ID в request extensions
    request.extensions_mut().insert(correlation_id.clone());
    
    // Добавляем correlation ID в текущий span
    let span = Span::current();
    span.record("correlation_id", &correlation_id.to_string());
    
    let mut response = next.run(request).await;
    
    // Добавляем correlation ID в response headers
    response.headers_mut().insert(
        "x-correlation-id",
        correlation_id.to_string().parse().unwrap(),
    );
    
    response
}
```

**Объяснение**: Создаем middleware для управления correlation ID - уникальным идентификатором запроса, который проходит через всю систему для трассировки запросов между сервисами.

#### Команда 9: Создание бизнес-событий логирования
```rust
pub struct BusinessEventLogger;

impl BusinessEventLogger {
    pub fn review_created(review_id: Uuid, offer_id: Uuid, author_id: Uuid, rating: i32) {
        tracing::info!(
            event_type = "review_created",
            review_id = %review_id,
            offer_id = %offer_id,
            author_id = %author_id,
            rating = rating,
            "Review created successfully"
        );
    }

    pub fn external_service_error(service: &str, error: &str, duration_ms: u64) {
        tracing::error!(
            event_type = "external_service_error",
            service = service,
            error = error,
            duration_ms = duration_ms,
            "External service call failed"
        );
    }
}
```

**Объяснение**: Создаем специализированные логгеры для бизнес-событий с структурированными полями. Это позволяет легко анализировать бизнес-метрики и события в системе мониторинга.

### 🐳 Этап 4: Настройка инфраструктуры мониторинга

#### Команда 10: Создание конфигурации Prometheus
```yaml
# Файл: prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "prometheus-alerts.yml"

scrape_configs:
  # Apollo Router metrics
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    scrape_interval: 5s
    metrics_path: /metrics

  # UGC Subgraph metrics
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:4001']
    scrape_interval: 5s
    metrics_path: /metrics
```

**Объяснение**: Настраиваем Prometheus для сбора метрик с Apollo Router и всех подграфов. Устанавливаем интервал сбора 5 секунд для детального мониторинга производительности.

#### Команда 11: Создание алертов Prometheus
```yaml
# Файл: prometheus-alerts.yml
groups:
  - name: ugc-subgraph-alerts
    rules:
      # High error rate alert
      - alert: UGCSubgraphHighErrorRate
        expr: rate(graphql_errors_total{service="ugc-subgraph"}[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
          service: ugc-subgraph
        annotations:
          summary: "UGC Subgraph has high error rate"
          description: "UGC Subgraph error rate is {{ $value }} errors per second"

      # Circuit breaker opened
      - alert: UGCSubgraphCircuitBreakerOpen
        expr: circuit_breaker_state{service="ugc-subgraph"} == 1
        for: 0m
        labels:
          severity: warning
        annotations:
          summary: "Circuit breaker opened"
          description: "Circuit breaker for {{ $labels.service_name }} is open"
```

**Объяснение**: Создаем алерты для критических метрик: высокий уровень ошибок, открытие circuit breaker, проблемы с базой данных. Алерты помогают быстро реагировать на проблемы в production.

#### Команда 12: Настройка Docker Compose для мониторинга
```yaml
# Добавление в docker-compose.yml
services:
  # Jaeger for distributed tracing
  jaeger:
    image: jaegertracing/all-in-one:1.49
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # HTTP collector
    environment:
      - COLLECTOR_OTLP_ENABLED=true
    networks:
      - federation-network

  # Prometheus for metrics
  prometheus:
    image: prom/prometheus:v2.47.0
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.enable-lifecycle'
    networks:
      - federation-network

  # Grafana for visualization
  grafana:
    image: grafana/grafana:10.1.0
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
    networks:
      - federation-network
```

**Объяснение**: Настраиваем полный стек мониторинга: Jaeger для трассировки, Prometheus для метрик, Grafana для визуализации. Все сервисы подключены к общей сети для взаимодействия.

### 🔧 Этап 5: Конфигурация Apollo Router для телеметрии

#### Команда 13: Настройка телеметрии в router.yaml
```yaml
# Файл: router.yaml
telemetry:
  # Metrics configuration
  metrics:
    prometheus:
      enabled: true
      listen: 0.0.0.0:9090
      path: /metrics
    common:
      service_name: "apollo-router"
      service_version: "1.0.0"
    
  # Distributed tracing configuration
  tracing:
    trace_config:
      service_name: "apollo-router"
      service_version: "1.0.0"
      sampler: 1.0  # Sample 100% of traces in development
      parent_based_sampler: true
    
    # OTLP exporter
    otlp:
      enabled: true
      endpoint: http://otel-collector:4317
      protocol: grpc
      batch_processor:
        max_export_batch_size: 512
        max_export_timeout: 30s
```

**Объяснение**: Настраиваем телеметрию в Apollo Router: включаем Prometheus метрики на порту 9090, настраиваем OTLP экспорт для трассировки, устанавливаем sampling rate 100% для development.

#### Команда 14: Настройка переменных окружения для телеметрии
```bash
# Файл: .env.telemetry
# Distributed Tracing
JAEGER_ENDPOINT=http://localhost:14268/api/traces
TRACE_SAMPLE_RATE=1.0
ENABLE_CONSOLE_LOGS=true

# OpenTelemetry
OTEL_SERVICE_NAME=ugc-subgraph
OTEL_SERVICE_VERSION=1.0.0
OTEL_EXPORTER_JAEGER_ENDPOINT=http://localhost:14268/api/traces
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318

# Prometheus Metrics
PROMETHEUS_ENDPOINT=http://localhost:9090

# Logging
LOG_LEVEL=debug
LOG_FORMAT=json
RUST_LOG=ugc_subgraph=debug,tower_http=debug,sqlx=info

# Correlation ID
CORRELATION_ID_HEADER=x-correlation-id

# Business Metrics Update Interval (seconds)
BUSINESS_METRICS_INTERVAL=60
```

**Объяснение**: Создаем конфигурационный файл с переменными окружения для всех аспектов телеметрии: endpoints для Jaeger и Prometheus, уровни логирования, интервалы обновления метрик.

### 📈 Этап 6: Интеграция телеметрии в приложение

#### Команда 15: Инициализация телеметрии в main.rs
```rust
// Файл: ugc-subgraph/src/main.rs
use crate::telemetry::{init_tracing, Metrics, TracingConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация трассировки
    let tracing_config = TracingConfig::default();
    init_tracing(tracing_config)?;

    // Создание метрик
    let metrics = Arc::new(Metrics::new()?);

    // Создание приложения с телеметрией
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_check))
        .merge(create_metrics_router(metrics.clone()))
        .layer(middleware::from_fn_with_state(
            metrics.clone(),
            http_metrics_middleware,
        ))
        .layer(middleware::from_fn(correlation_middleware))
        .with_state(AppState { metrics, /* ... */ });

    info!("UGC Subgraph server starting on port 4001");
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

**Объяснение**: Интегрируем все компоненты телеметрии в основное приложение: инициализируем трассировку, создаем метрики, добавляем middleware для автоматического сбора данных.

#### Команда 16: Создание макросов для удобного использования
```rust
// Макросы для трассировки
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {
        tracing::info_span!($name, correlation_id = %uuid::Uuid::new_v4())
    };
}

// Макросы для измерения времени операций
#[macro_export]
macro_rules! time_operation {
    ($metrics:expr, $histogram:ident, $operation:expr) => {{
        let _timer = $crate::telemetry::metrics::MetricsTimer::new($metrics.$histogram.clone());
        $operation
    }};
}

// Макросы для структурированного логирования
#[macro_export]
macro_rules! log_info {
    ($($field:tt)*) => {
        tracing::info!($($field)*)
    };
}
```

**Объяснение**: Создаем удобные макросы для использования телеметрии в коде: автоматическое создание spans с correlation ID, измерение времени операций, структурированное логирование.

### 🧪 Этап 7: Тестирование телеметрии

#### Команда 17: Создание тестов для метрик
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        assert_eq!(metrics.http_requests_total.get(), 0);
        assert_eq!(metrics.reviews_created_total.get(), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = Metrics::new().unwrap();
        metrics.http_requests_total.inc();
        assert_eq!(metrics.http_requests_total.get(), 1);
    }

    #[tokio::test]
    async fn test_correlation_id_generation() {
        let _span = trace_span!("test_span");
        let correlation_id = get_correlation_id();
        println!("Correlation ID: {:?}", correlation_id);
    }
}
```

**Объяснение**: Создаем unit тесты для проверки корректности работы метрик, correlation ID и других компонентов телеметрии.

#### Команда 18: Создание интеграционных тестов
```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    let metrics = Arc::new(Metrics::new().unwrap());
    let app = create_metrics_router(metrics.clone());
    
    let response = app
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    
    // Проверяем, что метрики присутствуют в ответе
    assert!(body_str.contains("http_requests_total"));
    assert!(body_str.contains("ugc-subgraph"));
}
```

**Объяснение**: Создаем интеграционные тесты для проверки работы endpoint'а метрик, корректности формата Prometheus и наличия всех необходимых метрик.

## 🎯 Итоговые результаты Task 8

### ✅ Достигнутые цели:

1. **Distributed Tracing (8.1)**:
   - OpenTelemetry интеграция с Jaeger
   - Автоматическая трассировка HTTP запросов
   - Correlation ID для связи запросов между сервисами
   - W3C Trace Context поддержка

2. **Metrics Collection (8.2)**:
   - Prometheus метрики для HTTP, GraphQL, бизнес-логики
   - Автоматический сбор через middleware
   - Endpoint `/metrics` для экспорта
   - Алерты для критических метрик

3. **Structured Logging (8.3)**:
   - JSON форматированные логи
   - Correlation ID в каждом логе
   - Бизнес-события логирование
   - Различные уровни логирования

### 📊 Инфраструктура мониторинга:
- **Jaeger**: Distributed tracing и анализ производительности
- **Prometheus**: Сбор и хранение метрик
- **Grafana**: Визуализация метрик и дашборды
- **AlertManager**: Уведомления о проблемах

### 🔧 Интеграция:
- Apollo Router телеметрия
- Middleware для автоматического сбора данных
- Environment-based конфигурация
- Docker Compose для локальной разработки

### 📈 Метрики:
- HTTP: requests/sec, latency, errors
- GraphQL: query complexity, execution time
- Business: reviews created/updated/deleted
- Infrastructure: DB connections, external services

Эта реализация обеспечивает полную наблюдаемость системы с enterprise-grade мониторингом и алертингом.