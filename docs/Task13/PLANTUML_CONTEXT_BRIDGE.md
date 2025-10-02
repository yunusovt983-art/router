# Task 13: Context Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_CONTEXT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Контекстная диаграмма Task 13 показывает высокоуровневые взаимодействия в системе миграции от REST к GraphQL. Каждый элемент диаграммы имеет прямое отражение в коде и конфигурационных файлах системы миграции.

### Архитектурные элементы и их реализация в коде

#### 1. Legacy REST Client
**PlantUML элемент:**
```plantuml
Person(legacy_client, "Legacy REST Client", "Existing applications using REST API")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/rest_adapter.rs
impl RestAdapter {
    // Обработка legacy REST запросов
    async fn get_reviews(
        State(adapter): State<RestAdapter>,
        Query(params): Query<ReviewsQueryParams>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        adapter.metrics.rest_request_total
            .with_label_values(&["GET", "/api/v1/reviews"])
            .inc();
        
        let user_context = extract_user_context(&headers)?;
        
        // Проверка feature flag для миграции
        if adapter.feature_flags
            .is_enabled("graphql_reviews_read", &user_context.user_id.to_string())
            .await 
        {
            // Маршрутизация к GraphQL
            Self::get_reviews_via_graphql(adapter, params, user_context).await
        } else {
            // Fallback к legacy REST
            Self::get_reviews_legacy(adapter, params, user_context).await
        }
    }
}
```

**HTTP запросы от legacy клиентов:**
```http
GET /api/v1/reviews?limit=10&offer_id=123 HTTP/1.1
Host: api.auto.ru
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
Content-Type: application/json
```

**Ожидаемый response format:**
```json
{
  "success": true,
  "data": [
    {
      "id": "review-123",
      "offer_id": "offer-456",
      "author_id": "user-789",
      "rating": 5,
      "text": "Excellent car!",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### 2. GraphQL Client
**PlantUML элемент:**
```plantuml
Person(new_client, "GraphQL Client", "New applications using GraphQL API")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/traffic_router.rs
impl TrafficRouter {
    pub async fn handle_graphql_request(&self, request: GraphQLRequest) -> GraphQLResponse {
        // Прямая маршрутизация к GraphQL сервису
        self.metrics.graphql_requests
            .with_label_values(&["direct"])
            .inc();
            
        self.graphql_client.execute(request).await
    }
}
```

**GraphQL запросы:**
```graphql
query GetReviews($first: Int, $offerId: ID) {
  reviews(first: $first, offerId: $offerId) {
    edges {
      node {
        id
        rating
        text
        createdAt
        author {
          id
          name
        }
        offer {
          id
          title
        }
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
```

#### 3. Developer
**PlantUML элемент:**
```plantuml
Person(developer, "Developer", "Manages migration process and feature flags")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/bin/migration-cli.rs
#[derive(Parser)]
#[command(name = "migration-cli")]
struct Cli {
    #[arg(short, long, default_value = "http://localhost:4001")]
    base_url: String,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all feature flags
    List,
    
    /// Enable a feature flag
    Enable { flag_name: String },
    
    /// Set rollout percentage
    Rollout { flag_name: String, percentage: f64 },
    
    /// Start canary deployment
    StartCanary { flag_name: String },
    
    /// Emergency rollback
    EmergencyRollback { reason: String },
}
```

**CLI команды разработчика:**
```bash
# Просмотр всех feature flags
cargo run --bin migration-cli list

# Включение GraphQL для чтения
cargo run --bin migration-cli enable graphql_reviews_read

# Установка rollout percentage
cargo run --bin migration-cli rollout graphql_reviews_read 25.0

# Запуск canary deployment
cargo run --bin migration-cli start-canary graphql_reviews_read

# Emergency rollback
cargo run --bin migration-cli emergency-rollback "High error rate detected"
```

#### 4. Operations Engineer
**PlantUML элемент:**
```plantuml
Person(ops_engineer, "Operations Engineer", "Monitors migration progress and handles incidents")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/monitoring.rs
impl MigrationMetrics {
    pub fn new() -> Self {
        Self {
            migration_progress: register_gauge_vec!(
                "migration_progress_percentage",
                "Migration progress by phase",
                &["phase"]
            ).unwrap(),
            
            error_rate: register_counter_vec!(
                "migration_error_rate_total",
                "Migration error rate by backend",
                &["backend", "error_type"]
            ).unwrap(),
            
            request_duration: register_histogram_vec!(
                "migration_request_duration_seconds",
                "Request duration by backend",
                &["backend", "endpoint"]
            ).unwrap(),
        }
    }
    
    pub async fn get_migration_status(&self) -> MigrationStatus {
        MigrationStatus {
            phase: self.get_current_phase().await,
            completion_percentage: self.get_completion_percentage().await,
            error_rate: self.get_current_error_rate().await,
            avg_response_time: self.get_avg_response_time().await,
        }
    }
}
```

**Monitoring dashboards и queries:**
```prometheus
# Migration progress
migration_progress_percentage{phase="read_operations"}

# Error rate
rate(migration_error_rate_total[5m])

# Response time P95
histogram_quantile(0.95, rate(migration_request_duration_seconds_bucket[5m]))

# Traffic distribution
sum(rate(migration_requests_total[5m])) by (backend)
```

### Системные компоненты и их код

#### 1. Migration Service
**PlantUML элемент:**
```plantuml
System(migration_service, "Migration Service", "Manages gradual transition from REST to GraphQL")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/mod.rs
pub struct MigrationService {
    pub rest_adapter: RestAdapter,
    pub traffic_router: TrafficRouter,
    pub feature_flags: Arc<FeatureFlagService>,
    pub monitoring: Arc<MigrationMetrics>,
}

impl MigrationService {
    pub fn new(
        schema: Schema,
        redis_client: Option<redis::Client>,
    ) -> Self {
        let feature_flags = Arc::new(
            FeatureFlagService::new()
                .with_redis(redis_client)
        );
        
        let monitoring = Arc::new(MigrationMetrics::new());
        
        let rest_adapter = RestAdapter::new(
            schema.clone(),
            feature_flags.clone(),
            monitoring.clone(),
        );
        
        let traffic_router = TrafficRouter::new(
            feature_flags.clone(),
            monitoring.clone(),
        );
        
        Self {
            rest_adapter,
            traffic_router,
            feature_flags,
            monitoring,
        }
    }
    
    pub fn router(&self) -> Router {
        Router::new()
            // REST API routes
            .nest("/api/v1", self.rest_adapter.router())
            // GraphQL route
            .route("/graphql", post(graphql_handler))
            // Migration management API
            .nest("/api/migration", self.management_api_router())
            // Metrics endpoint
            .route("/metrics", get(metrics_handler))
    }
}
```

#### 2. Feature Flag System
**PlantUML элемент:**
```plantuml
System(feature_flags, "Feature Flag System", "Controls rollout and A/B testing")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/feature_flags.rs
pub struct FeatureFlagService {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    redis_client: Option<redis::Client>,
}

impl FeatureFlagService {
    pub async fn is_enabled(&self, flag_name: &str, user_id: &str) -> bool {
        // 1. Проверка Redis cache
        if let Some(cached_result) = self.check_redis_cache(flag_name, user_id).await {
            return cached_result;
        }

        // 2. Evaluation флага
        let flags = self.flags.read().await;
        if let Some(flag) = flags.get(flag_name) {
            let result = self.evaluate_flag(flag, user_id).await;
            
            // 3. Кеширование результата
            self.cache_result_in_redis(flag_name, user_id, result).await;
            
            result
        } else {
            false
        }
    }
    
    async fn evaluate_flag(&self, flag: &FeatureFlag, user_id: &str) -> bool {
        // Проверка глобального включения
        if !flag.enabled {
            return false;
        }

        // Проверка blacklist
        if flag.user_blacklist.contains(&user_id.to_string()) {
            return false;
        }

        // Проверка whitelist
        if flag.user_whitelist.contains(&user_id.to_string()) {
            return true;
        }

        // Проверка rollout percentage через consistent hashing
        let user_hash = self.hash_user_id(user_id);
        let user_percentage = (user_hash % 100) as f64;
        
        user_percentage < flag.rollout_percentage
    }
}
```

**Configuration файл:**
```yaml
# ugc-subgraph/feature-flags.yaml
feature_flags:
  graphql_reviews_read:
    enabled: true
    rollout_percentage: 25.0
    description: "Enable GraphQL for reading reviews"
    user_whitelist: ["user-123"]
    user_blacklist: []
    conditions: []
    
  graphql_reviews_write:
    enabled: false
    rollout_percentage: 0.0
    description: "Enable GraphQL for writing reviews"
    user_whitelist: []
    user_blacklist: []
    conditions: []
```

#### 3. Migration Monitoring
**PlantUML элемент:**
```plantuml
System(monitoring, "Migration Monitoring", "Tracks migration progress and performance")
```

**Реализация в коде:**
```rust
// ugc-subgraph/src/migration/monitoring.rs
pub struct MigrationMetrics {
    // Request metrics
    pub rest_request_total: CounterVec,
    pub graphql_migration_requests: CounterVec,
    pub legacy_rest_requests: CounterVec,
    
    // Performance metrics
    pub request_duration: HistogramVec,
    pub error_rate: CounterVec,
    
    // Migration progress
    pub migration_progress: GaugeVec,
    pub flag_evaluations: CounterVec,
    
    // Business metrics
    pub user_adoption: CounterVec,
    pub conversion_rate: GaugeVec,
}

impl MigrationMetrics {
    pub fn record_request(&self, backend: &str, endpoint: &str, duration: f64) {
        // Запись метрик запроса
        match backend {
            "graphql" => {
                self.graphql_migration_requests
                    .with_label_values(&[endpoint, "success"])
                    .inc();
            }
            "rest" => {
                self.legacy_rest_requests
                    .with_label_values(&[endpoint, "success"])
                    .inc();
            }
            _ => {}
        }
        
        self.request_duration
            .with_label_values(&[backend, endpoint])
            .observe(duration);
    }
    
    pub fn record_migration_progress(&self, phase: &str, percentage: f64) {
        self.migration_progress
            .with_label_values(&[phase])
            .set(percentage);
    }
}
```

### Внешние системы и их интеграция

#### 1. UGC GraphQL API
**PlantUML связь:**
```plantuml
Rel(migration_service, ugc_graphql, "Routes to GraphQL", "GraphQL queries/mutations")
```

**Код интеграции:**
```rust
// ugc-subgraph/src/migration/traffic_router.rs
impl TrafficRouter {
    async fn route_to_graphql(&self, request: RestRequest) -> Result<RestResponse> {
        // Конвертация REST запроса в GraphQL
        let graphql_query = self.convert_rest_to_graphql(&request).await?;
        
        // Выполнение GraphQL запроса
        let graphql_response = self.graphql_client
            .execute_query(&graphql_query.query, graphql_query.variables)
            .await?;
        
        // Конвертация GraphQL ответа обратно в REST формат
        let rest_response = self.convert_graphql_to_rest(&graphql_response).await?;
        
        Ok(rest_response)
    }
    
    async fn convert_rest_to_graphql(&self, request: &RestRequest) -> Result<GraphQLQuery> {
        match request.endpoint.as_str() {
            "/api/v1/reviews" => {
                let query = r#"
                    query GetReviews($first: Int, $offerId: ID, $authorId: ID) {
                        reviews(first: $first, offerId: $offerId, authorId: $authorId) {
                            edges {
                                node {
                                    id
                                    offerId
                                    authorId
                                    rating
                                    text
                                    createdAt
                                    updatedAt
                                }
                            }
                        }
                    }
                "#;
                
                let variables = json!({
                    "first": request.params.get("limit").unwrap_or(&"20".to_string()).parse::<i32>()?,
                    "offerId": request.params.get("offer_id"),
                    "authorId": request.params.get("author_id")
                });
                
                Ok(GraphQLQuery { query: query.to_string(), variables })
            }
            _ => Err(ConversionError::UnsupportedEndpoint)
        }
    }
}
```

#### 2. Redis Cache Integration
**PlantUML связь:**
```plantuml
Rel(feature_flags, redis_cache, "Caches flags", "Redis protocol")
```

**Код интеграции:**
```rust
// ugc-subgraph/src/migration/feature_flags.rs
impl FeatureFlagService {
    async fn check_redis_cache(&self, flag_name: &str, user_id: &str) -> Option<bool> {
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_async_connection().await {
                let cache_key = format!("feature_flag:{}:{}", flag_name, user_id);
                
                if let Ok(cached_value) = redis::cmd("GET")
                    .arg(&cache_key)
                    .query_async::<_, Option<String>>(&mut conn)
                    .await
                {
                    return cached_value.and_then(|v| v.parse().ok());
                }
            }
        }
        None
    }
    
    async fn cache_result_in_redis(&self, flag_name: &str, user_id: &str, result: bool) {
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_async_connection().await {
                let cache_key = format!("feature_flag:{}:{}", flag_name, user_id);
                let _ = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(300) // 5 minutes TTL
                    .arg(result.to_string())
                    .query_async::<_, ()>(&mut conn)
                    .await;
            }
        }
    }
}
```

### Потоки взаимодействия и их реализация

#### 1. Legacy Client Request Flow
**Архитектурный поток:**
```
Legacy Client → Migration Service → Feature Flag Check → Route Decision → Backend
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/rest_adapter.rs
async fn handle_legacy_request(
    State(adapter): State<RestAdapter>,
    request: Request,
) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    
    // 1. Извлечение user context
    let user_context = extract_user_context(&request.headers())?;
    
    // 2. Проверка feature flag
    let use_graphql = adapter.feature_flags
        .is_enabled("graphql_reviews_read", &user_context.user_id.to_string())
        .await;
    
    // 3. Routing decision
    let response = if use_graphql {
        adapter.metrics.graphql_migration_requests
            .with_label_values(&["reviews", "read"])
            .inc();
        
        adapter.route_to_graphql(request).await?
    } else {
        adapter.metrics.legacy_rest_requests
            .with_label_values(&["reviews", "read"])
            .inc();
        
        adapter.route_to_legacy_rest(request).await?
    };
    
    // 4. Запись метрик
    let duration = start_time.elapsed().as_secs_f64();
    let backend = if use_graphql { "graphql" } else { "rest" };
    adapter.metrics.record_request(backend, "/api/v1/reviews", duration);
    
    Ok(response)
}
```

#### 2. Feature Flag Management Flow
**Архитектурный поток:**
```
Developer → CLI → Management API → Feature Flag Service → Redis Cache
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/management_api.rs
pub async fn set_rollout_percentage(
    Path(flag_name): Path<String>,
    Json(payload): Json<RolloutRequest>,
    State(service): State<Arc<FeatureFlagService>>,
) -> Result<Json<StatusResponse>, StatusCode> {
    // Валидация percentage
    if payload.percentage < 0.0 || payload.percentage > 100.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Обновление флага
    match service.set_rollout_percentage(&flag_name, payload.percentage).await {
        Ok(()) => {
            info!("Rollout percentage updated: {} -> {}%", flag_name, payload.percentage);
            
            Ok(Json(StatusResponse {
                success: true,
                message: format!("Rollout percentage set to {}%", payload.percentage),
                timestamp: chrono::Utc::now().to_rfc3339(),
            }))
        }
        Err(e) => {
            error!("Failed to update rollout percentage: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// CLI integration
// ugc-subgraph/src/bin/migration-cli.rs
async fn set_rollout(client: &Client, base_url: &str, flag_name: &str, percentage: f64) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .put(&format!("{}/api/migration/flags/{}/rollout", base_url, flag_name))
        .json(&json!({ "percentage": percentage }))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Rollout percentage updated"));
    } else {
        println!("✗ Failed to update rollout percentage");
    }

    Ok(())
}
```

### Configuration Management

#### Environment Variables
```bash
# ugc-subgraph/.env
MIGRATION_CONFIG_PATH=feature-flags.yaml
REDIS_URL=redis://localhost:6379
REDIS_ENABLED=true

# Feature flag overrides
FF_GRAPHQL_REVIEWS_READ_ENABLED=true
FF_GRAPHQL_REVIEWS_READ_ROLLOUT=25.0
FF_GRAPHQL_REVIEWS_WRITE_ENABLED=false

# Monitoring
PROMETHEUS_ENDPOINT=http://prometheus:9090
JAEGER_ENDPOINT=http://jaeger:14268/api/traces
```

#### Docker Compose Integration
```yaml
# docker-compose.yml
services:
  ugc-subgraph:
    environment:
      - MIGRATION_CONFIG_PATH=/app/feature-flags.yaml
      - REDIS_URL=redis://redis:6379
      - FF_GRAPHQL_REVIEWS_READ_ENABLED=${FF_GRAPHQL_REVIEWS_READ_ENABLED:-false}
    volumes:
      - ./ugc-subgraph/feature-flags.yaml:/app/feature-flags.yaml:ro
    depends_on:
      - redis
      
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

### Заключение

Контекстная диаграмма Task 13 служит мостом между архитектурным дизайном и конкретной реализацией кода:

1. **Участники системы** имеют прямое отражение в API endpoints и CLI commands
2. **Системные компоненты** реализованы как Rust сервисы с четкими интерфейсами
3. **Внешние интеграции** имеют конкретные клиенты и connection pools
4. **Потоки данных** трассируются через middleware и metrics
5. **Configuration** управляется через YAML файлы и environment variables

Каждый элемент диаграммы имеет конкретную реализацию в коде, что обеспечивает полную трассируемость от архитектурного дизайна до работающей системы миграции.