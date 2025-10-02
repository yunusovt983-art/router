use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::UgcError;
use super::external::{ExternalOffer, ExternalUser};

/// Cache entry with expiration
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// In-memory cache implementation
#[derive(Debug)]
pub struct InMemoryCache<T> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    default_ttl: Duration,
}

impl<T: Clone> InMemoryCache<T> {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    pub async fn get(&self, key: &str) -> Option<T> {
        let data = self.data.read().await;
        if let Some(entry) = data.get(key) {
            if !entry.is_expired() {
                debug!("Cache hit for key: {}", key);
                return Some(entry.value.clone());
            } else {
                debug!("Cache entry expired for key: {}", key);
            }
        } else {
            debug!("Cache miss for key: {}", key);
        }
        None
    }

    pub async fn set(&self, key: String, value: T) {
        self.set_with_ttl(key, value, self.default_ttl).await;
    }

    pub async fn set_with_ttl(&self, key: String, value: T, ttl: Duration) {
        let mut data = self.data.write().await;
        data.insert(key.clone(), CacheEntry::new(value, ttl));
        debug!("Cached value for key: {} with TTL: {:?}", key, ttl);
    }

    pub async fn remove(&self, key: &str) {
        let mut data = self.data.write().await;
        data.remove(key);
        debug!("Removed cache entry for key: {}", key);
    }

    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
        info!("Cache cleared");
    }

    pub async fn cleanup_expired(&self) {
        let mut data = self.data.write().await;
        let initial_size = data.len();
        data.retain(|_, entry| !entry.is_expired());
        let removed = initial_size - data.len();
        if removed > 0 {
            debug!("Cleaned up {} expired cache entries", removed);
        }
    }

    pub async fn size(&self) -> usize {
        self.data.read().await.len()
    }
}

/// Cache trait for different cache implementations
#[async_trait]
pub trait Cache<T>: Send + Sync {
    async fn get(&self, key: &str) -> Option<T>;
    async fn set(&self, key: String, value: T);
    async fn set_with_ttl(&self, key: String, value: T, ttl: Duration);
    async fn remove(&self, key: &str);
    async fn clear(&self);
}

#[async_trait]
impl<T: Clone + Send + Sync> Cache<T> for InMemoryCache<T> {
    async fn get(&self, key: &str) -> Option<T> {
        self.get(key).await
    }

    async fn set(&self, key: String, value: T) {
        self.set(key, value).await
    }

    async fn set_with_ttl(&self, key: String, value: T, ttl: Duration) {
        self.set_with_ttl(key, value, ttl).await
    }

    async fn remove(&self, key: &str) {
        self.remove(key).await
    }

    async fn clear(&self) {
        self.clear().await
    }
}

/// Fallback data provider for graceful degradation
#[derive(Debug, Clone)]
pub struct FallbackDataProvider {
    user_cache: Arc<InMemoryCache<ExternalUser>>,
    offer_cache: Arc<InMemoryCache<ExternalOffer>>,
}

impl FallbackDataProvider {
    pub fn new() -> Self {
        Self {
            user_cache: Arc::new(InMemoryCache::new(Duration::from_secs(300))), // 5 minutes
            offer_cache: Arc::new(InMemoryCache::new(Duration::from_secs(300))), // 5 minutes
        }
    }

    /// Cache user data for fallback
    pub async fn cache_user(&self, user: &ExternalUser) {
        let key = format!("user:{}", user.id);
        self.user_cache.set(key, user.clone()).await;
    }

    /// Cache offer data for fallback
    pub async fn cache_offer(&self, offer: &ExternalOffer) {
        let key = format!("offer:{}", offer.id);
        self.offer_cache.set(key, offer.clone()).await;
    }

    /// Get user from cache or return minimal fallback
    pub async fn get_user_fallback(&self, user_id: Uuid) -> ExternalUser {
        let key = format!("user:{}", user_id);
        
        if let Some(cached_user) = self.user_cache.get(&key).await {
            info!("Using cached user data for fallback: {}", user_id);
            cached_user
        } else {
            warn!("No cached user data available, using minimal fallback: {}", user_id);
            ExternalUser {
                id: user_id,
                name: "Unknown User".to_string(),
                email: None,
            }
        }
    }

    /// Get offer from cache or return minimal fallback
    pub async fn get_offer_fallback(&self, offer_id: Uuid) -> ExternalOffer {
        let key = format!("offer:{}", offer_id);
        
        if let Some(cached_offer) = self.offer_cache.get(&key).await {
            info!("Using cached offer data for fallback: {}", offer_id);
            cached_offer
        } else {
            warn!("No cached offer data available, using minimal fallback: {}", offer_id);
            ExternalOffer {
                id: offer_id,
                title: "Unknown Offer".to_string(),
                price: None,
            }
        }
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_expired(&self) {
        self.user_cache.cleanup_expired().await;
        self.offer_cache.cleanup_expired().await;
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            user_cache_size: self.user_cache.size().await,
            offer_cache_size: self.offer_cache.size().await,
        }
    }
}

impl Default for FallbackDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub user_cache_size: usize,
    pub offer_cache_size: usize,
}

/// Health check for external services
#[derive(Debug, Clone)]
pub struct ServiceHealthMonitor {
    service_status: Arc<RwLock<HashMap<String, ServiceHealth>>>,
}

#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub service_name: String,
    pub is_healthy: bool,
    pub last_check: Instant,
    pub consecutive_failures: usize,
    pub last_error: Option<String>,
}

impl ServiceHealthMonitor {
    pub fn new() -> Self {
        Self {
            service_status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record successful service call
    pub async fn record_success(&self, service_name: &str) {
        let mut status = self.service_status.write().await;
        status.insert(
            service_name.to_string(),
            ServiceHealth {
                service_name: service_name.to_string(),
                is_healthy: true,
                last_check: Instant::now(),
                consecutive_failures: 0,
                last_error: None,
            },
        );
        debug!("Recorded success for service: {}", service_name);
    }

    /// Record failed service call
    pub async fn record_failure(&self, service_name: &str, error: &str) {
        let mut status = self.service_status.write().await;
        let health = status.entry(service_name.to_string()).or_insert_with(|| ServiceHealth {
            service_name: service_name.to_string(),
            is_healthy: true,
            last_check: Instant::now(),
            consecutive_failures: 0,
            last_error: None,
        });

        health.is_healthy = false;
        health.last_check = Instant::now();
        health.consecutive_failures += 1;
        health.last_error = Some(error.to_string());

        warn!(
            "Recorded failure for service: {} (consecutive failures: {})",
            service_name, health.consecutive_failures
        );
    }

    /// Check if service is healthy
    pub async fn is_service_healthy(&self, service_name: &str) -> bool {
        let status = self.service_status.read().await;
        status
            .get(service_name)
            .map(|health| health.is_healthy)
            .unwrap_or(true) // Assume healthy if no data
    }

    /// Get health status for all services
    pub async fn get_all_health_status(&self) -> HashMap<String, ServiceHealth> {
        self.service_status.read().await.clone()
    }

    /// Get health status for specific service
    pub async fn get_service_health(&self, service_name: &str) -> Option<ServiceHealth> {
        self.service_status.read().await.get(service_name).cloned()
    }
}

impl Default for ServiceHealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_in_memory_cache() {
        let cache = InMemoryCache::new(Duration::from_millis(100));
        
        // Test set and get
        cache.set("key1".to_string(), "value1".to_string()).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));
        
        // Test cache miss
        assert_eq!(cache.get("nonexistent").await, None);
        
        // Test expiration
        cache.set_with_ttl("key2".to_string(), "value2".to_string(), Duration::from_millis(50)).await;
        assert_eq!(cache.get("key2").await, Some("value2".to_string()));
        
        sleep(Duration::from_millis(60)).await;
        assert_eq!(cache.get("key2").await, None);
    }

    #[tokio::test]
    async fn test_fallback_data_provider() {
        let provider = FallbackDataProvider::new();
        let user_id = Uuid::new_v4();
        let offer_id = Uuid::new_v4();
        
        // Test fallback without cache
        let user = provider.get_user_fallback(user_id).await;
        assert_eq!(user.id, user_id);
        assert_eq!(user.name, "Unknown User");
        
        // Test with cached data
        let cached_user = ExternalUser {
            id: user_id,
            name: "Cached User".to_string(),
            email: Some("cached@example.com".to_string()),
        };
        provider.cache_user(&cached_user).await;
        
        let user = provider.get_user_fallback(user_id).await;
        assert_eq!(user.name, "Cached User");
        assert_eq!(user.email, Some("cached@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_service_health_monitor() {
        let monitor = ServiceHealthMonitor::new();
        
        // Initially healthy (no data)
        assert!(monitor.is_service_healthy("test-service").await);
        
        // Record success
        monitor.record_success("test-service").await;
        assert!(monitor.is_service_healthy("test-service").await);
        
        // Record failure
        monitor.record_failure("test-service", "Connection failed").await;
        assert!(!monitor.is_service_healthy("test-service").await);
        
        let health = monitor.get_service_health("test-service").await.unwrap();
        assert_eq!(health.consecutive_failures, 1);
        assert_eq!(health.last_error, Some("Connection failed".to_string()));
    }
}