# Анализ производительности инфраструктуры Task 1

## Обзор

Данный документ анализирует производительность инфраструктуры, созданной в Task 1, включая метрики, узкие места и рекомендации по оптимизации.

## 1. Архитектурные факторы производительности

### 1.1 Выбор Rust как основного языка

**Преимущества для производительности**:
- **Zero-cost abstractions**: Высокоуровневые конструкции без runtime overhead
- **Отсутствие GC**: Предсказуемая производительность без пауз сборщика мусора
- **Эффективное управление памятью**: Stack allocation по умолчанию
- **Оптимизации компилятора**: LLVM backend с агрессивными оптимизациями

**Бенчмарки языков для GraphQL**:
```
Requests/sec (простой GraphQL запрос):
Rust (async-graphql):     ~45,000 req/s
Go (gqlgen):             ~35,000 req/s  
Node.js (Apollo Server): ~15,000 req/s
Java (Spring GraphQL):   ~25,000 req/s
```

### 1.2 Async/Await архитектура

**Tokio runtime конфигурация**:
```rust
#[tokio::main]
async fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get())
        .thread_stack_size(2 * 1024 * 1024) // 2MB stack
        .enable_all()
        .build()
        .unwrap();
}
```

**Преимущества**:
- **Высокая concurrency**: Тысячи одновременных соединений
- **Эффективное I/O**: Non-blocking операции
- **Низкое потребление памяти**: ~2KB на task vs ~2MB на thread

## 2. Производительность контейнеризации

### 2.1 Multi-stage Docker builds

**Анализ размеров образов**:
```
Single-stage build:
├── rust:1.75 base image:     ~1.8GB
├── Dependencies:             ~200MB  
├── Source code:              ~50MB
├── Build artifacts:          ~150MB
└── Total:                    ~2.2GB

Multi-stage build:
├── debian:bookworm-slim:     ~80MB
├── Binary:                   ~15MB
├── Runtime deps:             ~5MB
└── Total:                    ~100MB

Improvement: 95% reduction in image size
```

**Влияние на производительность**:
- **Время загрузки**: 2.2GB → 100MB (22x быстрее)
- **Время запуска контейнера**: ~5s → ~1s
- **Сетевой трафик**: Значительное снижение при деплое
- **Дисковое пространство**: Экономия в кластере

### 2.2 Container resource optimization

**Рекомендуемые лимиты ресурсов**:
```yaml
# docker-compose.yml
apollo-router:
  deploy:
    resources:
      limits:
        cpus: '1.0'
        memory: 512M
      reservations:
        cpus: '0.5'
        memory: 256M

ugc-subgraph:
  deploy:
    resources:
      limits:
        cpus: '0.5'
        memory: 256M
      reservations:
        cpus: '0.25'
        memory: 128M
```

**Обоснование лимитов**:
- **Apollo Router**: Больше CPU для маршрутизации и композиции схем
- **Subgraphs**: Меньше ресурсов, фокус на специфичной логике
- **Memory**: Rust эффективно использует память, запас для пиковых нагрузок

## 3. Производительность базы данных

### 3.1 PostgreSQL конфигурация

**Оптимизированные настройки для разработки**:
```sql
-- postgresql.conf оптимизации
shared_buffers = 256MB                    -- 25% от RAM
effective_cache_size = 1GB                -- 75% от RAM  
work_mem = 4MB                           -- Для сортировок
maintenance_work_mem = 64MB              -- Для VACUUM, CREATE INDEX
checkpoint_completion_target = 0.9       -- Сглаживание I/O
wal_buffers = 16MB                       -- WAL буферы
random_page_cost = 1.1                   -- Для SSD
effective_io_concurrency = 200           -- Для SSD
```

**Connection pooling с sqlx**:
```rust
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .max_connections(20)                 // Максимум соединений
    .min_connections(5)                  // Минимум соединений
    .acquire_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(600))
    .max_lifetime(Duration::from_secs(1800))
    .connect(&database_url)
    .await?;
```

### 3.2 Индексирование стратегии

**Базовые индексы для UGC**:
```sql
-- Индексы для таблицы reviews
CREATE INDEX CONCURRENTLY idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX CONCURRENTLY idx_reviews_user_id ON reviews(user_id);
CREATE INDEX CONCURRENTLY idx_reviews_created_at ON reviews(created_at DESC);
CREATE INDEX CONCURRENTLY idx_reviews_rating ON reviews(rating);

-- Композитные индексы для частых запросов
CREATE INDEX CONCURRENTLY idx_reviews_offer_moderated 
ON reviews(offer_id, is_moderated) WHERE is_moderated = true;

-- Частичные индексы для активных записей
CREATE INDEX CONCURRENTLY idx_reviews_active 
ON reviews(offer_id, created_at DESC) WHERE deleted_at IS NULL;
```

**Анализ производительности запросов**:
```sql
-- Включение статистики
SET track_io_timing = on;
SET log_min_duration_statement = 100; -- Логировать медленные запросы

-- Анализ плана выполнения
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) 
SELECT * FROM reviews 
WHERE offer_id = $1 AND is_moderated = true 
ORDER BY created_at DESC 
LIMIT 20;
```

## 4. Производительность кеширования

### 4.1 Redis оптимизация

**Конфигурация Redis для производительности**:
```redis
# redis.conf
maxmemory 256mb
maxmemory-policy allkeys-lru
tcp-keepalive 300
timeout 0

# Persistence настройки (для dev среды)
save 900 1      # Сохранять если 1+ изменений за 15 минут
save 300 10     # Сохранять если 10+ изменений за 5 минут
save 60 10000   # Сохранять если 10000+ изменений за 1 минуту

# Оптимизации
hash-max-ziplist-entries 512
hash-max-ziplist-value 64
list-max-ziplist-size -2
set-max-intset-entries 512
```

**Стратегии кеширования в Rust**:
```rust
use redis::AsyncCommands;

pub struct CacheService {
    redis: redis::Client,
}

impl CacheService {
    // Кеширование с TTL
    pub async fn set_with_ttl<T>(&self, key: &str, value: &T, ttl: u64) -> Result<()>
    where
        T: serde::Serialize,
    {
        let mut conn = self.redis.get_async_connection().await?;
        let serialized = serde_json::to_string(value)?;
        conn.setex(key, ttl, serialized).await?;
        Ok(())
    }
    
    // Пакетное получение
    pub async fn mget<T>(&self, keys: &[String]) -> Result<Vec<Option<T>>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut conn = self.redis.get_async_connection().await?;
        let values: Vec<Option<String>> = conn.mget(keys).await?;
        
        values.into_iter()
            .map(|v| v.map(|s| serde_json::from_str(&s)).transpose())
            .collect()
    }
}
```

### 4.2 Кеширование на уровне приложения

**Multi-level caching стратегия**:
```rust
use moka::future::Cache;
use std::time::Duration;

pub struct MultiLevelCache {
    l1_cache: Cache<String, String>,        // In-memory
    l2_cache: CacheService,                 // Redis
}

impl MultiLevelCache {
    pub fn new() -> Self {
        let l1_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(60))    // 1 минута
            .build();
            
        Self {
            l1_cache,
            l2_cache: CacheService::new(),
        }
    }
    
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned + Clone,
    {
        // Сначала проверяем L1 кеш
        if let Some(value) = self.l1_cache.get(key).await {
            return Ok(Some(serde_json::from_str(&value)?));
        }
        
        // Затем L2 кеш (Redis)
        if let Some(value) = self.l2_cache.get::<T>(key).await? {
            // Сохраняем в L1 для следующих запросов
            let serialized = serde_json::to_string(&value)?;
            self.l1_cache.insert(key.to_string(), serialized).await;
            return Ok(Some(value));
        }
        
        Ok(None)
    }
}
```

## 5. Сетевая производительность

### 5.1 HTTP/2 и connection pooling

**Axum сервер конфигурация**:
```rust
use axum::{Router, Server};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    timeout::TimeoutLayer,
};

let app = Router::new()
    .route("/graphql", post(graphql_handler))
    .layer(
        ServiceBuilder::new()
            .layer(CompressionLayer::new())           // gzip/brotli сжатие
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .into_inner(),
    );

// HTTP/2 с оптимизированными настройками
Server::bind(&"0.0.0.0:4000".parse()?)
    .http2_initial_stream_window_size(Some(1024 * 1024))  // 1MB
    .http2_initial_connection_window_size(Some(1024 * 1024 * 10)) // 10MB
    .serve(app.into_make_service())
    .await?;
```

### 5.2 GraphQL query optimization

**Query complexity limiting**:
```rust
use async_graphql::{extensions::analyzer::*, Schema};

let schema = Schema::build(Query, Mutation, Subscription)
    .extension(Analyzer::new()
        .depth_limit(10)                    // Максимальная глубина
        .complexity_limit(1000)             // Максимальная сложность
        .alias_limit(15)                    // Лимит алиасов
    )
    .finish();
```

**DataLoader для N+1 проблем**:
```rust
use async_graphql::dataloader::*;
use std::collections::HashMap;

pub struct UserLoader {
    pool: PgPool,
}

#[async_trait::async_trait]
impl Loader<UserId> for UserLoader {
    type Value = User;
    type Error = sqlx::Error;

    async fn load(&self, keys: &[UserId]) -> Result<HashMap<UserId, User>, Self::Error> {
        let users = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ANY($1)",
            &keys.iter().map(|id| id.0).collect::<Vec<_>>()
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users.into_iter().map(|user| (UserId(user.id), user)).collect())
    }
}
```

## 6. Мониторинг производительности

### 6.1 Prometheus метрики

**Ключевые метрики производительности**:
```rust
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};

lazy_static! {
    // Счетчики запросов
    static ref HTTP_REQUESTS_TOTAL: Counter = register_counter!(
        "http_requests_total",
        "Total number of HTTP requests"
    ).unwrap();
    
    // Время выполнения запросов
    static ref HTTP_REQUEST_DURATION: Histogram = register_histogram!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds",
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]
    ).unwrap();
    
    // Активные соединения
    static ref ACTIVE_CONNECTIONS: Gauge = register_gauge!(
        "active_connections",
        "Number of active connections"
    ).unwrap();
    
    // Database connection pool
    static ref DB_CONNECTIONS_ACTIVE: Gauge = register_gauge!(
        "db_connections_active",
        "Number of active database connections"
    ).unwrap();
    
    // Cache hit rate
    static ref CACHE_HITS_TOTAL: Counter = register_counter!(
        "cache_hits_total",
        "Total number of cache hits"
    ).unwrap();
    
    static ref CACHE_MISSES_TOTAL: Counter = register_counter!(
        "cache_misses_total", 
        "Total number of cache misses"
    ).unwrap();
}
```

### 6.2 Performance dashboards

**Grafana dashboard конфигурация**:
```json
{
  "dashboard": {
    "title": "Auto.ru GraphQL Federation Performance",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "Requests/sec"
          }
        ]
      },
      {
        "title": "Response Time Percentiles", 
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "50th percentile"
          },
          {
            "expr": "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          },
          {
            "expr": "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "99th percentile"
          }
        ]
      },
      {
        "title": "Cache Hit Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(cache_hits_total[5m]) / (rate(cache_hits_total[5m]) + rate(cache_misses_total[5m])) * 100",
            "legendFormat": "Hit Rate %"
          }
        ]
      }
    ]
  }
}
```

## 7. Бенчмарки и нагрузочное тестирование

### 7.1 Локальные бенчмарки

**Criterion.rs бенчмарки**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn graphql_query_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = create_test_schema();
    
    c.bench_function("simple_query", |b| {
        b.to_async(&rt).iter(|| async {
            let query = r#"
                query {
                    reviews(first: 10) {
                        edges {
                            node {
                                id
                                rating
                                text
                            }
                        }
                    }
                }
            "#;
            
            let result = schema.execute(query).await;
            black_box(result)
        })
    });
}

criterion_group!(benches, graphql_query_benchmark);
criterion_main!(benches);
```

### 7.2 Нагрузочное тестирование с k6

**k6 скрипт для GraphQL**:
```javascript
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  stages: [
    { duration: '2m', target: 100 },   // Разогрев до 100 пользователей
    { duration: '5m', target: 100 },   // Стабильная нагрузка
    { duration: '2m', target: 200 },   // Увеличение до 200
    { duration: '5m', target: 200 },   // Стабильная нагрузка
    { duration: '2m', target: 0 },     // Снижение нагрузки
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],  // 95% запросов быстрее 500ms
    http_req_failed: ['rate<0.1'],     // Менее 10% ошибок
  },
};

export default function() {
  const query = `
    query GetReviews($offerId: ID!) {
      reviews(offerId: $offerId, first: 20) {
        edges {
          node {
            id
            rating
            text
            createdAt
            user {
              id
              name
            }
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  `;

  const variables = {
    offerId: 'offer-123'
  };

  const response = http.post('http://localhost:4000/graphql', 
    JSON.stringify({ query, variables }), 
    {
      headers: { 'Content-Type': 'application/json' },
    }
  );

  check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 500ms': (r) => r.timings.duration < 500,
    'no GraphQL errors': (r) => !JSON.parse(r.body).errors,
  });
}
```

## 8. Ожидаемые показатели производительности

### 8.1 Целевые метрики для локальной среды

**Apollo Router**:
- **Throughput**: 10,000+ req/s (простые запросы)
- **Latency P95**: < 50ms
- **Memory usage**: < 256MB под нагрузкой
- **CPU usage**: < 50% на одном ядре

**Subgraphs**:
- **Throughput**: 5,000+ req/s (простые запросы)
- **Latency P95**: < 100ms
- **Memory usage**: < 128MB под нагрузкой
- **Database connections**: < 10 активных

**PostgreSQL**:
- **Query time P95**: < 10ms (с индексами)
- **Connection pool**: 80%+ hit rate
- **Cache hit ratio**: > 95%

**Redis**:
- **Latency P95**: < 1ms
- **Memory usage**: < 256MB
- **Hit rate**: > 90%

### 8.2 Масштабирование на продакшене

**Горизонтальное масштабирование**:
```yaml
# Kubernetes HPA
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: apollo-router-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: apollo-router
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

## 9. Рекомендации по оптимизации

### 9.1 Краткосрочные улучшения

1. **Включить HTTP/2 Server Push** для статических ресурсов
2. **Настроить connection pooling** для внешних API
3. **Добавить query result caching** на уровне Apollo Router
4. **Оптимизировать Docker образы** с помощью distroless

### 9.2 Долгосрочные улучшения

1. **Реализовать query planning cache** в Apollo Router
2. **Добавить CDN** для статического контента
3. **Внедрить database read replicas** для масштабирования чтения
4. **Рассмотреть sharding** для больших таблиц

### 9.3 Мониторинг и алертинг

**Критические алерты**:
```yaml
groups:
- name: performance
  rules:
  - alert: HighResponseTime
    expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 0.5
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High response time detected"
      
  - alert: LowCacheHitRate
    expr: rate(cache_hits_total[5m]) / (rate(cache_hits_total[5m]) + rate(cache_misses_total[5m])) < 0.8
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Cache hit rate below 80%"
```

## Заключение

Инфраструктура Task 1 обеспечивает прочную основу для высокопроизводительной федеративной GraphQL архитектуры:

- **Rust** обеспечивает отличную производительность и безопасность памяти
- **Multi-stage Docker builds** минимизируют размер образов и время развертывания  
- **Оптимизированная конфигурация** PostgreSQL и Redis для высокой производительности
- **Comprehensive monitoring** с Prometheus и Grafana для отслеживания производительности
- **Готовность к масштабированию** с помощью горизонтального масштабирования в Kubernetes

Эта инфраструктура готова обрабатывать значительные нагрузки и может быть легко масштабирована по мере роста требований Auto.ru.