# Task 9: Container Diagram - Подробное объяснение контейнерной архитектуры оптимизации производительности

## 🎯 Цель диаграммы

Container диаграмма Task 9 детализирует **внутреннюю архитектуру системы оптимизации производительности** на уровне контейнеров, показывая как различные слои кеширования, DataLoader оптимизации и rate limiting взаимодействуют друг с другом для обеспечения высокой производительности GraphQL федерации.

## 🏗️ Архитектурные слои оптимизации производительности

### 1. Caching Layer - Слой кеширования

#### Redis Cache - Distributed кеширование
```rust
// ugc-subgraph/src/cache/redis_cache.rs
use redis::{Client, Commands, Connection, RedisResult};
use serde::{Serialize, Deserialize};
use std::time::Duration;

#[derive(Clone)]
pub struct RedisCache {
    client: Client,
    config: RedisCacheConfig,
    serializer: Arc<CacheSerializer>,
    connection_pool: Arc<Mutex<Vec<Connection>>>,
}

#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    pub cluster_mode: bool,
    pub max_connections: usize,
    pub default_ttl: Duration,
    pub compression_enabled: bool,
    pub key_prefix: String,
}

impl RedisCache {
    pub async fn new(redis_url: &str, config: RedisCacheConfig) -> Result<Self, CacheError> {
        let client = if config.cluster_mode {
            // Redis Cluster configuration
            Client::open_cluster(vec![redis_url])?
        } else {
            // Single Redis instance
            Client::open(redis_url)?
        };
        
        // Test connection
        let mut conn = client.get_connection()?;
        let _: String = conn.ping()?;
        
        // Initialize connection pool
        let mut pool = Vec::with_capacity(config.max_connections);
        for _ in 0..config.max_connections {
            pool.push(client.get_connection()?);
        }
        
        Ok(Self {
            client,
            config,
            serializer: Arc::new(CacheSerializer::new(config.compression_enabled)),
            connection_pool: Arc::new(Mutex::new(pool)),
        })
    }

    /// Get value from cache with automatic deserialization
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let full_key = self.build_key(key);
        let mut conn = self.get_connection().await?;
        
        let start_time = std::time::Instant::now();
        
        let data: Option<Vec<u8>> = conn.get(&full_key)?;
        
        // Record cache operation metrics
        let operation_time = start_time.elapsed();
        CACHE_OPERATION_DURATION
            .with_label_values(&["get"])
            .observe(operation_time.as_secs_f64());
        
        match data {
            Some(bytes) => {
                let value = self.serializer.deserialize::<T>(&bytes)?;
                
                CACHE_HITS_TOTAL
                    .with_label_values(&[&self.extract_cache_type(key)])
                    .inc();
                
                tracing::debug!(
                    key = %full_key,
                    size_bytes = bytes.len(),
                    operation_time_ms = operation_time.as_millis(),
                    "Cache hit"
                );
                
                Ok(Some(value))
            }
            None => {
                CACHE_MISSES_TOTAL
                    .with_label_values(&[&self.extract_cache_type(key)])
                    .inc();
                
                tracing::debug!(
                    key = %full_key,
                    operation_time_ms = operation_time.as_millis(),
                    "Cache miss"
                );
                
                Ok(None)
            }
        }
    }

    /// Set value in cache with TTL
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let full_key = self.build_key(key);
        let bytes = self.serializer.serialize(value)?;
        let mut conn = self.get_connection().await?;
        
        let start_time = std::time::Instant::now();
        let ttl_seconds = ttl.unwrap_or(self.config.default_ttl).as_secs();
        
        let result: RedisResult<()> = conn.setex(&full_key, ttl_seconds, &bytes);
        
        let operation_time = start_time.elapsed();
        CACHE_OPERATION_DURATION
            .with_label_values(&["set"])
            .observe(operation_time.as_secs_f64());
        
        match result {
            Ok(_) => {
                CACHE_SETS_TOTAL
                    .with_label_values(&[&self.extract_cache_type(key)])
                    .inc();
                
                tracing::debug!(
                    key = %full_key,
                    size_bytes = bytes.len(),
                    ttl_seconds = ttl_seconds,
                    operation_time_ms = operation_time.as_millis(),
                    "Cache set successful"
                );
                
                Ok(())
            }
            Err(e) => {
                CACHE_ERRORS_TOTAL
                    .with_label_values(&["set", &e.to_string()])
                    .inc();
                
                tracing::error!(
                    key = %full_key,
                    error = %e,
                    operation_time_ms = operation_time.as_millis(),
                    "Cache set failed"
                );
                
                Err(CacheError::RedisError(e))
            }
        }
    }

    /// Delete key from cache
    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let full_key = self.build_key(key);
        let mut conn = self.get_connection().await?;
        
        let deleted: i32 = conn.del(&full_key)?;
        
        CACHE_DELETES_TOTAL
            .with_label_values(&[&self.extract_cache_type(key)])
            .inc();
        
        Ok(deleted > 0)
    }

    /// Batch delete keys by pattern
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u32, CacheError> {
        let full_pattern = self.build_key(pattern);
        let mut conn = self.get_connection().await?;
        
        // Scan for keys matching pattern
        let keys: Vec<String> = conn.keys(&full_pattern)?;
        
        if keys.is_empty() {
            return Ok(0);
        }
        
        // Delete in batches to avoid blocking Redis
        let batch_size = 100;
        let mut deleted_count = 0;
        
        for chunk in keys.chunks(batch_size) {
            let deleted: i32 = conn.del(chunk)?;
            deleted_count += deleted as u32;
        }
        
        CACHE_PATTERN_DELETES_TOTAL.inc();
        
        tracing::info!(
            pattern = %full_pattern,
            deleted_count = deleted_count,
            "Pattern delete completed"
        );
        
        Ok(deleted_count)
    }

    /// Get connection from pool
    async fn get_connection(&self) -> Result<Connection, CacheError> {
        // Try to get from pool first
        if let Ok(mut pool) = self.connection_pool.try_lock() {
            if let Some(conn) = pool.pop() {
                return Ok(conn);
            }
        }
        
        // Create new connection if pool is empty
        self.client.get_connection()
            .map_err(CacheError::RedisError)
    }

    /// Return connection to pool
    async fn return_connection(&self, conn: Connection) {
        if let Ok(mut pool) = self.connection_pool.try_lock() {
            if pool.len() < self.config.max_connections {
                pool.push(conn);
            }
        }
    }

    fn build_key(&self, key: &str) -> String {
        format!("{}:{}", self.config.key_prefix, key)
    }

    fn extract_cache_type(&self, key: &str) -> String {
        key.split(':').next().unwrap_or("unknown").to_string()
    }
}
```#### Cache 
Manager - Управление кешем
```rust
// ugc-subgraph/src/cache/cache_manager.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CacheManager {
    redis_cache: Arc<RedisCache>,
    local_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    config: CacheManagerConfig,
    invalidation_rules: Arc<RwLock<HashMap<String, Vec<InvalidationRule>>>>,
}

#[derive(Debug, Clone)]
pub struct CacheManagerConfig {
    pub local_cache_size: usize,
    pub local_cache_ttl: Duration,
    pub write_through: bool,
    pub write_behind: bool,
    pub cache_warming_enabled: bool,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    data: Vec<u8>,
    created_at: std::time::Instant,
    ttl: Duration,
    access_count: u64,
}

#[derive(Debug, Clone)]
pub struct InvalidationRule {
    pub pattern: String,
    pub cascade: bool,
    pub delay: Option<Duration>,
}

impl CacheManager {
    pub fn new(redis_cache: Arc<RedisCache>, config: CacheManagerConfig) -> Self {
        Self {
            redis_cache,
            local_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            invalidation_rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Multi-level cache get (L1: local, L2: Redis)
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de> + Clone,
    {
        let start_time = std::time::Instant::now();
        
        // L1 Cache: Check local cache first
        if let Some(value) = self.get_from_local_cache::<T>(key).await? {
            CACHE_HITS_TOTAL
                .with_label_values(&["local", &self.extract_cache_type(key)])
                .inc();
            
            let operation_time = start_time.elapsed();
            CACHE_OPERATION_DURATION
                .with_label_values(&["get_local"])
                .observe(operation_time.as_secs_f64());
            
            tracing::debug!(
                key = %key,
                cache_level = "L1",
                operation_time_ms = operation_time.as_millis(),
                "Local cache hit"
            );
            
            return Ok(Some(value));
        }
        
        // L2 Cache: Check Redis cache
        if let Some(value) = self.redis_cache.get::<T>(key).await? {
            // Store in local cache for future requests
            self.set_local_cache(key, &value).await?;
            
            CACHE_HITS_TOTAL
                .with_label_values(&["redis", &self.extract_cache_type(key)])
                .inc();
            
            let operation_time = start_time.elapsed();
            CACHE_OPERATION_DURATION
                .with_label_values(&["get_redis"])
                .observe(operation_time.as_secs_f64());
            
            tracing::debug!(
                key = %key,
                cache_level = "L2",
                operation_time_ms = operation_time.as_millis(),
                "Redis cache hit, promoted to local cache"
            );
            
            return Ok(Some(value));
        }
        
        // Cache miss on both levels
        CACHE_MISSES_TOTAL
            .with_label_values(&["all", &self.extract_cache_type(key)])
            .inc();
        
        let operation_time = start_time.elapsed();
        tracing::debug!(
            key = %key,
            operation_time_ms = operation_time.as_millis(),
            "Cache miss on all levels"
        );
        
        Ok(None)
    }

    /// Multi-level cache set
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>
    where
        T: Serialize + Clone,
    {
        let start_time = std::time::Instant::now();
        
        // Set in local cache
        self.set_local_cache(key, value).await?;
        
        // Set in Redis cache
        if self.config.write_through {
            // Write-through: synchronous write to Redis
            self.redis_cache.set(key, value, ttl).await?;
        } else if self.config.write_behind {
            // Write-behind: asynchronous write to Redis
            let redis_cache = self.redis_cache.clone();
            let key = key.to_string();
            let value = value.clone();
            
            tokio::spawn(async move {
                if let Err(e) = redis_cache.set(&key, &value, ttl).await {
                    tracing::error!(
                        key = %key,
                        error = %e,
                        "Write-behind cache set failed"
                    );
                }
            });
        }
        
        let operation_time = start_time.elapsed();
        CACHE_OPERATION_DURATION
            .with_label_values(&["set_multilevel"])
            .observe(operation_time.as_secs_f64());
        
        tracing::debug!(
            key = %key,
            write_mode = if self.config.write_through { "write_through" } else { "write_behind" },
            operation_time_ms = operation_time.as_millis(),
            "Multi-level cache set completed"
        );
        
        Ok(())
    }

    /// Cache invalidation with rules
    pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        let start_time = std::time::Instant::now();
        
        // Remove from local cache
        {
            let mut local_cache = self.local_cache.write().await;
            local_cache.remove(key);
        }
        
        // Remove from Redis cache
        self.redis_cache.delete(key).await?;
        
        // Apply invalidation rules
        self.apply_invalidation_rules(key).await?;
        
        let operation_time = start_time.elapsed();
        CACHE_INVALIDATIONS_TOTAL
            .with_label_values(&[&self.extract_cache_type(key)])
            .inc();
        
        tracing::info!(
            key = %key,
            operation_time_ms = operation_time.as_millis(),
            "Cache invalidation completed"
        );
        
        Ok(())
    }

    /// Get from local cache
    async fn get_from_local_cache<T>(&self, key: &str) -> Result<Option<T>, CacheError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let local_cache = self.local_cache.read().await;
        
        if let Some(entry) = local_cache.get(key) {
            // Check if entry is still valid
            if entry.created_at.elapsed() < entry.ttl {
                let value = serde_json::from_slice::<T>(&entry.data)
                    .map_err(|e| CacheError::SerializationError(e.to_string()))?;
                
                return Ok(Some(value));
            }
        }
        
        Ok(None)
    }

    /// Set in local cache
    async fn set_local_cache<T>(&self, key: &str, value: &T) -> Result<(), CacheError>
    where
        T: Serialize,
    {
        let data = serde_json::to_vec(value)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        
        let entry = CacheEntry {
            data,
            created_at: std::time::Instant::now(),
            ttl: self.config.local_cache_ttl,
            access_count: 1,
        };
        
        let mut local_cache = self.local_cache.write().await;
        
        // Evict old entries if cache is full
        if local_cache.len() >= self.config.local_cache_size {
            self.evict_lru_entries(&mut local_cache).await;
        }
        
        local_cache.insert(key.to_string(), entry);
        
        Ok(())
    }

    /// Apply invalidation rules
    async fn apply_invalidation_rules(&self, key: &str) -> Result<(), CacheError> {
        let rules = self.invalidation_rules.read().await;
        
        for (pattern, rule_list) in rules.iter() {
            if self.key_matches_pattern(key, pattern) {
                for rule in rule_list {
                    if rule.cascade {
                        // Apply delay if specified
                        if let Some(delay) = rule.delay {
                            tokio::time::sleep(delay).await;
                        }
                        
                        // Invalidate by pattern
                        self.redis_cache.delete_pattern(&rule.pattern).await?;
                        
                        tracing::debug!(
                            original_key = %key,
                            cascade_pattern = %rule.pattern,
                            "Applied cascade invalidation rule"
                        );
                    }
                }
            }
        }
        
        Ok(())
    }

    /// LRU eviction for local cache
    async fn evict_lru_entries(&self, cache: &mut HashMap<String, CacheEntry>) {
        // Find entries to evict (oldest and least accessed)
        let mut entries: Vec<_> = cache.iter().collect();
        entries.sort_by(|a, b| {
            a.1.access_count.cmp(&b.1.access_count)
                .then(a.1.created_at.cmp(&b.1.created_at))
        });
        
        // Remove 25% of entries
        let evict_count = cache.len() / 4;
        for (key, _) in entries.iter().take(evict_count) {
            cache.remove(*key);
        }
        
        CACHE_EVICTIONS_TOTAL
            .with_label_values(&["local"])
            .add(evict_count as u64);
        
        tracing::debug!(
            evicted_count = evict_count,
            remaining_count = cache.len(),
            "Local cache LRU eviction completed"
        );
    }

    fn key_matches_pattern(&self, key: &str, pattern: &str) -> bool {
        // Simple pattern matching (can be enhanced with regex)
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            key.starts_with(prefix)
        } else {
            key == pattern
        }
    }

    fn extract_cache_type(&self, key: &str) -> String {
        key.split(':').next().unwrap_or("unknown").to_string()
    }
}
```

#### Cache Invalidator - Инвалидация кеша
```rust
// ugc-subgraph/src/cache/cache_invalidator.rs
use tokio::sync::mpsc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum InvalidationEvent {
    EntityChanged {
        entity_type: String,
        entity_id: Uuid,
        change_type: ChangeType,
    },
    PatternInvalidation {
        pattern: String,
        reason: String,
    },
    ScheduledInvalidation {
        keys: Vec<String>,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    },
}

#[derive(Debug, Clone)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

pub struct CacheInvalidator {
    cache_manager: Arc<CacheManager>,
    event_receiver: mpsc::UnboundedReceiver<InvalidationEvent>,
    event_sender: mpsc::UnboundedSender<InvalidationEvent>,
    invalidation_rules: HashMap<String, Vec<InvalidationRule>>,
}

impl CacheInvalidator {
    pub fn new(cache_manager: Arc<CacheManager>) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let mut invalidation_rules = HashMap::new();
        
        // Define invalidation rules for different entity types
        invalidation_rules.insert("review".to_string(), vec![
            InvalidationRule {
                pattern: "reviews:*".to_string(),
                cascade: true,
                delay: None,
            },
            InvalidationRule {
                pattern: "aggregations:review:*".to_string(),
                cascade: true,
                delay: Some(Duration::from_secs(1)),
            },
            InvalidationRule {
                pattern: "query:reviews_by_offer:*".to_string(),
                cascade: true,
                delay: None,
            },
        ]);
        
        invalidation_rules.insert("offer".to_string(), vec![
            InvalidationRule {
                pattern: "offers:*".to_string(),
                cascade: true,
                delay: None,
            },
            InvalidationRule {
                pattern: "aggregations:offer:*".to_string(),
                cascade: true,
                delay: Some(Duration::from_secs(2)),
            },
        ]);
        
        Self {
            cache_manager,
            event_receiver,
            event_sender,
            invalidation_rules,
        }
    }

    /// Start the invalidation event processor
    pub async fn start_processing(&mut self) {
        tracing::info!("Starting cache invalidation event processor");
        
        while let Some(event) = self.event_receiver.recv().await {
            if let Err(e) = self.process_invalidation_event(event).await {
                tracing::error!(
                    error = %e,
                    "Failed to process invalidation event"
                );
            }
        }
    }

    /// Send invalidation event
    pub fn invalidate(&self, event: InvalidationEvent) -> Result<(), CacheError> {
        self.event_sender.send(event)
            .map_err(|e| CacheError::InvalidationError(e.to_string()))?;
        Ok(())
    }

    /// Process invalidation event
    async fn process_invalidation_event(&self, event: InvalidationEvent) -> Result<(), CacheError> {
        let start_time = std::time::Instant::now();
        
        match event {
            InvalidationEvent::EntityChanged { entity_type, entity_id, change_type } => {
                self.handle_entity_change(&entity_type, entity_id, change_type).await?;
            }
            InvalidationEvent::PatternInvalidation { pattern, reason } => {
                self.handle_pattern_invalidation(&pattern, &reason).await?;
            }
            InvalidationEvent::ScheduledInvalidation { keys, scheduled_at } => {
                self.handle_scheduled_invalidation(keys, scheduled_at).await?;
            }
        }
        
        let processing_time = start_time.elapsed();
        INVALIDATION_PROCESSING_TIME
            .observe(processing_time.as_secs_f64());
        
        Ok(())
    }

    /// Handle entity change invalidation
    async fn handle_entity_change(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        change_type: ChangeType,
    ) -> Result<(), CacheError> {
        tracing::info!(
            entity_type = %entity_type,
            entity_id = %entity_id,
            change_type = ?change_type,
            "Processing entity change invalidation"
        );
        
        // Direct entity cache invalidation
        let entity_key = format!("{}:{}", entity_type, entity_id);
        self.cache_manager.invalidate(&entity_key).await?;
        
        // Apply entity-specific invalidation rules
        if let Some(rules) = self.invalidation_rules.get(entity_type) {
            for rule in rules {
                let pattern = rule.pattern.replace("*", &entity_id.to_string());
                
                if let Some(delay) = rule.delay {
                    tokio::time::sleep(delay).await;
                }
                
                self.cache_manager.redis_cache.delete_pattern(&pattern).await?;
                
                tracing::debug!(
                    entity_type = %entity_type,
                    entity_id = %entity_id,
                    pattern = %pattern,
                    "Applied invalidation rule"
                );
            }
        }
        
        // Special handling for different change types
        match change_type {
            ChangeType::Created => {
                // Invalidate list caches that might include this new entity
                let list_pattern = format!("query:{}s_*", entity_type);
                self.cache_manager.redis_cache.delete_pattern(&list_pattern).await?;
            }
            ChangeType::Updated => {
                // Invalidate aggregation caches
                let agg_pattern = format!("aggregations:{}:{}:*", entity_type, entity_id);
                self.cache_manager.redis_cache.delete_pattern(&agg_pattern).await?;
            }
            ChangeType::Deleted => {
                // Full invalidation for deletions
                let full_pattern = format!("*:{}:{}*", entity_type, entity_id);
                self.cache_manager.redis_cache.delete_pattern(&full_pattern).await?;
            }
        }
        
        ENTITY_INVALIDATIONS_TOTAL
            .with_label_values(&[entity_type, &format!("{:?}", change_type)])
            .inc();
        
        Ok(())
    }

    /// Handle pattern-based invalidation
    async fn handle_pattern_invalidation(&self, pattern: &str, reason: &str) -> Result<(), CacheError> {
        tracing::info!(
            pattern = %pattern,
            reason = %reason,
            "Processing pattern invalidation"
        );
        
        let deleted_count = self.cache_manager.redis_cache.delete_pattern(pattern).await?;
        
        PATTERN_INVALIDATIONS_TOTAL
            .with_label_values(&[reason])
            .inc();
        
        tracing::info!(
            pattern = %pattern,
            deleted_count = deleted_count,
            reason = %reason,
            "Pattern invalidation completed"
        );
        
        Ok(())
    }

    /// Handle scheduled invalidation
    async fn handle_scheduled_invalidation(
        &self,
        keys: Vec<String>,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), CacheError> {
        let now = chrono::Utc::now();
        
        if scheduled_at > now {
            let delay = (scheduled_at - now).to_std()
                .unwrap_or(Duration::from_secs(0));
            
            tracing::info!(
                keys_count = keys.len(),
                delay_seconds = delay.as_secs(),
                "Scheduling invalidation"
            );
            
            tokio::time::sleep(delay).await;
        }
        
        for key in keys {
            self.cache_manager.invalidate(&key).await?;
        }
        
        SCHEDULED_INVALIDATIONS_TOTAL.inc();
        
        Ok(())
    }
}
```

### 2. Performance Optimization Layer - Слой оптимизации производительности

#### DataLoader Service - Сервис DataLoader
```rust
// ugc-subgraph/src/dataloader/dataloader_service.rs
use async_graphql::dataloader::{DataLoader, Loader};
use std::collections::HashMap;
use std::sync::Arc;

pub struct DataLoaderService {
    review_loader: DataLoader<ReviewDataLoader>,
    user_loader: DataLoader<UserDataLoader>,
    offer_loader: DataLoader<OfferDataLoader>,
    aggregation_loader: DataLoader<AggregationDataLoader>,
}

impl DataLoaderService {
    pub fn new(
        db_pool: PgPool,
        cache_manager: Arc<CacheManager>,
        config: DataLoaderConfig,
    ) -> Self {
        Self {
            review_loader: DataLoader::new(
                ReviewDataLoader::new(db_pool.clone(), cache_manager.clone()),
                tokio::spawn
            ).with_delay(config.batch_delay)
             .with_max_batch_size(config.max_batch_size),
            
            user_loader: DataLoader::new(
                UserDataLoader::new(db_pool.clone(), cache_manager.clone()),
                tokio::spawn
            ).with_delay(config.batch_delay)
             .with_max_batch_size(config.max_batch_size),
            
            offer_loader: DataLoader::new(
                OfferDataLoader::new(db_pool.clone(), cache_manager.clone()),
                tokio::spawn
            ).with_delay(config.batch_delay)
             .with_max_batch_size(config.max_batch_size),
            
            aggregation_loader: DataLoader::new(
                AggregationDataLoader::new(db_pool.clone(), cache_manager.clone()),
                tokio::spawn
            ).with_delay(config.batch_delay)
             .with_max_batch_size(config.max_batch_size),
        }
    }

    pub fn review_loader(&self) -> &DataLoader<ReviewDataLoader> {
        &self.review_loader
    }

    pub fn user_loader(&self) -> &DataLoader<UserDataLoader> {
        &self.user_loader
    }

    pub fn offer_loader(&self) -> &DataLoader<OfferDataLoader> {
        &self.offer_loader
    }

    pub fn aggregation_loader(&self) -> &DataLoader<AggregationDataLoader> {
        &self.aggregation_loader
    }
}

#[derive(Debug, Clone)]
pub struct DataLoaderConfig {
    pub max_batch_size: usize,
    pub batch_delay: Duration,
    pub cache_enabled: bool,
    pub cache_ttl: Duration,
}

impl Default for DataLoaderConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            batch_delay: Duration::from_millis(10),
            cache_enabled: true,
            cache_ttl: Duration::from_secs(300),
        }
    }
}

/// Review DataLoader implementation
pub struct ReviewDataLoader {
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
}

impl ReviewDataLoader {
    pub fn new(db_pool: PgPool, cache_manager: Arc<CacheManager>) -> Self {
        Self {
            db_pool,
            cache_manager,
        }
    }
}

#[async_trait]
impl Loader<Uuid> for ReviewDataLoader {
    type Value = Review;
    type Error = DataLoaderError;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let start_time = std::time::Instant::now();
        let mut results = HashMap::new();
        let mut missing_keys = Vec::new();

        // Check cache for each key
        for &key in keys {
            let cache_key = format!("review:{}", key);
            
            if let Ok(Some(review)) = self.cache_manager.get::<Review>(&cache_key).await {
                results.insert(key, review);
            } else {
                missing_keys.push(key);
            }
        }

        // Batch load missing keys from database
        if !missing_keys.is_empty() {
            let reviews = sqlx::query_as::<_, Review>(
                "SELECT id, offer_id, author_id, content, rating, created_at, updated_at, is_moderated
                 FROM reviews 
                 WHERE id = ANY($1) AND deleted_at IS NULL"
            )
            .bind(&missing_keys)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| DataLoaderError::DatabaseError(e.to_string()))?;

            // Cache loaded reviews and add to results
            for review in reviews {
                let cache_key = format!("review:{}", review.id);
                
                // Cache with TTL
                if let Err(e) = self.cache_manager.set(
                    &cache_key, 
                    &review, 
                    Some(Duration::from_secs(600))
                ).await {
                    tracing::warn!(
                        error = %e,
                        review_id = %review.id,
                        "Failed to cache review"
                    );
                }
                
                results.insert(review.id, review);
            }
        }

        // Record DataLoader metrics
        let batch_time = start_time.elapsed();
        DATALOADER_BATCH_SIZE
            .with_label_values(&["review"])
            .observe(keys.len() as f64);
        
        DATALOADER_BATCH_TIME
            .with_label_values(&["review"])
            .observe(batch_time.as_secs_f64());
        
        DATALOADER_CACHE_HIT_RATE
            .with_label_values(&["review"])
            .observe((keys.len() - missing_keys.len()) as f64 / keys.len() as f64);

        tracing::debug!(
            total_keys = keys.len(),
            cached_keys = keys.len() - missing_keys.len(),
            db_keys = missing_keys.len(),
            batch_time_ms = batch_time.as_millis(),
            "Review DataLoader batch completed"
        );

        Ok(results)
    }
}
```

Эта Container диаграмма демонстрирует детальную архитектуру оптимизации производительности на уровне контейнеров, показывая как различные слои (Caching, Performance Optimization, Rate Limiting) работают вместе для обеспечения высокой производительности с comprehensive кешированием, N+1 оптимизацией и защитой от злоупотреблений.