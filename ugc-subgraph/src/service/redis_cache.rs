use async_trait::async_trait;
use redis::{AsyncCommands, Client, RedisResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::UgcError;
use crate::models::{OfferRating, Review};

/// Redis cache configuration
#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    pub url: String,
    pub default_ttl: Duration,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub command_timeout: Duration,
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            default_ttl: Duration::from_secs(300), // 5 minutes
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(3),
        }
    }
}

/// Redis-based cache implementation
#[derive(Debug, Clone)]
pub struct RedisCache {
    client: Client,
    config: RedisCacheConfig,
}

impl RedisCache {
    pub async fn new(config: RedisCacheConfig) -> Result<Self, UgcError> {
        let client = Client::open(config.url.as_str())
            .map_err(|e| UgcError::CacheError(format!("Failed to create Redis client: {}", e)))?;

        // Test connection
        let mut conn = client
            .get_async_connection()
            .await
            .map_err(|e| UgcError::CacheError(format!("Failed to connect to Redis: {}", e)))?;

        // Ping to verify connection
        let _: String = conn
            .ping()
            .await
            .map_err(|e| UgcError::CacheError(format!("Redis ping failed: {}", e)))?;

        info!("Successfully connected to Redis at {}", config.url);

        Ok(Self { client, config })
    }

    /// Get connection with timeout
    async fn get_connection(&self) -> Result<redis::aio::Connection, UgcError> {
        tokio::time::timeout(
            self.config.connection_timeout,
            self.client.get_async_connection(),
        )
        .await
        .map_err(|_| UgcError::CacheError("Redis connection timeout".to_string()))?
        .map_err(|e| UgcError::CacheError(format!("Failed to get Redis connection: {}", e)))
    }

    /// Execute Redis command with timeout
    async fn execute_with_timeout<F, T>(&self, operation: F) -> Result<T, UgcError>
    where
        F: std::future::Future<Output = RedisResult<T>>,
    {
        tokio::time::timeout(self.config.command_timeout, operation)
            .await
            .map_err(|_| UgcError::CacheError("Redis command timeout".to_string()))?
            .map_err(|e| UgcError::CacheError(format!("Redis command failed: {}", e)))
    }

    /// Get value from cache
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>, UgcError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.get_connection().await?;
        
        let result: Option<String> = self
            .execute_with_timeout(conn.get(key))
            .await?;

        match result {
            Some(json_str) => {
                match serde_json::from_str(&json_str) {
                    Ok(value) => {
                        debug!("Cache hit for key: {}", key);
                        Ok(Some(value))
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached value for key {}: {}", key, e);
                        // Remove corrupted data
                        let _ = self.remove(key).await;
                        Ok(None)
                    }
                }
            }
            None => {
                debug!("Cache miss for key: {}", key);
                Ok(None)
            }
        }
    }

    /// Set value in cache with default TTL
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<(), UgcError>
    where
        T: Serialize,
    {
        self.set_with_ttl(key, value, self.config.default_ttl).await
    }

    /// Set value in cache with custom TTL
    pub async fn set_with_ttl<T>(&self, key: &str, value: &T, ttl: Duration) -> Result<(), UgcError>
    where
        T: Serialize,
    {
        let json_str = serde_json::to_string(value)
            .map_err(|e| UgcError::CacheError(format!("Failed to serialize value: {}", e)))?;

        let mut conn = self.get_connection().await?;
        
        self.execute_with_timeout(conn.set_ex(key, json_str, ttl.as_secs()))
            .await?;

        debug!("Cached value for key: {} with TTL: {:?}", key, ttl);
        Ok(())
    }

    /// Remove value from cache
    pub async fn remove(&self, key: &str) -> Result<(), UgcError> {
        let mut conn = self.get_connection().await?;
        
        let _: i32 = self.execute_with_timeout(conn.del(key)).await?;
        
        debug!("Removed cache entry for key: {}", key);
        Ok(())
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool, UgcError> {
        let mut conn = self.get_connection().await?;
        
        let exists: bool = self.execute_with_timeout(conn.exists(key)).await?;
        
        Ok(exists)
    }

    /// Set expiration for existing key
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, UgcError> {
        let mut conn = self.get_connection().await?;
        
        let result: bool = self
            .execute_with_timeout(conn.expire(key, ttl.as_secs() as usize))
            .await?;
        
        Ok(result)
    }

    /// Get multiple values at once
    pub async fn mget<T>(&self, keys: &[String]) -> Result<Vec<Option<T>>, UgcError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.get_connection().await?;
        
        let results: Vec<Option<String>> = self
            .execute_with_timeout(conn.get(keys))
            .await?;

        let mut values = Vec::with_capacity(results.len());
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(json_str) => {
                    match serde_json::from_str(&json_str) {
                        Ok(value) => values.push(Some(value)),
                        Err(e) => {
                            warn!("Failed to deserialize cached value for key {}: {}", keys[i], e);
                            values.push(None);
                        }
                    }
                }
                None => values.push(None),
            }
        }

        Ok(values)
    }

    /// Set multiple values at once
    pub async fn mset<T>(&self, pairs: &[(String, T)]) -> Result<(), UgcError>
    where
        T: Serialize,
    {
        if pairs.is_empty() {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        let mut redis_pairs = Vec::with_capacity(pairs.len() * 2);

        for (key, value) in pairs {
            let json_str = serde_json::to_string(value)
                .map_err(|e| UgcError::CacheError(format!("Failed to serialize value: {}", e)))?;
            redis_pairs.push(key.clone());
            redis_pairs.push(json_str);
        }

        self.execute_with_timeout(conn.mset(&redis_pairs)).await?;

        debug!("Set {} key-value pairs in cache", pairs.len());
        Ok(())
    }

    /// Increment counter
    pub async fn incr(&self, key: &str, delta: i64) -> Result<i64, UgcError> {
        let mut conn = self.get_connection().await?;
        
        let result: i64 = self
            .execute_with_timeout(conn.incr(key, delta))
            .await?;
        
        Ok(result)
    }

    /// Get cache info
    pub async fn info(&self) -> Result<String, UgcError> {
        let mut conn = self.get_connection().await?;
        
        let info: String = self.execute_with_timeout(conn.info()).await?;
        
        Ok(info)
    }

    /// Flush all cache data (use with caution)
    pub async fn flush_all(&self) -> Result<(), UgcError> {
        let mut conn = self.get_connection().await?;
        
        let _: String = self.execute_with_timeout(conn.flushall()).await?;
        
        warn!("Flushed all cache data");
        Ok(())
    }
}

/// Cache service that combines Redis with fallback strategies
#[derive(Debug, Clone)]
pub struct CacheService {
    redis: RedisCache,
    fallback_enabled: bool,
}

impl CacheService {
    pub async fn new(config: RedisCacheConfig) -> Result<Self, UgcError> {
        let redis = RedisCache::new(config).await?;
        
        Ok(Self {
            redis,
            fallback_enabled: true,
        })
    }

    /// Cache key generators
    pub fn review_key(review_id: Uuid) -> String {
        format!("review:{}", review_id)
    }

    pub fn reviews_by_offer_key(offer_id: Uuid, page: u32, limit: u32) -> String {
        format!("reviews:offer:{}:page:{}:limit:{}", offer_id, page, limit)
    }

    pub fn reviews_by_author_key(author_id: Uuid, page: u32, limit: u32) -> String {
        format!("reviews:author:{}:page:{}:limit:{}", author_id, page, limit)
    }

    pub fn offer_rating_key(offer_id: Uuid) -> String {
        format!("offer_rating:{}", offer_id)
    }

    pub fn offer_reviews_count_key(offer_id: Uuid) -> String {
        format!("offer_reviews_count:{}", offer_id)
    }

    /// Cache a single review
    pub async fn cache_review(&self, review: &Review) -> Result<(), UgcError> {
        let key = Self::review_key(review.id);
        self.redis.set_with_ttl(&key, review, Duration::from_secs(600)).await // 10 minutes
    }

    /// Get cached review
    pub async fn get_review(&self, review_id: Uuid) -> Result<Option<Review>, UgcError> {
        let key = Self::review_key(review_id);
        self.redis.get(&key).await
    }

    /// Cache offer rating
    pub async fn cache_offer_rating(&self, offer_rating: &OfferRating) -> Result<(), UgcError> {
        let key = Self::offer_rating_key(offer_rating.offer_id);
        self.redis.set_with_ttl(&key, offer_rating, Duration::from_secs(1800)).await // 30 minutes
    }

    /// Get cached offer rating
    pub async fn get_offer_rating(&self, offer_id: Uuid) -> Result<Option<OfferRating>, UgcError> {
        let key = Self::offer_rating_key(offer_id);
        self.redis.get(&key).await
    }

    /// Cache reviews count for an offer
    pub async fn cache_offer_reviews_count(&self, offer_id: Uuid, count: i64) -> Result<(), UgcError> {
        let key = Self::offer_reviews_count_key(offer_id);
        self.redis.set_with_ttl(&key, &count, Duration::from_secs(300)).await // 5 minutes
    }

    /// Get cached reviews count
    pub async fn get_offer_reviews_count(&self, offer_id: Uuid) -> Result<Option<i64>, UgcError> {
        let key = Self::offer_reviews_count_key(offer_id);
        self.redis.get(&key).await
    }

    /// Invalidate all cache entries for an offer
    pub async fn invalidate_offer_cache(&self, offer_id: Uuid) -> Result<(), UgcError> {
        let keys = vec![
            Self::offer_rating_key(offer_id),
            Self::offer_reviews_count_key(offer_id),
        ];

        for key in keys {
            if let Err(e) = self.redis.remove(&key).await {
                warn!("Failed to invalidate cache key {}: {}", key, e);
            }
        }

        // Also invalidate paginated reviews cache (this is a simplified approach)
        // In production, you might want to use Redis patterns or maintain a separate index
        info!("Invalidated cache for offer: {}", offer_id);
        Ok(())
    }

    /// Invalidate cache for a specific review
    pub async fn invalidate_review_cache(&self, review: &Review) -> Result<(), UgcError> {
        let review_key = Self::review_key(review.id);
        self.redis.remove(&review_key).await?;

        // Also invalidate related offer cache
        self.invalidate_offer_cache(review.offer_id).await?;

        info!("Invalidated cache for review: {}", review.id);
        Ok(())
    }

    /// Warm up cache with frequently accessed data
    pub async fn warmup_cache(&self, reviews: &[Review], ratings: &[OfferRating]) -> Result<(), UgcError> {
        info!("Starting cache warmup with {} reviews and {} ratings", reviews.len(), ratings.len());

        // Cache reviews
        for review in reviews {
            if let Err(e) = self.cache_review(review).await {
                warn!("Failed to cache review {}: {}", review.id, e);
            }
        }

        // Cache ratings
        for rating in ratings {
            if let Err(e) = self.cache_offer_rating(rating).await {
                warn!("Failed to cache rating for offer {}: {}", rating.offer_id, e);
            }
        }

        info!("Cache warmup completed");
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats, UgcError> {
        let info = self.redis.info().await?;
        
        // Parse Redis info to extract relevant statistics
        let mut stats = CacheStats::default();
        
        for line in info.lines() {
            if line.starts_with("used_memory:") {
                if let Some(memory_str) = line.split(':').nth(1) {
                    stats.memory_usage = memory_str.parse().unwrap_or(0);
                }
            } else if line.starts_with("keyspace_hits:") {
                if let Some(hits_str) = line.split(':').nth(1) {
                    stats.hits = hits_str.parse().unwrap_or(0);
                }
            } else if line.starts_with("keyspace_misses:") {
                if let Some(misses_str) = line.split(':').nth(1) {
                    stats.misses = misses_str.parse().unwrap_or(0);
                }
            }
        }

        stats.hit_rate = if stats.hits + stats.misses > 0 {
            stats.hits as f64 / (stats.hits + stats.misses) as f64
        } else {
            0.0
        };

        Ok(stats)
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub memory_usage: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModerationStatus, Review};
    use chrono::Utc;
    use rust_decimal::Decimal;

    async fn create_test_cache() -> CacheService {
        let config = RedisCacheConfig {
            url: "redis://localhost:6379".to_string(),
            ..Default::default()
        };
        
        CacheService::new(config).await.expect("Failed to create test cache")
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_review_caching() {
        let cache = create_test_cache().await;
        
        let review = Review {
            id: Uuid::new_v4(),
            offer_id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            rating: 5,
            text: "Great car!".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_moderated: true,
            moderation_status: ModerationStatus::Approved,
        };

        // Cache the review
        cache.cache_review(&review).await.unwrap();

        // Retrieve from cache
        let cached_review = cache.get_review(review.id).await.unwrap().unwrap();
        assert_eq!(cached_review.id, review.id);
        assert_eq!(cached_review.text, review.text);
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_offer_rating_caching() {
        let cache = create_test_cache().await;
        
        let offer_id = Uuid::new_v4();
        let rating = OfferRating {
            offer_id,
            average_rating: Decimal::new(45, 1), // 4.5
            reviews_count: 10,
            rating_distribution: serde_json::json!({"5": 5, "4": 3, "3": 2}),
            updated_at: Utc::now(),
        };

        // Cache the rating
        cache.cache_offer_rating(&rating).await.unwrap();

        // Retrieve from cache
        let cached_rating = cache.get_offer_rating(offer_id).await.unwrap().unwrap();
        assert_eq!(cached_rating.offer_id, offer_id);
        assert_eq!(cached_rating.reviews_count, 10);
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_cache_invalidation() {
        let cache = create_test_cache().await;
        
        let offer_id = Uuid::new_v4();
        
        // Cache some data
        cache.cache_offer_reviews_count(offer_id, 5).await.unwrap();
        
        // Verify it's cached
        let count = cache.get_offer_reviews_count(offer_id).await.unwrap();
        assert_eq!(count, Some(5));
        
        // Invalidate cache
        cache.invalidate_offer_cache(offer_id).await.unwrap();
        
        // Verify it's gone
        let count = cache.get_offer_reviews_count(offer_id).await.unwrap();
        assert_eq!(count, None);
    }
}