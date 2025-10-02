# C4 Component Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `C4_Component_Diagram.puml`  
**Уровень:** Component (Level 3)  
**Цель:** Детальная архитектура компонентов внутри UGC Subgraph с фокусом на слои оптимизации производительности

## 🎯 Архитектурное назначение

Эта диаграмма показывает **внутреннюю структуру Task 14** на уровне компонентов, демонстрируя как различные слои системы взаимодействуют для достижения оптимальной производительности.

## 🏗️ Архитектурные слои

### 1. GraphQL Layer (Слой GraphQL)

#### 1.1 GraphQL Schema
```rust
// Фактическая реализация: src/graphql/schema.rs
use async_graphql::{Schema, Object, Context, Result, ComplexObject};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // Task 14: Каждый resolver интегрирован с performance layer
    async fn offers(&self, ctx: &Context<'_>, 
        #[graphql(desc = "Maximum number of offers")] limit: i32
    ) -> Result<Vec<Offer>> {
        // Получаем performance services из context
        let query_analyzer = ctx.data::<QueryComplexityAnalyzer>()?;
        let dataloader_manager = ctx.data::<DataLoaderManager>()?;
        
        // Task 14: Query complexity check
        query_analyzer.check_field_complexity("offers", limit as u32)?;
        
        // Task 14: DataLoader для предотвращения N+1
        let offer_loader = dataloader_manager.get_offer_loader();
        let offer_ids = self.get_offer_ids(limit).await?;
        
        offer_loader.load_many(offer_ids).await
    }
    
    async fn reviews(&self, ctx: &Context<'_>,
        offer_id: ID,
        first: Option<i32>
    ) -> Result<Vec<Review>> {
        let cache_manager = ctx.data::<CacheManager>()?;
        let cache_key = format!("reviews:offer:{}:first:{}", 
            offer_id, first.unwrap_or(10));
        
        // Task 14: Cache-first strategy
        cache_manager.get_or_compute(
            &cache_key,
            Duration::from_secs(600), // 10 minutes TTL
            || async {
                let dataloader_manager = ctx.data::<DataLoaderManager>()?;
                let review_loader = dataloader_manager.get_review_loader();
                review_loader.load_by_offer_id(offer_id.parse()?).await
            }
        ).await
    }
}

// Task 14: Complex object с performance optimizations
#[ComplexObject]
impl Offer {
    async fn reviews(&self, ctx: &Context<'_>) -> Result<Vec<Review>> {
        let dataloader_manager = ctx.data::<DataLoaderManager>()?;
        let review_loader = dataloader_manager.get_review_loader();
        
        // Task 14: Automatic batching через DataLoader
        review_loader.load_by_offer_id(self.id).await
    }
    
    async fn average_rating(&self, ctx: &Context<'_>) -> Result<f64> {
        let cache_manager = ctx.data::<CacheManager>()?;
        let cache_key = format!("rating:offer:{}", self.id);
        
        // Task 14: Агрессивное кеширование для expensive calculations
        cache_manager.get_or_compute(
            &cache_key,
            Duration::from_secs(1800), // 30 minutes TTL
            || async {
                let rating_service = ctx.data::<RatingService>()?;
                rating_service.calculate_average_rating(self.id).await
            }
        ).await
    }
}
```

#### 1.2 Query Resolver
```rust
// Фактическая реализация: src/graphql/query.rs
pub struct QueryResolver {
    review_service: Arc<ReviewService>,
    rating_service: Arc<RatingService>,
    metrics: Arc<MetricsCollector>,
}

impl QueryResolver {
    pub async fn resolve_reviews_field(&self, 
        ctx: &Context<'_>,
        offer_id: OfferId,
        pagination: PaginationArgs
    ) -> Result<ReviewConnection> {
        
        let start_time = Instant::now();
        
        // Task 14: Performance monitoring
        let _span = tracing::info_span!("resolve_reviews", 
            offer_id = %offer_id,
            first = pagination.first,
            after = ?pagination.after
        ).entered();
        
        // Task 14: Query complexity validation
        let query_analyzer = ctx.data::<QueryComplexityAnalyzer>()?;
        let complexity = query_analyzer.calculate_field_complexity(
            "reviews", 
            pagination.first.unwrap_or(10) as u32
        );
        
        if complexity > 100 {
            return Err(QueryTooComplexError::new(complexity));
        }
        
        // Task 14: Cache check
        let cache_manager = ctx.data::<CacheManager>()?;
        let cache_key = format!("reviews:{}:{}:{}", 
            offer_id, 
            pagination.first.unwrap_or(10),
            pagination.after.as_deref().unwrap_or("")
        );
        
        let result = cache_manager.get_or_compute(
            &cache_key,
            Duration::from_secs(300), // 5 minutes
            || async {
                // Task 14: DataLoader для batch loading
                let dataloader_manager = ctx.data::<DataLoaderManager>()?;
                let review_loader = dataloader_manager.get_review_loader();
                
                review_loader.load_paginated(offer_id, pagination).await
            }
        ).await?;
        
        // Task 14: Metrics collection
        self.metrics.record_query_duration(
            "reviews", 
            start_time.elapsed()
        );
        self.metrics.record_query_complexity("reviews", complexity);
        
        Ok(result)
    }
}
```

#### 1.3 Mutation Resolver
```rust
// Фактическая реализация: src/graphql/mutation.rs
pub struct MutationResolver {
    review_service: Arc<ReviewService>,
    auth_service: Arc<AuthService>,
    cache_manager: Arc<CacheManager>,
}

impl MutationResolver {
    pub async fn create_review(&self,
        ctx: &Context<'_>,
        input: CreateReviewInput
    ) -> Result<Review> {
        
        // Task 14: Rate limiting для mutations
        let rate_limiter = ctx.data::<RateLimitService>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        rate_limiter.check_mutation_rate_limit(
            user_context.user_id,
            "create_review"
        ).await?;
        
        // Authentication & Authorization
        self.auth_service.verify_user_can_review(
            user_context.user_id,
            input.offer_id
        ).await?;
        
        // Create review
        let review = self.review_service
            .create_review(input)
            .await?;
        
        // Task 14: Cache invalidation strategy
        self.invalidate_related_caches(review.offer_id).await?;
        
        Ok(review)
    }
    
    // Task 14: Intelligent cache invalidation
    async fn invalidate_related_caches(&self, offer_id: OfferId) -> Result<()> {
        let patterns = vec![
            format!("reviews:offer:{}", offer_id),
            format!("rating:offer:{}", offer_id),
            format!("reviews:offer:{}:*", offer_id), // Pagination caches
        ];
        
        for pattern in patterns {
            self.cache_manager.invalidate_pattern(&pattern).await?;
        }
        
        Ok(())
    }
}
```

### 2. Performance Layer (Слой производительности)

#### 2.1 DataLoader Manager
```rust
// Фактическая реализация: src/performance/dataloader_manager.rs
pub struct DataLoaderManager {
    loaders: HashMap<String, Box<dyn DataLoaderTrait>>,
    metrics: Arc<MetricsCollector>,
    config: DataLoaderConfig,
}

impl DataLoaderManager {
    pub fn new(
        db_pool: PgPool,
        external_clients: ExternalClients,
        config: DataLoaderConfig
    ) -> Self {
        let mut loaders: HashMap<String, Box<dyn DataLoaderTrait>> = HashMap::new();
        
        // Task 14: Регистрация всех DataLoader'ов
        loaders.insert("review".to_string(), Box::new(
            ReviewDataLoader::new(
                ReviewRepository::new(db_pool.clone()),
                config.clone()
            )
        ));
        
        loaders.insert("rating".to_string(), Box::new(
            RatingDataLoader::new(
                RatingRepository::new(db_pool.clone()),
                config.clone()
            )
        ));
        
        loaders.insert("offer".to_string(), Box::new(
            OfferDataLoader::new(
                external_clients.offers_client,
                config.clone()
            )
        ));
        
        Self {
            loaders,
            metrics: Arc::new(MetricsCollector::new()),
            config,
        }
    }
    
    // Task 14: Generic DataLoader access
    pub fn get_loader<T>(&self, loader_type: &str) -> Result<&T> 
    where T: DataLoaderTrait + 'static 
    {
        self.loaders.get(loader_type)
            .and_then(|loader| loader.as_any().downcast_ref::<T>())
            .ok_or_else(|| DataLoaderError::LoaderNotFound(loader_type.to_string()))
    }
    
    // Task 14: Batch execution coordination
    pub async fn execute_pending_batches(&self) -> Result<()> {
        let start_time = Instant::now();
        
        // Выполняем все pending batches параллельно
        let batch_futures: Vec<_> = self.loaders.values()
            .map(|loader| loader.execute_pending_batch())
            .collect();
        
        let results = futures::future::try_join_all(batch_futures).await?;
        
        // Task 14: Metrics для batch execution
        let total_batches = results.len();
        let total_items: usize = results.iter().sum();
        
        self.metrics.record_batch_execution(
            total_batches,
            total_items,
            start_time.elapsed()
        );
        
        Ok(())
    }
}
```

#### 2.2 Review DataLoader
```rust
// Фактическая реализация: src/performance/review_dataloader.rs
pub struct ReviewDataLoader {
    repository: Arc<ReviewRepository>,
    batch_queue: Arc<Mutex<Vec<BatchRequest<ReviewId, Review>>>>,
    cache: Arc<RwLock<HashMap<ReviewId, Review>>>,
    config: DataLoaderConfig,
    metrics: Arc<MetricsCollector>,
}

impl ReviewDataLoader {
    pub fn new(repository: ReviewRepository, config: DataLoaderConfig) -> Self {
        Self {
            repository: Arc::new(repository),
            batch_queue: Arc::new(Mutex::new(Vec::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }
    
    // Task 14: Load single review with batching
    pub async fn load(&self, review_id: ReviewId) -> Result<Review> {
        // Check request-scoped cache first
        {
            let cache = self.cache.read().await;
            if let Some(review) = cache.get(&review_id) {
                self.metrics.record_cache_hit("dataloader_memory");
                return Ok(review.clone());
            }
        }
        
        // Add to batch queue
        let (sender, receiver) = oneshot::channel();
        {
            let mut queue = self.batch_queue.lock().await;
            queue.push(BatchRequest {
                key: review_id,
                sender,
            });
            
            // Task 14: Trigger batch execution if queue is full
            if queue.len() >= self.config.max_batch_size {
                self.execute_batch().await?;
            }
        }
        
        // Wait for batch result or timeout
        tokio::select! {
            result = receiver => {
                result.map_err(|_| DataLoaderError::BatchTimeout)?
            }
            _ = tokio::time::sleep(self.config.batch_timeout) => {
                self.execute_batch().await?;
                // Retry after batch execution
                self.load_from_cache(review_id).await
            }
        }
    }
    
    // Task 14: Load multiple reviews efficiently
    pub async fn load_many(&self, review_ids: Vec<ReviewId>) -> Result<Vec<Review>> {
        let mut results = Vec::with_capacity(review_ids.len());
        let mut missing_ids = Vec::new();
        
        // Check cache for existing items
        {
            let cache = self.cache.read().await;
            for id in &review_ids {
                if let Some(review) = cache.get(id) {
                    results.push(review.clone());
                } else {
                    missing_ids.push(*id);
                }
            }
        }
        
        // Batch load missing items
        if !missing_ids.is_empty() {
            let loaded_reviews = self.repository
                .find_by_ids(missing_ids)
                .await?;
            
            // Update cache
            {
                let mut cache = self.cache.write().await;
                for review in &loaded_reviews {
                    cache.insert(review.id, review.clone());
                }
            }
            
            results.extend(loaded_reviews);
        }
        
        // Task 14: Metrics для batch efficiency
        self.metrics.record_dataloader_batch(
            "review",
            review_ids.len(),
            missing_ids.len()
        );
        
        Ok(results)
    }
    
    // Task 14: Specialized method для N+1 prevention
    pub async fn load_by_offer_id(&self, offer_id: OfferId) -> Result<Vec<Review>> {
        let cache_key = format!("reviews_by_offer:{}", offer_id);
        
        // Check if we have this offer's reviews cached
        if let Some(reviews) = self.get_offer_reviews_from_cache(offer_id).await? {
            return Ok(reviews);
        }
        
        // Load from repository
        let reviews = self.repository
            .find_by_offer_id(offer_id)
            .await?;
        
        // Cache individual reviews and the collection
        self.cache_offer_reviews(offer_id, &reviews).await?;
        
        Ok(reviews)
    }
    
    // Task 14: Batch execution implementation
    async fn execute_batch(&self) -> Result<()> {
        let batch_requests = {
            let mut queue = self.batch_queue.lock().await;
            std::mem::take(&mut *queue)
        };
        
        if batch_requests.is_empty() {
            return Ok(());
        }
        
        let review_ids: Vec<ReviewId> = batch_requests.iter()
            .map(|req| req.key)
            .collect();
        
        // Task 14: Single database query для всего batch
        let reviews = self.repository
            .find_by_ids(review_ids.clone())
            .await?;
        
        // Create lookup map
        let review_map: HashMap<ReviewId, Review> = reviews.into_iter()
            .map(|review| (review.id, review))
            .collect();
        
        // Update cache and send results
        {
            let mut cache = self.cache.write().await;
            for request in batch_requests {
                let review = review_map.get(&request.key).cloned();
                
                if let Some(ref review) = review {
                    cache.insert(request.key, review.clone());
                }
                
                let _ = request.sender.send(
                    review.ok_or_else(|| DataLoaderError::NotFound(request.key))
                );
            }
        }
        
        // Task 14: Batch metrics
        self.metrics.record_dataloader_batch_execution(
            "review",
            review_ids.len(),
            review_map.len()
        );
        
        Ok(())
    }
}
```

#### 2.3 Cache Manager
```rust
// Фактическая реализация: src/performance/cache_manager.rs
pub struct CacheManager {
    l1_cache: Arc<MemoryCache>,
    l2_cache: Arc<RedisCache>,
    circuit_breaker: Arc<CircuitBreaker>,
    invalidation_service: Arc<CacheInvalidationService>,
    metrics: Arc<MetricsCollector>,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> Result<Self> {
        let l1_cache = Arc::new(MemoryCache::new(config.l1_config));
        let l2_cache = Arc::new(RedisCache::new(config.l2_config).await?);
        let circuit_breaker = Arc::new(CircuitBreaker::new(config.circuit_breaker_config));
        
        Ok(Self {
            l1_cache,
            l2_cache: l2_cache.clone(),
            circuit_breaker,
            invalidation_service: Arc::new(
                CacheInvalidationService::new(l2_cache)
            ),
            metrics: Arc::new(MetricsCollector::new()),
        })
    }
    
    // Task 14: Multi-level cache get
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where T: DeserializeOwned + Clone + Send + 'static
    {
        let start_time = Instant::now();
        
        // L1 Cache (Memory) - fastest
        if let Some(value) = self.l1_cache.get::<T>(key).await? {
            self.metrics.record_cache_hit("l1", start_time.elapsed());
            return Ok(Some(value));
        }
        
        // L2 Cache (Redis) - with circuit breaker protection
        let l2_result = self.circuit_breaker.call(|| {
            let l2_cache = self.l2_cache.clone();
            let key = key.to_string();
            async move {
                l2_cache.get::<T>(&key).await
            }
        }).await;
        
        match l2_result {
            Ok(Some(value)) => {
                // Populate L1 cache
                self.l1_cache.set(key, &value, Duration::from_secs(300)).await?;
                self.metrics.record_cache_hit("l2", start_time.elapsed());
                Ok(Some(value))
            }
            Ok(None) => {
                self.metrics.record_cache_miss("all", start_time.elapsed());
                Ok(None)
            }
            Err(e) => {
                // Circuit breaker is open or Redis error
                self.metrics.record_cache_error("l2");
                tracing::warn!("L2 cache error: {}", e);
                Ok(None)
            }
        }
    }
    
    // Task 14: Multi-level cache set
    pub async fn set<T>(&self, 
        key: &str, 
        value: &T, 
        ttl: Duration
    ) -> Result<()>
    where T: Serialize + Clone + Send + Sync + 'static
    {
        // Always set L1 cache
        self.l1_cache.set(key, value, ttl).await?;
        
        // Set L2 cache with circuit breaker protection
        let l2_result = self.circuit_breaker.call(|| {
            let l2_cache = self.l2_cache.clone();
            let key = key.to_string();
            let value = value.clone();
            async move {
                l2_cache.set(&key, &value, ttl).await
            }
        }).await;
        
        if let Err(e) = l2_result {
            self.metrics.record_cache_error("l2");
            tracing::warn!("Failed to set L2 cache: {}", e);
            // Continue execution - L1 cache is still available
        }
        
        Ok(())
    }
    
    // Task 14: Intelligent cache warming
    pub async fn warm_cache(&self, patterns: Vec<String>) -> Result<()> {
        for pattern in patterns {
            let warming_future = self.warm_cache_pattern(&pattern);
            
            // Task 14: Non-blocking cache warming
            tokio::spawn(async move {
                if let Err(e) = warming_future.await {
                    tracing::warn!("Cache warming failed for pattern {}: {}", pattern, e);
                }
            });
        }
        
        Ok(())
    }
}
```

#### 2.4 Query Complexity Analyzer
```rust
// Фактическая реализação: src/performance/query_complexity.rs
pub struct QueryComplexityAnalyzer {
    max_depth: u32,
    max_complexity: u32,
    field_complexity_map: HashMap<String, u32>,
    user_limits: Arc<RwLock<HashMap<UserId, QueryLimits>>>,
    metrics: Arc<MetricsCollector>,
}

impl QueryComplexityAnalyzer {
    pub fn new(config: QueryLimitsConfig) -> Self {
        let mut field_complexity_map = HashMap::new();
        
        // Task 14: Field complexity configuration
        field_complexity_map.insert("Query.offers".to_string(), 5);
        field_complexity_map.insert("Query.reviews".to_string(), 10);
        field_complexity_map.insert("Offer.reviews".to_string(), 15);
        field_complexity_map.insert("Review.author".to_string(), 3);
        field_complexity_map.insert("Mutation.createReview".to_string(), 25);
        
        Self {
            max_depth: config.max_depth,
            max_complexity: config.max_complexity,
            field_complexity_map,
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
        }
    }
    
    // Task 14: Comprehensive query analysis
    pub async fn analyze_query(&self, 
        query: &str, 
        user_context: &UserContext
    ) -> Result<QueryAnalysisResult> {
        
        let start_time = Instant::now();
        
        // Parse GraphQL query
        let document = async_graphql_parser::parse_query(query)
            .map_err(|e| QueryAnalysisError::ParseError(e.to_string()))?;
        
        let mut analysis = QueryAnalysis::new();
        
        // Task 14: Depth analysis
        self.analyze_depth(&document, &mut analysis, 0)?;
        
        // Task 14: Complexity analysis
        self.analyze_complexity(&document, &mut analysis)?;
        
        // Task 14: User-specific limits
        let user_limits = self.get_user_limits(user_context.user_id).await;
        
        let result = QueryAnalysisResult {
            depth: analysis.depth,
            complexity: analysis.complexity,
            estimated_cost: self.estimate_execution_cost(&analysis),
            field_count: analysis.field_count,
            is_valid: self.validate_against_limits(&analysis, &user_limits),
            violations: analysis.violations,
            user_limits,
        };
        
        // Task 14: Analysis metrics
        self.metrics.record_query_analysis(
            result.depth,
            result.complexity,
            start_time.elapsed()
        );
        
        if !result.is_valid {
            self.metrics.record_query_rejection(&result.violations);
        }
        
        Ok(result)
    }
    
    // Task 14: Dynamic complexity calculation
    fn calculate_field_complexity(&self, 
        field: &async_graphql_parser::types::Field,
        parent_type: &str,
        variables: &Variables
    ) -> u32 {
        let field_key = format!("{}.{}", parent_type, field.name);
        let base_complexity = self.field_complexity_map
            .get(&field_key)
            .copied()
            .unwrap_or(1);
        
        // Task 14: Argument-based complexity multipliers
        let multiplier = self.calculate_argument_multiplier(field, variables);
        
        // Task 14: Selection set complexity
        let selection_complexity = field.selection_set.items.len() as u32;
        
        base_complexity * multiplier + selection_complexity
    }
    
    // Task 14: Argument multiplier calculation
    fn calculate_argument_multiplier(&self, 
        field: &async_graphql_parser::types::Field,
        variables: &Variables
    ) -> u32 {
        for (arg_name, arg_value) in &field.arguments {
            match arg_name.as_str() {
                "first" | "limit" | "last" => {
                    if let Some(value) = self.resolve_argument_value(arg_value, variables) {
                        if let Ok(num) = value.parse::<u32>() {
                            return num.min(100); // Cap at 100 to prevent abuse
                        }
                    }
                }
                _ => {}
            }
        }
        1 // Default multiplier
    }
    
    // Task 14: Cost estimation based on complexity
    fn estimate_execution_cost(&self, analysis: &QueryAnalysis) -> u32 {
        // Base cost from complexity
        let complexity_cost = analysis.complexity;
        
        // Depth penalty (exponential growth)
        let depth_penalty = if analysis.depth > 5 {
            2_u32.pow(analysis.depth - 5)
        } else {
            1
        };
        
        // Field count penalty
        let field_penalty = (analysis.field_count as f64 / 10.0).ceil() as u32;
        
        complexity_cost * depth_penalty + field_penalty
    }
}
```

#### 2.5 Rate Limit Service
```rust
// Фактическая реализация: src/performance/rate_limit.rs
pub struct RateLimitService {
    redis_client: Arc<RedisCache>,
    default_limits: RateLimitConfig,
    user_limits: Arc<RwLock<HashMap<UserId, UserRateLimits>>>,
    metrics: Arc<MetricsCollector>,
}

impl RateLimitService {
    pub fn new(redis_client: Arc<RedisCache>, config: RateLimitConfig) -> Self {
        Self {
            redis_client,
            default_limits: config,
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
        }
    }
    
    // Task 14: Comprehensive rate limiting
    pub async fn check_rate_limit(&self, 
        user_id: UserId,
        query_complexity: u32,
        operation_type: OperationType
    ) -> Result<RateLimitResult> {
        
        let user_limits = self.get_user_limits(user_id).await;
        let window_duration = Duration::from_secs(60); // 1 minute window
        let current_window = self.get_current_window();
        
        // Task 14: Multiple rate limit checks
        let checks = vec![
            self.check_requests_per_minute(user_id, current_window, &user_limits),
            self.check_complexity_per_minute(user_id, query_complexity, current_window, &user_limits),
            self.check_operation_type_limit(user_id, operation_type, current_window, &user_limits),
        ];
        
        let results = futures::future::try_join_all(checks).await?;
        
        // Find the most restrictive limit
        let most_restrictive = results.into_iter()
            .min_by_key(|result| result.remaining)
            .unwrap();
        
        // Task 14: Update counters if allowed
        if most_restrictive.allowed {
            self.increment_counters(user_id, query_complexity, operation_type, current_window).await?;
        }
        
        // Task 14: Rate limit metrics
        self.metrics.record_rate_limit_check(
            user_id,
            most_restrictive.allowed,
            query_complexity
        );
        
        Ok(most_restrictive)
    }
    
    // Task 14: Sliding window rate limiting
    async fn check_requests_per_minute(&self,
        user_id: UserId,
        window: u64,
        limits: &UserRateLimits
    ) -> Result<RateLimitResult> {
        
        let key = format!("rate_limit:requests:{}:{}", user_id, window);
        let current_count = self.redis_client
            .get::<u32>(&key)
            .await?
            .unwrap_or(0);
        
        let allowed = current_count < limits.requests_per_minute;
        let remaining = limits.requests_per_minute.saturating_sub(current_count);
        
        Ok(RateLimitResult {
            allowed,
            remaining,
            reset_time: (window + 1) * 60, // Next window
            retry_after: if allowed { 
                Duration::from_secs(0) 
            } else { 
                Duration::from_secs(60 - (SystemTime::now()
                    .duration_since(UNIX_EPOCH)?
                    .as_secs() % 60))
            },
        })
    }
    
    // Task 14: Complexity-based rate limiting
    async fn check_complexity_per_minute(&self,
        user_id: UserId,
        query_complexity: u32,
        window: u64,
        limits: &UserRateLimits
    ) -> Result<RateLimitResult> {
        
        let key = format!("rate_limit:complexity:{}:{}", user_id, window);
        let current_complexity = self.redis_client
            .get::<u32>(&key)
            .await?
            .unwrap_or(0);
        
        let new_complexity = current_complexity + query_complexity;
        let allowed = new_complexity <= limits.complexity_per_minute;
        let remaining = limits.complexity_per_minute.saturating_sub(new_complexity);
        
        Ok(RateLimitResult {
            allowed,
            remaining,
            reset_time: (window + 1) * 60,
            retry_after: if allowed { 
                Duration::from_secs(0) 
            } else { 
                Duration::from_secs(60)
            },
        })
    }
    
    // Task 14: Operation type specific limits
    async fn check_operation_type_limit(&self,
        user_id: UserId,
        operation_type: OperationType,
        window: u64,
        limits: &UserRateLimits
    ) -> Result<RateLimitResult> {
        
        let limit = match operation_type {
            OperationType::Query => limits.queries_per_minute,
            OperationType::Mutation => limits.mutations_per_minute,
            OperationType::Subscription => limits.subscriptions_per_minute,
        };
        
        let key = format!("rate_limit:{}:{}:{}", 
            operation_type.as_str(), user_id, window);
        let current_count = self.redis_client
            .get::<u32>(&key)
            .await?
            .unwrap_or(0);
        
        let allowed = current_count < limit;
        let remaining = limit.saturating_sub(current_count);
        
        Ok(RateLimitResult {
            allowed,
            remaining,
            reset_time: (window + 1) * 60,
            retry_after: if allowed { 
                Duration::from_secs(0) 
            } else { 
                Duration::from_secs(60)
            },
        })
    }
}
```

### 3. Business Layer (Бизнес-слой)

#### 3.1 Review Service
```rust
// Фактическая реализация: src/service/review_service.rs
pub struct ReviewService {
    repository: Arc<ReviewRepository>,
    cache_manager: Arc<CacheManager>,
    external_clients: Arc<ExternalClients>,
    metrics: Arc<MetricsCollector>,
}

impl ReviewService {
    // Task 14: Optimized review retrieval
    pub async fn get_reviews_by_offer_id(&self, 
        offer_id: OfferId,
        pagination: PaginationArgs
    ) -> Result<ReviewConnection> {
        
        let cache_key = format!("reviews:offer:{}:page:{}:limit:{}", 
            offer_id, 
            pagination.after.as_deref().unwrap_or(""),
            pagination.first.unwrap_or(10)
        );
        
        // Task 14: Cache-first strategy
        self.cache_manager.get_or_compute(
            &cache_key,
            Duration::from_secs(300), // 5 minutes TTL
            || async {
                let reviews = self.repository
                    .find_by_offer_id_paginated(offer_id, pagination)
                    .await?;
                
                // Task 14: Enrich with external data using DataLoader pattern
                self.enrich_reviews_with_external_data(reviews).await
            }
        ).await
    }
    
    // Task 14: Batch enrichment to prevent N+1
    async fn enrich_reviews_with_external_data(&self, 
        reviews: Vec<Review>
    ) -> Result<ReviewConnection> {
        
        let user_ids: Vec<UserId> = reviews.iter()
            .map(|review| review.author_id)
            .collect();
        
        // Task 14: Batch load users to prevent N+1
        let users = self.external_clients.users_client
            .get_users_by_ids(user_ids)
            .await?;
        
        let user_map: HashMap<UserId, User> = users.into_iter()
            .map(|user| (user.id, user))
            .collect();
        
        // Enrich reviews with user data
        let enriched_reviews: Vec<EnrichedReview> = reviews.into_iter()
            .map(|review| EnrichedReview {
                id: review.id,
                content: review.content,
                rating: review.rating,
                created_at: review.created_at,
                author: user_map.get(&review.author_id).cloned(),
                offer_id: review.offer_id,
            })
            .collect();
        
        Ok(ReviewConnection::from_reviews(enriched_reviews))
    }
    
    // Task 14: Optimized review creation with cache invalidation
    pub async fn create_review(&self, input: CreateReviewInput) -> Result<Review> {
        // Create review in database
        let review = self.repository.create(input.clone()).await?;
        
        // Task 14: Intelligent cache invalidation
        self.invalidate_related_caches(input.offer_id).await?;
        
        // Task 14: Update aggregated data asynchronously
        let rating_service = self.rating_service.clone();
        let offer_id = input.offer_id;
        tokio::spawn(async move {
            if let Err(e) = rating_service.recalculate_offer_rating(offer_id).await {
                tracing::warn!("Failed to recalculate rating for offer {}: {}", offer_id, e);
            }
        });
        
        Ok(review)
    }
    
    // Task 14: Smart cache invalidation
    async fn invalidate_related_caches(&self, offer_id: OfferId) -> Result<()> {
        let invalidation_patterns = vec![
            format!("reviews:offer:{}", offer_id),
            format!("reviews:offer:{}:*", offer_id), // All pagination variants
            format!("rating:offer:{}", offer_id),
            format!("offer_stats:{}", offer_id),
        ];
        
        for pattern in invalidation_patterns {
            self.cache_manager.invalidate_pattern(&pattern).await?;
        }
        
        Ok(())
    }
}
```

### 4. Data Layer (Слой данных)

#### 4.1 Review Repository
```rust
// Фактическая реализация: src/repository/review_repository.rs
pub struct ReviewRepository {
    pool: PgPool,
    metrics: Arc<MetricsCollector>,
}

impl ReviewRepository {
    // Task 14: Optimized batch loading
    pub async fn find_by_ids(&self, ids: Vec<ReviewId>) -> Result<Vec<Review>> {
        let start_time = Instant::now();
        
        // Task 14: Use ANY() for efficient batch loading
        let query = sqlx::query_as!(
            Review,
            r#"
            SELECT id, content, rating, author_id, offer_id, created_at, updated_at, is_moderated
            FROM reviews 
            WHERE id = ANY($1) AND is_moderated = true
            ORDER BY created_at DESC
            "#,
            &ids
        );
        
        let reviews = query.fetch_all(&self.pool).await?;
        
        // Task 14: Repository metrics
        self.metrics.record_repository_query(
            "find_by_ids",
            ids.len(),
            reviews.len(),
            start_time.elapsed()
        );
        
        Ok(reviews)
    }
    
    // Task 14: Optimized pagination with cursor
    pub async fn find_by_offer_id_paginated(&self,
        offer_id: OfferId,
        pagination: PaginationArgs
    ) -> Result<Vec<Review>> {
        
        let start_time = Instant::now();
        
        let query = if let Some(after_cursor) = pagination.after {
            // Cursor-based pagination
            sqlx::query_as!(
                Review,
                r#"
                SELECT id, content, rating, author_id, offer_id, created_at, updated_at, is_moderated
                FROM reviews 
                WHERE offer_id = $1 
                  AND is_moderated = true 
                  AND created_at < $2
                ORDER BY created_at DESC, id DESC
                LIMIT $3
                "#,
                offer_id,
                after_cursor,
                pagination.first.unwrap_or(10) as i64
            )
        } else {
            // First page
            sqlx::query_as!(
                Review,
                r#"
                SELECT id, content, rating, author_id, offer_id, created_at, updated_at, is_moderated
                FROM reviews 
                WHERE offer_id = $1 AND is_moderated = true
                ORDER BY created_at DESC, id DESC
                LIMIT $2
                "#,
                offer_id,
                pagination.first.unwrap_or(10) as i64
            )
        };
        
        let reviews = query.fetch_all(&self.pool).await?;
        
        // Task 14: Query performance metrics
        self.metrics.record_repository_query(
            "find_by_offer_id_paginated",
            1, // Single offer
            reviews.len(),
            start_time.elapsed()
        );
        
        Ok(reviews)
    }
    
    // Task 14: Batch insert for performance testing
    pub async fn batch_create(&self, reviews: Vec<CreateReviewInput>) -> Result<Vec<Review>> {
        let start_time = Instant::now();
        
        // Prepare batch insert
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO reviews (content, rating, author_id, offer_id, created_at, is_moderated) "
        );
        
        query_builder.push_values(reviews.iter(), |mut b, review| {
            b.push_bind(&review.content)
             .push_bind(review.rating)
             .push_bind(review.author_id)
             .push_bind(review.offer_id)
             .push_bind(Utc::now())
             .push_bind(false); // Requires moderation
        });
        
        query_builder.push(" RETURNING id, content, rating, author_id, offer_id, created_at, updated_at, is_moderated");
        
        let query = query_builder.build_query_as::<Review>();
        let inserted_reviews = query.fetch_all(&self.pool).await?;
        
        // Task 14: Batch operation metrics
        self.metrics.record_repository_batch_operation(
            "batch_create",
            reviews.len(),
            inserted_reviews.len(),
            start_time.elapsed()
        );
        
        Ok(inserted_reviews)
    }
}
```

#### 4.2 Connection Pool
```rust
// Фактическая реализация: src/database/pool.rs
pub struct ConnectionPool {
    pool: PgPool,
    metrics: Arc<MetricsCollector>,
    config: DatabaseConfig,
}

impl ConnectionPool {
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        // Task 14: Optimized connection pool configuration
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
            .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .max_lifetime(Duration::from_secs(config.max_lifetime_secs))
            // Task 14: Connection testing
            .test_before_acquire(true)
            .connect(&config.database_url)
            .await?;
        
        Ok(Self {
            pool,
            metrics: Arc::new(MetricsCollector::new()),
            config,
        })
    }
    
    // Task 14: Instrumented connection acquisition
    pub async fn acquire(&self) -> Result<PoolConnection<Postgres>> {
        let start_time = Instant::now();
        
        let connection = self.pool.acquire().await?;
        
        // Task 14: Connection pool metrics
        self.metrics.record_connection_acquisition(start_time.elapsed());
        self.metrics.record_pool_stats(
            self.pool.size(),
            self.pool.num_idle()
        );
        
        Ok(connection)
    }
    
    // Task 14: Health check for connection pool
    pub async fn health_check(&self) -> Result<PoolHealth> {
        let start_time = Instant::now();
        
        // Test connection
        let mut conn = self.pool.acquire().await?;
        sqlx::query("SELECT 1").execute(&mut *conn).await?;
        
        let health = PoolHealth {
            total_connections: self.pool.size(),
            idle_connections: self.pool.num_idle(),
            active_connections: self.pool.size() - self.pool.num_idle(),
            health_check_duration: start_time.elapsed(),
            is_healthy: true,
        };
        
        Ok(health)
    }
}
```

## 🔄 Component Interactions

### Performance Layer Coordination
```rust
// Фактическая реализация: Component coordination
impl PerformanceCoordinator {
    pub async fn handle_graphql_request(&self, 
        request: GraphQLRequest,
        user_context: UserContext
    ) -> Result<GraphQLResponse> {
        
        // 1. Query Analysis
        let analysis = self.query_analyzer
            .analyze_query(&request.query, &user_context)
            .await?;
        
        if !analysis.is_valid {
            return Ok(GraphQLResponse::error(analysis.violations));
        }
        
        // 2. Rate Limiting
        let rate_limit_result = self.rate_limiter
            .check_rate_limit(
                user_context.user_id,
                analysis.complexity,
                request.operation_type()
            )
            .await?;
        
        if !rate_limit_result.allowed {
            return Ok(GraphQLResponse::rate_limited(rate_limit_result));
        }
        
        // 3. Cache Check
        let cache_key = self.generate_cache_key(&request, &user_context);
        if let Some(cached_response) = self.cache_manager
            .get::<GraphQLResponse>(&cache_key)
            .await? 
        {
            return Ok(cached_response);
        }
        
        // 4. Execute with DataLoader
        let dataloader_context = self.dataloader_manager
            .create_request_context();
        
        let response = self.execute_with_performance_optimizations(
            request,
            user_context,
            dataloader_context
        ).await?;
        
        // 5. Cache Response
        if response.is_cacheable() {
            self.cache_manager
                .set(&cache_key, &response, Duration::from_secs(300))
                .await?;
        }
        
        Ok(response)
    }
}
```

## 📊 Performance Metrics Integration

```rust
// Фактическая реализация: Comprehensive metrics
impl ComponentMetrics {
    pub fn record_component_interaction(&self,
        source_component: &str,
        target_component: &str,
        operation: &str,
        duration: Duration,
        success: bool
    ) {
        let labels = &[
            ("source", source_component),
            ("target", target_component),
            ("operation", operation),
            ("success", &success.to_string())
        ];
        
        self.component_interaction_duration
            .with_label_values(labels)
            .observe(duration.as_secs_f64());
        
        self.component_interaction_total
            .with_label_values(labels)
            .inc();
    }
}
```

## 🔗 Связь с реализацией

Эта Component диаграмма напрямую отражается в структуре кода:

- **`src/graphql/`** - GraphQL Layer components
- **`src/performance/`** - Performance Layer components  
- **`src/service/`** - Business Layer components
- **`src/repository/`** - Data Layer components
- **`src/database/`** - Connection Pool implementation

Диаграмма служит **implementation blueprint**, показывая точную структуру компонентов Task 14 и их взаимодействия для достижения оптимальной производительности.