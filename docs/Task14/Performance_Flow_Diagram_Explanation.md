# Performance Flow Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `Performance_Flow_Diagram.puml`  
**Тип:** Sequence Diagram  
**Цель:** Показать полный flow обработки GraphQL запроса с оптимизациями производительности Task 14

## 🎯 Архитектурное назначение

Эта диаграмма демонстрирует **пошаговый процесс обработки запроса** с применением всех оптимизаций Task 14, показывая как компоненты взаимодействуют для достижения максимальной производительности.

## 🔄 Детальный Flow процесса

### 1. Client Request Processing

```typescript
// Фактическая реализация: Client-side GraphQL request
const client = new ApolloClient({
  uri: 'https://api.auto.ru/graphql',
  cache: new InMemoryCache(),
  defaultOptions: {
    watchQuery: {
      // Task 14: Client-side caching strategy
      fetchPolicy: 'cache-first',
      errorPolicy: 'all'
    }
  }
});

// Complex query that triggers Task 14 optimizations
const COMPLEX_QUERY = gql`
  query GetOffersWithReviews($limit: Int!) {
    offers(limit: $limit) {           # Complexity: 5 * $limit
      id
      name
      reviews(first: 10) {            # Complexity: 15 * 10 = 150 per offer
        id
        content
        rating
        author {                      # Complexity: 3 per review
          id
          name
        }
      }
      averageRating                   # Complexity: 5 (cached calculation)
    }
  }
`;

// Task 14: Client sends request with complexity hints
const response = await client.query({
  query: COMPLEX_QUERY,
  variables: { limit: 5 },
  context: {
    // Performance hints for server
    performance: {
      maxComplexity: 1000,
      enableCaching: true,
      enableDataLoader: true
    }
  }
});
```

### 2. Gateway Processing

```rust
// Фактическая реализация: Gateway request handling
impl GraphQLGateway {
    pub async fn handle_request(&self, request: GraphQLRequest) -> Result<GraphQLResponse> {
        let start_time = Instant::now();
        
        // Task 14: Request tracing
        let span = tracing::info_span!("gateway_request",
            query_hash = %self.hash_query(&request.query),
            user_id = ?request.context.user_id
        );
        
        async move {
            // Route to appropriate subgraph
            let subgraph_request = self.route_to_subgraph(request).await?;
            
            // Task 14: Gateway-level performance monitoring
            let response = self.execute_subgraph_request(subgraph_request).await?;
            
            self.metrics.record_gateway_request(
                start_time.elapsed(),
                response.errors.is_empty()
            );
            
            Ok(response)
        }.instrument(span).await
    }
}
```

### 3. Query Analysis & Validation

```rust
// Фактическая реализация: Query analysis step
impl QueryAnalyzer {
    pub async fn validate_query(&self, 
        query: &str, 
        user_context: &UserContext
    ) -> Result<ValidationResult> {
        
        let start_time = Instant::now();
        
        // Parse GraphQL query
        let document = async_graphql_parser::parse_query(query)?;
        
        // Task 14: Depth analysis
        let depth_result = self.analyze_depth(&document)?;
        if depth_result.depth > self.max_depth {
            return Ok(ValidationResult::rejected(
                format!("Query depth {} exceeds maximum {}", 
                    depth_result.depth, self.max_depth)
            ));
        }
        
        // Task 14: Complexity analysis
        let complexity_result = self.analyze_complexity(&document, &user_context)?;
        if complexity_result.complexity > self.max_complexity {
            return Ok(ValidationResult::rejected(
                format!("Query complexity {} exceeds maximum {}", 
                    complexity_result.complexity, self.max_complexity)
            ));
        }
        
        // Task 14: Analysis metrics
        self.metrics.record_query_analysis(
            depth_result.depth,
            complexity_result.complexity,
            start_time.elapsed()
        );
        
        Ok(ValidationResult::valid(depth_result, complexity_result))
    }
    
    // Task 14: Dynamic complexity calculation
    fn analyze_complexity(&self, 
        document: &ExecutableDocument, 
        user_context: &UserContext
    ) -> Result<ComplexityResult> {
        
        let mut total_complexity = 0;
        let user_limits = self.get_user_limits(user_context.user_id);
        
        for operation in &document.operations {
            match operation {
                Operation::Query(query) => {
                    total_complexity += self.calculate_selection_complexity(
                        &query.selection_set,
                        "Query",
                        &user_limits
                    )?;
                }
                Operation::Mutation(mutation) => {
                    // Mutations have higher base complexity
                    total_complexity += self.calculate_selection_complexity(
                        &mutation.selection_set,
                        "Mutation", 
                        &user_limits
                    )? * 2;
                }
                _ => {}
            }
        }
        
        Ok(ComplexityResult {
            complexity: total_complexity,
            breakdown: self.complexity_breakdown.clone(),
        })
    }
}
```

### 4. Rate Limiting Check

```rust
// Фактическая реализация: Rate limiting step
impl RateLimiter {
    pub async fn check_rate_limit(&self, 
        user_id: UserId,
        query_complexity: u32
    ) -> Result<RateLimitResult> {
        
        let current_window = self.get_current_window();
        let user_limits = self.get_user_limits(user_id).await;
        
        // Task 14: Multiple rate limit checks in parallel
        let (request_check, complexity_check, burst_check) = tokio::try_join!(
            self.check_request_rate(user_id, current_window, &user_limits),
            self.check_complexity_rate(user_id, query_complexity, current_window, &user_limits),
            self.check_burst_protection(user_id, current_window)
        )?;
        
        // Find most restrictive limit
        let most_restrictive = [request_check, complexity_check, burst_check]
            .into_iter()
            .min_by_key(|result| result.remaining)
            .unwrap();
        
        if most_restrictive.allowed {
            // Task 14: Update all counters atomically
            self.increment_counters(user_id, query_complexity, current_window).await?;
        }
        
        // Task 14: Rate limit metrics
        self.metrics.record_rate_limit_check(
            user_id,
            most_restrictive.allowed,
            query_complexity,
            most_restrictive.remaining
        );
        
        Ok(most_restrictive)
    }
    
    // Task 14: Sliding window implementation
    async fn check_request_rate(&self,
        user_id: UserId,
        window: u64,
        limits: &UserRateLimits
    ) -> Result<RateLimitCheck> {
        
        let key = format!("rate:requests:{}:{}", user_id, window);
        
        // Use Redis for distributed rate limiting
        let current_count: u32 = self.redis_client
            .get(&key)
            .await?
            .unwrap_or(0);
        
        let allowed = current_count < limits.requests_per_minute;
        let remaining = limits.requests_per_minute.saturating_sub(current_count);
        
        Ok(RateLimitCheck {
            allowed,
            remaining,
            reset_time: (window + 1) * 60,
            limit_type: "requests_per_minute".to_string(),
        })
    }
}
```

### 5. Multi-Level Cache Check

```rust
// Фактическая реализация: Cache check flow
impl CacheManager {
    pub async fn get_cached_response(&self, 
        cache_key: &str
    ) -> Result<Option<GraphQLResponse>> {
        
        let start_time = Instant::now();
        
        // Task 14: L1 Cache check (Memory)
        if let Some(response) = self.l1_cache.get(cache_key).await? {
            self.metrics.record_cache_hit("l1", start_time.elapsed());
            return Ok(Some(response));
        }
        
        // Task 14: L2 Cache check (Redis) with circuit breaker
        let l2_result = self.circuit_breaker.call(|| {
            let redis_client = self.redis_client.clone();
            let key = cache_key.to_string();
            async move {
                redis_client.get::<GraphQLResponse>(&key).await
            }
        }).await;
        
        match l2_result {
            Ok(Some(response)) => {
                // Task 14: Populate L1 cache for next request
                self.l1_cache.set(cache_key, &response, Duration::from_secs(300)).await?;
                self.metrics.record_cache_hit("l2", start_time.elapsed());
                Ok(Some(response))
            }
            Ok(None) => {
                self.metrics.record_cache_miss("all", start_time.elapsed());
                Ok(None)
            }
            Err(e) => {
                // Circuit breaker open or Redis error
                self.metrics.record_cache_error("l2");
                tracing::warn!("L2 cache error: {}", e);
                Ok(None)
            }
        }
    }
    
    // Task 14: Cache key generation with user context
    pub fn generate_cache_key(&self, 
        request: &GraphQLRequest,
        user_context: &UserContext
    ) -> String {
        
        let query_hash = self.hash_query(&request.query);
        let variables_hash = self.hash_variables(&request.variables);
        
        // Include user permissions in cache key for security
        let permissions_hash = self.hash_permissions(&user_context.permissions);
        
        format!("gql:{}:{}:{}:v1", 
            query_hash, 
            variables_hash, 
            permissions_hash
        )
    }
}
```

### 6. DataLoader Batch Processing

```rust
// Фактическая реализация: DataLoader execution
impl DataLoaderManager {
    pub async fn execute_with_dataloader(&self, 
        resolver_context: &ResolverContext
    ) -> Result<ResolverResult> {
        
        // Task 14: Create request-scoped DataLoader context
        let dataloader_context = DataLoaderContext::new();
        
        // Execute resolver with batching enabled
        let result = resolver_context.execute_with_context(dataloader_context.clone()).await?;
        
        // Task 14: Execute all pending batches
        self.execute_pending_batches(&dataloader_context).await?;
        
        Ok(result)
    }
    
    async fn execute_pending_batches(&self, 
        context: &DataLoaderContext
    ) -> Result<()> {
        
        let batch_futures = vec![
            context.review_loader.execute_pending_batch(),
            context.rating_loader.execute_pending_batch(),
            context.offer_loader.execute_pending_batch(),
            context.user_loader.execute_pending_batch(),
        ];
        
        // Task 14: Execute all batches in parallel
        let batch_results = futures::future::try_join_all(batch_futures).await?;
        
        // Task 14: Batch execution metrics
        let total_items: usize = batch_results.iter().sum();
        self.metrics.record_batch_execution(
            batch_results.len(),
            total_items
        );
        
        Ok(())
    }
}

// Task 14: Specific DataLoader batch execution
impl ReviewDataLoader {
    async fn execute_pending_batch(&self) -> Result<usize> {
        let pending_requests = self.get_pending_requests().await;
        
        if pending_requests.is_empty() {
            return Ok(0);
        }
        
        let review_ids: Vec<ReviewId> = pending_requests.iter()
            .map(|req| req.key)
            .collect();
        
        // Task 14: Single optimized database query
        let reviews = self.repository.find_by_ids(review_ids.clone()).await?;
        
        // Create lookup map for O(1) access
        let review_map: HashMap<ReviewId, Review> = reviews.into_iter()
            .map(|review| (review.id, review))
            .collect();
        
        // Task 14: Distribute results to waiting resolvers
        for request in pending_requests {
            let review = review_map.get(&request.key).cloned();
            let _ = request.sender.send(review);
        }
        
        Ok(review_map.len())
    }
}
```

### 7. Database Query Optimization

```sql
-- Фактическая реализация: Optimized database queries
-- Task 14: Batch loading query with optimized indexes
EXPLAIN (ANALYZE, BUFFERS) 
SELECT id, content, rating, author_id, offer_id, created_at, updated_at, is_moderated
FROM reviews 
WHERE id = ANY($1) AND is_moderated = true
ORDER BY created_at DESC;

-- Query Plan:
-- Index Scan using idx_reviews_batch_load on reviews
-- Index Cond: (id = ANY($1))
-- Filter: is_moderated = true
-- Buffers: shared hit=45 read=2

-- Task 14: N+1 prevention query
EXPLAIN (ANALYZE, BUFFERS)
SELECT r.id, r.content, r.rating, r.author_id, r.offer_id, r.created_at
FROM reviews r
WHERE r.offer_id = ANY($1) AND r.is_moderated = true
ORDER BY r.offer_id, r.created_at DESC;

-- Query Plan:
-- Index Scan using idx_reviews_offer_performance on reviews r
-- Index Cond: (offer_id = ANY($1))
-- Filter: is_moderated = true
-- Buffers: shared hit=123 read=5
```

```rust
// Фактическая реализация: Repository with optimized queries
impl ReviewRepository {
    // Task 14: Batch loading implementation
    pub async fn find_by_ids(&self, ids: Vec<ReviewId>) -> Result<Vec<Review>> {
        let start_time = Instant::now();
        
        // Use prepared statement for better performance
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
        
        // Task 14: Query performance metrics
        self.metrics.record_database_query(
            "find_by_ids",
            ids.len(),
            reviews.len(),
            start_time.elapsed()
        );
        
        Ok(reviews)
    }
}
```

### 8. Response Caching & Assembly

```rust
// Фактическая реализация: Response caching
impl ResponseProcessor {
    pub async fn process_and_cache_response(&self,
        response: GraphQLResponse,
        cache_key: &str,
        user_context: &UserContext
    ) -> Result<GraphQLResponse> {
        
        // Task 14: Determine cacheability
        let cache_policy = self.determine_cache_policy(&response, user_context);
        
        if cache_policy.is_cacheable {
            // Task 14: Cache response with appropriate TTL
            self.cache_manager.set_multi_level(
                cache_key,
                &response,
                cache_policy.ttl
            ).await?;
            
            // Task 14: Cache warming for related queries
            if cache_policy.enable_warming {
                self.warm_related_caches(&response, user_context).await?;
            }
        }
        
        // Task 14: Add performance headers
        let mut enhanced_response = response;
        enhanced_response.extensions.insert(
            "performance".to_string(),
            json!({
                "cached": false,
                "cache_key": cache_key,
                "ttl": cache_policy.ttl.as_secs(),
                "dataloader_stats": self.get_dataloader_stats(),
                "query_complexity": cache_policy.complexity
            })
        );
        
        Ok(enhanced_response)
    }
    
    // Task 14: Cache policy determination
    fn determine_cache_policy(&self, 
        response: &GraphQLResponse,
        user_context: &UserContext
    ) -> CachePolicy {
        
        // Don't cache responses with errors
        if !response.errors.is_empty() {
            return CachePolicy::no_cache();
        }
        
        // Don't cache user-specific data for shared cache
        if self.contains_user_specific_data(&response) {
            return CachePolicy::private_cache(Duration::from_secs(60));
        }
        
        // Cache public data aggressively
        CachePolicy::public_cache(Duration::from_secs(600))
    }
}
```

## 📊 Performance Metrics Flow

```rust
// Фактическая реализация: End-to-end metrics collection
impl PerformanceMetricsCollector {
    pub async fn collect_request_metrics(&self, 
        request_context: &RequestContext
    ) -> RequestMetrics {
        
        RequestMetrics {
            // Task 14: Query analysis metrics
            query_depth: request_context.analysis.depth,
            query_complexity: request_context.analysis.complexity,
            analysis_duration: request_context.analysis.duration,
            
            // Task 14: Rate limiting metrics
            rate_limit_checks: request_context.rate_limit.checks_performed,
            rate_limit_remaining: request_context.rate_limit.remaining,
            
            // Task 14: Cache metrics
            cache_hits: request_context.cache.hits,
            cache_misses: request_context.cache.misses,
            cache_hit_ratio: request_context.cache.hit_ratio(),
            
            // Task 14: DataLoader metrics
            dataloader_batches: request_context.dataloader.batches_executed,
            dataloader_items: request_context.dataloader.items_loaded,
            dataloader_efficiency: request_context.dataloader.efficiency(),
            
            // Task 14: Database metrics
            database_queries: request_context.database.queries_executed,
            database_duration: request_context.database.total_duration,
            
            // Task 14: Overall performance
            total_duration: request_context.total_duration,
            success: request_context.success,
        }
    }
}
```

## 🔗 Связь с реализацией

Этот Performance Flow напрямую реализован в:

- **`src/graphql/middleware/`** - Request processing pipeline
- **`src/performance/`** - All performance optimization components
- **`src/telemetry/`** - Metrics collection at each step
- **`src/database/`** - Optimized database queries
- **Integration tests** - End-to-end performance validation

Диаграмма служит **execution blueprint**, показывая точную последовательность оптимизаций Task 14 в production environment.