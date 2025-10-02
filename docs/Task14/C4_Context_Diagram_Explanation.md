# C4 Context Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `C4_Context_Diagram.puml`  
**Уровень:** System Context (Level 1)  
**Цель:** Показать UGC GraphQL Federation систему в контексте внешних систем и пользователей

## 🎯 Архитектурное назначение

Эта диаграмма определяет **системные границы** и показывает, как Task 14 "Оптимизация производительности" влияет на взаимодействие с внешними системами.

## 🏗️ Компоненты диаграммы

### 👥 Actors (Пользователи)

#### 1. GraphQL Client
```typescript
// Фактическая реализация: Frontend приложения
const client = new ApolloClient({
  uri: 'https://api.auto.ru/graphql',
  cache: new InMemoryCache(),
  // Task 14: Клиент получает оптимизированные ответы
  defaultOptions: {
    watchQuery: {
      errorPolicy: 'all',
      fetchPolicy: 'cache-first' // Использует кеширование
    }
  }
});

// Пример запроса, который оптимизируется Task 14
const GET_OFFERS_WITH_REVIEWS = gql`
  query GetOffersWithReviews($limit: Int!) {
    offers(limit: $limit) {
      id
      name
      reviews(first: 10) {  # DataLoader оптимизирует N+1 problem
        id
        content
        rating
        author {            # Еще один уровень N+1, решаемый DataLoader
          name
        }
      }
      averageRating        # Кешируется в Redis
    }
  }
`;
```

#### 2. System Administrator
```bash
# Фактические команды мониторинга Task 14
# Мониторинг производительности кеширования
curl http://localhost:4001/metrics | grep cache_hit_ratio

# Проверка DataLoader эффективности
curl http://localhost:4001/metrics | grep dataloader_batch_size

# Анализ query complexity
curl http://localhost:4001/api/query-stats

# Мониторинг rate limiting
curl http://localhost:4001/api/rate-limit-stats
```

### 🏢 Systems

#### 1. UGC Subgraph (Центральная система)
```rust
// Фактическая реализация: src/main.rs
use crate::performance::{
    cache::CacheManager,
    dataloader::DataLoaderManager,
    query_limits::QueryComplexityAnalyzer,
    rate_limit::RateLimitService
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Task 14: Инициализация компонентов производительности
    let cache_manager = CacheManager::new(redis_config).await?;
    let dataloader_manager = DataLoaderManager::new(db_pool.clone());
    let query_analyzer = QueryComplexityAnalyzer::new(query_limits_config);
    let rate_limiter = RateLimitService::new(cache_manager.clone());
    
    // Создание оптимизированной GraphQL схемы
    let schema = create_enhanced_schema(
        db_pool,
        cache_manager,
        dataloader_manager,
        query_analyzer,
        rate_limiter
    ).await?;
    
    // Запуск сервера с оптимизациями
    start_server(schema).await
}
```

#### 2. External Systems Integration

##### Users Service
```rust
// Фактическая реализация: src/service/external.rs
pub struct UsersServiceClient {
    client: reqwest::Client,
    cache: Arc<CacheManager>, // Task 14: Кеширование внешних запросов
    circuit_breaker: CircuitBreaker,
}

impl UsersServiceClient {
    pub async fn get_user_by_id(&self, user_id: UserId) -> Result<User> {
        let cache_key = format!("user:{}", user_id);
        
        // Task 14: Проверяем кеш перед внешним запросом
        if let Some(cached_user) = self.cache.get(&cache_key).await? {
            return Ok(cached_user);
        }
        
        // Circuit breaker защищает от cascade failures
        let user = self.circuit_breaker.call(|| async {
            self.client
                .get(&format!("{}/users/{}", self.base_url, user_id))
                .send()
                .await?
                .json::<User>()
                .await
        }).await?;
        
        // Кешируем результат
        self.cache.set(&cache_key, &user, Duration::from_secs(300)).await?;
        Ok(user)
    }
}
```

##### Offers Service
```rust
// Фактическая реализация: DataLoader для batch loading offers
pub struct OffersDataLoader {
    client: OffersServiceClient,
    batch_fn: BatchFn<OfferId, Offer>,
}

impl OffersDataLoader {
    pub fn new(client: OffersServiceClient) -> Self {
        let batch_fn = move |offer_ids: Vec<OfferId>| {
            let client = client.clone();
            async move {
                // Task 14: Batch запрос вместо N отдельных
                client.get_offers_by_ids(offer_ids).await
            }
        };
        
        Self { client, batch_fn: BatchFn::new(batch_fn, 50) }
    }
}
```

#### 3. Infrastructure Systems

##### Redis Cache
```rust
// Фактическая реализация: src/service/redis_cache.rs
pub struct RedisCache {
    client: redis::Client,
    connection_pool: Pool<Connection>,
    metrics: Arc<MetricsCollector>,
}

impl RedisCache {
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>> 
    where 
        T: DeserializeOwned 
    {
        let start = Instant::now();
        let mut conn = self.connection_pool.get().await?;
        
        match conn.get::<_, Option<String>>(key).await {
            Ok(Some(value)) => {
                // Task 14: Метрики cache hit
                self.metrics.record_cache_hit("redis");
                self.metrics.record_cache_operation_duration(start.elapsed());
                
                let deserialized: T = serde_json::from_str(&value)?;
                Ok(Some(deserialized))
            }
            Ok(None) => {
                // Task 14: Метрики cache miss
                self.metrics.record_cache_miss("redis");
                Ok(None)
            }
            Err(e) => {
                self.metrics.record_cache_error("redis");
                Err(e.into())
            }
        }
    }
}
```

##### PostgreSQL Database
```sql
-- Фактическая реализация: Оптимизированные индексы для Task 14
-- migrations/20240101_performance_indexes.sql

-- DataLoader batch loading optimization
CREATE INDEX CONCURRENTLY idx_reviews_batch_load 
ON reviews (id) 
WHERE is_moderated = true;

-- N+1 prevention для reviews by offer
CREATE INDEX CONCURRENTLY idx_reviews_offer_performance 
ON reviews (offer_id, created_at DESC) 
WHERE is_moderated = true
INCLUDE (id, content, rating, author_id);

-- Query complexity optimization
CREATE INDEX CONCURRENTLY idx_reviews_complex_queries 
ON reviews (offer_id, is_moderated, rating, created_at DESC)
WHERE is_moderated = true;

-- Connection pooling optimization
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '256MB';
ALTER SYSTEM SET effective_cache_size = '1GB';
```

## 🔄 Взаимодействия (Relationships)

### 1. Client → UGC Subgraph
```graphql
# Фактический GraphQL запрос с оптимизациями Task 14
query OptimizedOffersQuery($limit: Int!, $complexity: Int!) {
  offers(limit: $limit) @complexity(value: 5) {
    id
    name
    # Task 14: DataLoader предотвращает N+1 queries
    reviews(first: 10) @complexity(value: 10) {
      id
      content
      rating
      # Еще один уровень DataLoader optimization
      author @complexity(value: 3) {
        id
        name
      }
    }
    # Task 14: Кешированное значение из Redis
    averageRating @cached(ttl: 1800)
  }
}
```

### 2. UGC Subgraph → External Services
```rust
// Фактическая реализация: src/graphql/query.rs
impl QueryRoot {
    async fn offers(&self, ctx: &Context<'_>, limit: i32) -> Result<Vec<Offer>> {
        let dataloader_manager = ctx.data::<DataLoaderManager>()?;
        let cache_manager = ctx.data::<CacheManager>()?;
        
        // Task 14: Используем DataLoader для batch loading
        let offer_loader = dataloader_manager.get_offer_loader();
        
        // Получаем IDs из базы
        let offer_ids = self.repository
            .get_offer_ids(limit)
            .await?;
        
        // Task 14: Batch load вместо N отдельных запросов
        let offers = offer_loader
            .load_many(offer_ids)
            .await?;
        
        Ok(offers)
    }
    
    async fn reviews_for_offer(
        &self, 
        ctx: &Context<'_>, 
        offer_id: OfferId
    ) -> Result<Vec<Review>> {
        let cache_key = format!("reviews:offer:{}", offer_id);
        let cache_manager = ctx.data::<CacheManager>()?;
        
        // Task 14: Проверяем multi-level cache
        if let Some(cached_reviews) = cache_manager.get(&cache_key).await? {
            return Ok(cached_reviews);
        }
        
        // Task 14: DataLoader для batch loading reviews
        let review_loader = ctx.data::<DataLoaderManager>()?
            .get_review_loader();
            
        let reviews = review_loader
            .load_by_offer_id(offer_id)
            .await?;
        
        // Кешируем результат
        cache_manager
            .set(&cache_key, &reviews, Duration::from_secs(600))
            .await?;
            
        Ok(reviews)
    }
}
```

### 3. Monitoring Integration
```rust
// Фактическая реализация: src/telemetry/metrics.rs
pub struct PerformanceMetrics {
    // Task 14: Специфичные метрики производительности
    cache_hit_ratio: Gauge,
    dataloader_batch_size: Histogram,
    query_complexity_score: Histogram,
    rate_limit_violations: Counter,
    database_query_duration: Histogram,
}

impl PerformanceMetrics {
    pub fn record_cache_operation(&self, cache_type: &str, hit: bool) {
        let labels = &[("cache_type", cache_type)];
        if hit {
            self.cache_hit_ratio.with_label_values(labels).inc();
        } else {
            self.cache_miss_ratio.with_label_values(labels).inc();
        }
    }
    
    pub fn record_dataloader_batch(&self, loader_type: &str, batch_size: usize) {
        self.dataloader_batch_size
            .with_label_values(&[("loader_type", loader_type)])
            .observe(batch_size as f64);
    }
}
```

## 🎯 Архитектурные решения Task 14

### 1. System Boundaries
- **Внутри границы:** Все компоненты оптимизации производительности
- **Вне границы:** External services, которые мы оптимизируем через кеширование

### 2. Performance Optimization Points
- **Client Interface:** GraphQL с query complexity analysis
- **External Integration:** Circuit breakers и caching
- **Data Access:** DataLoader pattern и connection pooling
- **Monitoring:** Comprehensive metrics collection

### 3. Scalability Considerations
- **Horizontal Scaling:** Redis cluster для shared cache
- **Vertical Scaling:** Connection pooling и resource optimization
- **Fault Tolerance:** Circuit breakers и graceful degradation

## 📊 Метрики производительности

```rust
// Фактические метрики, собираемые на Context уровне
pub struct SystemMetrics {
    pub total_requests: u64,
    pub cache_hit_ratio: f64,        // Task 14: >80% target
    pub average_response_time: f64,   // Task 14: <100ms cached, <500ms DB
    pub dataloader_efficiency: f64,  // Task 14: >90% batch reduction
    pub query_complexity_avg: f64,   // Task 14: <1000 points average
    pub rate_limit_violations: u64,  // Task 14: <1% of requests
}
```

## 🔗 Связь с реализацией

Эта Context диаграмма напрямую отражается в:
- **`src/main.rs`** - Инициализация системы
- **`src/config.rs`** - Конфигурация внешних систем
- **`src/telemetry/`** - Мониторинг взаимодействий
- **`docker-compose.yml`** - Инфраструктурные зависимости
- **`.env.performance`** - Конфигурация производительности

Диаграмма служит **архитектурным контрактом** между Task 14 и остальной системой, определяя границы оптимизации и точки интеграции.