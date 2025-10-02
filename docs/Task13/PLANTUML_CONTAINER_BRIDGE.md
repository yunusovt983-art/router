# Task 13: Container Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_CONTAINER.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Контейнерная диаграмма Task 13 детализирует внутреннюю структуру системы миграции, показывая конкретные контейнеры и их взаимодействие. Каждый контейнер имеет прямое отражение в коде и конфигурационных файлах.

### Migration System Containers - Код реализация

#### 1. REST Adapter Container
**PlantUML элемент:**
```plantuml
Container(rest_adapter, "REST Adapter", "Rust/Axum", "Provides REST API compatibility with GraphQL backend")
```

**Файловая структура:**
```
ugc-subgraph/src/migration/
├── rest_adapter.rs          # ← Этот контейнер
├── mod.rs
└── ...
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/rest_adapter.rs
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post, put, delete},
    Router,
};

#[derive(Clone)]
pub struct RestAdapter {
    schema: Schema,
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
}

impl RestAdapter {
    pub fn new(
        schema: Schema,
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
    ) -> Self {
        Self { schema, feature_flags, metrics }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/reviews", get(Self::get_reviews).post(Self::create_review))
            .route("/reviews/:id", get(Self::get_review).put(Self::update_review).delete(Self::delete_review))
            .route("/offers/:offer_id/reviews", get(Self::get_offer_reviews))
            .with_state(self.clone())
    }
}
```**REST
 Endpoints Implementation:**
```rust
async fn get_reviews(
    State(adapter): State<RestAdapter>,
    Query(params): Query<ReviewsQueryParams>,
    headers: HeaderMap,
) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
    adapter.metrics.rest_request_total
        .with_label_values(&["GET", "/api/v1/reviews"])
        .inc();
    
    let user_context = extract_user_context(&headers)?;
    
    // Feature flag check - мост к Traffic Router
    if adapter.feature_flags
        .is_enabled("graphql_reviews_read", &user_context.user_id.to_string())
        .await 
    {
        Self::get_reviews_via_graphql(adapter, params, user_context).await
    } else {
        Self::get_reviews_legacy(adapter, params, user_context).await
    }
}
```

#### 2. Traffic Router Container
**PlantUML элемент:**
```plantuml
Container(traffic_router, "Traffic Router", "Rust", "Routes requests based on feature flags")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/traffic_router.rs
pub struct TrafficRouter {
    feature_flags: Arc<FeatureFlagService>,
    graphql_client: GraphQLClient,
    rest_client: RestClient,
    metrics: Arc<MigrationMetrics>,
}

impl TrafficRouter {
    pub async fn route_request(&self, request: Request) -> Response {
        let user_id = self.extract_user_id(&request)?;
        
        // Routing decision based on feature flags
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

#### 3. Feature Flag Service Container
**PlantUML элемент:**
```plantuml
Container(feature_flag_service, "Feature Flag Service", "Rust", "Manages feature flags and rollout logic")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/feature_flags.rs
pub struct FeatureFlagService {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    redis_client: Option<redis::Client>,
}

impl FeatureFlagService {
    pub async fn is_enabled(&self, flag_name: &str, user_id: &str) -> bool {
        // 1. Check Redis cache first
        if let Some(cached_result) = self.check_redis_cache(flag_name, user_id).await {
            return cached_result;
        }

        // 2. Evaluate flag
        let flags = self.flags.read().await;
        if let Some(flag) = flags.get(flag_name) {
            let result = self.evaluate_flag(flag, user_id).await;
            
            // 3. Cache result
            self.cache_result_in_redis(flag_name, user_id, result).await;
            result
        } else {
            false
        }
    }
}
```

#### 4. Migration CLI Container
**PlantUML элемент:**
```plantuml
Container(migration_cli, "Migration CLI", "Rust/Clap", "Command-line tool for migration control")
```

**Код реализации:**
```rust
// ugc-subgraph/src/bin/migration-cli.rs
use clap::{Parser, Subcommand};

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
    List,
    Enable { flag_name: String },
    Rollout { flag_name: String, percentage: f64 },
    StartCanary { flag_name: String },
    EmergencyRollback { reason: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Enable { flag_name } => {
            enable_flag(&client, &cli.base_url, &flag_name).await
        }
        Commands::Rollout { flag_name, percentage } => {
            set_rollout(&client, &cli.base_url, &flag_name, percentage).await
        }
        // ... other commands
    }
}
```

### Data Storage Containers

#### 1. Redis Cache Container
**PlantUML элемент:**
```plantuml
ContainerDb(redis, "Redis Cache", "Redis", "Feature flag cache and session storage")
```

**Integration код:**
```rust
// Redis integration в Feature Flag Service
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
```

**Docker Compose конфигурация:**
```yaml
# docker-compose.yml
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes --maxmemory 256mb
```

#### 2. Configuration Store Container
**PlantUML элемент:**
```plantuml
ContainerDb(config_store, "Configuration Store", "YAML Files", "Feature flag and migration configuration")
```

**Configuration файлы:**
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

ab_tests:
  graphql_migration_test:
    name: "GraphQL Migration Effectiveness"
    variants:
      control: { name: "REST API", traffic_percentage: 50.0 }
      treatment: { name: "GraphQL API", traffic_percentage: 50.0 }
```

**Config loader код:**
```rust
// ugc-subgraph/src/migration/config_loader.rs
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct MigrationConfig {
    pub feature_flags: HashMap<String, FeatureFlagConfig>,
    pub ab_tests: HashMap<String, ABTestConfig>,
    pub canary_deployments: HashMap<String, CanaryConfig>,
}

impl MigrationConfig {
    pub fn load_from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: MigrationConfig = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
}
```

### Container Relationships - Network Implementation

#### 1. REST Adapter → Traffic Router
**PlantUML связь:**
```plantuml
Rel(rest_adapter, traffic_router, "Routes requests", "Internal calls")
```

**Код реализации:**
```rust
// В REST Adapter
impl RestAdapter {
    async fn handle_request(&self, request: Request) -> Response {
        // Делегирование к Traffic Router
        self.traffic_router.route_request(request).await
    }
}
```

#### 2. Feature Flag Service → Redis
**PlantUML связь:**
```plantuml
Rel(feature_flag_service, redis, "Cache flags", "Redis protocol")
```

**Код реализации:**
```rust
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
```

#### 3. Migration CLI → Management API
**PlantUML связь:**
```plantuml
Rel(migration_cli, migration_api, "API calls", "HTTP/REST")
```

**Код реализации:**
```rust
// CLI делает HTTP запросы к Management API
async fn enable_flag(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/flags/{}/enable", base_url, flag_name))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ Flag '{}' enabled", flag_name);
    }

    Ok(())
}
```

### External System Integration

#### 1. UGC GraphQL Service Integration
**PlantUML связь:**
```plantuml
Rel(traffic_router, ugc_graphql, "GraphQL requests", "HTTP/GraphQL")
```

**Код реализации:**
```rust
// GraphQL client integration
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
            
        response.json().await
    }
}
```

#### 2. Prometheus Integration
**PlantUML связь:**
```plantuml
Rel(monitoring_service, prometheus, "Send metrics", "HTTP")
```

**Код реализации:**
```rust
// Prometheus metrics export
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

### Service Startup и Configuration

**Main application setup:**
```rust
// ugc-subgraph/src/main.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = MigrationConfig::load_from_file("feature-flags.yaml")?;
    
    // Setup Redis connection
    let redis_client = redis::Client::open(std::env::var("REDIS_URL")?)?;
    
    // Initialize migration service
    let migration_service = MigrationService::new(schema, Some(redis_client));
    
    // Setup HTTP server
    let app = Router::new()
        .nest("/api/v1", migration_service.rest_adapter.router())
        .route("/graphql", post(graphql_handler))
        .nest("/api/migration", migration_service.management_api_router())
        .route("/metrics", get(metrics_handler));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

### Environment Configuration

```bash
# .env file
MIGRATION_CONFIG_PATH=feature-flags.yaml
REDIS_URL=redis://localhost:6379
REDIS_ENABLED=true

# Feature flag overrides
FF_GRAPHQL_REVIEWS_READ_ENABLED=true
FF_GRAPHQL_REVIEWS_READ_ROLLOUT=25.0

# External services
UGC_GRAPHQL_ENDPOINT=http://localhost:4002/graphql
LEGACY_REST_ENDPOINT=http://localhost:4003/api/v1

# Monitoring
PROMETHEUS_ENDPOINT=http://prometheus:9090
```

### Заключение

Контейнерная диаграмма Task 13 обеспечивает прямую трассируемость между архитектурными контейнерами и кодом:

1. **Migration Containers** → Конкретные Rust сервисы и модули
2. **Data Storage** → Redis integration и YAML configuration
3. **External Integration** → HTTP clients и API endpoints
4. **Container Communication** → Internal function calls и HTTP APIs
5. **Configuration Management** → Environment variables и config files

Каждый контейнер имеет четко определенную реализацию в коде с proper separation of concerns и clear interfaces.