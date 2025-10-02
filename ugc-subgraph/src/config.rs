use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub log_level: String,
    pub cors_origins: Vec<String>,
    pub users_service_url: String,
    pub offers_service_url: String,
    pub redis: RedisConfig,
    pub query_limits: QueryLimitsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub enabled: bool,
    pub default_ttl_seconds: u64,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
    pub command_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryLimitsConfig {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub default_field_complexity: usize,
    pub enable_introspection_limits: bool,
    pub default_rate_limit_per_minute: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let config = Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "4001".to_string())
                .parse()?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/ugc_db".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key".to_string()),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            cors_origins: env::var("CORS_ORIGINS")
                .unwrap_or_else(|_| "*".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            users_service_url: env::var("USERS_SERVICE_URL")
                .unwrap_or_else(|_| "http://users-service:4002".to_string()),
            offers_service_url: env::var("OFFERS_SERVICE_URL")
                .unwrap_or_else(|_| "http://offers-service:4004".to_string()),
            redis: RedisConfig {
                url: env::var("REDIS_URL")
                    .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
                enabled: env::var("REDIS_ENABLED")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                default_ttl_seconds: env::var("REDIS_DEFAULT_TTL")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300),
                max_connections: env::var("REDIS_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                connection_timeout_seconds: env::var("REDIS_CONNECTION_TIMEOUT")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .unwrap_or(5),
                command_timeout_seconds: env::var("REDIS_COMMAND_TIMEOUT")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .unwrap_or(3),
            },
            query_limits: QueryLimitsConfig {
                max_depth: env::var("QUERY_MAX_DEPTH")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                max_complexity: env::var("QUERY_MAX_COMPLEXITY")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .unwrap_or(1000),
                default_field_complexity: env::var("QUERY_DEFAULT_FIELD_COMPLEXITY")
                    .unwrap_or_else(|_| "1".to_string())
                    .parse()
                    .unwrap_or(1),
                enable_introspection_limits: env::var("QUERY_ENABLE_INTROSPECTION_LIMITS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                default_rate_limit_per_minute: env::var("QUERY_DEFAULT_RATE_LIMIT")
                    .unwrap_or_else(|_| "60".to_string())
                    .parse()
                    .unwrap_or(60),
            },
        };

        Ok(config)
    }

    pub fn to_redis_cache_config(&self) -> crate::service::RedisCacheConfig {
        use std::time::Duration;
        
        crate::service::RedisCacheConfig {
            url: self.redis.url.clone(),
            default_ttl: Duration::from_secs(self.redis.default_ttl_seconds),
            max_connections: self.redis.max_connections,
            connection_timeout: Duration::from_secs(self.redis.connection_timeout_seconds),
            command_timeout: Duration::from_secs(self.redis.command_timeout_seconds),
        }
    }

    pub fn to_query_limits_config(&self) -> crate::service::QueryLimitsConfig {
        use std::collections::HashMap;
        
        crate::service::QueryLimitsConfig {
            max_depth: self.query_limits.max_depth,
            max_complexity: self.query_limits.max_complexity,
            default_field_complexity: self.query_limits.default_field_complexity,
            enable_introspection_limits: self.query_limits.enable_introspection_limits,
            per_user_limits: HashMap::new(), // Can be populated from database or config
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 4001,
            database_url: "postgresql://postgres:password@localhost:5432/ugc_db".to_string(),
            jwt_secret: "your-secret-key".to_string(),
            log_level: "info".to_string(),
            cors_origins: vec!["*".to_string()],
            users_service_url: "http://users-service:4002".to_string(),
            offers_service_url: "http://offers-service:4004".to_string(),
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                enabled: true,
                default_ttl_seconds: 300,
                max_connections: 10,
                connection_timeout_seconds: 5,
                command_timeout_seconds: 3,
            },
            query_limits: QueryLimitsConfig {
                max_depth: 10,
                max_complexity: 1000,
                default_field_complexity: 1,
                enable_introspection_limits: false,
                default_rate_limit_per_minute: 60,
            },
        }
    }
}