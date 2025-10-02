# Task 13: Container Level Architecture Explanation
## Реализация стратегии миграции - Контейнерная диаграмма

### Обзор контейнерной архитектуры

Контейнерная диаграмма Task 13 детализирует внутреннюю структуру системы миграции, показывая конкретные контейнеры (сервисы, базы данных, инструменты) и их взаимодействие. Каждый контейнер имеет четко определенную ответственность в процессе миграции.

### Migration System Containers

#### 1. REST Adapter Container
**Технология:** Rust/Axum
**Назначение:** Обеспечение backward compatibility для REST клиентов

**Ключевые функции:**
```rust
pub struct RestAdapter {
    schema: Schema,
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
}

impl RestAdapter {
    // Legacy REST endpoints
    async fn get_reviews() -> Result<Json<RestResponse<Vec<ReviewResponse>>>>
    async fn create_review() -> Result<Json<RestResponse<ReviewResponse>>>
    async fn get_review() -> Result<Json<RestResponse<ReviewResponse>>>
    async fn update_review() -> Result<Json<RestResponse<ReviewResponse>>>
    async fn delete_review() -> Result<Json<RestResponse<()>>>
}
```

**Supported Endpoints:**
- `GET /api/v1/reviews` - List reviews with filtering
- `POST /api/v1/reviews` - Create new review
- `GET /api/v1/reviews/:id` - Get specific review
- `PUT /api/v1/reviews/:id` - Update review
- `DELETE /api/v1/reviews/:id` - Delete review
- `GET /api/v1/offers/:id/reviews` - Get reviews for offer
- `GET /api/v1/users/:id/reviews` - Get user's reviews

**Response Format Compatibility:**
```json
{
  "success": true,
  "data": {
    "id": "review-123",
    "offer_id": "offer-456",
    "author_id": "user-789",
    "rating": 5,
    "text": "Excellent car!",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:00Z",
    "is_moderated": true
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### 2. Traffic Router Container
**Технология:** Rust
**Назначение:** Intelligent request routing на основе feature flags

**Routing Logic:**
```rust
pub struct TrafficRouter {
    feature_flags: Arc<FeatureFlagService>,
    graphql_client: GraphQLClient,
    rest_client: RestClient,
    metrics: Arc<MigrationMetrics>,
}

impl TrafficRouter {
    pub async fn route_request(&self, request: Request) -> Response {
        let user_id = self.extract_user_id(&request)?;
        
        // Check feature flag
        if self.should_use_graphql(&user_id).await {
            self.metrics.graphql_requests.inc();
            self.route_to_graphql(request).await
        } else {
            self.metrics.rest_requests.inc();
            self.route_to_rest(request).await
        }
    }
    
    async fn should_use_graphql(&self, user_id: &str) -> bool {
        self.feature_flags
            .is_enabled("graphql_reviews_read", user_id)
            .await
    }
}
```

**Routing Strategies:**
- **User-based routing:** На основе user ID и feature flags
- **Percentage rollout:** Gradual increase трафика к GraphQL
- **Conditional routing:** Complex conditions для targeting
- **Emergency override:** Instant fallback к REST

#### 3. Feature Flag Service Container
**Технология:** Rust
**Назначение:** Управление feature flags и rollout logic

**Core Functionality:**
```rust
pub struct FeatureFlagService {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    redis_client: Option<redis::Client>,
}

impl FeatureFlagService {
    pub async fn is_enabled(&self, flag_name: &str, user_id: &str) -> bool {
        // 1. Check Redis cache
        if let Some(cached) = self.check_cache(flag_name, user_id).await {
            return cached;
        }
        
        // 2. Evaluate flag
        let result = self.evaluate_flag(flag_name, user_id).await;
        
        // 3. Cache result
        self.cache_result(flag_name, user_id, result).await;
        
        result
    }
    
    async fn evaluate_flag(&self, flag_name: &str, user_id: &str) -> bool {
        let flags = self.flags.read().await;
        if let Some(flag) = flags.get(flag_name) {
            self.evaluate_flag_conditions(flag, user_id).await
        } else {
            false
        }
    }
}
```

**Flag Evaluation Logic:**
```rust
async fn evaluate_flag_conditions(&self, flag: &FeatureFlag, user_id: &str) -> bool {
    // 1. Check if globally enabled
    if !flag.enabled {
        return false;
    }
    
    // 2. Check blacklist
    if flag.user_blacklist.contains(&user_id.to_string()) {
        return false;
    }
    
    // 3. Check whitelist
    if flag.user_whitelist.contains(&user_id.to_string()) {
        return true;
    }
    
    // 4. Check rollout percentage using consistent hashing
    let user_hash = self.hash_user_id(user_id);
    let user_percentage = (user_hash % 100) as f64;
    
    user_percentage < flag.rollout_percentage
}
```

#### 4. A/B Test Service Container
**Технология:** Rust
**Назначение:** A/B testing и experiment management

**Experiment Management:**
```rust
pub struct ABTestService {
    feature_flags: Arc<FeatureFlagService>,
    experiments: Arc<RwLock<HashMap<String, Experiment>>>,
}

impl ABTestService {
    pub async fn assign_variant(&self, experiment_name: &str, user_id: &str) -> Variant {
        // Use consistent hashing for stable assignment
        let user_hash = self.hash_user_id(user_id);
        let experiment = self.get_experiment(experiment_name).await?;
        
        match user_hash % 100 {
            0..=49 => Variant::Control,
            50..=99 => Variant::Treatment,
        }
    }
    
    pub async fn track_conversion(&self, experiment_name: &str, user_id: &str, event: &str) {
        let variant = self.assign_variant(experiment_name, user_id).await;
        
        // Record conversion event
        self.metrics.conversions
            .with_label_values(&[experiment_name, &format!("{:?}", variant), event])
            .inc();
    }
}
```

**Experiment Configuration:**
```yaml
experiments:
  graphql_migration_test:
    name: "GraphQL vs REST Performance"
    traffic_percentage: 50.0
    variants:
      control: "REST API"
      treatment: "GraphQL API"
    success_metrics:
      - response_time
      - error_rate
      - user_satisfaction
```

#### 5. Canary Service Container
**Технология:** Rust
**Назначение:** Automated canary deployments и rollback management

**Canary Management:**
```rust
pub struct CanaryService {
    feature_flags: Arc<FeatureFlagService>,
    health_checker: Arc<HealthChecker>,
    metrics: Arc<MigrationMetrics>,
}

impl CanaryService {
    pub async fn start_canary(&self, flag_name: &str) -> Result<(), CanaryError> {
        let config = self.get_canary_config(flag_name)?;
        
        // Start with initial percentage
        self.feature_flags
            .set_rollout_percentage(flag_name, config.initial_percentage)
            .await?;
        
        info!("Canary started for {} at {}%", flag_name, config.initial_percentage);
        Ok(())
    }
    
    pub async fn promote_canary(&self, flag_name: &str) -> Result<(), CanaryError> {
        let current_percentage = self.get_current_percentage(flag_name).await?;
        let next_step = self.get_next_promotion_step(current_percentage)?;
        
        // Check health before promotion
        if self.check_promotion_criteria(flag_name).await? {
            self.feature_flags
                .set_rollout_percentage(flag_name, next_step)
                .await?;
            
            info!("Canary promoted to {}%", next_step);
        } else {
            warn!("Promotion criteria not met, pausing canary");
        }
        
        Ok(())
    }
    
    async fn check_promotion_criteria(&self, flag_name: &str) -> Result<bool, CanaryError> {
        let config = self.get_canary_config(flag_name)?;
        
        // Check error rate
        let error_rate = self.metrics.get_error_rate(flag_name).await?;
        if error_rate > config.max_error_rate {
            return Ok(false);
        }
        
        // Check response time
        let p95_latency = self.metrics.get_p95_latency(flag_name).await?;
        if p95_latency > config.max_response_time {
            return Ok(false);
        }
        
        Ok(true)
    }
}
```

#### 6. Migration Management API Container
**Технология:** Rust/Axum
**Назначение:** REST API для управления миграцией

**API Endpoints:**
```rust
pub fn migration_routes() -> Router {
    Router::new()
        // Feature flag management
        .route("/api/migration/flags", get(list_flags))
        .route("/api/migration/flags/:name", get(get_flag))
        .route("/api/migration/flags/:name/enable", post(enable_flag))
        .route("/api/migration/flags/:name/disable", post(disable_flag))
        .route("/api/migration/flags/:name/rollout", put(set_rollout))
        
        // Canary management
        .route("/api/migration/canary/:name/start", post(start_canary))
        .route("/api/migration/canary/:name/promote", post(promote_canary))
        .route("/api/migration/canary/:name/rollback", post(rollback_canary))
        
        // A/B testing
        .route("/api/migration/experiments", get(list_experiments))
        .route("/api/migration/experiments/:name/assign/:user", get(assign_variant))
        
        // Status and metrics
        .route("/api/migration/status", get(get_migration_status))
        .route("/api/migration/metrics", get(get_migration_metrics))
        
        // Emergency procedures
        .route("/api/migration/emergency/rollback", post(emergency_rollback))
}
```

**API Response Examples:**
```json
// GET /api/migration/flags
{
  "flags": [
    {
      "name": "graphql_reviews_read",
      "enabled": true,
      "rollout_percentage": 25.0,
      "description": "Enable GraphQL for reading reviews",
      "user_whitelist": ["user-123"],
      "user_blacklist": []
    }
  ]
}

// GET /api/migration/status
{
  "migration_progress": {
    "phase": "read_operations",
    "completion_percentage": 25.0,
    "active_flags": ["graphql_reviews_read"],
    "traffic_distribution": {
      "graphql": 25.0,
      "rest": 75.0
    }
  },
  "health": {
    "status": "healthy",
    "error_rate": 0.02,
    "avg_response_time": 150
  }
}
```

#### 7. Migration CLI Container
**Технология:** Rust/Clap
**Назначение:** Command-line interface для migration management

**CLI Commands:**
```rust
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
    
    /// Get migration status
    Status,
    
    /// Create A/B test
    CreateAbTest {
        test_name: String,
        description: String,
        #[arg(default_value = "50.0")]
        traffic_percentage: f64,
    },
}
```

**Usage Examples:**
```bash
# List all flags
cargo run --bin migration-cli list

# Enable GraphQL for reads
cargo run --bin migration-cli enable graphql_reviews_read

# Set 25% rollout
cargo run --bin migration-cli rollout graphql_reviews_read 25.0

# Start canary deployment
cargo run --bin migration-cli start-canary graphql_reviews_read

# Check migration status
cargo run --bin migration-cli status

# Emergency rollback
cargo run --bin migration-cli emergency-rollback "High error rate detected"
```

#### 8. Migration Monitoring Container
**Технология:** Rust
**Назначение:** Сбор и отправка migration metrics

**Metrics Collection:**
```rust
pub struct MigrationMetrics {
    // Request metrics
    pub rest_requests: Counter,
    pub graphql_requests: Counter,
    pub request_duration: HistogramVec,
    
    // Migration metrics
    pub migration_progress: GaugeVec,
    pub flag_evaluations: CounterVec,
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    
    // Business metrics
    pub user_adoption: CounterVec,
    pub conversion_rate: GaugeVec,
    
    // Error metrics
    pub error_rate: CounterVec,
    pub rollback_events: CounterVec,
}

impl MigrationMetrics {
    pub fn record_request(&self, backend: &str, endpoint: &str, duration: f64) {
        match backend {
            "graphql" => self.graphql_requests.inc(),
            "rest" => self.rest_requests.inc(),
            _ => {}
        }
        
        self.request_duration
            .with_label_values(&[backend, endpoint])
            .observe(duration);
    }
    
    pub fn record_flag_evaluation(&self, flag_name: &str, result: bool, cached: bool) {
        self.flag_evaluations
            .with_label_values(&[flag_name, &result.to_string()])
            .inc();
            
        if cached {
            self.cache_hits.inc();
        } else {
            self.cache_misses.inc();
        }
    }
}
```

### Data Storage Containers

#### 1. Redis Cache Container
**Технология:** Redis
**Назначение:** Feature flag caching и session storage

**Cache Structure:**
```redis
# Feature flag cache
feature_flag:graphql_reviews_read:user-123 -> "true" (TTL: 300s)
feature_flag:graphql_reviews_write:user-456 -> "false" (TTL: 300s)

# A/B test assignments
ab_test:graphql_migration_test:user-123 -> "treatment"
ab_test:graphql_migration_test:user-456 -> "control"

# Canary state
canary:graphql_reviews_read:percentage -> "25.0"
canary:graphql_reviews_read:last_promotion -> "2024-01-15T10:30:00Z"

# Migration metrics cache
migration:stats:error_rate -> "0.02"
migration:stats:avg_response_time -> "150"
```

**Cache Operations:**
```rust
impl FeatureFlagService {
    async fn cache_flag_result(&self, flag_name: &str, user_id: &str, result: bool) {
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await?;
            let cache_key = format!("feature_flag:{}:{}", flag_name, user_id);
            
            redis::cmd("SETEX")
                .arg(&cache_key)
                .arg(300) // 5 minutes TTL
                .arg(result.to_string())
                .query_async::<_, ()>(&mut conn)
                .await?;
        }
    }
}
```

#### 2. Configuration Store Container
**Технология:** YAML Files
**Назначение:** Feature flag и migration configuration

**Configuration Structure:**
```yaml
# feature-flags.yaml
feature_flags:
  graphql_reviews_read:
    enabled: true
    rollout_percentage: 25.0
    description: "Enable GraphQL for reading reviews"
    user_whitelist: ["user-123"]
    user_blacklist: []
    conditions:
      - type: "time_window"
        start: "2024-01-01T00:00:00Z"
        end: "2024-12-31T23:59:59Z"

canary_deployments:
  graphql_reviews_read:
    initial_percentage: 1.0
    promotion_steps: [1, 5, 10, 25, 50, 75, 100]
    step_duration_minutes: 60
    success_criteria:
      max_error_rate: 0.05
      max_response_time_p95: 500
    rollback_criteria:
      max_error_rate: 0.1
      max_response_time_p95: 1000

ab_tests:
  graphql_migration_test:
    name: "GraphQL Migration Effectiveness"
    variants:
      control: { name: "REST API", traffic_percentage: 50.0 }
      treatment: { name: "GraphQL API", traffic_percentage: 50.0 }
```

### External System Integration

#### 1. UGC GraphQL Service Integration
**Connection Pattern:**
```rust
pub struct GraphQLClient {
    client: reqwest::Client,
    endpoint: String,
}

impl GraphQLClient {
    pub async fn execute_query(&self, query: &str, variables: Value) -> Result<Value> {
        let request_body = json!({
            "query": query,
            "variables": variables
        });
        
        let response = self.client
            .post(&self.endpoint)
            .json(&request_body)
            .send()
            .await?;
            
        let result: Value = response.json().await?;
        Ok(result)
    }
}
```

#### 2. Legacy REST Service Integration
**Connection Pattern:**
```rust
pub struct RestClient {
    client: reqwest::Client,
    base_url: String,
}

impl RestClient {
    pub async fn get_reviews(&self, params: &ReviewsParams) -> Result<Vec<Review>> {
        let url = format!("{}/api/v1/reviews", self.base_url);
        let response = self.client
            .get(&url)
            .query(params)
            .send()
            .await?;
            
        let reviews: Vec<Review> = response.json().await?;
        Ok(reviews)
    }
}
```

#### 3. Prometheus Integration
**Metrics Export:**
```rust
use prometheus::{Encoder, TextEncoder, gather};

pub async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = gather();
    let mut buffer = Vec::new();
    
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    Response::builder()
        .header("content-type", "text/plain; version=0.0.4")
        .body(buffer.into())
        .unwrap()
}
```

### Container Communication Patterns

#### 1. Synchronous Communication
```rust
// REST Adapter → Traffic Router
let routing_decision = self.traffic_router
    .should_use_graphql(&user_id)
    .await?;

// Traffic Router → Feature Flag Service
let flag_enabled = self.feature_flags
    .is_enabled("graphql_reviews_read", &user_id)
    .await;
```

#### 2. Asynchronous Communication
```rust
// Migration Monitoring → Prometheus
tokio::spawn(async move {
    loop {
        let metrics = collect_migration_metrics().await;
        send_to_prometheus(metrics).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
});
```

#### 3. Event-Driven Communication
```rust
// Canary Service → Feature Flag Service (rollback event)
pub async fn handle_rollback_event(&self, event: RollbackEvent) {
    match event.trigger {
        RollbackTrigger::HighErrorRate => {
            self.emergency_rollback(&event.flag_name).await?;
        }
        RollbackTrigger::PerformanceDegradation => {
            self.gradual_rollback(&event.flag_name, 0.0).await?;
        }
    }
}
```

### Заключение

Контейнерная архитектура Task 13 обеспечивает:

- **Modular Design:** Четкое разделение ответственности между контейнерами
- **Scalability:** Независимое масштабирование каждого компонента
- **Maintainability:** Простота обновления и модификации отдельных сервисов
- **Observability:** Comprehensive monitoring всех компонентов
- **Reliability:** Fault tolerance и graceful degradation
- **Performance:** Optimized communication patterns и caching strategies

Каждый контейнер имеет четко определенную роль в процессе миграции, что обеспечивает надежную и контролируемую миграцию от REST к GraphQL.