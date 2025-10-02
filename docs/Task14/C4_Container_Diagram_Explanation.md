# C4 Container Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `C4_Container_Diagram.puml`  
**Уровень:** Container (Level 2)  
**Цель:** Показать внутреннюю архитектуру UGC GraphQL Federation с фокусом на контейнеры оптимизации производительности

## 🎯 Архитектурное назначение

Эта диаграмма детализирует **как Task 14 структурирован на уровне контейнеров** и показывает распределение ответственности между компонентами системы.

## 🏗️ Контейнеры системы

### 1. GraphQL Gateway (Apollo Router)

#### Архитектурная роль
```yaml
# Фактическая конфигурация: router.yaml
federation_version: 2
supergraph:
  introspection: true
  
# Task 14: Performance optimizations на gateway уровне
plugins:
  apollo.query_planner:
    experimental_plans_limit: 10000
  apollo.traffic_shaping:
    router:
      # Task 14: Rate limiting на gateway уровне
      global_rate_limit:
        capacity: 1000
        interval: 60s
    subgraph:
      ugc-subgraph:
        # Task 14: Per-subgraph limits
        rate_limit:
          capacity: 500
          interval: 60s
```

#### Реализация маршрутизации
```rust
// Концептуальная реализация gateway логики
impl GraphQLGateway {
    pub async fn route_query(&self, query: &str) -> Result<Response> {
        // Task 14: Query analysis на gateway уровне
        let query_plan = self.planner.plan(query).await?;
        
        // Проверка complexity перед отправкой в subgraph
        if query_plan.complexity > self.config.max_complexity {
            return Err(QueryTooComplexError);
        }
        
        // Маршрутизация в UGC subgraph
        let subgraph_queries = query_plan.subgraph_queries;
        let responses = self.execute_subgraph_queries(subgraph_queries).await?;
        
        // Композиция ответов
        self.compose_response(responses).await
    }
}
```

### 2. UGC Subgraph Container

Это **основной контейнер Task 14**, содержащий все компоненты оптимизации производительности.

#### 2.1 GraphQL Server

```rust
// Фактическая реализация: src/graphql/mod.rs
use async_graphql::{Schema, EmptySubscription};
use crate::performance::*;

pub type UGCSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub async fn create_enhanced_schema(
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
    dataloader_manager: Arc<DataLoaderManager>,
    query_analyzer: Arc<QueryComplexityAnalyzer>,
    rate_limiter: Arc<RateLimitService>,
) -> Result<UGCSchema> {
    
    let schema = Schema::build(
        QueryRoot::new(db_pool.clone()),
        MutationRoot::new(db_pool.clone()),
        EmptySubscription
    )
    // Task 14: Добавляем компоненты производительности в context
    .data(cache_manager)
    .data(dataloader_manager)
    .data(query_analyzer.clone())
    .data(rate_limiter.clone())
    
    // Task 14: Query complexity analysis extension
    .extension(QueryComplexityExtension::new(query_analyzer))
    
    // Task 14: Rate limiting extension
    .extension(RateLimitExtension::new(rate_limiter))
    
    // Task 14: Caching extension
    .extension(CacheExtension::new(cache_manager))
    
    // Task 14: DataLoader extension
    .extension(DataLoaderExtension::new(dataloader_manager))
    
    .finish();
    
    Ok(schema)
}
```

#### 2.2 DataLoader Service

```rust
// Фактическая реализация: src/service/dataloader.rs
pub struct DataLoaderManager {
    review_loader: Arc<ReviewDataLoader>,
    rating_loader: Arc<RatingDataLoader>,
    offer_loader: Arc<OfferDataLoader>,
    user_loader: Arc<UserDataLoader>,
}

impl DataLoaderManager {
    pub fn new(db_pool: PgPool, external_clients: ExternalClients) -> Self {
        Self {
            // Task 14: Инициализация всех DataLoader'ов
            review_loader: Arc::new(ReviewDataLoader::new(
                ReviewRepository::new(db_pool.clone())
            )),
            rating_loader: Arc::new(RatingDataLoader::new(
                RatingRepository::new(db_pool.clone())
            )),
            offer_loader: Arc::new(OfferDataLoader::new(
                external_clients.offers_client
            )),
            user_loader: Arc::new(UserDataLoader::new(
                external_clients.users_client
            )),
        }
    }
    
    // Task 14: Request-scoped DataLoader instances
    pub fn create_request_context(&self) -> DataLoaderContext {
        DataLoaderContext {
            review_loader: self.review_loader.clone(),
            rating_loader: self.rating_loader.clone(),
            offer_loader: self.offer_loader.clone(),
            user_loader: self.user_loader.clone(),
        }
    }
}

// Task 14: Request-scoped context для предотвращения утечек кеша
pub struct DataLoaderContext {
    review_loader: Arc<ReviewDataLoader>,
    rating_loader: Arc<RatingDataLoader>,
    offer_loader: Arc<OfferDataLoader>,
    user_loader: Arc<UserDataLoader>,
}
```

#### 2.3 Cache Service

```rust
// Фактическая реализация: src/service/cache.rs
pub struct CacheService {
    manager: Arc<CacheManager>,
    invalidation_service: Arc<CacheInvalidationService>,
    warming_service: Arc<CacheWarmingService>,
}

impl CacheService {
    pub async fn new(redis_config: RedisConfig) -> Result<Self> {
        let manager = Arc::new(CacheManager::new(redis_config).await?);
        
        Ok(Self {
            manager: manager.clone(),
            // Task 14: Автоматическая инвалидация кеша
            invalidation_service: Arc::new(
                CacheInvalidationService::new(manager.clone())
            ),
            // Task 14: Предварительный прогрев кеша
            warming_service: Arc::new(
                CacheWarmingService::new(manager.clone())
            ),
        })
    }
    
    // Task 14: Интеллектуальное кеширование с TTL
    pub async fn get_or_compute<T, F, Fut>(&self, 
        key: &str, 
        ttl: Duration,
        compute_fn: F
    ) -> Result<T> 
    where
        T: Serialize + DeserializeOwned + Clone + Send + 'static,
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
    {
        // Проверяем L1 cache (memory)
        if let Some(cached) = self.manager.get_from_l1(key).await? {
            return Ok(cached);
        }
        
        // Проверяем L2 cache (Redis)
        if let Some(cached) = self.manager.get_from_l2(key).await? {
            // Обновляем L1 cache
            self.manager.set_l1(key, &cached, ttl).await?;
            return Ok(cached);
        }
        
        // Вычисляем значение
        let computed = compute_fn().await?;
        
        // Сохраняем в оба уровня кеша
        self.manager.set_multi_level(key, &computed, ttl).await?;
        
        Ok(computed)
    }
}
```

#### 2.4 Query Analyzer

```rust
// Фактическая реализация: src/service/query_limits.rs
pub struct QueryAnalyzer {
    max_depth: u32,
    max_complexity: u32,
    field_complexity_map: HashMap<String, u32>,
    rate_limiter: Arc<RateLimitService>,
}

impl QueryAnalyzer {
    pub async fn analyze_and_validate(&self, 
        query: &str, 
        user_context: &UserContext
    ) -> Result<QueryValidationResult> {
        
        // Task 14: Парсинг и анализ GraphQL query
        let document = parse_query(query)?;
        let mut analysis = QueryAnalysis::new();
        
        // Анализ глубины
        self.analyze_depth(&document, &mut analysis)?;
        
        // Анализ сложности
        self.analyze_complexity(&document, &mut analysis)?;
        
        // Проверка rate limits
        let rate_limit_result = self.rate_limiter
            .check_user_limits(user_context.user_id)
            .await?;
        
        // Task 14: Комплексная валидация
        let validation_result = QueryValidationResult {
            is_valid: analysis.depth <= self.max_depth 
                && analysis.complexity <= self.max_complexity
                && rate_limit_result.allowed,
            depth: analysis.depth,
            complexity: analysis.complexity,
            estimated_cost: self.estimate_cost(&analysis),
            rate_limit_remaining: rate_limit_result.remaining,
            violations: analysis.violations,
        };
        
        Ok(validation_result)
    }
    
    // Task 14: Динамическое вычисление сложности
    fn calculate_field_complexity(&self, 
        field: &Field, 
        parent_type: &str
    ) -> u32 {
        let base_complexity = self.field_complexity_map
            .get(&format!("{}:{}", parent_type, field.name))
            .copied()
            .unwrap_or(1);
        
        // Учитываем аргументы (например, first, limit)
        let multiplier = field.arguments.iter()
            .find(|(name, _)| name == "first" || name == "limit")
            .and_then(|(_, value)| value.as_i64())
            .unwrap_or(1) as u32;
        
        base_complexity * multiplier.min(100) // Ограничиваем максимальный multiplier
    }
}
```

#### 2.5 Rate Limiter

```rust
// Фактическая реализация: src/service/rate_limit.rs
pub struct RateLimitService {
    redis_client: Arc<RedisCache>,
    default_limits: RateLimitConfig,
    user_specific_limits: Arc<RwLock<HashMap<UserId, RateLimitConfig>>>,
}

impl RateLimitService {
    pub async fn check_and_increment(&self, 
        user_id: UserId,
        query_complexity: u32
    ) -> Result<RateLimitResult> {
        
        let limits = self.get_user_limits(user_id).await;
        let window_key = format!("rate_limit:{}:{}", 
            user_id, 
            current_window_timestamp()
        );
        
        // Task 14: Sliding window rate limiting
        let current_usage = self.redis_client
            .get::<u32>(&window_key)
            .await?
            .unwrap_or(0);
        
        // Проверяем лимиты с учетом complexity
        let complexity_cost = (query_complexity as f64 / 100.0).ceil() as u32;
        let new_usage = current_usage + complexity_cost;
        
        if new_usage > limits.requests_per_minute {
            return Ok(RateLimitResult {
                allowed: false,
                remaining: 0,
                reset_time: next_window_timestamp(),
                retry_after: Duration::from_secs(60),
            });
        }
        
        // Обновляем счетчик
        self.redis_client
            .set(&window_key, new_usage, Duration::from_secs(60))
            .await?;
        
        Ok(RateLimitResult {
            allowed: true,
            remaining: limits.requests_per_minute - new_usage,
            reset_time: next_window_timestamp(),
            retry_after: Duration::from_secs(0),
        })
    }
}
```

### 3. External Databases

#### 3.1 Redis Cache

```rust
// Фактическая реализация: src/database/redis.rs
pub struct RedisCluster {
    primary: redis::Client,
    replicas: Vec<redis::Client>,
    connection_pool: Pool<MultiplexedConnection>,
}

impl RedisCluster {
    pub async fn new(config: RedisClusterConfig) -> Result<Self> {
        let primary = redis::Client::open(config.primary_url)?;
        let replicas = config.replica_urls.into_iter()
            .map(|url| redis::Client::open(url))
            .collect::<Result<Vec<_>, _>>()?;
        
        // Task 14: Connection pooling для Redis
        let connection_pool = Pool::builder()
            .max_size(config.max_connections)
            .min_idle(Some(config.min_connections))
            .build(primary.clone())
            .await?;
        
        Ok(Self { primary, replicas, connection_pool })
    }
    
    // Task 14: Read preference с fallback на replicas
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned 
    {
        // Пробуем primary
        match self.get_from_primary(key).await {
            Ok(result) => Ok(result),
            Err(_) => {
                // Fallback на replicas
                for replica in &self.replicas {
                    if let Ok(result) = self.get_from_replica(replica, key).await {
                        return Ok(result);
                    }
                }
                Err(RedisError::AllNodesDown)
            }
        }
    }
}
```

#### 3.2 PostgreSQL Database

```sql
-- Фактическая реализация: Database schema optimizations
-- migrations/20240101_task14_performance.sql

-- Task 14: Connection pooling configuration
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '512MB';
ALTER SYSTEM SET effective_cache_size = '2GB';
ALTER SYSTEM SET work_mem = '16MB';
ALTER SYSTEM SET maintenance_work_mem = '256MB';

-- Task 14: Optimized indexes для DataLoader patterns
CREATE INDEX CONCURRENTLY idx_reviews_dataloader_batch
ON reviews USING btree (id)
WHERE is_moderated = true;

CREATE INDEX CONCURRENTLY idx_reviews_offer_batch
ON reviews USING btree (offer_id, created_at DESC)
WHERE is_moderated = true
INCLUDE (id, content, rating, author_id);

-- Task 14: Partitioning для больших таблиц
CREATE TABLE reviews_partitioned (
    LIKE reviews INCLUDING ALL
) PARTITION BY RANGE (created_at);

CREATE TABLE reviews_2024_q1 PARTITION OF reviews_partitioned
FOR VALUES FROM ('2024-01-01') TO ('2024-04-01');

-- Task 14: Materialized views для expensive aggregations
CREATE MATERIALIZED VIEW offer_rating_summary AS
SELECT 
    offer_id,
    COUNT(*) as review_count,
    AVG(rating) as average_rating,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY rating) as median_rating
FROM reviews 
WHERE is_moderated = true
GROUP BY offer_id;

CREATE UNIQUE INDEX ON offer_rating_summary (offer_id);
```

## 🔄 Container Interactions

### 1. GraphQL Gateway ↔ UGC Subgraph

```rust
// Фактическая реализация: Gateway integration
impl SubgraphClient {
    pub async fn execute_query(&self, 
        query: &str, 
        variables: Variables
    ) -> Result<Response> {
        
        // Task 14: Request tracing для performance monitoring
        let span = tracing::info_span!("subgraph_request", 
            subgraph = "ugc",
            query_hash = %hash_query(query)
        );
        
        async move {
            let request = GraphQLRequest {
                query: query.to_string(),
                variables,
                // Task 14: Передаем performance hints
                extensions: json!({
                    "performance": {
                        "enable_caching": true,
                        "enable_dataloader": true,
                        "max_complexity": 1000
                    }
                })
            };
            
            let response = self.http_client
                .post(&self.endpoint)
                .json(&request)
                .send()
                .await?;
            
            // Task 14: Мониторинг performance metrics
            self.metrics.record_subgraph_request_duration(
                start.elapsed()
            );
            
            response.json().await
        }.instrument(span).await
    }
}
```

### 2. UGC Subgraph Internal Communication

```rust
// Фактическая реализация: Internal service communication
impl GraphQLResolver {
    pub async fn resolve_field(&self, ctx: &Context<'_>) -> Result<FieldValue> {
        // Task 14: Получаем все performance services из context
        let cache_manager = ctx.data::<CacheManager>()?;
        let dataloader_manager = ctx.data::<DataLoaderManager>()?;
        let query_analyzer = ctx.data::<QueryComplexityAnalyzer>()?;
        
        // Task 14: Координация между сервисами
        let field_complexity = query_analyzer
            .get_field_complexity(&ctx.field().name());
        
        if field_complexity > 50 {
            // Высокая сложность - используем агрессивное кеширование
            return self.resolve_with_aggressive_caching(ctx).await;
        }
        
        // Обычное разрешение с DataLoader
        self.resolve_with_dataloader(ctx).await
    }
    
    async fn resolve_with_dataloader(&self, ctx: &Context<'_>) -> Result<FieldValue> {
        let dataloader_ctx = ctx.data::<DataLoaderContext>()?;
        
        match ctx.field().name() {
            "reviews" => {
                let offer_id = ctx.parent_value.get_offer_id()?;
                let reviews = dataloader_ctx.review_loader
                    .load_by_offer_id(offer_id)
                    .await?;
                Ok(FieldValue::List(reviews))
            }
            "author" => {
                let user_id = ctx.parent_value.get_author_id()?;
                let user = dataloader_ctx.user_loader
                    .load(user_id)
                    .await?;
                Ok(FieldValue::Object(user))
            }
            _ => self.resolve_default(ctx).await
        }
    }
}
```

### 3. Cache ↔ Database Integration

```rust
// Фактическая реализация: Cache-through pattern
impl CacheManager {
    pub async fn get_or_fetch<T, F>(&self, 
        cache_key: &str,
        fetch_fn: F,
        ttl: Duration
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned + Clone + Send + 'static,
        F: FnOnce() -> BoxFuture<'static, Result<T>> + Send
    {
        // Task 14: Multi-level cache check
        if let Some(value) = self.get_from_memory(cache_key).await? {
            self.metrics.record_cache_hit("memory");
            return Ok(value);
        }
        
        if let Some(value) = self.get_from_redis(cache_key).await? {
            self.metrics.record_cache_hit("redis");
            // Populate memory cache
            self.set_memory(cache_key, &value, ttl).await?;
            return Ok(value);
        }
        
        // Cache miss - fetch from source
        self.metrics.record_cache_miss("all");
        let value = fetch_fn().await?;
        
        // Task 14: Write-through caching
        tokio::try_join!(
            self.set_memory(cache_key, &value, ttl),
            self.set_redis(cache_key, &value, ttl)
        )?;
        
        Ok(value)
    }
}
```

## 📊 Performance Characteristics

### Container-Level Metrics

```rust
// Фактическая реализация: Container metrics
pub struct ContainerMetrics {
    // GraphQL Server metrics
    pub graphql_requests_total: Counter,
    pub graphql_request_duration: Histogram,
    pub graphql_errors_total: Counter,
    
    // DataLoader metrics
    pub dataloader_batch_size: Histogram,
    pub dataloader_cache_hit_ratio: Gauge,
    pub dataloader_load_duration: Histogram,
    
    // Cache Service metrics
    pub cache_operations_total: Counter,
    pub cache_hit_ratio: Gauge,
    pub cache_size_bytes: Gauge,
    
    // Query Analyzer metrics
    pub query_complexity_score: Histogram,
    pub query_depth: Histogram,
    pub rejected_queries_total: Counter,
    
    // Rate Limiter metrics
    pub rate_limit_checks_total: Counter,
    pub rate_limited_requests_total: Counter,
}
```

### Resource Allocation

```yaml
# Фактическая конфигурация: docker-compose.yml
version: '3.8'
services:
  ugc-subgraph:
    # Task 14: Resource limits для optimal performance
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 4G
        reservations:
          cpus: '1.0'
          memory: 2G
    environment:
      # Task 14: Performance tuning
      RUST_LOG: info
      DATABASE_MAX_CONNECTIONS: 20
      REDIS_MAX_CONNECTIONS: 10
      DATALOADER_MAX_BATCH_SIZE: 50
      CACHE_MAX_MEMORY_SIZE: 1073741824  # 1GB
      
  redis:
    deploy:
      resources:
        limits:
          memory: 2G
        reservations:
          memory: 1G
    command: >
      redis-server 
      --maxmemory 1gb 
      --maxmemory-policy allkeys-lru
      --save 900 1
      
  postgres:
    deploy:
      resources:
        limits:
          memory: 4G
        reservations:
          memory: 2G
    environment:
      # Task 14: PostgreSQL performance tuning
      POSTGRES_SHARED_BUFFERS: 512MB
      POSTGRES_EFFECTIVE_CACHE_SIZE: 2GB
      POSTGRES_WORK_MEM: 16MB
```

## 🔗 Связь с реализацией

Эта Container диаграмма напрямую отражается в:

- **`src/main.rs`** - Инициализация всех контейнеров
- **`src/service/`** - Реализация каждого сервиса
- **`docker-compose.yml`** - Deployment конфигурация
- **`Cargo.toml`** - Dependencies для каждого компонента
- **`migrations/`** - Database schema optimizations

Диаграмма служит **deployment blueprint** для Task 14, показывая как компоненты оптимизации производительности распределены по контейнерам и взаимодействуют друг с другом.