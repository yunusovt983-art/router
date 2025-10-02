use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, instrument};
use uuid::Uuid;

/// Feature flag service for controlling migration rollout
#[derive(Clone)]
pub struct FeatureFlagService {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    redis_client: Option<redis::Client>,
}

impl FeatureFlagService {
    pub fn new() -> Self {
        let mut flags = HashMap::new();
        
        // Initialize default feature flags for migration
        flags.insert("graphql_reviews_read".to_string(), FeatureFlag {
            name: "graphql_reviews_read".to_string(),
            enabled: false,
            rollout_percentage: 0.0,
            user_whitelist: vec![],
            user_blacklist: vec![],
            conditions: vec![],
            description: "Enable GraphQL for reading reviews".to_string(),
        });

        flags.insert("graphql_reviews_write".to_string(), FeatureFlag {
            name: "graphql_reviews_write".to_string(),
            enabled: false,
            rollout_percentage: 0.0,
            user_whitelist: vec![],
            user_blacklist: vec![],
            conditions: vec![],
            description: "Enable GraphQL for writing reviews".to_string(),
        });

        flags.insert("rest_api_deprecation_warning".to_string(), FeatureFlag {
            name: "rest_api_deprecation_warning".to_string(),
            enabled: false,
            rollout_percentage: 100.0,
            user_whitelist: vec![],
            user_blacklist: vec![],
            conditions: vec![],
            description: "Show deprecation warnings for REST API usage".to_string(),
        });

        Self {
            flags: Arc::new(RwLock::new(flags)),
            redis_client: None,
        }
    }

    pub fn with_redis(mut self, redis_url: &str) -> Result<Self, redis::RedisError> {
        self.redis_client = Some(redis::Client::open(redis_url)?);
        Ok(self)
    }

    #[instrument(skip(self))]
    pub async fn is_enabled(&self, flag_name: &str, user_id: &str) -> bool {
        // First check Redis cache if available
        if let Some(cached_result) = self.check_redis_cache(flag_name, user_id).await {
            return cached_result;
        }

        // Fallback to in-memory flags
        let flags = self.flags.read().await;
        if let Some(flag) = flags.get(flag_name) {
            let result = self.evaluate_flag(flag, user_id).await;
            
            // Cache result in Redis if available
            self.cache_result_in_redis(flag_name, user_id, result).await;
            
            result
        } else {
            warn!("Feature flag '{}' not found", flag_name);
            false
        }
    }

    #[instrument(skip(self))]
    pub async fn update_flag(&self, flag_name: &str, flag: FeatureFlag) -> Result<(), String> {
        let mut flags = self.flags.write().await;
        flags.insert(flag_name.to_string(), flag.clone());
        
        info!("Updated feature flag '{}': enabled={}, rollout={}%", 
              flag_name, flag.enabled, flag.rollout_percentage);

        // Invalidate Redis cache for this flag
        self.invalidate_redis_cache(flag_name).await;
        
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_flag(&self, flag_name: &str) -> Option<FeatureFlag> {
        let flags = self.flags.read().await;
        flags.get(flag_name).cloned()
    }

    #[instrument(skip(self))]
    pub async fn list_flags(&self) -> Vec<FeatureFlag> {
        let flags = self.flags.read().await;
        flags.values().cloned().collect()
    }

    #[instrument(skip(self))]
    pub async fn enable_flag_for_user(&self, flag_name: &str, user_id: &str) -> Result<(), String> {
        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(flag_name) {
            if !flag.user_whitelist.contains(&user_id.to_string()) {
                flag.user_whitelist.push(user_id.to_string());
                info!("Added user {} to whitelist for flag '{}'", user_id, flag_name);
            }
            Ok(())
        } else {
            Err(format!("Feature flag '{}' not found", flag_name))
        }
    }

    #[instrument(skip(self))]
    pub async fn disable_flag_for_user(&self, flag_name: &str, user_id: &str) -> Result<(), String> {
        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(flag_name) {
            if !flag.user_blacklist.contains(&user_id.to_string()) {
                flag.user_blacklist.push(user_id.to_string());
                info!("Added user {} to blacklist for flag '{}'", user_id, flag_name);
            }
            Ok(())
        } else {
            Err(format!("Feature flag '{}' not found", flag_name))
        }
    }

    #[instrument(skip(self))]
    pub async fn set_rollout_percentage(&self, flag_name: &str, percentage: f64) -> Result<(), String> {
        if percentage < 0.0 || percentage > 100.0 {
            return Err("Rollout percentage must be between 0 and 100".to_string());
        }

        let mut flags = self.flags.write().await;
        if let Some(flag) = flags.get_mut(flag_name) {
            flag.rollout_percentage = percentage;
            info!("Set rollout percentage for flag '{}' to {}%", flag_name, percentage);
            Ok(())
        } else {
            Err(format!("Feature flag '{}' not found", flag_name))
        }
    }

    async fn evaluate_flag(&self, flag: &FeatureFlag, user_id: &str) -> bool {
        // If flag is globally disabled, return false
        if !flag.enabled {
            return false;
        }

        // Check blacklist first
        if flag.user_blacklist.contains(&user_id.to_string()) {
            return false;
        }

        // Check whitelist
        if flag.user_whitelist.contains(&user_id.to_string()) {
            return true;
        }

        // Evaluate conditions
        for condition in &flag.conditions {
            if !self.evaluate_condition(condition, user_id).await {
                return false;
            }
        }

        // Check rollout percentage using consistent hashing
        let user_hash = self.hash_user_id(user_id);
        let user_percentage = (user_hash % 100) as f64;
        
        user_percentage < flag.rollout_percentage
    }

    async fn evaluate_condition(&self, condition: &FlagCondition, user_id: &str) -> bool {
        match condition {
            FlagCondition::UserIdStartsWith(prefix) => user_id.starts_with(prefix),
            FlagCondition::UserIdEndsWith(suffix) => user_id.ends_with(suffix),
            FlagCondition::UserIdMatches(pattern) => {
                // Simple pattern matching - could be extended with regex
                user_id.contains(pattern)
            }
            FlagCondition::TimeWindow { start, end } => {
                let now = chrono::Utc::now();
                now >= *start && now <= *end
            }
        }
    }

    fn hash_user_id(&self, user_id: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        hasher.finish() as u32
    }

    async fn check_redis_cache(&self, flag_name: &str, user_id: &str) -> Option<bool> {
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_async_connection().await {
                let cache_key = format!("feature_flag:{}:{}", flag_name, user_id);
                if let Ok(cached_value) = redis::cmd("GET")
                    .arg(&cache_key)
                    .query_async::<_, Option<String>>(&mut conn)
                    .await
                {
                    return cached_value.and_then(|v| v.parse().ok());
                }
            }
        }
        None
    }

    async fn cache_result_in_redis(&self, flag_name: &str, user_id: &str, result: bool) {
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_async_connection().await {
                let cache_key = format!("feature_flag:{}:{}", flag_name, user_id);
                let _ = redis::cmd("SETEX")
                    .arg(&cache_key)
                    .arg(300) // 5 minutes TTL
                    .arg(result.to_string())
                    .query_async::<_, ()>(&mut conn)
                    .await;
            }
        }
    }

    async fn invalidate_redis_cache(&self, flag_name: &str) {
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_async_connection().await {
                let pattern = format!("feature_flag:{}:*", flag_name);
                if let Ok(keys) = redis::cmd("KEYS")
                    .arg(&pattern)
                    .query_async::<_, Vec<String>>(&mut conn)
                    .await
                {
                    if !keys.is_empty() {
                        let _ = redis::cmd("DEL")
                            .arg(&keys)
                            .query_async::<_, ()>(&mut conn)
                            .await;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub name: String,
    pub enabled: bool,
    pub rollout_percentage: f64,
    pub user_whitelist: Vec<String>,
    pub user_blacklist: Vec<String>,
    pub conditions: Vec<FlagCondition>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlagCondition {
    UserIdStartsWith(String),
    UserIdEndsWith(String),
    UserIdMatches(String),
    TimeWindow {
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    },
}

/// A/B Testing service for migration
#[derive(Clone)]
pub struct ABTestService {
    feature_flags: Arc<FeatureFlagService>,
}

impl ABTestService {
    pub fn new(feature_flags: Arc<FeatureFlagService>) -> Self {
        Self { feature_flags }
    }

    #[instrument(skip(self))]
    pub async fn assign_user_to_variant(&self, test_name: &str, user_id: &str) -> ABTestVariant {
        let flag_name = format!("ab_test_{}", test_name);
        
        if self.feature_flags.is_enabled(&flag_name, user_id).await {
            // Use consistent hashing to assign variant
            let user_hash = self.hash_user_id(user_id);
            match user_hash % 2 {
                0 => ABTestVariant::Control,
                1 => ABTestVariant::Treatment,
                _ => ABTestVariant::Control,
            }
        } else {
            ABTestVariant::Control
        }
    }

    #[instrument(skip(self))]
    pub async fn track_conversion(&self, test_name: &str, user_id: &str, event: &str) {
        let variant = self.assign_user_to_variant(test_name, user_id).await;
        info!("A/B Test conversion: test={}, user={}, variant={:?}, event={}", 
              test_name, user_id, variant, event);
        
        // In a real implementation, this would send metrics to an analytics service
    }

    fn hash_user_id(&self, user_id: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        hasher.finish() as u32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ABTestVariant {
    Control,
    Treatment,
}

/// Migration rollback service
#[derive(Clone)]
pub struct RollbackService {
    feature_flags: Arc<FeatureFlagService>,
}

impl RollbackService {
    pub fn new(feature_flags: Arc<FeatureFlagService>) -> Self {
        Self { feature_flags }
    }

    #[instrument(skip(self))]
    pub async fn emergency_rollback(&self, reason: &str) -> Result<(), String> {
        warn!("Emergency rollback initiated: {}", reason);

        // Disable all GraphQL migration flags
        let migration_flags = vec![
            "graphql_reviews_read",
            "graphql_reviews_write",
        ];

        for flag_name in migration_flags {
            if let Some(mut flag) = self.feature_flags.get_flag(flag_name).await {
                flag.enabled = false;
                flag.rollout_percentage = 0.0;
                self.feature_flags.update_flag(flag_name, flag).await?;
            }
        }

        info!("Emergency rollback completed successfully");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn gradual_rollback(&self, flag_name: &str, target_percentage: f64) -> Result<(), String> {
        info!("Starting gradual rollback for flag '{}' to {}%", flag_name, target_percentage);

        if let Some(current_flag) = self.feature_flags.get_flag(flag_name).await {
            let current_percentage = current_flag.rollout_percentage;
            
            if target_percentage >= current_percentage {
                return Err("Target percentage must be lower than current percentage for rollback".to_string());
            }

            // Gradually reduce rollout percentage
            let step_size = 10.0;
            let mut current = current_percentage;
            
            while current > target_percentage {
                current = (current - step_size).max(target_percentage);
                self.feature_flags.set_rollout_percentage(flag_name, current).await?;
                
                info!("Rolled back flag '{}' to {}%", flag_name, current);
                
                // Wait between steps to monitor for issues
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }

            info!("Gradual rollback completed for flag '{}'", flag_name);
        } else {
            return Err(format!("Feature flag '{}' not found", flag_name));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn health_check_rollback(&self, error_rate_threshold: f64) -> Result<(), String> {
        // This would integrate with monitoring systems to check error rates
        // and automatically rollback if thresholds are exceeded
        
        // Mock implementation
        let current_error_rate = 0.05; // 5%
        
        if current_error_rate > error_rate_threshold {
            warn!("Error rate {} exceeds threshold {}, initiating rollback", 
                  current_error_rate, error_rate_threshold);
            
            self.emergency_rollback("High error rate detected").await?;
        }

        Ok(())
    }
}