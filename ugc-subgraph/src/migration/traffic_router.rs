use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{info, warn, instrument, error};
use serde::{Deserialize, Serialize};

use crate::migration::feature_flags::FeatureFlagService;
use crate::migration::monitoring::MigrationMetrics;
use crate::auth::UserContext;

/// Traffic routing service for gradual migration
#[derive(Clone)]
pub struct TrafficRouter {
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
    config: TrafficRouterConfig,
}

impl TrafficRouter {
    pub fn new(
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
        config: TrafficRouterConfig,
    ) -> Self {
        Self {
            feature_flags,
            metrics,
            config,
        }
    }

    #[instrument(skip(self, request))]
    pub async fn route_request(&self, request: &Request, user_context: &UserContext) -> RoutingDecision {
        let path = request.uri().path();
        let method = request.method().as_str();
        
        // Determine if this is a REST API request that should be migrated
        let migration_candidate = self.is_migration_candidate(path, method);
        
        if !migration_candidate {
            return RoutingDecision::Legacy;
        }

        // Check feature flags for this user
        let flag_name = self.get_flag_name_for_path(path, method);
        let use_graphql = self.feature_flags.is_enabled(&flag_name, &user_context.user_id.to_string()).await;

        if use_graphql {
            self.metrics.traffic_routed.with_label_values(&["graphql", path]).inc();
            RoutingDecision::GraphQL
        } else {
            self.metrics.traffic_routed.with_label_values(&["rest", path]).inc();
            RoutingDecision::Legacy
        }
    }

    fn is_migration_candidate(&self, path: &str, method: &str) -> bool {
        // Define which endpoints are candidates for migration
        let migration_paths = vec![
            ("/api/v1/reviews", "GET"),
            ("/api/v1/reviews", "POST"),
            ("/api/v1/reviews/", "GET"),
            ("/api/v1/reviews/", "PUT"),
            ("/api/v1/reviews/", "DELETE"),
            ("/api/v1/offers/", "GET"), // for /offers/:id/reviews
            ("/api/v1/users/", "GET"),  // for /users/:id/reviews
        ];

        migration_paths.iter().any(|(p, m)| {
            (path.starts_with(p) || path == *p) && method == *m
        })
    }

    fn get_flag_name_for_path(&self, path: &str, method: &str) -> String {
        match (path, method) {
            (p, "GET") if p.contains("/reviews") => "graphql_reviews_read".to_string(),
            (p, "POST") if p.contains("/reviews") => "graphql_reviews_write".to_string(),
            (p, "PUT") if p.contains("/reviews") => "graphql_reviews_write".to_string(),
            (p, "DELETE") if p.contains("/reviews") => "graphql_reviews_write".to_string(),
            _ => "graphql_reviews_read".to_string(),
        }
    }

    #[instrument(skip(self))]
    pub async fn get_routing_stats(&self) -> RoutingStats {
        // This would collect actual metrics from Prometheus
        // For now, return mock data
        RoutingStats {
            total_requests: 1000,
            graphql_requests: 250,
            rest_requests: 750,
            error_rate: 0.02,
            avg_response_time_ms: 150.0,
        }
    }
}

/// Middleware for automatic traffic routing
pub async fn traffic_routing_middleware(
    State(router): State<Arc<TrafficRouter>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract user context from headers
    let user_context = extract_user_context_from_headers(&headers)?;
    
    // Make routing decision
    let decision = router.route_request(&request, &user_context).await;
    
    // Add routing decision to request extensions for downstream handlers
    let mut request = request;
    request.extensions_mut().insert(decision);
    
    Ok(next.run(request).await)
}

fn extract_user_context_from_headers(headers: &HeaderMap) -> Result<UserContext, StatusCode> {
    // This would extract and validate JWT token from headers
    // For now, return a mock user context
    Ok(UserContext {
        user_id: uuid::Uuid::new_v4(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
    })
}

#[derive(Debug, Clone)]
pub enum RoutingDecision {
    GraphQL,
    Legacy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficRouterConfig {
    pub enable_routing: bool,
    pub default_to_legacy: bool,
    pub routing_rules: Vec<RoutingRule>,
}

impl Default for TrafficRouterConfig {
    fn default() -> Self {
        Self {
            enable_routing: true,
            default_to_legacy: true,
            routing_rules: vec![
                RoutingRule {
                    path_pattern: "/api/v1/reviews".to_string(),
                    methods: vec!["GET".to_string(), "POST".to_string()],
                    feature_flag: "graphql_reviews".to_string(),
                    priority: 1,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub path_pattern: String,
    pub methods: Vec<String>,
    pub feature_flag: String,
    pub priority: i32,
}

#[derive(Debug, Serialize)]
pub struct RoutingStats {
    pub total_requests: u64,
    pub graphql_requests: u64,
    pub rest_requests: u64,
    pub error_rate: f64,
    pub avg_response_time_ms: f64,
}

/// Load balancer for gradual traffic shifting
#[derive(Clone)]
pub struct LoadBalancer {
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
}

impl LoadBalancer {
    pub fn new(
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
    ) -> Self {
        Self {
            feature_flags,
            metrics,
        }
    }

    #[instrument(skip(self))]
    pub async fn should_use_graphql(&self, user_id: &str, endpoint: &str) -> bool {
        let flag_name = match endpoint {
            path if path.contains("reviews") && path.contains("GET") => "graphql_reviews_read",
            path if path.contains("reviews") => "graphql_reviews_write",
            _ => "graphql_reviews_read",
        };

        let use_graphql = self.feature_flags.is_enabled(flag_name, user_id).await;
        
        if use_graphql {
            self.metrics.load_balancer_decisions.with_label_values(&["graphql", endpoint]).inc();
        } else {
            self.metrics.load_balancer_decisions.with_label_values(&["rest", endpoint]).inc();
        }

        use_graphql
    }

    #[instrument(skip(self))]
    pub async fn record_response_time(&self, endpoint: &str, backend: &str, duration_ms: f64) {
        self.metrics.response_time_histogram
            .with_label_values(&[backend, endpoint])
            .observe(duration_ms / 1000.0);
    }

    #[instrument(skip(self))]
    pub async fn record_error(&self, endpoint: &str, backend: &str, error_type: &str) {
        self.metrics.error_counter
            .with_label_values(&[backend, endpoint, error_type])
            .inc();
    }
}

/// Circuit breaker for migration safety
#[derive(Clone)]
pub struct MigrationCircuitBreaker {
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
    config: CircuitBreakerConfig,
}

impl MigrationCircuitBreaker {
    pub fn new(
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
        config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            feature_flags,
            metrics,
            config,
        }
    }

    #[instrument(skip(self))]
    pub async fn check_circuit_breaker(&self, endpoint: &str) -> CircuitBreakerState {
        // Check error rate for GraphQL backend
        let error_rate = self.get_error_rate(endpoint, "graphql").await;
        
        if error_rate > self.config.error_threshold {
            warn!("Circuit breaker OPEN for endpoint {} (error rate: {})", endpoint, error_rate);
            
            // Automatically disable GraphQL for this endpoint
            self.emergency_fallback(endpoint).await;
            
            self.metrics.circuit_breaker_state
                .with_label_values(&[endpoint, "open"])
                .set(1.0);
            
            CircuitBreakerState::Open
        } else if error_rate > self.config.warning_threshold {
            warn!("Circuit breaker HALF_OPEN for endpoint {} (error rate: {})", endpoint, error_rate);
            
            self.metrics.circuit_breaker_state
                .with_label_values(&[endpoint, "half_open"])
                .set(0.5);
            
            CircuitBreakerState::HalfOpen
        } else {
            self.metrics.circuit_breaker_state
                .with_label_values(&[endpoint, "closed"])
                .set(0.0);
            
            CircuitBreakerState::Closed
        }
    }

    async fn get_error_rate(&self, endpoint: &str, backend: &str) -> f64 {
        // This would query Prometheus for actual error rates
        // For now, return a mock value
        0.01 // 1% error rate
    }

    async fn emergency_fallback(&self, endpoint: &str) {
        let flag_name = if endpoint.contains("reviews") {
            if endpoint.contains("GET") {
                "graphql_reviews_read"
            } else {
                "graphql_reviews_write"
            }
        } else {
            "graphql_reviews_read"
        };

        if let Err(e) = self.feature_flags.set_rollout_percentage(flag_name, 0.0).await {
            error!("Failed to disable feature flag {}: {}", flag_name, e);
        } else {
            info!("Emergency fallback: disabled {} due to high error rate", flag_name);
        }
    }
}

#[derive(Debug, Clone)]
pub enum CircuitBreakerState {
    Closed,
    HalfOpen,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub error_threshold: f64,
    pub warning_threshold: f64,
    pub recovery_timeout_seconds: u64,
    pub min_requests_threshold: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            error_threshold: 0.1,  // 10% error rate triggers circuit breaker
            warning_threshold: 0.05, // 5% error rate triggers warning
            recovery_timeout_seconds: 60,
            min_requests_threshold: 10,
        }
    }
}

/// Canary deployment controller
#[derive(Clone)]
pub struct CanaryController {
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
}

impl CanaryController {
    pub fn new(
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
    ) -> Self {
        Self {
            feature_flags,
            metrics,
        }
    }

    #[instrument(skip(self))]
    pub async fn start_canary_deployment(&self, flag_name: &str) -> Result<(), String> {
        info!("Starting canary deployment for flag '{}'", flag_name);

        // Start with 1% traffic
        self.feature_flags.set_rollout_percentage(flag_name, 1.0).await?;
        
        // Enable the flag
        if let Some(mut flag) = self.feature_flags.get_flag(flag_name).await {
            flag.enabled = true;
            self.feature_flags.update_flag(flag_name, flag).await?;
        }

        self.metrics.canary_deployments.with_label_values(&[flag_name, "started"]).inc();
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn promote_canary(&self, flag_name: &str, target_percentage: f64) -> Result<(), String> {
        info!("Promoting canary for flag '{}' to {}%", flag_name, target_percentage);

        if let Some(current_flag) = self.feature_flags.get_flag(flag_name).await {
            let current_percentage = current_flag.rollout_percentage;
            
            if target_percentage <= current_percentage {
                return Err("Target percentage must be higher than current percentage".to_string());
            }

            // Gradually increase rollout percentage
            let step_size = 5.0;
            let mut current = current_percentage;
            
            while current < target_percentage {
                current = (current + step_size).min(target_percentage);
                self.feature_flags.set_rollout_percentage(flag_name, current).await?;
                
                info!("Promoted canary for flag '{}' to {}%", flag_name, current);
                
                // Wait between steps to monitor for issues
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                
                // Check for issues before continuing
                if self.should_halt_promotion(flag_name).await {
                    warn!("Halting canary promotion for flag '{}' due to issues", flag_name);
                    break;
                }
            }

            self.metrics.canary_deployments.with_label_values(&[flag_name, "promoted"]).inc();
        } else {
            return Err(format!("Feature flag '{}' not found", flag_name));
        }

        Ok(())
    }

    async fn should_halt_promotion(&self, _flag_name: &str) -> bool {
        // This would check various health metrics
        // For now, always return false (continue promotion)
        false
    }

    #[instrument(skip(self))]
    pub async fn rollback_canary(&self, flag_name: &str) -> Result<(), String> {
        warn!("Rolling back canary deployment for flag '{}'", flag_name);

        self.feature_flags.set_rollout_percentage(flag_name, 0.0).await?;
        
        if let Some(mut flag) = self.feature_flags.get_flag(flag_name).await {
            flag.enabled = false;
            self.feature_flags.update_flag(flag_name, flag).await?;
        }

        self.metrics.canary_deployments.with_label_values(&[flag_name, "rolled_back"]).inc();
        info!("Canary rollback completed for flag '{}'", flag_name);
        Ok(())
    }
}