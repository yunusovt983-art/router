use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, instrument, warn, info, debug, Span};
use uuid::Uuid;

use crate::error::UgcError;
use crate::telemetry::{
    logging::BusinessEventLogger,
    tracing::add_span_attributes,
};
use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, RetryMechanism, RetryConfig};
use super::cache::{FallbackDataProvider, ServiceHealthMonitor};

/// External service client for communicating with other subgraphs
#[derive(Clone)]
pub struct ExternalServiceClient {
    client: Client,
    users_service_url: String,
    offers_service_url: String,
    timeout: Duration,
    users_circuit_breaker: Arc<CircuitBreaker>,
    offers_circuit_breaker: Arc<CircuitBreaker>,
    retry_mechanism: Arc<RetryMechanism>,
    fallback_provider: Arc<FallbackDataProvider>,
    health_monitor: Arc<ServiceHealthMonitor>,
}

impl ExternalServiceClient {
    pub fn new(users_service_url: String, offers_service_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        // Configure circuit breakers for each service
        let cb_config = CircuitBreakerConfig {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
        };

        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        };

        Self {
            client,
            users_service_url,
            offers_service_url,
            timeout: Duration::from_secs(10),
            users_circuit_breaker: Arc::new(CircuitBreaker::new("users".to_string(), cb_config.clone())),
            offers_circuit_breaker: Arc::new(CircuitBreaker::new("offers".to_string(), cb_config)),
            retry_mechanism: Arc::new(RetryMechanism::new(retry_config)),
            fallback_provider: Arc::new(FallbackDataProvider::new()),
            health_monitor: Arc::new(ServiceHealthMonitor::new()),
        }
    }

    pub fn with_config(
        users_service_url: String,
        offers_service_url: String,
        circuit_breaker_config: CircuitBreakerConfig,
        retry_config: RetryConfig,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            users_service_url,
            offers_service_url,
            timeout: Duration::from_secs(10),
            users_circuit_breaker: Arc::new(CircuitBreaker::new("users".to_string(), circuit_breaker_config.clone())),
            offers_circuit_breaker: Arc::new(CircuitBreaker::new("offers".to_string(), circuit_breaker_config)),
            retry_mechanism: Arc::new(RetryMechanism::new(retry_config)),
            fallback_provider: Arc::new(FallbackDataProvider::new()),
            health_monitor: Arc::new(ServiceHealthMonitor::new()),
        }
    }

    /// Get user by ID from Users subgraph
    #[instrument(skip(self), fields(user_id = %user_id, service = "users"))]
    pub async fn get_user(&self, user_id: Uuid) -> Result<Option<ExternalUser>, UgcError> {
        let start_time = std::time::Instant::now();
        info!("Fetching user {} from Users service", user_id);
        
        add_span_attributes(vec![
            ("operation", "get_user".to_string()),
            ("service", "users".to_string()),
            ("user_id", user_id.to_string()),
        ]);

        let client = self.client.clone();
        let url = format!("{}/users/{}", self.users_service_url, user_id);
        let timeout = self.timeout;

        // Use circuit breaker and retry mechanism
        let result = self.users_circuit_breaker
            .call(|| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    self.retry_mechanism
                        .call(|| {
                            let client = client.clone();
                            let url = url.clone();
                            async move {
                                self.make_user_request(client, url, user_id, timeout).await
                            }
                        })
                        .await
                }
            })
            .await;

        let duration = start_time.elapsed();
        
        match &result {
            Ok(Some(user)) => {
                info!("Successfully fetched user {} in {:?}", user_id, duration);
                // Cache successful response and record health
                self.fallback_provider.cache_user(user).await;
                self.health_monitor.record_success("users").await;
            }
            Ok(None) => {
                info!("User {} not found in {:?}", user_id, duration);
            }
            Err(err) => {
                error!("Failed to fetch user {} in {:?}: {}", user_id, duration, err);
                // Record failure for health monitoring
                self.health_monitor.record_failure("users", &err.to_string()).await;
                
                // Log business event for external service error
                BusinessEventLogger::external_service_error(
                    "users",
                    &err.to_string(),
                    duration.as_millis() as u64,
                );
            }
        }

        result
    }

    /// Make the actual HTTP request to users service
    async fn make_user_request(
        &self,
        client: Client,
        url: String,
        user_id: Uuid,
        timeout: Duration,
    ) -> Result<Option<ExternalUser>, UgcError> {
        match client.get(&url).timeout(timeout).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<ExternalUser>().await {
                        Ok(user) => Ok(Some(user)),
                        Err(e) => {
                            error!("Failed to parse user response: {}", e);
                            Err(UgcError::ExternalServiceError {
                                service: "users".to_string(),
                                message: "Failed to parse response".to_string(),
                            })
                        }
                    }
                } else if response.status() == 404 {
                    Ok(None)
                } else {
                    warn!("Users service returned error: {}", response.status());
                    
                    // Determine if error is retryable based on status code
                    let error = if response.status().is_server_error() {
                        UgcError::ExternalServiceError {
                            service: "users".to_string(),
                            message: format!("HTTP {}", response.status()),
                        }
                    } else {
                        // Client errors are not retryable
                        UgcError::ValidationError {
                            message: format!("Users service client error: {}", response.status()),
                        }
                    };
                    
                    Err(error)
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    error!("Users service timeout: {}", e);
                    Err(UgcError::ServiceTimeout {
                        service: "users".to_string(),
                    })
                } else {
                    error!("Failed to call users service: {}", e);
                    Err(UgcError::ExternalServiceError {
                        service: "users".to_string(),
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    /// Get offer by ID from Offers subgraph
    #[instrument(skip(self))]
    pub async fn get_offer(&self, offer_id: Uuid) -> Result<Option<ExternalOffer>, UgcError> {
        let client = self.client.clone();
        let url = format!("{}/offers/{}", self.offers_service_url, offer_id);
        let timeout = self.timeout;

        // Use circuit breaker and retry mechanism
        let result = self.offers_circuit_breaker
            .call(|| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    self.retry_mechanism
                        .call(|| {
                            let client = client.clone();
                            let url = url.clone();
                            async move {
                                self.make_offer_request(client, url, offer_id, timeout).await
                            }
                        })
                        .await
                }
            })
            .await;

        match &result {
            Ok(Some(offer)) => {
                // Cache successful response and record health
                self.fallback_provider.cache_offer(offer).await;
                self.health_monitor.record_success("offers").await;
            }
            Err(err) => {
                // Record failure for health monitoring
                self.health_monitor.record_failure("offers", &err.to_string()).await;
            }
            _ => {}
        }

        result
    }

    /// Make the actual HTTP request to offers service
    async fn make_offer_request(
        &self,
        client: Client,
        url: String,
        offer_id: Uuid,
        timeout: Duration,
    ) -> Result<Option<ExternalOffer>, UgcError> {
        match client.get(&url).timeout(timeout).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<ExternalOffer>().await {
                        Ok(offer) => Ok(Some(offer)),
                        Err(e) => {
                            error!("Failed to parse offer response: {}", e);
                            Err(UgcError::ExternalServiceError {
                                service: "offers".to_string(),
                                message: "Failed to parse response".to_string(),
                            })
                        }
                    }
                } else if response.status() == 404 {
                    Ok(None)
                } else {
                    warn!("Offers service returned error: {}", response.status());
                    
                    // Determine if error is retryable based on status code
                    let error = if response.status().is_server_error() {
                        UgcError::ExternalServiceError {
                            service: "offers".to_string(),
                            message: format!("HTTP {}", response.status()),
                        }
                    } else {
                        // Client errors are not retryable
                        UgcError::ValidationError {
                            message: format!("Offers service client error: {}", response.status()),
                        }
                    };
                    
                    Err(error)
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    error!("Offers service timeout: {}", e);
                    Err(UgcError::ServiceTimeout {
                        service: "offers".to_string(),
                    })
                } else {
                    error!("Failed to call offers service: {}", e);
                    Err(UgcError::ExternalServiceError {
                        service: "offers".to_string(),
                        message: e.to_string(),
                    })
                }
            }
        }
    }

    /// Get user with graceful degradation to cached/minimal data
    #[instrument(skip(self))]
    pub async fn get_user_with_fallback(&self, user_id: Uuid) -> ExternalUser {
        match self.get_user(user_id).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!("User {} not found, using fallback", user_id);
                self.fallback_provider.get_user_fallback(user_id).await
            }
            Err(e) => {
                error!("Failed to fetch user {}: {}, using fallback", user_id, e);
                self.fallback_provider.get_user_fallback(user_id).await
            }
        }
    }

    /// Get offer with graceful degradation to cached/minimal data
    #[instrument(skip(self))]
    pub async fn get_offer_with_fallback(&self, offer_id: Uuid) -> ExternalOffer {
        match self.get_offer(offer_id).await {
            Ok(Some(offer)) => offer,
            Ok(None) => {
                warn!("Offer {} not found, using fallback", offer_id);
                self.fallback_provider.get_offer_fallback(offer_id).await
            }
            Err(e) => {
                error!("Failed to fetch offer {}: {}, using fallback", offer_id, e);
                self.fallback_provider.get_offer_fallback(offer_id).await
            }
        }
    }

    /// Get circuit breaker metrics for monitoring
    pub async fn get_users_circuit_breaker_metrics(&self) -> super::circuit_breaker::CircuitBreakerMetrics {
        self.users_circuit_breaker.get_metrics().await
    }

    /// Get circuit breaker metrics for monitoring
    pub async fn get_offers_circuit_breaker_metrics(&self) -> super::circuit_breaker::CircuitBreakerMetrics {
        self.offers_circuit_breaker.get_metrics().await
    }

    /// Get service health status
    pub async fn get_service_health(&self, service_name: &str) -> Option<super::cache::ServiceHealth> {
        self.health_monitor.get_service_health(service_name).await
    }

    /// Get all service health statuses
    pub async fn get_all_service_health(&self) -> std::collections::HashMap<String, super::cache::ServiceHealth> {
        self.health_monitor.get_all_health_status().await
    }

    /// Check if a service is healthy
    pub async fn is_service_healthy(&self, service_name: &str) -> bool {
        self.health_monitor.is_service_healthy(service_name).await
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> super::cache::CacheStats {
        self.fallback_provider.get_cache_stats().await
    }

    /// Cleanup expired cache entries
    pub async fn cleanup_expired_cache(&self) {
        self.fallback_provider.cleanup_expired().await
    }

    /// Get comprehensive service status for monitoring
    pub async fn get_service_status(&self) -> ServiceStatus {
        ServiceStatus {
            users_circuit_breaker: self.get_users_circuit_breaker_metrics().await,
            offers_circuit_breaker: self.get_offers_circuit_breaker_metrics().await,
            users_health: self.get_service_health("users").await,
            offers_health: self.get_service_health("offers").await,
            cache_stats: self.get_cache_stats().await,
        }
    }
}

/// Comprehensive service status for monitoring
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub users_circuit_breaker: super::circuit_breaker::CircuitBreakerMetrics,
    pub offers_circuit_breaker: super::circuit_breaker::CircuitBreakerMetrics,
    pub users_health: Option<super::cache::ServiceHealth>,
    pub offers_health: Option<super::cache::ServiceHealth>,
    pub cache_stats: super::cache::CacheStats,
}

/// External user data from Users subgraph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalUser {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
}

/// External offer data from Offers subgraph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalOffer {
    pub id: Uuid,
    pub title: String,
    pub price: Option<i32>,
}

/// Trait for external service operations
#[async_trait]
pub trait ExternalService: Send + Sync {
    async fn get_user(&self, user_id: Uuid) -> Result<Option<ExternalUser>, UgcError>;
    async fn get_offer(&self, offer_id: Uuid) -> Result<Option<ExternalOffer>, UgcError>;
}

#[async_trait]
impl ExternalService for ExternalServiceClient {
    async fn get_user(&self, user_id: Uuid) -> Result<Option<ExternalUser>, UgcError> {
        self.get_user(user_id).await
    }

    async fn get_offer(&self, offer_id: Uuid) -> Result<Option<ExternalOffer>, UgcError> {
        self.get_offer(offer_id).await
    }
}

/// Mock external service for testing
#[derive(Clone)]
pub struct MockExternalService;

#[async_trait]
impl ExternalService for MockExternalService {
    async fn get_user(&self, user_id: Uuid) -> Result<Option<ExternalUser>, UgcError> {
        Ok(Some(ExternalUser {
            id: user_id,
            name: format!("Test User {}", user_id),
            email: Some(format!("user{}@example.com", user_id)),
        }))
    }

    async fn get_offer(&self, offer_id: Uuid) -> Result<Option<ExternalOffer>, UgcError> {
        Ok(Some(ExternalOffer {
            id: offer_id,
            title: format!("Test Offer {}", offer_id),
            price: Some(1000000),
        }))
    }
}