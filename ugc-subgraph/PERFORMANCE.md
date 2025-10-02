# UGC Subgraph Performance Optimizations

This document describes the performance optimizations implemented in the UGC subgraph to handle high-scale GraphQL operations efficiently.

## Overview

The UGC subgraph implements multiple layers of performance optimizations:

1. **Redis Caching** - Distributed caching for frequently accessed data
2. **DataLoader Pattern** - Batching and caching for N+1 query prevention
3. **Query Complexity Analysis** - Preventing expensive queries from overwhelming the system
4. **Database Optimizations** - Indexes and optimized queries
5. **Rate Limiting** - Per-user request throttling

## 1. Redis Caching

### Features
- Distributed caching with Redis
- Automatic cache invalidation on mutations
- Configurable TTL per data type
- Fallback to database on cache miss
- Circuit breaker for Redis failures

### Configuration
```env
REDIS_URL=redis://localhost:6379
REDIS_ENABLED=true
REDIS_DEFAULT_TTL=300
REDIS_MAX_CONNECTIONS=10
REDIS_CONNECTION_TIMEOUT=5
REDIS_COMMAND_TIMEOUT=3
```

### Cached Data Types
- **Reviews** (TTL: 10 minutes) - Individual review data
- **Offer Ratings** (TTL: 30 minutes) - Aggregated rating statistics
- **Reviews Count** (TTL: 5 minutes) - Count of reviews per offer

### Cache Keys
```
review:{review_id}
offer_rating:{offer_id}
offer_reviews_count:{offer_id}
reviews:offer:{offer_id}:page:{page}:limit:{limit}
reviews:author:{author_id}:page:{page}:limit:{limit}
```

### Usage Example
```rust
// Service automatically uses cache when available
let review = review_service.get_review_by_id(review_id).await?;
let rating = review_service.get_offer_rating(offer_id).await?;
```

## 2. DataLoader Pattern

### Features
- Automatic batching of database queries
- In-memory caching within request scope
- Prevents N+1 query problems
- Configurable batch sizes
- Automatic cache invalidation

### Implemented DataLoaders
- **ReviewDataLoader** - Batch load reviews by IDs
- **OfferRatingDataLoader** - Batch load offer ratings
- **ReviewsByOfferDataLoader** - Batch load reviews for multiple offers
- **ReviewsByAuthorDataLoader** - Batch load reviews for multiple authors

### Usage Example
```rust
// Instead of N+1 queries, this batches all review lookups
let reviews = review_service.get_reviews_by_ids(vec![id1, id2, id3]).await?;

// Automatically batches when resolving GraphQL fields
// query {
//   offers {
//     reviews { ... }  # Batched into single query
//   }
// }
```

### Performance Impact
- **Before**: N+1 queries for N offers = N+1 database roundtrips
- **After**: 2 queries total (offers + batched reviews) = 2 database roundtrips

## 3. Query Complexity Analysis

### Features
- Depth limiting to prevent deeply nested queries
- Complexity scoring based on field types and arguments
- Per-user rate limiting
- Introspection query handling
- Configurable limits per user type

### Configuration
```env
QUERY_MAX_DEPTH=10
QUERY_MAX_COMPLEXITY=1000
QUERY_DEFAULT_FIELD_COMPLEXITY=1
QUERY_ENABLE_INTROSPECTION_LIMITS=false
QUERY_DEFAULT_RATE_LIMIT=60
```

### Field Complexity Scores
```rust
reviews: 5 points
reviewsConnection: 10 points
offer: 3 points
user: 2 points
averageRating: 3 points
createReview: 10 points (mutations are more expensive)
```

### Example Analysis
```graphql
query {
  offer(id: "123") {           # 3 points
    reviews(first: 10) {       # 5 * 10 = 50 points (multiplied by 'first' argument)
      author {                 # 2 points per review
        name
      }
    }
  }
}
# Total complexity: 3 + 50 + (2 * 10) = 73 points
```

### Rate Limiting
- Default: 60 requests per minute per user
- Configurable per user type (premium, basic, etc.)
- Automatic cleanup of expired tracking data

## 4. Database Optimizations

### Indexes Added
```sql
-- Batch loading optimization
CREATE INDEX idx_reviews_id_batch ON reviews (id) WHERE is_moderated = true;

-- N+1 prevention for reviews by offer
CREATE INDEX idx_reviews_offer_id_moderated_created 
ON reviews (offer_id, created_at DESC) WHERE is_moderated = true;

-- N+1 prevention for reviews by author
CREATE INDEX idx_reviews_author_id_moderated_created 
ON reviews (author_id, created_at DESC) WHERE is_moderated = true;

-- Complex filtering optimization
CREATE INDEX idx_reviews_offer_moderated_rating 
ON reviews (offer_id, is_moderated, rating, created_at DESC);

-- Cursor-based pagination
CREATE INDEX idx_reviews_cursor_pagination 
ON reviews (created_at DESC, id DESC) WHERE is_moderated = true;
```

### Optimized Queries
- Use of `ANY()` operator for batch loading
- Proper JOIN strategies for related data
- Prepared statements for security and performance
- Connection pooling with configurable limits

## 5. Monitoring and Metrics

### Performance Metrics
- Cache hit/miss rates
- Query complexity distribution
- Rate limit violations
- Database query duration
- DataLoader batch efficiency

### Health Checks
- Redis connectivity
- Database health
- Query performance thresholds
- Memory usage monitoring

## Usage Examples

### Basic Setup
```rust
// Create service with all optimizations
let config = Config::from_env()?;
let schema = create_enhanced_schema(pool, external_service, &config).await?;
```

### Custom Configuration
```rust
// Configure Redis cache
let cache_config = RedisCacheConfig {
    url: "redis://localhost:6379".to_string(),
    default_ttl: Duration::from_secs(300),
    max_connections: 10,
    connection_timeout: Duration::from_secs(5),
    command_timeout: Duration::from_secs(3),
};

// Configure query limits
let query_limits = QueryLimitsConfig {
    max_depth: 10,
    max_complexity: 1000,
    default_field_complexity: 1,
    enable_introspection_limits: false,
    per_user_limits: HashMap::new(),
};

// Create optimized service
let service = create_review_service_full(pool, cache_config, metrics).await?;
```

## Performance Testing

### Load Testing Scenarios
1. **High Read Load** - Many concurrent review queries
2. **Complex Queries** - Deep nested GraphQL queries
3. **Cache Invalidation** - Heavy write load with cache updates
4. **Rate Limiting** - Burst traffic from single users

### Expected Performance
- **Cache Hit Ratio**: >80% for frequently accessed data
- **Query Batching**: 90% reduction in database queries for N+1 scenarios
- **Response Time**: <100ms for cached data, <500ms for database queries
- **Throughput**: 1000+ requests/second with proper caching

### Monitoring Commands
```bash
# Redis cache statistics
redis-cli info stats

# Database query performance
SELECT query, mean_time, calls FROM pg_stat_statements ORDER BY mean_time DESC;

# Application metrics
curl http://localhost:4001/metrics
```

## Best Practices

### For Developers
1. Always use the service methods that leverage caching and DataLoader
2. Design GraphQL queries to be cache-friendly
3. Consider complexity scores when adding new fields
4. Test with realistic data volumes

### For Operations
1. Monitor cache hit rates and adjust TTL accordingly
2. Set up alerts for query complexity violations
3. Regularly analyze slow query logs
4. Scale Redis cluster for high availability

### For GraphQL Clients
1. Use query fragments to improve caching
2. Implement client-side query complexity estimation
3. Respect rate limits and implement backoff
4. Use persisted queries for better performance

## Troubleshooting

### Common Issues
1. **Cache Misses** - Check Redis connectivity and TTL settings
2. **N+1 Queries** - Verify DataLoader is being used in resolvers
3. **Query Rejected** - Reduce query depth or complexity
4. **Rate Limited** - Implement client-side throttling

### Debug Commands
```bash
# Check Redis connection
redis-cli ping

# Monitor database queries
tail -f /var/log/postgresql/postgresql.log

# View application logs
docker logs ugc-subgraph --follow

# Check metrics
curl http://localhost:4001/metrics | grep -E "(cache|query|rate)"
```

## Future Optimizations

### Planned Improvements
1. **Query Result Caching** - Cache entire GraphQL responses
2. **Adaptive Batching** - Dynamic batch sizes based on load
3. **Predictive Caching** - Pre-warm cache based on usage patterns
4. **Edge Caching** - CDN integration for static data
5. **Database Read Replicas** - Separate read/write workloads

### Experimental Features
1. **Query Whitelisting** - Only allow pre-approved queries
2. **Automatic Query Optimization** - Rewrite inefficient queries
3. **Machine Learning** - Predict optimal cache TTL and batch sizes