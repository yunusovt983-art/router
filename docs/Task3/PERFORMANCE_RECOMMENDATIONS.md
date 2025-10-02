# Рекомендации по производительности федеративной GraphQL системы

## Обзор

Данный документ содержит практические рекомендации по оптимизации производительности федеративной GraphQL архитектуры Auto.ru, основанные на анализе Task 1 и Task 2.

## 1. Оптимизация GraphQL запросов

### Проблема N+1 запросов

#### Неоптимальный подход:
```graphql
# Этот запрос вызывает N+1 проблему
query GetOffersWithReviews {
  offers(first: 10) {
    edges {
      node {
        id
        title
        reviews {  # Отдельный запрос для каждого offer
          rating
          text
          user {   # Еще один запрос для каждого review
            name
            avatar
          }
        }
      }
    }
  }
}
```

#### Оптимизированное решение через DataLoader:
```rust
// Реализация DataLoader для батчинга запросов
use async_graphql::dataloader::{DataLoader, Loader};
use std::collections::HashMap;

pub struct ReviewLoader {
    pool: PgPool,
}

#[async_trait::async_trait]
impl Loader<OfferId> for ReviewLoader {
    type Value = Vec<Review>;
    type Error = sqlx::Error;

    async fn load(&self, keys: &[OfferId]) -> Result<HashMap<OfferId, Vec<Review>>, Self::Error> {
        let offer_uuids: Vec<uuid::Uuid> = keys.iter().map(|k| k.0).collect();
        
        let reviews = sqlx::query_as!(
            Review,
            r#"
            SELECT r.*, u.name as user_name, u.avatar as user_avatar
            FROM reviews r
            JOIN users u ON r.user_id = u.id
            WHERE r.offer_id = ANY($1) 
              AND r.deleted_at IS NULL 
              AND r.is_moderated = TRUE
            ORDER BY r.created_at DESC
            "#,
            &offer_uuids
        )
        .fetch_all(&self.pool)
        .await?;
        
        // Группировка результатов по offer_id
        let mut result: HashMap<OfferId, Vec<Review>> = HashMap::new();
        for review in reviews {
            result
                .entry(OfferId(review.offer_id))
                .or_insert_with(Vec::new)
                .push(review);
        }
        
        // Заполнение пустых результатов для ключей без данных
        for key in keys {
            result.entry(*key).or_insert_with(Vec::new);
        }
        
        Ok(result)
    }
}

// Использование в резолвере
#[Object]
impl Offer {
    async fn reviews(&self, ctx: &Context<'_>) -> Result<Vec<Review>> {
        let loader = ctx.data::<DataLoader<ReviewLoader>>()?;
        let reviews = loader.load_one(self.id).await?;
        Ok(reviews.unwrap_or_default())
    }
}
```

#### Федеративный DataLoader:
```typescript
// DataLoader для федеративных запросов
class FederatedUserLoader {
  private userSubgraphUrl: string;
  
  constructor(userSubgraphUrl: string) {
    this.userSubgraphUrl = userSubgraphUrl;
  }
  
  async batchLoadUsers(userIds: string[]): Promise<User[]> {
    const query = `
      query GetUsersByIds($ids: [ID!]!) {
        _entities(representations: $ids) {
          ... on User {
            id
            name
            avatar
            email
          }
        }
      }
    `;
    
    const representations = userIds.map(id => ({
      __typename: 'User',
      id
    }));
    
    const response = await fetch(this.userSubgraphUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query,
        variables: { ids: representations }
      })
    });
    
    const result = await response.json();
    return result.data._entities;
  }
}
```

### Оптимизация сложных запросов

#### Query Complexity Analysis:
```typescript
// Анализ сложности запросов
import { createComplexityLimitRule } from 'graphql-query-complexity';

const complexityLimitRule = createComplexityLimitRule(1000, {
  // Настройка весов для разных полей
  fieldExtensions: {
    complexity: {
      // Простые скалярные поля
      'User.name': 1,
      'User.email': 1,
      
      // Поля с JOIN'ами
      'User.reviews': 10,
      'Offer.reviews': 15,
      
      // Агрегированные поля
      'Offer.averageRating': 5,
      'User.reviewsCount': 3,
      
      // Дорогие вычисления
      'Review.sentiment': 20,
      'Offer.recommendations': 50,
    }
  }
});

// Применение в схеме
const schema = buildSchema({
  typeDefs,
  resolvers,
  validationRules: [complexityLimitRule]
});
```

#### Query Depth Limiting:
```typescript
// Ограничение глубины запросов
import depthLimit from 'graphql-depth-limit';

const schema = buildSchema({
  typeDefs,
  resolvers,
  validationRules: [
    depthLimit(10), // Максимальная глубина 10 уровней
    complexityLimitRule
  ]
});
```

## 2. Стратегии кеширования

### Многоуровневое кеширование

#### L1: In-Memory Cache (Node.js)
```typescript
// Кеш в памяти для часто используемых данных
class MemoryCache {
  private cache = new Map<string, { value: any; expiry: number }>();
  private maxSize = 1000;
  
  set(key: string, value: any, ttlMs: number = 60000): void {
    // LRU eviction при превышении размера
    if (this.cache.size >= this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }
    
    this.cache.set(key, {
      value,
      expiry: Date.now() + ttlMs
    });
  }
  
  get(key: string): any | null {
    const item = this.cache.get(key);
    if (!item) return null;
    
    if (Date.now() > item.expiry) {
      this.cache.delete(key);
      return null;
    }
    
    return item.value;
  }
}
```

#### L2: Redis Cache (Distributed)
```rust
// Распределенный кеш для подграфов
use redis::{AsyncCommands, Client};
use serde::{Serialize, Deserialize};

pub struct DistributedCache {
    client: Client,
}

impl DistributedCache {
    pub async fn get_or_set<T, F, Fut>(
        &self,
        key: &str,
        ttl_seconds: u64,
        fetch_fn: F,
    ) -> Result<T, CacheError>
    where
        T: Serialize + for<'de> Deserialize<'de>,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error>>>,
    {
        let mut conn = self.client.get_async_connection().await?;
        
        // Попытка получить из кеша
        if let Ok(cached_data) = conn.get::<_, String>(key).await {
            if let Ok(value) = serde_json::from_str::<T>(&cached_data) {
                return Ok(value);
            }
        }
        
        // Получение данных и кеширование
        let value = fetch_fn().await?;
        let serialized = serde_json::to_string(&value)?;
        
        let _: () = conn.set_ex(key, serialized, ttl_seconds).await?;
        
        Ok(value)
    }
}

// Использование в сервисе
impl ReviewService {
    pub async fn get_offer_rating(&self, offer_id: OfferId) -> UgcResult<OfferRating> {
        let cache_key = format!("offer_rating:{}", offer_id);
        
        self.cache.get_or_set(
            &cache_key,
            3600, // 1 час TTL
            || async {
                self.calculate_offer_rating(offer_id).await
            }
        ).await
    }
}
```

#### L3: CDN Cache (Edge)
```yaml
# CloudFlare/CloudFront конфигурация для GraphQL
cache_rules:
  # Кеширование статических запросов
  - pattern: "query IntrospectionQuery"
    ttl: 86400  # 24 часа
    
  - pattern: "query GetStaticData"
    ttl: 3600   # 1 час
    
  # Персонализированные запросы не кешируются
  - pattern: "query GetUserProfile"
    ttl: 0
    
  # Кеширование по заголовкам
  - pattern: "query GetOffers"
    ttl: 300    # 5 минут
    vary_headers: ["Accept-Language", "X-User-Region"]
```

### Умная инвалидация кеша

```typescript
// Система инвалидации кеша на основе тегов
class TaggedCache {
  private cache = new Map<string, any>();
  private tags = new Map<string, Set<string>>(); // tag -> keys
  private keyTags = new Map<string, Set<string>>(); // key -> tags
  
  set(key: string, value: any, tags: string[] = []): void {
    this.cache.set(key, value);
    
    // Обновление индексов тегов
    this.keyTags.set(key, new Set(tags));
    
    for (const tag of tags) {
      if (!this.tags.has(tag)) {
        this.tags.set(tag, new Set());
      }
      this.tags.get(tag)!.add(key);
    }
  }
  
  invalidateByTag(tag: string): void {
    const keys = this.tags.get(tag);
    if (!keys) return;
    
    for (const key of keys) {
      this.cache.delete(key);
      
      // Очистка индексов
      const keyTagsSet = this.keyTags.get(key);
      if (keyTagsSet) {
        for (const keyTag of keyTagsSet) {
          this.tags.get(keyTag)?.delete(key);
        }
        this.keyTags.delete(key);
      }
    }
    
    this.tags.delete(tag);
  }
}

// Использование в мутациях
class ReviewMutations {
  async createReview(input: CreateReviewInput): Promise<Review> {
    const review = await this.reviewService.createReview(input);
    
    // Инвалидация связанных кешей
    this.cache.invalidateByTag(`offer:${input.offerId}`);
    this.cache.invalidateByTag(`user:${input.userId}`);
    this.cache.invalidateByTag('offer_ratings');
    
    return review;
  }
}
```

## 3. Оптимизация базы данных

### Индексы для GraphQL запросов

```sql
-- Оптимизированные индексы для частых GraphQL запросов
-- Составные индексы для фильтрации и сортировки
CREATE INDEX CONCURRENTLY idx_reviews_offer_created_rating 
ON reviews(offer_id, created_at DESC, rating) 
WHERE deleted_at IS NULL AND is_moderated = TRUE;

-- Индекс для пагинации
CREATE INDEX CONCURRENTLY idx_reviews_cursor_pagination 
ON reviews(offer_id, created_at, id) 
WHERE deleted_at IS NULL AND is_moderated = TRUE;

-- Частичные индексы для активных записей
CREATE INDEX CONCURRENTLY idx_reviews_active_user_offer 
ON reviews(user_id, offer_id) 
WHERE deleted_at IS NULL;

-- Индекс для полнотекстового поиска
CREATE INDEX CONCURRENTLY idx_reviews_fulltext_search 
ON reviews USING gin(to_tsvector('russian', title || ' ' || text)) 
WHERE deleted_at IS NULL AND is_moderated = TRUE;

-- Индекс для агрегированных запросов
CREATE INDEX CONCURRENTLY idx_reviews_aggregation 
ON reviews(offer_id, rating, created_at) 
WHERE deleted_at IS NULL AND is_moderated = TRUE;
```

### Connection Pool оптимизация

```rust
// Оптимизированная конфигурация пула подключений
use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use std::time::Duration;

pub async fn create_optimized_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let connect_options = database_url.parse::<PgConnectOptions>()?
        .application_name("ugc-subgraph")
        .statement_cache_capacity(100)  // Кеш prepared statements
        .log_statements(tracing::log::LevelFilter::Debug);
    
    PgPoolOptions::new()
        .max_connections(20)           // Максимум подключений
        .min_connections(5)            // Минимум подключений
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .test_before_acquire(true)     // Проверка подключения
        .connect_with(connect_options)
        .await
}
```

### Prepared Statements

```rust
// Использование prepared statements для частых запросов
pub struct OptimizedQueries {
    get_reviews_by_offer: String,
    get_user_reviews: String,
    calculate_rating: String,
}

impl OptimizedQueries {
    pub fn new() -> Self {
        Self {
            get_reviews_by_offer: r#"
                SELECT r.*, u.name as user_name, u.avatar as user_avatar
                FROM reviews r
                LEFT JOIN users u ON r.user_id = u.id
                WHERE r.offer_id = $1 
                  AND r.deleted_at IS NULL 
                  AND r.is_moderated = TRUE
                ORDER BY r.created_at DESC
                LIMIT $2 OFFSET $3
            "#.to_string(),
            
            get_user_reviews: r#"
                SELECT r.*, o.title as offer_title
                FROM reviews r
                LEFT JOIN offers o ON r.offer_id = o.id
                WHERE r.user_id = $1 
                  AND r.deleted_at IS NULL
                ORDER BY r.created_at DESC
                LIMIT $2 OFFSET $3
            "#.to_string(),
            
            calculate_rating: r#"
                SELECT 
                    COUNT(*) as total_reviews,
                    AVG(rating) as average_rating,
                    COUNT(*) FILTER (WHERE rating = 1) as rating_1,
                    COUNT(*) FILTER (WHERE rating = 2) as rating_2,
                    COUNT(*) FILTER (WHERE rating = 3) as rating_3,
                    COUNT(*) FILTER (WHERE rating = 4) as rating_4,
                    COUNT(*) FILTER (WHERE rating = 5) as rating_5
                FROM reviews 
                WHERE offer_id = $1 
                  AND deleted_at IS NULL 
                  AND is_moderated = TRUE
            "#.to_string(),
        }
    }
}
```

## 4. Федеративная оптимизация

### Query Planning оптимизация

```typescript
// Оптимизация планирования федеративных запросов
class OptimizedQueryPlanner {
  planQuery(query: DocumentNode): QueryPlan {
    const plan = this.baseQueryPlanner.plan(query);
    
    // Оптимизация: параллельные запросы к независимым подграфам
    const parallelGroups = this.identifyParallelGroups(plan);
    
    // Оптимизация: батчинг запросов к одному подграфу
    const batchedPlan = this.batchSubgraphCalls(plan);
    
    // Оптимизация: предварительная загрузка связанных данных
    const prefetchedPlan = this.addPrefetching(batchedPlan);
    
    return prefetchedPlan;
  }
  
  private identifyParallelGroups(plan: QueryPlan): ParallelGroup[] {
    // Анализ зависимостей между подграфами
    const dependencies = this.analyzeDependencies(plan);
    
    // Группировка независимых запросов
    return this.groupIndependentQueries(dependencies);
  }
}
```

### Subgraph Response Caching

```typescript
// Кеширование ответов подграфов
class SubgraphResponseCache {
  private cache = new LRUCache<string, any>({ max: 1000 });
  
  async executeWithCache(
    subgraphName: string,
    query: string,
    variables: any,
    executor: Function
  ): Promise<any> {
    const cacheKey = this.generateCacheKey(subgraphName, query, variables);
    
    // Проверка кеша
    const cached = this.cache.get(cacheKey);
    if (cached && !this.isExpired(cached)) {
      return cached.data;
    }
    
    // Выполнение запроса
    const result = await executor();
    
    // Кеширование результата
    if (this.isCacheable(query)) {
      this.cache.set(cacheKey, {
        data: result,
        timestamp: Date.now(),
        ttl: this.getTTL(query)
      });
    }
    
    return result;
  }
  
  private isCacheable(query: string): boolean {
    // Не кешируем мутации и персонализированные запросы
    return !query.includes('mutation') && 
           !query.includes('currentUser') &&
           !query.includes('personalizedRecommendations');
  }
}
```

## 5. Мониторинг производительности

### Метрики производительности

```typescript
// Комплексные метрики для федеративной системы
class PerformanceMetrics {
  // GraphQL метрики
  private graphqlRequestDuration = new prometheus.Histogram({
    name: 'graphql_request_duration_seconds',
    help: 'GraphQL request duration',
    labelNames: ['operation_name', 'operation_type', 'subgraph'],
    buckets: [0.01, 0.05, 0.1, 0.2, 0.5, 1, 2, 5]
  });
  
  private graphqlRequestComplexity = new prometheus.Histogram({
    name: 'graphql_request_complexity',
    help: 'GraphQL request complexity score',
    labelNames: ['operation_name'],
    buckets: [1, 10, 50, 100, 500, 1000, 5000]
  });
  
  // Федеративные метрики
  private federationPlanningTime = new prometheus.Histogram({
    name: 'federation_planning_duration_seconds',
    help: 'Time spent planning federated queries',
    buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.2]
  });
  
  private subgraphCallsPerQuery = new prometheus.Histogram({
    name: 'subgraph_calls_per_query',
    help: 'Number of subgraph calls per federated query',
    buckets: [1, 2, 3, 5, 10, 20]
  });
  
  // Кеш метрики
  private cacheHitRate = new prometheus.Gauge({
    name: 'cache_hit_rate',
    help: 'Cache hit rate percentage',
    labelNames: ['cache_type', 'cache_level']
  });
  
  recordGraphQLRequest(
    operationName: string,
    operationType: string,
    duration: number,
    complexity: number,
    subgraphCalls: number
  ): void {
    this.graphqlRequestDuration
      .labels(operationName, operationType, 'gateway')
      .observe(duration);
    
    this.graphqlRequestComplexity
      .labels(operationName)
      .observe(complexity);
    
    this.subgraphCallsPerQuery
      .observe(subgraphCalls);
  }
}
```

### Автоматическая оптимизация

```typescript
// Система автоматической оптимизации на основе метрик
class AutoOptimizer {
  private metrics: PerformanceMetrics;
  private thresholds = {
    slowQueryThreshold: 1000, // ms
    highComplexityThreshold: 500,
    lowCacheHitThreshold: 0.8
  };
  
  async analyzeAndOptimize(): Promise<OptimizationReport> {
    const report = new OptimizationReport();
    
    // Анализ медленных запросов
    const slowQueries = await this.identifySlowQueries();
    for (const query of slowQueries) {
      const optimization = await this.optimizeSlowQuery(query);
      report.addOptimization(optimization);
    }
    
    // Анализ сложных запросов
    const complexQueries = await this.identifyComplexQueries();
    for (const query of complexQueries) {
      const optimization = await this.optimizeComplexQuery(query);
      report.addOptimization(optimization);
    }
    
    // Анализ эффективности кеша
    const cacheAnalysis = await this.analyzeCacheEfficiency();
    if (cacheAnalysis.hitRate < this.thresholds.lowCacheHitThreshold) {
      const optimization = await this.optimizeCacheStrategy(cacheAnalysis);
      report.addOptimization(optimization);
    }
    
    return report;
  }
  
  private async optimizeSlowQuery(query: SlowQuery): Promise<Optimization> {
    // Анализ причин медленности
    const bottlenecks = await this.analyzeQueryBottlenecks(query);
    
    const suggestions = [];
    
    if (bottlenecks.includes('n_plus_one')) {
      suggestions.push({
        type: 'add_dataloader',
        description: 'Add DataLoader for batching requests',
        impact: 'high'
      });
    }
    
    if (bottlenecks.includes('missing_index')) {
      suggestions.push({
        type: 'add_database_index',
        description: 'Add database index for frequent queries',
        impact: 'high'
      });
    }
    
    if (bottlenecks.includes('subgraph_latency')) {
      suggestions.push({
        type: 'add_caching',
        description: 'Add response caching for subgraph',
        impact: 'medium'
      });
    }
    
    return new Optimization(query, suggestions);
  }
}
```

## 6. Практические рекомендации

### Чек-лист оптимизации производительности

#### Уровень запросов:
- [ ] Используйте DataLoader для предотвращения N+1 проблем
- [ ] Ограничьте глубину и сложность запросов
- [ ] Реализуйте пагинацию для больших списков
- [ ] Используйте фрагменты для переиспользования частей запросов
- [ ] Избегайте избыточных полей в запросах

#### Уровень кеширования:
- [ ] Реализуйте многоуровневое кеширование (L1/L2/L3)
- [ ] Используйте умную инвалидацию кеша по тегам
- [ ] Кешируйте результаты дорогих вычислений
- [ ] Настройте CDN для статических данных
- [ ] Мониторьте hit rate кешей

#### Уровень базы данных:
- [ ] Создайте индексы для частых запросов
- [ ] Оптимизируйте пул подключений
- [ ] Используйте prepared statements
- [ ] Реализуйте read replicas для чтения
- [ ] Мониторьте медленные запросы

#### Уровень федерации:
- [ ] Оптимизируйте планирование запросов
- [ ] Используйте параллельные запросы к подграфам
- [ ] Реализуйте батчинг запросов
- [ ] Кешируйте ответы подграфов
- [ ] Мониторьте межсервисную латентность

#### Уровень мониторинга:
- [ ] Настройте метрики производительности
- [ ] Реализуйте distributed tracing
- [ ] Создайте алерты для SLA нарушений
- [ ] Анализируйте тренды производительности
- [ ] Автоматизируйте оптимизацию

### Целевые показатели производительности

```yaml
# SLA для федеративной GraphQL системы
performance_targets:
  response_time:
    p50: 100ms    # 50% запросов быстрее 100ms
    p95: 500ms    # 95% запросов быстрее 500ms
    p99: 1000ms   # 99% запросов быстрее 1s
  
  throughput:
    target: 10000  # RPS
    peak: 50000    # Peak RPS
  
  availability:
    target: 99.9%  # Uptime
    
  cache_efficiency:
    l1_hit_rate: 90%   # In-memory cache
    l2_hit_rate: 80%   # Redis cache
    l3_hit_rate: 70%   # CDN cache
  
  database:
    connection_pool_utilization: 80%
    slow_query_threshold: 100ms
    
  federation:
    subgraph_calls_per_query: 3  # Среднее количество
    planning_time: 10ms          # Время планирования
```

Эти рекомендации обеспечивают высокую производительность федеративной GraphQL системы при сохранении гибкости и масштабируемости архитектуры.