# Cache Architecture Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `Cache_Architecture_Diagram.puml`  
**Тип:** Component Architecture Diagram  
**Цель:** Детальная архитектура multi-level кеширования Task 14 с показом всех уровней и стратегий

## 🎯 Архитектурное назначение

Эта диаграмма демонстрирует **comprehensive caching strategy** Task 14, показывая как различные уровни кеширования работают вместе для достижения максимальной производительности и надежности.

## 🏗️ Архитектура кеширования

### 1. Application Layer Integration

```rust
// Фактическая реализация: Application layer cache integration
impl GraphQLResolver {
    pub async fn resolve_field(&self, ctx: &Context<'_>) -> Result<FieldValue> {
        let cache_manager = ctx.data::<CacheManager>()?;
        let field_name = ctx.field().name();
        
        // Task 14: Field-level caching strategy
        match field_name {
            "reviews" => self.resolve_reviews_cached(ctx, cache_manager).await,
            "averageRating" => self.resolve_rating_cached(ctx, cache_manager).await,
            "offer" => self.resolve_offer_cached(ctx, cache_manager).await,
            _ => self.resolve_default(ctx).await
        }
    }
    
    // Task 14: Reviews caching with pagination
    async fn resolve_reviews_cached(&self, 
        ctx: &Context<'_>,
        cache_manager: &CacheManager
    ) -> Result<FieldValue> {
        
        let offer_id = ctx.parent_value.get_offer_id()?;
        let pagination = ctx.args.get_pagination()?;
        
        let cache_key = format!("reviews:offer:{}:page:{}:limit:{}", 
            offer_id, 
            pagination.after.as_deref().unwrap_or(""),
            pagination.first.unwrap_or(10)
        );
        
        // Task 14: Cache-first strategy with fallback
        cache_manager.get_or_compute(
            &cache_key,
            Duration::from_secs(300), // 5 minutes TTL
            || async {
                let review_service = ctx.data::<ReviewService>()?;
                review_service.get_reviews_paginated(offer_id, pagination).await
            }
        ).await
    }
}
```

### 2. Cache Manager - Central Coordination

```rust
// Фактическая реализация: src/performance/cache/manager.rs
pub struct CacheManager {
    l1_cache: Arc<MemoryCache>,
    l2_cache: Arc<RedisCache>,
    circuit_breaker: Arc<CircuitBreaker>,
    invalidation_service: Arc<CacheInvalidationService>,
    warming_service: Arc<CacheWarmingService>,
    metrics: Arc<MetricsCollector>,
    config: CacheConfig,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let l1_cache = Arc::new(MemoryCache::new(config.l1_config.clone()));
        let l2_cache = Arc::new(RedisCache::new(config.l2_config.clone()).await?);
        let circuit_breaker = Arc::new(CircuitBreaker::new(config.circuit_breaker_config));
        
        Ok(Self {
            l1_cache,
            l2_cache: l2_cache.clone(),
            circuit_breaker,
            invalidation_service: Arc::new(
                CacheInvalidationService::new(l1_cache.clone(), l2_cache.clone())
            ),
            warming_service: Arc::new(
                CacheWarmingService::new(l2_cache.clone())
            ),
            metrics: Arc::new(MetricsCollector::new()),
            config,
        })
    }
    
    // Task 14: Multi-level get with intelligent fallback
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned + Clone + Send + 'static
    {
        let start_time = Instant::now();
        
        // L1 Cache check (fastest)
        if let Some(value) = self.l1_cache.get::<T>(key).await? {
            self.metrics.record_cache_operation("l1", "hit", start_time.elapsed());
            return Ok(Some(value));
        }
        
        // L2 Cache check with circuit breaker protection
        let l2_result = self.circuit_breaker.call(|| {
            let l2_cache = self.l2_cache.clone();
            let key = key.to_string();
            async move {
                l2_cache.get::<T>(&key).await
            }
        }).await;
        
        match l2_result {
            Ok(Some(value)) => {
                // Task 14: Populate L1 for next access
                self.populate_l1_async(key, &value).await;
                self.metrics.record_cache_operation("l2", "hit", start_time.elapsed());
                Ok(Some(value))
            }
            Ok(None) => {
                self.metrics.record_cache_operation("all", "miss", start_time.elapsed());
                Ok(None)
            }
            Err(e) => {
                // Circuit breaker open or Redis failure
                self.metrics.record_cache_operation("l2", "error", start_time.elapsed());
                tracing::warn!("L2 cache error: {}", e);
                Ok(None)
            }
        }
    }
    
    // Task 14: Intelligent cache warming
    async fn populate_l1_async<T>(&self, key: &str, value: &T) 
    where T: Serialize + Clone + Send + Sync + 'static
    {
        let l1_cache = self.l1_cache.clone();
        let key = key.to_string();
        let value = value.clone();
        
        // Non-blocking L1 population
        tokio::spawn(async move {
            if let Err(e) = l1_cache.set(&key, &value, Duration::from_secs(300)).await {
                tracing::warn!("Failed to populate L1 cache: {}", e);
            }
        });
    }
}
```

### 3. L1 Cache (Memory) - Ultra-Fast Access

```rust
// Фактическая реализация: src/performance/cache/memory_cache.rs
pub struct MemoryCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    lru_tracker: Arc<Mutex<LruTracker>>,
    ttl_manager: Arc<TtlManager>,
    max_size: usize,
    max_memory: usize,
    metrics: Arc<MetricsCollector>,
}

#[derive(Clone)]
struct CacheEntry {
    data: Bytes,
    created_at: Instant,
    expires_at: Instant,
    access_count: AtomicU64,
    size: usize,
}

impl MemoryCache {
    pub fn new(config: MemoryCacheConfig) -> Self {
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let lru_tracker = Arc::new(Mutex::new(LruTracker::new(config.max_size)));
        let ttl_manager = Arc::new(TtlManager::new());
        
        let instance = Self {
            cache,
            lru_tracker,
            ttl_manager: ttl_manager.clone(),
            max_size: config.max_size,
            max_memory: config.max_memory_bytes,
            metrics: Arc::new(MetricsCollector::new()),
        };
        
        // Task 14: Background TTL cleanup
        instance.start_ttl_cleanup_task();
        
        instance
    }
    
    // Task 14: High-performance get with LRU tracking
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned
    {
        let start_time = Instant::now();
        
        // Fast path: read lock only
        let entry = {
            let cache = self.cache.read().await;
            cache.get(key).cloned()
        };
        
        match entry {
            Some(entry) if entry.expires_at > Instant::now() => {
                // Task 14: Update LRU and access tracking
                self.update_access_tracking(key, &entry).await;
                
                let value: T = bincode::deserialize(&entry.data)?;
                self.metrics.record_memory_cache_hit(start_time.elapsed());
                Ok(Some(value))
            }
            Some(_) => {
                // Expired entry - remove it
                self.remove_expired_entry(key).await;
                self.metrics.record_memory_cache_miss("expired", start_time.elapsed());
                Ok(None)
            }
            None => {
                self.metrics.record_memory_cache_miss("not_found", start_time.elapsed());
                Ok(None)
            }
        }
    }
    
    // Task 14: Intelligent set with eviction
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Duration) -> Result<()>
    where T: Serialize
    {
        let serialized = bincode::serialize(value)?;
        let entry_size = serialized.len() + key.len() + 64; // Approximate overhead
        
        // Task 14: Memory pressure check
        if self.would_exceed_memory_limit(entry_size).await {
            self.evict_to_make_space(entry_size).await?;
        }
        
        let entry = CacheEntry {
            data: Bytes::from(serialized),
            created_at: Instant::now(),
            expires_at: Instant::now() + ttl,
            access_count: AtomicU64::new(1),
            size: entry_size,
        };
        
        // Task 14: Atomic cache update
        {
            let mut cache = self.cache.write().await;
            cache.insert(key.to_string(), entry);
        }
        
        // Task 14: Update LRU tracking
        {
            let mut lru = self.lru_tracker.lock().await;
            lru.access(key);
        }
        
        self.metrics.record_memory_cache_set(entry_size);
        Ok(())
    }
    
    // Task 14: LRU-based eviction
    async fn evict_to_make_space(&self, needed_space: usize) -> Result<()> {
        let mut evicted_size = 0;
        let mut keys_to_evict = Vec::new();
        
        // Determine keys to evict
        {
            let lru = self.lru_tracker.lock().await;
            let cache = self.cache.read().await;
            
            for key in lru.least_recently_used_keys() {
                if let Some(entry) = cache.get(key) {
                    keys_to_evict.push(key.clone());
                    evicted_size += entry.size;
                    
                    if evicted_size >= needed_space {
                        break;
                    }
                }
            }
        }
        
        // Evict selected keys
        {
            let mut cache = self.cache.write().await;
            for key in &keys_to_evict {
                cache.remove(key);
            }
        }
        
        self.metrics.record_memory_cache_eviction(keys_to_evict.len(), evicted_size);
        Ok(())
    }
}
```

### 4. L2 Cache (Redis) - Distributed Persistence

```rust
// Фактическая реализация: src/performance/cache/redis_cache.rs
pub struct RedisCache {
    primary: redis::Client,
    replicas: Vec<redis::Client>,
    connection_pool: Pool<MultiplexedConnection>,
    serializer: Arc<CacheSerializer>,
    metrics: Arc<MetricsCollector>,
    config: RedisCacheConfig,
}

impl RedisCache {
    pub async fn new(config: RedisCacheConfig) -> Result<Self> {
        let primary = redis::Client::open(config.primary_url.clone())?;
        
        let replicas = config.replica_urls.iter()
            .map(|url| redis::Client::open(url.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        
        // Task 14: Connection pooling for Redis
        let connection_pool = Pool::builder()
            .max_size(config.max_connections)
            .min_idle(Some(config.min_connections))
            .connection_timeout(Duration::from_secs(config.connection_timeout_secs))
            .idle_timeout(Some(Duration::from_secs(config.idle_timeout_secs)))
            .build(primary.clone())
            .await?;
        
        Ok(Self {
            primary,
            replicas,
            connection_pool,
            serializer: Arc::new(CacheSerializer::new()),
            metrics: Arc::new(MetricsCollector::new()),
            config,
        })
    }
    
    // Task 14: Resilient get with replica fallback
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned
    {
        let start_time = Instant::now();
        
        // Try primary first
        match self.get_from_primary(key).await {
            Ok(result) => {
                self.metrics.record_redis_operation("primary", "get", "success", start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                self.metrics.record_redis_operation("primary", "get", "error", start_time.elapsed());
                tracing::warn!("Primary Redis error: {}", e);
                
                // Task 14: Fallback to replicas
                self.get_from_replicas(key).await
            }
        }
    }
    
    async fn get_from_primary<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned
    {
        let mut conn = self.connection_pool.get().await?;
        
        let raw_value: Option<String> = conn.get(key).await?;
        
        match raw_value {
            Some(serialized) => {
                let value = self.serializer.deserialize::<T>(&serialized)?;
                Ok(Some(value))
            }
            None => Ok(None)
        }
    }
    
    // Task 14: Replica fallback strategy
    async fn get_from_replicas<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned
    {
        for (i, replica) in self.replicas.iter().enumerate() {
            let start_time = Instant::now();
            
            match self.get_from_replica(replica, key).await {
                Ok(result) => {
                    self.metrics.record_redis_operation(
                        &format!("replica_{}", i), 
                        "get", 
                        "success", 
                        start_time.elapsed()
                    );
                    return Ok(result);
                }
                Err(e) => {
                    self.metrics.record_redis_operation(
                        &format!("replica_{}", i), 
                        "get", 
                        "error", 
                        start_time.elapsed()
                    );
                    tracing::warn!("Replica {} error: {}", i, e);
                    continue;
                }
            }
        }
        
        Err(RedisError::AllNodesDown)
    }
    
    // Task 14: Atomic set with TTL
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Duration) -> Result<()>
    where T: Serialize
    {
        let start_time = Instant::now();
        let serialized = self.serializer.serialize(value)?;
        
        let mut conn = self.connection_pool.get().await?;
        
        // Use SETEX for atomic set with TTL
        let result: Result<(), redis::RedisError> = conn.set_ex(
            key, 
            serialized, 
            ttl.as_secs() as usize
        ).await;
        
        match result {
            Ok(()) => {
                self.metrics.record_redis_operation("primary", "set", "success", start_time.elapsed());
                Ok(())
            }
            Err(e) => {
                self.metrics.record_redis_operation("primary", "set", "error", start_time.elapsed());
                Err(e.into())
            }
        }
    }
    
    // Task 14: Pattern-based invalidation
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<u64> {
        let start_time = Instant::now();
        
        // Use SCAN to find matching keys (safer than KEYS)
        let mut conn = self.connection_pool.get().await?;
        let mut cursor = 0;
        let mut total_deleted = 0;
        
        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(1000)
                .query_async(&mut conn)
                .await?;
            
            if !keys.is_empty() {
                let deleted: u64 = conn.del(&keys).await?;
                total_deleted += deleted;
            }
            
            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }
        
        self.metrics.record_redis_operation(
            "primary", 
            "invalidate_pattern", 
            "success", 
            start_time.elapsed()
        );
        self.metrics.record_cache_invalidation(pattern, total_deleted);
        
        Ok(total_deleted)
    }
}
```

### 5. Circuit Breaker - Fault Tolerance

```rust
// Фактическая реализация: src/performance/cache/circuit_breaker.rs
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
    success_threshold: u32,
    metrics: Arc<MetricsCollector>,
}

#[derive(Debug, Clone)]
enum CircuitState {
    Closed { failure_count: u32 },
    Open { opened_at: Instant },
    HalfOpen { success_count: u32 },
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed { failure_count: 0 })),
            failure_threshold: config.failure_threshold,
            recovery_timeout: config.recovery_timeout,
            success_threshold: config.success_threshold,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }
    
    // Task 14: Protected operation execution
    pub async fn call<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> BoxFuture<'static, Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        // Check current state
        let can_execute = {
            let mut state = self.state.lock().await;
            match &*state {
                CircuitState::Closed { .. } => true,
                CircuitState::Open { opened_at } => {
                    if opened_at.elapsed() >= self.recovery_timeout {
                        // Transition to half-open
                        *state = CircuitState::HalfOpen { success_count: 0 };
                        self.metrics.record_circuit_breaker_state_change("half_open");
                        true
                    } else {
                        false
                    }
                }
                CircuitState::HalfOpen { .. } => true,
            }
        };
        
        if !can_execute {
            self.metrics.record_circuit_breaker_rejection();
            return Err(CircuitBreakerError::CircuitOpen);
        }
        
        // Execute operation
        let start_time = Instant::now();
        match operation().await {
            Ok(result) => {
                self.on_success().await;
                self.metrics.record_circuit_breaker_success(start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                self.metrics.record_circuit_breaker_failure(start_time.elapsed());
                Err(CircuitBreakerError::OperationFailed(e))
            }
        }
    }
    
    // Task 14: State management on success
    async fn on_success(&self) {
        let mut state = self.state.lock().await;
        match &*state {
            CircuitState::Closed { .. } => {
                // Reset failure count on success
                *state = CircuitState::Closed { failure_count: 0 };
            }
            CircuitState::HalfOpen { success_count } => {
                let new_success_count = success_count + 1;
                if new_success_count >= self.success_threshold {
                    // Transition back to closed
                    *state = CircuitState::Closed { failure_count: 0 };
                    self.metrics.record_circuit_breaker_state_change("closed");
                } else {
                    *state = CircuitState::HalfOpen { success_count: new_success_count };
                }
            }
            CircuitState::Open { .. } => {
                // Should not happen, but handle gracefully
                *state = CircuitState::Closed { failure_count: 0 };
            }
        }
    }
    
    // Task 14: State management on failure
    async fn on_failure(&self) {
        let mut state = self.state.lock().await;
        match &*state {
            CircuitState::Closed { failure_count } => {
                let new_failure_count = failure_count + 1;
                if new_failure_count >= self.failure_threshold {
                    // Transition to open
                    *state = CircuitState::Open { opened_at: Instant::now() };
                    self.metrics.record_circuit_breaker_state_change("open");
                } else {
                    *state = CircuitState::Closed { failure_count: new_failure_count };
                }
            }
            CircuitState::HalfOpen { .. } => {
                // Transition back to open on any failure
                *state = CircuitState::Open { opened_at: Instant::now() };
                self.metrics.record_circuit_breaker_state_change("open");
            }
            CircuitState::Open { .. } => {
                // Already open, no change needed
            }
        }
    }
}
```

### 6. Cache Invalidation Service

```rust
// Фактическая реализация: src/performance/cache/invalidation.rs
pub struct CacheInvalidationService {
    l1_cache: Arc<MemoryCache>,
    l2_cache: Arc<RedisCache>,
    invalidation_queue: Arc<Mutex<VecDeque<InvalidationRequest>>>,
    metrics: Arc<MetricsCollector>,
}

#[derive(Debug, Clone)]
struct InvalidationRequest {
    pattern: String,
    timestamp: Instant,
    priority: InvalidationPriority,
}

impl CacheInvalidationService {
    pub fn new(l1_cache: Arc<MemoryCache>, l2_cache: Arc<RedisCache>) -> Self {
        let service = Self {
            l1_cache,
            l2_cache,
            invalidation_queue: Arc::new(Mutex::new(VecDeque::new())),
            metrics: Arc::new(MetricsCollector::new()),
        };
        
        // Task 14: Background invalidation processing
        service.start_invalidation_processor();
        
        service
    }
    
    // Task 14: Smart invalidation with dependency tracking
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<InvalidationResult> {
        let start_time = Instant::now();
        
        // Task 14: Parallel invalidation of both cache levels
        let (l1_result, l2_result) = tokio::join!(
            self.invalidate_l1_pattern(pattern),
            self.invalidate_l2_pattern(pattern)
        );
        
        let l1_count = l1_result.unwrap_or(0);
        let l2_count = l2_result.unwrap_or(0);
        
        // Task 14: Invalidate dependent patterns
        self.invalidate_dependent_patterns(pattern).await?;
        
        let result = InvalidationResult {
            pattern: pattern.to_string(),
            l1_invalidated: l1_count,
            l2_invalidated: l2_count,
            duration: start_time.elapsed(),
        };
        
        self.metrics.record_cache_invalidation_result(&result);
        Ok(result)
    }
    
    // Task 14: Dependency-based invalidation
    async fn invalidate_dependent_patterns(&self, pattern: &str) -> Result<()> {
        let dependent_patterns = self.get_dependent_patterns(pattern);
        
        for dependent_pattern in dependent_patterns {
            let request = InvalidationRequest {
                pattern: dependent_pattern,
                timestamp: Instant::now(),
                priority: InvalidationPriority::Low,
            };
            
            let mut queue = self.invalidation_queue.lock().await;
            queue.push_back(request);
        }
        
        Ok(())
    }
    
    // Task 14: Pattern dependency mapping
    fn get_dependent_patterns(&self, pattern: &str) -> Vec<String> {
        let mut dependent = Vec::new();
        
        // Example: invalidating reviews also invalidates ratings
        if pattern.starts_with("reviews:offer:") {
            if let Some(offer_id) = self.extract_offer_id_from_pattern(pattern) {
                dependent.push(format!("rating:offer:{}", offer_id));
                dependent.push(format!("offer_stats:{}", offer_id));
            }
        }
        
        // Example: invalidating user data affects user-specific caches
        if pattern.starts_with("user:") {
            if let Some(user_id) = self.extract_user_id_from_pattern(pattern) {
                dependent.push(format!("reviews:author:{}", user_id));
                dependent.push(format!("user_activity:{}", user_id));
            }
        }
        
        dependent
    }
}
```

## 📊 Cache Performance Metrics

```rust
// Фактическая реализация: Cache metrics collection
impl CacheMetricsCollector {
    pub async fn collect_cache_stats(&self) -> CacheStats {
        let (l1_stats, l2_stats) = tokio::join!(
            self.collect_l1_stats(),
            self.collect_l2_stats()
        );
        
        CacheStats {
            l1: l1_stats,
            l2: l2_stats,
            overall_hit_ratio: self.calculate_overall_hit_ratio(&l1_stats, &l2_stats),
            memory_usage: self.calculate_memory_usage(&l1_stats, &l2_stats),
            performance_score: self.calculate_performance_score(&l1_stats, &l2_stats),
        }
    }
    
    // Task 14: L1 cache statistics
    async fn collect_l1_stats(&self) -> L1CacheStats {
        L1CacheStats {
            total_requests: self.l1_cache.get_total_requests(),
            hits: self.l1_cache.get_hits(),
            misses: self.l1_cache.get_misses(),
            hit_ratio: self.l1_cache.get_hit_ratio(),
            memory_usage_bytes: self.l1_cache.get_memory_usage(),
            entry_count: self.l1_cache.get_entry_count(),
            evictions: self.l1_cache.get_evictions(),
            average_access_time: self.l1_cache.get_average_access_time(),
        }
    }
    
    // Task 14: L2 cache statistics
    async fn collect_l2_stats(&self) -> L2CacheStats {
        L2CacheStats {
            total_requests: self.l2_cache.get_total_requests(),
            hits: self.l2_cache.get_hits(),
            misses: self.l2_cache.get_misses(),
            hit_ratio: self.l2_cache.get_hit_ratio(),
            connection_pool_size: self.l2_cache.get_pool_size(),
            active_connections: self.l2_cache.get_active_connections(),
            network_latency: self.l2_cache.get_average_latency(),
            circuit_breaker_state: self.l2_cache.get_circuit_breaker_state(),
        }
    }
}
```

## 🔗 Связь с реализацией

Эта Cache Architecture напрямую реализована в:

- **`src/performance/cache/`** - Все компоненты кеширования
- **`src/performance/cache/manager.rs`** - Central cache coordination
- **`src/performance/cache/memory_cache.rs`** - L1 cache implementation
- **`src/performance/cache/redis_cache.rs`** - L2 cache implementation
- **`src/performance/cache/circuit_breaker.rs`** - Fault tolerance
- **`src/performance/cache/invalidation.rs`** - Cache invalidation logic

Диаграмма служит **caching blueprint**, показывая как Task 14 реализует enterprise-grade multi-level caching с fault tolerance и intelligent invalidation.