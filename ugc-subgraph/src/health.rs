use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

use crate::service::{ExternalServiceClient, ServiceStatus};

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub version: String,
    pub database: DatabaseHealth,
    pub external_services: ExternalServicesHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub status: String,
    pub connection_pool_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServicesHealth {
    pub users_service: ServiceHealthStatus,
    pub offers_service: ServiceHealthStatus,
    pub circuit_breakers: CircuitBreakerStatus,
    pub cache: CacheStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealthStatus {
    pub is_healthy: bool,
    pub last_check: Option<String>,
    pub consecutive_failures: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub users_circuit_breaker: CircuitBreakerInfo,
    pub offers_circuit_breaker: CircuitBreakerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerInfo {
    pub state: String,
    pub failure_count: usize,
    pub success_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatus {
    pub user_cache_size: usize,
    pub offer_cache_size: usize,
}

/// Application state for health checks
#[derive(Clone)]
pub struct HealthState {
    pub db_pool: sqlx::PgPool,
    pub external_client: Arc<ExternalServiceClient>,
}

/// Basic health check endpoint
pub async fn health_check(State(state): State<HealthState>) -> Json<Value> {
    let db_status = check_database_health(&state.db_pool).await;
    
    let overall_status = if db_status.status == "healthy" {
        "healthy"
    } else {
        "unhealthy"
    };

    Json(json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_status
    }))
}

/// Detailed health check endpoint with external services
pub async fn detailed_health_check(State(state): State<HealthState>) -> Json<HealthResponse> {
    let db_health = check_database_health(&state.db_pool).await;
    let external_health = check_external_services_health(&state.external_client).await;
    
    let overall_status = if db_health.status == "healthy" 
        && external_health.users_service.is_healthy 
        && external_health.offers_service.is_healthy {
        "healthy"
    } else {
        "degraded"
    };

    HealthResponse {
        status: overall_status.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: db_health,
        external_services: external_health,
    }
}

/// Readiness probe - checks if service is ready to accept traffic
pub async fn readiness_check(State(state): State<HealthState>) -> Json<Value> {
    let db_status = check_database_health(&state.db_pool).await;
    
    // Service is ready if database is accessible
    let is_ready = db_status.status == "healthy";
    
    Json(json!({
        "ready": is_ready,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "checks": {
            "database": db_status.status
        }
    }))
}

/// Liveness probe - checks if service is alive
pub async fn liveness_check() -> Json<Value> {
    // Simple liveness check - if we can respond, we're alive
    Json(json!({
        "alive": true,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Check database health
async fn check_database_health(pool: &sqlx::PgPool) -> DatabaseHealth {
    match sqlx::query("SELECT 1").fetch_one(pool).await {
        Ok(_) => {
            info!("Database health check passed");
            DatabaseHealth {
                status: "healthy".to_string(),
                connection_pool_size: Some(pool.size()),
            }
        }
        Err(e) => {
            error!("Database health check failed: {}", e);
            DatabaseHealth {
                status: "unhealthy".to_string(),
                connection_pool_size: None,
            }
        }
    }
}

/// Check external services health
async fn check_external_services_health(client: &ExternalServiceClient) -> ExternalServicesHealth {
    let service_status = client.get_service_status().await;
    
    ExternalServicesHealth {
        users_service: map_service_health(service_status.users_health),
        offers_service: map_service_health(service_status.offers_health),
        circuit_breakers: CircuitBreakerStatus {
            users_circuit_breaker: CircuitBreakerInfo {
                state: format!("{:?}", service_status.users_circuit_breaker.state),
                failure_count: service_status.users_circuit_breaker.failure_count,
                success_count: service_status.users_circuit_breaker.success_count,
            },
            offers_circuit_breaker: CircuitBreakerInfo {
                state: format!("{:?}", service_status.offers_circuit_breaker.state),
                failure_count: service_status.offers_circuit_breaker.failure_count,
                success_count: service_status.offers_circuit_breaker.success_count,
            },
        },
        cache: CacheStatus {
            user_cache_size: service_status.cache_stats.user_cache_size,
            offer_cache_size: service_status.cache_stats.offer_cache_size,
        },
    }
}

/// Map internal service health to API response format
fn map_service_health(health: Option<crate::service::cache::ServiceHealth>) -> ServiceHealthStatus {
    match health {
        Some(h) => ServiceHealthStatus {
            is_healthy: h.is_healthy,
            last_check: Some(format!("{:?}", h.last_check)),
            consecutive_failures: h.consecutive_failures,
            last_error: h.last_error,
        },
        None => ServiceHealthStatus {
            is_healthy: true, // Assume healthy if no data
            last_check: None,
            consecutive_failures: 0,
            last_error: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ExternalServiceClient;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_liveness_check() {
        let response = liveness_check().await;
        let value = response.0;
        
        assert_eq!(value["alive"], true);
        assert!(value["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_map_service_health() {
        let health = Some(crate::service::cache::ServiceHealth {
            service_name: "test".to_string(),
            is_healthy: false,
            last_check: std::time::Instant::now(),
            consecutive_failures: 3,
            last_error: Some("Connection failed".to_string()),
        });
        
        let mapped = map_service_health(health);
        assert!(!mapped.is_healthy);
        assert_eq!(mapped.consecutive_failures, 3);
        assert_eq!(mapped.last_error, Some("Connection failed".to_string()));
    }
}