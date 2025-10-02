# Task 8: Context Diagram - Подробное объяснение системы телеметрии и мониторинга

## 🎯 Цель диаграммы

Context диаграмма Task 8 демонстрирует **комплексную enterprise-grade систему наблюдаемости** для федеративной GraphQL платформы Auto.ru, показывая как система телеметрии интегрируется с инфраструктурой мониторинга, логирования и алертинга для обеспечения полной видимости производительности и поведения системы.

## 🏗️ Архитектурная эволюция: от "слепой" к наблюдаемой системе

### От системы без наблюдаемости к comprehensive telemetry

#### Было: Система без наблюдаемости
```rust
// Простой GraphQL резолвер без инструментации
async fn create_review(ctx: &Context<'_>, input: CreateReviewInput) -> FieldResult<Review> {
    // Нет трассировки запросов
    let review = review_service.create_review(input).await?;
    
    // Нет метрик производительности
    // Нет структурированного логирования
    // Нет correlation ID для debugging
    // Нет business metrics
    
    Ok(review)
}

// Проблемы:
// - Невозможно отследить производительность
// - Нет visibility в distributed системе
// - Сложный debugging при проблемах
// - Отсутствие business intelligence
// - Нет proactive мониторинга
```

#### Стало: Полная наблюдаемость с Three Pillars
```rust
// GraphQL резолвер с полной инструментацией
#[tracing::instrument(skip(ctx), fields(correlation_id = %correlation_id))]
async fn create_review_instrumented(
    ctx: &Context<'_>, 
    input: CreateReviewInput
) -> FieldResult<Review> {
    let metrics = ctx.data::<Arc<Metrics>>()?;
    let correlation_id = ctx.data::<CorrelationId>()?;
    
    // 1. METRICS: Prometheus метрики для производительности
    let _timer = MetricsTimer::new(metrics.graphql_request_duration.clone());
    metrics.graphql_requests_total.inc();
    
    // 2. TRACING: OpenTelemetry distributed tracing
    let span = trace_span!("create_review", 
        user_id = %input.author_id,
        offer_id = %input.offer_id
    );
    
    let result = async move {
        // 3. LOGGING: Structured business event logging
        match review_service.create_review(input).await {
            Ok(review) => {
                // Business metrics
                metrics.reviews_created_total.inc();
                
                // Business event logging
                BusinessEventLogger::review_created(
                    review.id, review.offer_id, review.author_id, review.rating
                );
                
                // Performance metrics
                metrics.average_rating_gauge.set(
                    calculate_new_average_rating(&review).await
                );
                
                Ok(review)
            }
            Err(e) => {
                // Error metrics and logging
                metrics.graphql_errors_total.inc();
                SecurityEventLogger::suspicious_activity(
                    Some(input.author_id), "review_creation_failed", &e.to_string()
                );
                Err(e)
            }
        }
    }.instrument(span).await;
    
    result
}

// Преимущества:
// ✅ Полная трассировка через distributed систему
// ✅ Real-time метрики производительности и бизнеса
// ✅ Structured logging для debugging и анализа
// ✅ Correlation ID для связи событий
// ✅ Proactive alerting при проблемах
// ✅ Business intelligence и KPI tracking
```

**Объяснение**: Наблюдаемая архитектура превращает "черный ящик" в полностью прозрачную систему с тремя столпами наблюдаемости: Metrics (количественные данные), Logs (качественные события), Traces (flow запросов).
## 
🔧 Ключевые компоненты и их реализация

### 1. Auto.ru Telemetry & Monitoring Federation - Основная система с инструментацией

#### UGC Subgraph (Instrumented) - Инструментированный подграф
```rust
// ugc-subgraph/src/main.rs
use std::sync::Arc;
use axum::{routing::post, Router, Extension};
use tower::ServiceBuilder;
use crate::telemetry::{init_tracing, Metrics, TracingConfig, BusinessMetricsService};

#[derive(Clone)]
pub struct InstrumentedUgcService {
    // Telemetry компоненты
    metrics: Arc<Metrics>,
    tracing_config: TracingConfig,
    business_metrics: Arc<BusinessMetricsService>,
    
    // Application компоненты
    db_pool: PgPool,
    review_service: Arc<ReviewService>,
    external_service_client: Arc<ExternalServiceClient>,
}

impl InstrumentedUgcService {
    pub async fn new() -> Result<Self, ServiceError> {
        // 1. Инициализация трассировки
        let tracing_config = TracingConfig {
            service_name: "ugc-subgraph".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            sample_rate: std::env::var("TRACE_SAMPLE_RATE")
                .unwrap_or_else(|_| "1.0".to_string())
                .parse()
                .unwrap_or(1.0),
            enable_console: std::env::var("ENABLE_CONSOLE_LOGS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        };
        
        init_tracing(tracing_config.clone())?;
        
        // 2. Создание метрик
        let metrics = Arc::new(Metrics::new()?);
        
        // 3. Настройка базы данных с мониторингом
        let db_pool = PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&database_url)
            .await?;
        
        // 4. Бизнес-метрики сервис
        let business_metrics = Arc::new(BusinessMetricsService::new(
            metrics.clone(),
            db_pool.clone(),
            Duration::from_secs(60), // Update interval
        ));
        
        Ok(Self {
            metrics,
            tracing_config,
            business_metrics,
            db_pool,
            review_service: Arc::new(ReviewService::new(db_pool.clone())),
            external_service_client: Arc::new(ExternalServiceClient::new()),
        })
    }

    /// Создание веб-сервера с полной телеметрией
    pub fn create_server(&self) -> Result<Router, ServiceError> {
        let schema = self.create_instrumented_graphql_schema();
        
        let app = Router::new()
            .route("/graphql", post(graphql_handler))
            .route("/health", get(health_check))
            .route("/metrics", get(metrics_handler)) // Prometheus endpoint
            .layer(Extension(schema))
            .layer(Extension(self.clone()))
            .layer(
                ServiceBuilder::new()
                    // Telemetry middleware stack (порядок важен!)
                    .layer(self.create_correlation_middleware())
                    .layer(self.create_tracing_middleware())
                    .layer(self.create_metrics_middleware())
                    .layer(self.create_logging_middleware())
            );

        Ok(app)
    }

    /// Создание GraphQL схемы с инструментацией
    fn create_instrumented_graphql_schema(&self) -> Schema<Query, Mutation, Subscription> {
        Schema::build(Query, Mutation, Subscription)
            .data(self.metrics.clone())
            .data(self.business_metrics.clone())
            .data(self.db_pool.clone())
            .enable_federation()
            // Добавляем telemetry extensions
            .extension(async_graphql::extensions::Tracing)
            .extension(MetricsExtension::new(self.metrics.clone()))
            .extension(QueryComplexityExtension::new(1000)) // Max complexity
            .finish()
    }

    /// Middleware для correlation ID
    fn create_correlation_middleware(&self) -> impl Layer<Router> {
        tower::layer::layer_fn(move |service| {
            tower::service_fn(move |mut request| {
                let service = service.clone();
                async move {
                    // Извлекаем или создаем correlation ID
                    let correlation_id = extract_correlation_id(request.headers());
                    
                    // Добавляем в request extensions
                    request.extensions_mut().insert(correlation_id.clone());
                    
                    // Добавляем в текущий span
                    let span = tracing::Span::current();
                    span.record("correlation_id", &correlation_id.to_string());
                    
                    let mut response = service.call(request).await?;
                    
                    // Добавляем correlation ID в response headers
                    response.headers_mut().insert(
                        "x-correlation-id",
                        correlation_id.to_string().parse().unwrap(),
                    );
                    
                    Ok(response)
                }
            })
        })
    }

    /// Middleware для HTTP метрик
    fn create_metrics_middleware(&self) -> impl Layer<Router> {
        let metrics = self.metrics.clone();
        
        tower::layer::layer_fn(move |service| {
            let metrics = metrics.clone();
            
            tower::service_fn(move |request| {
                let metrics = metrics.clone();
                let service = service.clone();
                let start_time = std::time::Instant::now();
                
                async move {
                    // Увеличиваем счетчики
                    metrics.http_requests_total.inc();
                    metrics.http_requests_in_flight.inc();
                    
                    let result = service.call(request).await;
                    
                    // Записываем время выполнения
                    let duration = start_time.elapsed().as_secs_f64();
                    metrics.http_request_duration.observe(duration);
                    metrics.http_requests_in_flight.dec();
                    
                    // Записываем статус код
                    if let Ok(ref response) = result {
                        let status_code = response.status().as_u16().to_string();
                        metrics.http_responses_by_status
                            .with_label_values(&[&status_code])
                            .inc();
                    }
                    
                    result
                }
            })
        })
    }

    /// Запуск фоновых задач телеметрии
    pub async fn start_background_tasks(&self) {
        let business_metrics = self.business_metrics.clone();
        
        // Задача обновления бизнес-метрик
        tokio::spawn(async move {
            business_metrics.start_metrics_collection().await;
        });
        
        // Задача очистки старых метрик
        let metrics = self.metrics.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Каждый час
            loop {
                interval.tick().await;
                metrics.cleanup_old_metrics().await;
            }
        });
    }
}

/// Извлечение correlation ID из headers
pub fn extract_correlation_id(headers: &HeaderMap) -> CorrelationId {
    headers
        .get("x-correlation-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| CorrelationId::from_string(s).ok())
        .unwrap_or_else(CorrelationId::new)
}

/// Health check с телеметрией
pub async fn health_check(Extension(service): Extension<InstrumentedUgcService>) -> impl IntoResponse {
    let health_span = trace_span!("health_check");
    
    async move {
        // Проверяем состояние компонентов
        let db_healthy = service.check_database_health().await;
        let external_services_healthy = service.check_external_services_health().await;
        let telemetry_healthy = service.check_telemetry_health().await;
        
        let overall_health = db_healthy && external_services_healthy && telemetry_healthy;
        
        let health_status = json!({
            "status": if overall_health { "healthy" } else { "unhealthy" },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "components": {
                "database": db_healthy,
                "external_services": external_services_healthy,
                "telemetry": telemetry_healthy
            },
            "version": env!("CARGO_PKG_VERSION"),
            "service": "ugc-subgraph"
        });
        
        if overall_health {
            (StatusCode::OK, Json(health_status))
        } else {
            (StatusCode::SERVICE_UNAVAILABLE, Json(health_status))
        }
    }.instrument(health_span).await
}
```

#### Apollo Router (Telemetry) - Роутер с телеметрией
```yaml
# router.yaml - конфигурация телеметрии
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
      attributes:
        supergraph:
          static:
            - name: "environment"
              value: "${ENVIRONMENT:-development}"
            - name: "datacenter"
              value: "${DATACENTER:-local}"
        subgraph:
          all:
            static:
              - name: "federation_version"
                value: "2.0"
    
  # Distributed tracing configuration
  tracing:
    trace_config:
      service_name: "apollo-router"
      service_version: "1.0.0"
      sampler: "${TRACE_SAMPLE_RATE:-1.0}"
      parent_based_sampler: true
      max_events_per_span: 128
      max_attributes_per_span: 128
      max_links_per_span: 128
    
    # OTLP exporter для Jaeger
    otlp:
      enabled: true
      endpoint: "${OTEL_EXPORTER_OTLP_ENDPOINT:-http://otel-collector:4317}"
      protocol: grpc
      batch_processor:
        max_export_batch_size: 512
        max_export_timeout: 30s
        max_queue_size: 2048
        scheduled_delay: 5s
      
  # Logging configuration
  apollo:
    # Apollo Studio integration (опционально)
    graph_ref: "${APOLLO_GRAPH_REF:-auto-ru-federation@development}"
    key: "${APOLLO_KEY:-}"
    schema_reporting:
      enabled: false
    usage_reporting:
      enabled: "${APOLLO_USAGE_REPORTING:-false}"

# Headers для propagation трассировки
headers:
  all:
    request:
      # Propagate tracing headers
      - propagate:
          named: "traceparent"
      - propagate:
          named: "tracestate"
      - propagate:
          named: "baggage"
      - propagate:
          named: "x-correlation-id"
      # Business headers
      - propagate:
          named: "authorization"
      - propagate:
          named: "x-user-id"
    response:
      # Add telemetry headers
      - insert:
          name: "x-apollo-router-version"
          value: "1.33.0"
      - insert:
          name: "x-correlation-id"
          from_request: "x-correlation-id"
```

### 2. Monitoring Infrastructure - Инфраструктура мониторинга

#### Jaeger System - Система distributed tracing
```yaml
# docker-compose.yml - Jaeger configuration
services:
  jaeger:
    image: jaegertracing/all-in-one:1.49
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # HTTP collector
      - "4317:4317"    # OTLP gRPC receiver
      - "4318:4318"    # OTLP HTTP receiver
    environment:
      # Storage configuration
      - SPAN_STORAGE_TYPE=cassandra
      - CASSANDRA_SERVERS=cassandra:9042
      - CASSANDRA_KEYSPACE=jaeger_v1_dc1
      
      # Collector configuration
      - COLLECTOR_OTLP_ENABLED=true
      - COLLECTOR_ZIPKIN_HOST_PORT=:9411
      
      # Query configuration
      - QUERY_BASE_PATH=/jaeger
      
      # Sampling configuration
      - SAMPLING_STRATEGIES_FILE=/etc/jaeger/sampling_strategies.json
    volumes:
      - ./jaeger/sampling_strategies.json:/etc/jaeger/sampling_strategies.json
    networks:
      - telemetry-network
    depends_on:
      - cassandra

  # Cassandra для хранения трассировок
  cassandra:
    image: cassandra:4.0
    ports:
      - "9042:9042"
    environment:
      - CASSANDRA_CLUSTER_NAME=jaeger
      - CASSANDRA_DC=dc1
      - CASSANDRA_RACK=rack1
      - CASSANDRA_ENDPOINT_SNITCH=GossipingPropertyFileSnitch
    volumes:
      - cassandra_data:/var/lib/cassandra
    networks:
      - telemetry-network
```

```json
// jaeger/sampling_strategies.json
{
  "service_strategies": [
    {
      "service": "ugc-subgraph",
      "type": "probabilistic",
      "param": 1.0,
      "max_traces_per_second": 100,
      "operation_strategies": [
        {
          "operation": "create_review",
          "type": "probabilistic", 
          "param": 1.0
        },
        {
          "operation": "health_check",
          "type": "probabilistic",
          "param": 0.1
        }
      ]
    },
    {
      "service": "apollo-router",
      "type": "probabilistic",
      "param": 0.5,
      "max_traces_per_second": 200
    }
  ],
  "default_strategy": {
    "type": "probabilistic",
    "param": 0.1,
    "max_traces_per_second": 50
  }
}
```

#### Prometheus System - Система сбора метрик
```yaml
# prometheus.yml - конфигурация сбора метрик
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'auto-ru-federation'
    environment: 'production'

rule_files:
  - "prometheus-alerts.yml"
  - "business-alerts.yml"

scrape_configs:
  # Apollo Router metrics
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    scrape_interval: 5s
    metrics_path: /metrics
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
        replacement: 'apollo-router'

  # UGC Subgraph metrics
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:4001']
    scrape_interval: 5s
    metrics_path: /metrics
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
        replacement: 'ugc-subgraph'

  # Infrastructure metrics
  - job_name: 'node-exporter'
    static_configs:
      - targets: ['node-exporter:9100']
    scrape_interval: 15s

  # Database metrics
  - job_name: 'postgres-exporter'
    static_configs:
      - targets: ['postgres-exporter:9187']
    scrape_interval: 30s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
      path_prefix: /alertmanager
      scheme: http
```

```yaml
# prometheus-alerts.yml - алерты производительности
groups:
  - name: ugc-subgraph-performance
    rules:
      # High latency alert
      - alert: UGCSubgraphHighLatency
        expr: histogram_quantile(0.95, rate(graphql_request_duration_seconds_bucket{service="ugc-subgraph"}[5m])) > 1.0
        for: 2m
        labels:
          severity: warning
          service: ugc-subgraph
          team: backend
        annotations:
          summary: "UGC Subgraph high latency detected"
          description: "95th percentile latency is {{ $value }}s for more than 2 minutes"
          runbook_url: "https://runbooks.auto.ru/ugc-high-latency"
          dashboard_url: "https://grafana.auto.ru/d/ugc-performance"

      # High error rate alert
      - alert: UGCSubgraphHighErrorRate
        expr: rate(graphql_errors_total{service="ugc-subgraph"}[5m]) / rate(graphql_requests_total{service="ugc-subgraph"}[5m]) > 0.05
        for: 1m
        labels:
          severity: critical
          service: ugc-subgraph
          team: backend
        annotations:
          summary: "UGC Subgraph high error rate"
          description: "Error rate is {{ $value | humanizePercentage }} over the last 5 minutes"

  - name: business-metrics
    rules:
      # Low review creation rate
      - alert: LowReviewCreationRate
        expr: rate(reviews_created_total{service="ugc-subgraph"}[1h]) < 0.01
        for: 30m
        labels:
          severity: warning
          team: product
        annotations:
          summary: "Low review creation rate detected"
          description: "Review creation rate is {{ $value }} reviews per second over the last hour"
```

## 🔗 Интеграция с внешними системами

### Logging Infrastructure - Инфраструктура логирования

#### Elasticsearch + Kibana - Поиск и анализ логов
```yaml
# docker-compose.yml - ELK Stack
services:
  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.10.0
    environment:
      - discovery.type=single-node
      - xpack.security.enabled=false
      - "ES_JAVA_OPTS=-Xms2g -Xmx2g"
      - cluster.name=auto-ru-logs
      - node.name=elasticsearch-1
    ports:
      - "9200:9200"
    volumes:
      - elasticsearch_data:/usr/share/elasticsearch/data
    networks:
      - telemetry-network

  kibana:
    image: docker.elastic.co/kibana/kibana:8.10.0
    ports:
      - "5601:5601"
    environment:
      - ELASTICSEARCH_HOSTS=http://elasticsearch:9200
      - SERVER_NAME=kibana
      - SERVER_HOST=0.0.0.0
    depends_on:
      - elasticsearch
    networks:
      - telemetry-network

  logstash:
    image: docker.elastic.co/logstash/logstash:8.10.0
    ports:
      - "5044:5044"
      - "9600:9600"
    environment:
      - "LS_JAVA_OPTS=-Xmx1g -Xms1g"
    volumes:
      - ./logstash/pipeline:/usr/share/logstash/pipeline
      - ./logstash/config:/usr/share/logstash/config
    depends_on:
      - elasticsearch
    networks:
      - telemetry-network
```

```ruby
# logstash/pipeline/ugc-logs.conf
input {
  beats {
    port => 5044
  }
  
  http {
    port => 8080
    codec => json
  }
}

filter {
  # Parse JSON logs from UGC subgraph
  if [fields][service] == "ugc-subgraph" {
    json {
      source => "message"
    }
    
    # Extract correlation ID
    if [correlation_id] {
      mutate {
        add_field => { "[@metadata][correlation_id]" => "%{correlation_id}" }
      }
    }
    
    # Parse business events
    if [event_type] {
      mutate {
        add_tag => [ "business_event" ]
        add_field => { "[@metadata][event_type]" => "%{event_type}" }
      }
    }
    
    # Parse error events
    if [level] == "ERROR" {
      mutate {
        add_tag => [ "error_event" ]
      }
      
      # Extract error details
      if [error] {
        mutate {
          add_field => { "error_message" => "%{error}" }
        }
      }
    }
    
    # Add timestamp
    date {
      match => [ "timestamp", "ISO8601" ]
      target => "@timestamp"
    }
  }
}

output {
  elasticsearch {
    hosts => ["elasticsearch:9200"]
    index => "ugc-logs-%{+YYYY.MM.dd}"
    
    # Template для оптимизации поиска
    template_name => "ugc-logs"
    template => "/usr/share/logstash/templates/ugc-logs-template.json"
    template_overwrite => true
  }
  
  # Debug output
  if "debug" in [tags] {
    stdout {
      codec => rubydebug
    }
  }
}
```

### Alerting Infrastructure - Инфраструктура алертинга

#### AlertManager - Управление уведомлениями
```yaml
# alertmanager.yml
global:
  smtp_smarthost: 'smtp.auto.ru:587'
  smtp_from: 'alerts@auto.ru'
  slack_api_url: '${SLACK_WEBHOOK_URL}'

route:
  group_by: ['alertname', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 1h
  receiver: 'web.hook'
  routes:
  # Critical alerts go to PagerDuty
  - match:
      severity: critical
    receiver: 'pagerduty'
    group_wait: 0s
    repeat_interval: 5m
    
  # Business alerts go to product team
  - match:
      team: product
    receiver: 'product-team-slack'
    
  # Backend alerts go to backend team
  - match:
      team: backend
    receiver: 'backend-team-slack'

receivers:
- name: 'web.hook'
  webhook_configs:
  - url: 'http://webhook-service:5001/alerts'

- name: 'pagerduty'
  pagerduty_configs:
  - service_key: '${PAGERDUTY_SERVICE_KEY}'
    description: 'Critical alert: {{ .GroupLabels.alertname }}'
    details:
      firing: '{{ .Alerts.Firing | len }}'
      resolved: '{{ .Alerts.Resolved | len }}'
      
- name: 'backend-team-slack'
  slack_configs:
  - channel: '#backend-alerts'
    title: 'Backend Alert: {{ .GroupLabels.alertname }}'
    text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
    
- name: 'product-team-slack'
  slack_configs:
  - channel: '#product-metrics'
    title: 'Business Alert: {{ .GroupLabels.alertname }}'
    text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

inhibit_rules:
- source_match:
    severity: 'critical'
  target_match:
    severity: 'warning'
  equal: ['alertname', 'service']
```

## 🚀 Практическое применение

### Полный пример использования наблюдаемой системы
```rust
// Пример GraphQL мутации с полной телеметрией
impl Mutation {
    #[tracing::instrument(
        skip(self, ctx),
        fields(
            correlation_id = %correlation_id,
            user_id = %input.author_id,
            offer_id = %input.offer_id
        )
    )]
    async fn create_review_with_full_telemetry(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> FieldResult<Review> {
        let metrics = ctx.data::<Arc<Metrics>>()?;
        let correlation_id = ctx.data::<CorrelationId>()?;
        let business_metrics = ctx.data::<Arc<BusinessMetricsService>>()?;
        
        // 1. Metrics: Request tracking
        let _timer = MetricsTimer::new(metrics.graphql_request_duration.clone());
        metrics.graphql_requests_total.inc();
        
        // 2. Tracing: Business context
        tracing::info!(
            event_type = "review_creation_started",
            user_id = %input.author_id,
            offer_id = %input.offer_id,
            rating = input.rating,
            "Starting review creation process"
        );
        
        // 3. Validation с метриками
        if let Err(validation_error) = self.validate_review_input(&input).await {
            metrics.graphql_errors_total
                .with_label_values(&["validation_error"])
                .inc();
            
            SecurityEventLogger::suspicious_activity(
                Some(input.author_id),
                "invalid_review_input",
                &validation_error.to_string()
            );
            
            return Err(validation_error.into());
        }
        
        // 4. External service calls с трассировкой
        let user_span = trace_span!("fetch_user_data", user_id = %input.author_id);
        let user = async {
            self.external_service_client
                .get_user_with_telemetry(input.author_id)
                .await
        }.instrument(user_span).await?;
        
        let offer_span = trace_span!("fetch_offer_data", offer_id = %input.offer_id);
        let offer = async {
            self.external_service_client
                .get_offer_with_telemetry(input.offer_id)
                .await
        }.instrument(offer_span).await?;
        
        // 5. Database operation с мониторингом
        let db_span = trace_span!("create_review_db", 
            table = "reviews",
            operation = "insert"
        );
        
        let review = async {
            let db_timer = MetricsTimer::new(metrics.db_query_duration.clone());
            metrics.db_queries_total.inc();
            
            match self.review_service.create_review(input, &user, &offer).await {
                Ok(review) => {
                    metrics.db_queries_successful.inc();
                    Ok(review)
                }
                Err(e) => {
                    metrics.db_errors_total.inc();
                    Err(e)
                }
            }
        }.instrument(db_span).await?;
        
        // 6. Business metrics update
        metrics.reviews_created_total.inc();
        business_metrics.update_review_metrics(&review).await;
        
        // 7. Business event logging
        BusinessEventLogger::review_created(
            review.id,
            review.offer_id, 
            review.author_id,
            review.rating
        );
        
        // 8. Success metrics
        metrics.graphql_requests_successful.inc();
        
        tracing::info!(
            event_type = "review_creation_completed",
            review_id = %review.id,
            processing_time_ms = _timer.elapsed().as_millis(),
            "Review creation completed successfully"
        );
        
        Ok(review)
    }
}
```

Эта Context диаграмма демонстрирует комплексную enterprise-grade систему наблюдаемости, которая превращает "черный ящик" в полностью прозрачную систему с тремя столпами наблюдаемости (Metrics, Logs, Traces), обеспечивая полную видимость производительности, поведения и бизнес-метрик федеративной GraphQL платформы Auto.ru.