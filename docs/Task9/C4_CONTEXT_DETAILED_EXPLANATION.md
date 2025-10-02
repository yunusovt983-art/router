# Task 9: Context Diagram - Подробное объяснение системы кеширования и оптимизации производительности

## 🎯 Цель диаграммы

Context диаграмма Task 9 демонстрирует **комплексную enterprise-grade систему оптимизации производительности** для федеративной GraphQL платформы Auto.ru, показывая как кеширование, DataLoader оптимизация и rate limiting интегрируются для обеспечения высокой производительности и защиты от злоупотреблений.

## 🏗️ Архитектурная эволюция: от медленной к высокопроизводительной системе

### От неоптимизированной системы к performance-first архитектуре

#### Было: Система без оптимизации производительности
```rust
// Простой GraphQL резолвер без оптимизации
async fn reviews_by_offer(ctx: &Context<'_>, offer_id: Uuid) -> FieldResult<Vec<Review>> {
    let db_pool = ctx.data::<PgPool>()?;
    
    // N+1 проблема - отдельный запрос для каждого offer
    let reviews = sqlx::query_as::<_, Review>(
        "SELECT * FROM reviews WHERE offer_id = $1"
    )
    .bind(offer_id)
    .fetch_all(db_pool)
    .await?;
    
    // Нет кеширования - каждый запрос идет в БД
    // Нет rate limiting - возможны злоупотребления
    // Нет анализа сложности запросов
    // Нет мониторинга производительности
    
    Ok(reviews)
}

// Проблемы:
// - N+1 queries при загрузке связанных данных
// - Отсутствие кеширования часто запрашиваемых данных
// - Нет защиты от сложных/глубоких запросов
// - Отсутствие rate limiting
// - Плохая производительность под нагрузкой
```###
# Стало: Высокопроизводительная система с comprehensive оптимизацией
```rust
// GraphQL резолвер с полной оптимизацией производительности
#[tracing::instrument(skip(ctx), fields(offer_id = %offer_id))]
async fn reviews_by_offer_optimized(
    ctx: &Context<'_>, 
    offer_id: Uuid
) -> FieldResult<Vec<Review>> {
    let cache_service = ctx.data::<Arc<CacheService>>()?;
    let dataloader = ctx.data::<DataLoader<ReviewDataLoader>>()?;
    let metrics = ctx.data::<Arc<PerformanceMetrics>>()?;
    let rate_limiter = ctx.data::<Arc<RateLimiter>>()?;
    
    let start_time = std::time::Instant::now();
    
    // 1. RATE LIMITING: Проверка ограничений запросов
    let user_id = ctx.data::<UserId>()?.0;
    if !rate_limiter.check_rate_limit(user_id, 10.0).await? {
        metrics.rate_limit_violations.inc();
        return Err("Rate limit exceeded".into());
    }
    
    // 2. CACHING: Cache-first подход
    let cache_key = CacheKeyBuilder::query_key(
        "reviews_by_offer", 
        &serde_json::json!({"offer_id": offer_id}),
        Some(user_id)
    );
    
    if let Ok(Some(cached_reviews)) = cache_service.get::<Vec<Review>>(&cache_key).await {
        metrics.cache_hits.with_label_values(&["query_result"]).inc();
        tracing::info!(
            cache_key = %cache_key,
            "Cache hit for reviews query"
        );
        return Ok(cached_reviews);
    }
    
    metrics.cache_misses.with_label_values(&["query_result"]).inc();
    
    // 3. DATALOADER: N+1 оптимизация через batch loading
    let reviews = dataloader.load(offer_id).await
        .map_err(|e| {
            metrics.dataloader_errors.inc();
            format!("DataLoader error: {}", e)
        })?;
    
    // 4. CACHING: Сохранение результата в кеш
    let cache_ttl = Duration::from_secs(300); // 5 минут
    if let Err(e) = cache_service.set(&cache_key, &reviews, Some(cache_ttl)).await {
        tracing::warn!(
            error = %e,
            cache_key = %cache_key,
            "Failed to cache query result"
        );
    }
    
    // 5. METRICS: Запись метрик производительности
    let execution_time = start_time.elapsed();
    metrics.query_execution_time
        .with_label_values(&["reviews_by_offer"])
        .observe(execution_time.as_secs_f64());
    
    metrics.dataloader_batch_size
        .with_label_values(&["review"])
        .observe(reviews.len() as f64);
    
    tracing::info!(
        offer_id = %offer_id,
        reviews_count = reviews.len(),
        execution_time_ms = execution_time.as_millis(),
        cache_key = %cache_key,
        "Reviews query completed successfully"
    );
    
    Ok(reviews)
}

// Преимущества:
// ✅ Cache-first подход для быстрых ответов
// ✅ DataLoader устраняет N+1 проблемы
// ✅ Rate limiting защищает от злоупотреблений
// ✅ Comprehensive метрики производительности
// ✅ Structured logging для debugging
// ✅ Graceful error handling с fallbacks
```

**Объяснение**: Оптимизированная архитектура превращает медленную систему в высокопроизводительную платформу с тремя уровнями оптимизации: Caching (быстрый доступ к данным), DataLoader (N+1 prevention), Rate Limiting (защита ресурсов).

## 🔧 Ключевые компоненты и их реализация

### 1. Auto.ru Performance-Optimized Federation - Основная система с оптимизацией

#### UGC Subgraph (Cached) - Подграф с кешированием
```rust
// ugc-subgraph/src/main.rs
use std::sync::Arc;
use axum::{routing::post, Router, Extension};
use tower::ServiceBuilder;

#[derive(Clone)]
pub struct PerformanceOptimizedUgcService {
    // Performance optimization components
    cache_service: Arc<CacheService>,
    dataloader_registry: Arc<DataLoaderRegistry>,
    rate_limiter: Arc<RateLimiter>,
    query_complexity_analyzer: Arc<QueryComplexityAnalyzer>,
    performance_metrics: Arc<PerformanceMetrics>,
    
    // Application components
    db_pool: PgPool,
    review_service: Arc<ReviewService>,
}

impl PerformanceOptimizedUgcService {
    pub async fn new() -> Result<Self, ServiceError> {
        // 1. Initialize Redis cache service
        let cache_config = CacheConfig {
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            default_ttl: Duration::from_secs(300),
            max_connections: 20,
            cluster_mode: std::env::var("REDIS_CLUSTER_MODE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            compression: true,
        };
        
        let cache_service = Arc::new(CacheService::new(cache_config).await?);
        
        // 2. Initialize DataLoader registry
        let db_pool = create_optimized_database_pool().await?;
        let dataloader_registry = Arc::new(
            DataLoaderRegistry::new(db_pool.clone(), cache_service.clone())
        );
        
        // 3. Initialize rate limiter
        let rate_limiter = Arc::new(RateLimiter::new(
            cache_service.clone(),
            RateLimitConfig {
                requests_per_minute: 1000,
                burst_size: 100,
                window_size: Duration::from_secs(60),
                complexity_factor: true,
            }
        ));
        
        // 4. Initialize query complexity analyzer
        let query_complexity_analyzer = Arc::new(QueryComplexityAnalyzer::new(
            ComplexityConfig {
                max_depth: 10,
                max_complexity: 100.0,
                field_weights: create_field_weights(),
            }
        ));
        
        // 5. Initialize performance metrics
        let performance_metrics = Arc::new(PerformanceMetrics::new()?);
        
        Ok(Self {
            cache_service,
            dataloader_registry,
            rate_limiter,
            query_complexity_analyzer,
            performance_metrics,
            db_pool,
            review_service: Arc::new(ReviewService::new(db_pool.clone())),
        })
    }

    /// Создание веб-сервера с performance optimization
    pub fn create_server(&self) -> Result<Router, ServiceError> {
        let schema = self.create_performance_optimized_schema();
        
        let app = Router::new()
            .route("/graphql", post(graphql_handler))
            .route("/health", get(health_check))
            .route("/metrics", get(metrics_handler))
            .route("/cache/stats", get(cache_stats_handler))
            .layer(Extension(schema))
            .layer(Extension(self.clone()))
            .layer(
                ServiceBuilder::new()
                    // Performance middleware stack
                    .layer(self.create_performance_middleware())
                    .layer(self.create_rate_limiting_middleware())
                    .layer(self.create_query_complexity_middleware())
                    .layer(self.create_caching_middleware())
            );

        Ok(app)
    }

    /// Создание GraphQL схемы с performance optimization
    fn create_performance_optimized_schema(&self) -> Schema<Query, Mutation, Subscription> {
        Schema::build(Query, Mutation, Subscription)
            .data(self.cache_service.clone())
            .data(self.dataloader_registry.clone())
            .data(self.rate_limiter.clone())
            .data(self.query_complexity_analyzer.clone())
            .data(self.performance_metrics.clone())
            .data(self.db_pool.clone())
            .enable_federation()
            // Performance extensions
            .extension(CacheExtension::new(self.cache_service.clone()))
            .extension(DataLoaderExtension::new(self.dataloader_registry.clone()))
            .extension(QueryComplexityExtension::new(
                self.query_complexity_analyzer.clone()
            ))
            .extension(PerformanceMetricsExtension::new(
                self.performance_metrics.clone()
            ))
            .finish()
    }

    /// Performance middleware для comprehensive optimization
    fn create_performance_middleware(&self) -> impl Layer<Router> {
        let metrics = self.performance_metrics.clone();
        
        tower::layer::layer_fn(move |service| {
            let metrics = metrics.clone();
            
            tower::service_fn(move |request| {
                let metrics = metrics.clone();
                let service = service.clone();
                let start_time = std::time::Instant::now();
                
                async move {
                    // Pre-request metrics
                    metrics.http_requests_in_flight.inc();
                    metrics.http_requests_total.inc();
                    
                    let result = service.call(request).await;
                    
                    // Post-request metrics
                    let duration = start_time.elapsed();
                    metrics.http_request_duration.observe(duration.as_secs_f64());
                    metrics.http_requests_in_flight.dec();
                    
                    // Record success/error metrics
                    match &result {
                        Ok(response) => {
                            let status = response.status().as_u16();
                            if status < 400 {
                                metrics.http_requests_success.inc();
                            } else {
                                metrics.http_requests_error
                                    .with_label_values(&[&status.to_string()])
                                    .inc();
                            }
                        }
                        Err(_) => {
                            metrics.http_requests_error
                                .with_label_values(&["500"])
                                .inc();
                        }
                    }
                    
                    result
                }
            })
        })
    }
}

/// Создание оптимизированного пула подключений к БД
async fn create_optimized_database_pool() -> Result<PgPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    PgPoolOptions::new()
        .max_connections(50) // Увеличенный пул для DataLoader
        .min_connections(10) // Минимальные подключения для быстрого старта
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        // Оптимизация для batch queries
        .test_before_acquire(true)
        .connect(&database_url)
        .await
}

/// Создание весов полей для анализа сложности
fn create_field_weights() -> HashMap<String, f64> {
    let mut weights = HashMap::new();
    
    // Простые поля
    weights.insert("id".to_string(), 1.0);
    weights.insert("createdAt".to_string(), 1.0);
    weights.insert("updatedAt".to_string(), 1.0);
    
    // Средние поля
    weights.insert("content".to_string(), 2.0);
    weights.insert("rating".to_string(), 2.0);
    
    // Сложные поля (требуют дополнительных запросов)
    weights.insert("author".to_string(), 5.0);
    weights.insert("offer".to_string(), 5.0);
    weights.insert("comments".to_string(), 10.0);
    
    // Агрегации (очень дорогие)
    weights.insert("averageRating".to_string(), 15.0);
    weights.insert("reviewsCount".to_string(), 10.0);
    
    weights
}
```

#### Apollo Router (Optimized) - Роутер с оптимизацией
```yaml
# router.yaml - конфигурация с performance optimization
# Query planning optimization
supergraph:
  query_planning:
    cache:
      in_memory:
        limit: 1024  # Увеличенный кеш для query plans
    experimental_reuse_query_fragments: true
    experimental_parallelism: true

# Response caching
caching:
  redis:
    urls: ["redis://redis-cluster:6379"]
    timeout: 5s
    ttl: 300s
    namespace: "apollo_router"
  
  query_plan:
    enabled: true
    ttl: 3600s  # 1 hour cache for query plans
  
  subgraph_response:
    enabled: true
    ttl: 300s   # 5 minutes for subgraph responses
    
# Rate limiting configuration
rate_limiting:
  global:
    capacity: 10000
    interval: 60s
  
  per_user:
    capacity: 1000
    interval: 60s
    
  query_complexity:
    max_depth: 10
    max_complexity: 100
    
# Headers для performance optimization
headers:
  all:
    request:
      - propagate:
          named: "x-cache-control"
      - propagate:
          named: "x-user-id"
      - propagate:
          named: "x-request-id"
    response:
      - insert:
          name: "cache-control"
          value: "public, max-age=300"
      - insert:
          name: "x-cache-status"
          from_context: "cache_status"

# Telemetry для performance monitoring
telemetry:
  metrics:
    prometheus:
      enabled: true
      listen: 0.0.0.0:9090
    
    common:
      attributes:
        supergraph:
          static:
            - name: "cache_enabled"
              value: "true"
            - name: "optimization_level"
              value: "high"
```

### 2. Performance & Monitoring Infrastructure - Инфраструктура производительности

#### Redis Cluster - Система distributed caching
```yaml
# docker-compose.yml - Redis Cluster configuration
services:
  redis-master-1:
    image: redis:7-alpine
    ports:
      - "7001:7001"
    command: >
      redis-server
      --port 7001
      --cluster-enabled yes
      --cluster-config-file nodes-7001.conf
      --cluster-node-timeout 5000
      --appendonly yes
      --appendfsync everysec
      --maxmemory 1gb
      --maxmemory-policy allkeys-lru
    volumes:
      - redis_data_1:/data
    networks:
      - performance-network

  redis-master-2:
    image: redis:7-alpine
    ports:
      - "7002:7002"
    command: >
      redis-server
      --port 7002
      --cluster-enabled yes
      --cluster-config-file nodes-7002.conf
      --cluster-node-timeout 5000
      --appendonly yes
      --appendfsync everysec
      --maxmemory 1gb
      --maxmemory-policy allkeys-lru
    volumes:
      - redis_data_2:/data
    networks:
      - performance-network

  redis-master-3:
    image: redis:7-alpine
    ports:
      - "7003:7003"
    command: >
      redis-server
      --port 7003
      --cluster-enabled yes
      --cluster-config-file nodes-7003.conf
      --cluster-node-timeout 5000
      --appendonly yes
      --appendfsync everysec
      --maxmemory 1gb
      --maxmemory-policy allkeys-lru
    volumes:
      - redis_data_3:/data
    networks:
      - performance-network

  # Redis cluster initialization
  redis-cluster-init:
    image: redis:7-alpine
    depends_on:
      - redis-master-1
      - redis-master-2
      - redis-master-3
    command: >
      redis-cli --cluster create
      redis-master-1:7001
      redis-master-2:7002
      redis-master-3:7003
      --cluster-replicas 0
      --cluster-yes
    networks:
      - performance-network
```

#### Performance Monitoring - Мониторинг производительности
```yaml
# prometheus.yml - конфигурация для performance metrics
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'auto-ru-performance'
    environment: 'production'

rule_files:
  - "performance-alerts.yml"
  - "cache-alerts.yml"
  - "dataloader-alerts.yml"

scrape_configs:
  # UGC Subgraph performance metrics
  - job_name: 'ugc-subgraph-performance'
    static_configs:
      - targets: ['ugc-subgraph:4001']
    scrape_interval: 5s
    metrics_path: /metrics
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
        replacement: 'ugc-subgraph-performance'

  # Apollo Router performance metrics
  - job_name: 'apollo-router-performance'
    static_configs:
      - targets: ['apollo-router:9090']
    scrape_interval: 5s
    metrics_path: /metrics

  # Redis Cluster metrics
  - job_name: 'redis-cluster'
    static_configs:
      - targets: 
        - 'redis-master-1:7001'
        - 'redis-master-2:7002'
        - 'redis-master-3:7003'
    scrape_interval: 10s

  # Database performance metrics
  - job_name: 'postgres-performance'
    static_configs:
      - targets: ['postgres-exporter:9187']
    scrape_interval: 30s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

```yaml
# performance-alerts.yml - алерты производительности
groups:
  - name: cache-performance
    rules:
      # Low cache hit rate
      - alert: LowCacheHitRate
        expr: rate(cache_hits_total[5m]) / (rate(cache_hits_total[5m]) + rate(cache_misses_total[5m])) < 0.8
        for: 2m
        labels:
          severity: warning
          component: cache
        annotations:
          summary: "Low cache hit rate detected"
          description: "Cache hit rate is {{ $value | humanizePercentage }} which is below 80%"

      # High cache memory usage
      - alert: HighCacheMemoryUsage
        expr: redis_memory_used_bytes / redis_memory_max_bytes > 0.9
        for: 1m
        labels:
          severity: critical
          component: redis
        annotations:
          summary: "Redis memory usage is high"
          description: "Redis memory usage is {{ $value | humanizePercentage }}"

  - name: dataloader-performance
    rules:
      # Low DataLoader efficiency
      - alert: LowDataLoaderEfficiency
        expr: avg(dataloader_batch_size) < 5
        for: 5m
        labels:
          severity: warning
          component: dataloader
        annotations:
          summary: "DataLoader batch efficiency is low"
          description: "Average batch size is {{ $value }}, indicating potential N+1 issues"

      # High DataLoader errors
      - alert: HighDataLoaderErrors
        expr: rate(dataloader_errors_total[5m]) > 0.01
        for: 1m
        labels:
          severity: critical
          component: dataloader
        annotations:
          summary: "High DataLoader error rate"
          description: "DataLoader error rate is {{ $value }} errors per second"

  - name: query-performance
    rules:
      # Slow query execution
      - alert: SlowQueryExecution
        expr: histogram_quantile(0.95, rate(query_execution_time_seconds_bucket[5m])) > 1.0
        for: 2m
        labels:
          severity: warning
          component: graphql
        annotations:
          summary: "Slow GraphQL query execution"
          description: "95th percentile query execution time is {{ $value }}s"

      # High query complexity
      - alert: HighQueryComplexity
        expr: avg(query_complexity) > 80
        for: 5m
        labels:
          severity: warning
          component: graphql
        annotations:
          summary: "High average query complexity"
          description: "Average query complexity is {{ $value }}"

  - name: rate-limiting
    rules:
      # High rate limit violations
      - alert: HighRateLimitViolations
        expr: rate(rate_limit_violations_total[5m]) > 10
        for: 1m
        labels:
          severity: critical
          component: rate-limiter
        annotations:
          summary: "High rate limit violations"
          description: "Rate limit violations: {{ $value }} per second"
```

## 🚀 Практическое применение

### Полный пример оптимизированного GraphQL запроса
```rust
// Пример Query с полной оптимизацией производительности
impl Query {
    #[tracing::instrument(
        skip(self, ctx),
        fields(
            offer_id = %offer_id,
            user_id = tracing::field::Empty,
            cache_key = tracing::field::Empty
        )
    )]
    async fn reviews_with_full_optimization(
        &self,
        ctx: &Context<'_>,
        offer_id: Uuid,
        first: Option<i32>,
        after: Option<String>,
    ) -> FieldResult<ReviewConnection> {
        let cache_service = ctx.data::<Arc<CacheService>>()?;
        let dataloader = ctx.data::<DataLoader<ReviewDataLoader>>()?;
        let rate_limiter = ctx.data::<Arc<RateLimiter>>()?;
        let metrics = ctx.data::<Arc<PerformanceMetrics>>()?;
        let query_analyzer = ctx.data::<Arc<QueryComplexityAnalyzer>>()?;
        
        let start_time = std::time::Instant::now();
        
        // 1. Extract user context
        let user_id = ctx.data::<UserId>().ok().map(|u| u.0);
        let span = tracing::Span::current();
        span.record("user_id", &tracing::field::display(&user_id.unwrap_or_default()));
        
        // 2. Query complexity analysis
        let query_source = ctx.query_env.query.as_ref()
            .ok_or("Query source not available")?;
        let complexity_result = query_analyzer.analyze(query_source)?;
        
        if !complexity_result.is_valid {
            metrics.query_complexity_violations.inc();
            return Err("Query complexity exceeds limits".into());
        }
        
        metrics.query_complexity.observe(complexity_result.complexity);
        metrics.query_depth.observe(complexity_result.depth as f64);
        
        // 3. Rate limiting check
        if let Some(uid) = user_id {
            if !rate_limiter.check_rate_limit(uid, complexity_result.complexity).await? {
                metrics.rate_limit_violations.inc();
                return Err("Rate limit exceeded".into());
            }
        }
        
        // 4. Cache key generation
        let cache_key = CacheKeyBuilder::query_key(
            "reviews_connection",
            &serde_json::json!({
                "offer_id": offer_id,
                "first": first,
                "after": after
            }),
            user_id
        );
        
        span.record("cache_key", &tracing::field::display(&cache_key));
        
        // 5. Cache lookup
        if let Ok(Some(cached_connection)) = cache_service
            .get::<ReviewConnection>(&cache_key).await 
        {
            metrics.cache_hits.with_label_values(&["review_connection"]).inc();
            
            let execution_time = start_time.elapsed();
            metrics.query_execution_time
                .with_label_values(&["reviews_cached"])
                .observe(execution_time.as_secs_f64());
            
            tracing::info!(
                cache_hit = true,
                execution_time_ms = execution_time.as_millis(),
                "Returned cached review connection"
            );
            
            return Ok(cached_connection);
        }
        
        metrics.cache_misses.with_label_values(&["review_connection"]).inc();
        
        // 6. DataLoader batch loading
        let dataloader_start = std::time::Instant::now();
        let reviews = dataloader.load(offer_id).await
            .map_err(|e| {
                metrics.dataloader_errors.inc();
                tracing::error!(
                    error = %e,
                    offer_id = %offer_id,
                    "DataLoader failed to load reviews"
                );
                format!("Failed to load reviews: {}", e)
            })?;
        
        let dataloader_time = dataloader_start.elapsed();
        metrics.dataloader_load_time
            .with_label_values(&["review"])
            .observe(dataloader_time.as_secs_f64());
        
        // 7. Apply pagination
        let connection = self.paginate_reviews(reviews, first, after)?;
        
        // 8. Cache the result
        let cache_ttl = self.calculate_cache_ttl(&connection);
        if let Err(e) = cache_service.set(&cache_key, &connection, Some(cache_ttl)).await {
            tracing::warn!(
                error = %e,
                cache_key = %cache_key,
                "Failed to cache review connection"
            );
        }
        
        // 9. Record comprehensive metrics
        let total_execution_time = start_time.elapsed();
        
        metrics.query_execution_time
            .with_label_values(&["reviews_uncached"])
            .observe(total_execution_time.as_secs_f64());
        
        metrics.dataloader_batch_size
            .with_label_values(&["review"])
            .observe(connection.edges.len() as f64);
        
        tracing::info!(
            cache_hit = false,
            total_execution_time_ms = total_execution_time.as_millis(),
            dataloader_time_ms = dataloader_time.as_millis(),
            reviews_count = connection.edges.len(),
            complexity = complexity_result.complexity,
            depth = complexity_result.depth,
            "Review connection query completed"
        );
        
        Ok(connection)
    }
    
    /// Расчет TTL для кеша на основе данных
    fn calculate_cache_ttl(&self, connection: &ReviewConnection) -> Duration {
        // Адаптивный TTL на основе количества данных и времени
        let base_ttl = Duration::from_secs(300); // 5 минут базовый TTL
        
        // Увеличиваем TTL для больших результатов (они дороже вычислять)
        let size_factor = (connection.edges.len() as f64 / 10.0).min(3.0);
        
        // Уменьшаем TTL в рабочее время (данные чаще обновляются)
        let time_factor = if self.is_business_hours() { 0.5 } else { 1.5 };
        
        Duration::from_secs(
            (base_ttl.as_secs() as f64 * size_factor * time_factor) as u64
        )
    }
    
    fn is_business_hours(&self) -> bool {
        let now = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Moscow);
        let hour = now.hour();
        hour >= 9 && hour <= 18 && now.weekday().num_days_from_monday() < 5
    }
}
```

Эта Context диаграмма демонстрирует комплексную enterprise-grade систему оптимизации производительности, которая превращает медленную систему в высокопроизводительную платформу с тремя уровнями оптимизации (Caching, DataLoader, Rate Limiting), обеспечивая excellent user experience и защиту ресурсов.