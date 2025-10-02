use prometheus::{
    Counter, Histogram, Gauge, IntCounter, IntGauge,
    register_counter, register_histogram, register_gauge, register_int_counter, register_int_gauge,
    opts, HistogramOpts,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, instrument, error};
use tokio::time::{Duration, interval};

/// Metrics collection for migration monitoring
#[derive(Clone)]
pub struct MigrationMetrics {
    // Request routing metrics
    pub rest_request_total: Counter,
    pub graphql_migration_requests: Counter,
    pub legacy_rest_requests: Counter,
    pub traffic_routed: Counter,
    pub load_balancer_decisions: Counter,

    // Performance metrics
    pub response_time_histogram: Histogram,
    pub graphql_query_duration: Histogram,
    pub rest_endpoint_duration: Histogram,

    // Error tracking
    pub error_counter: Counter,
    pub graphql_errors: Counter,
    pub rest_errors: Counter,

    // Feature flag metrics
    pub feature_flag_evaluations: Counter,
    pub feature_flag_cache_hits: Counter,
    pub feature_flag_cache_misses: Counter,

    // Circuit breaker metrics
    pub circuit_breaker_state: Gauge,
    pub circuit_breaker_trips: Counter,

    // Canary deployment metrics
    pub canary_deployments: Counter,
    pub canary_success_rate: Gauge,

    // Business metrics
    pub migration_completion_percentage: Gauge,
    pub active_users_on_graphql: IntGauge,
    pub data_consistency_checks: Counter,
}

impl MigrationMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        Ok(Self {
            rest_request_total: register_counter!(
                "migration_rest_requests_total",
                "Total number of REST API requests during migration",
                &["method", "endpoint"]
            )?,
            
            graphql_migration_requests: register_counter!(
                "migration_graphql_requests_total", 
                "Total number of requests routed to GraphQL during migration",
                &["operation", "type"]
            )?,
            
            legacy_rest_requests: register_counter!(
                "migration_legacy_requests_total",
                "Total number of requests still using legacy REST",
                &["operation", "type"]
            )?,
            
            traffic_routed: register_counter!(
                "migration_traffic_routed_total",
                "Total traffic routed by migration system",
                &["backend", "endpoint"]
            )?,
            
            load_balancer_decisions: register_counter!(
                "migration_load_balancer_decisions_total",
                "Load balancer routing decisions",
                &["backend", "endpoint"]
            )?,

            response_time_histogram: register_histogram!(
                HistogramOpts::new(
                    "migration_response_time_seconds",
                    "Response time distribution during migration"
                ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
                &["backend", "endpoint"]
            )?,
            
            graphql_query_duration: register_histogram!(
                HistogramOpts::new(
                    "migration_graphql_query_duration_seconds",
                    "GraphQL query execution time during migration"
                ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
                &["operation", "complexity"]
            )?,
            
            rest_endpoint_duration: register_histogram!(
                HistogramOpts::new(
                    "migration_rest_endpoint_duration_seconds", 
                    "REST endpoint response time during migration"
                ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
                &["method", "endpoint"]
            )?,

            error_counter: register_counter!(
                "migration_errors_total",
                "Total errors during migration",
                &["backend", "endpoint", "error_type"]
            )?,
            
            graphql_errors: register_counter!(
                "migration_graphql_errors_total",
                "GraphQL-specific errors during migration", 
                &["operation", "error_type"]
            )?,
            
            rest_errors: register_counter!(
                "migration_rest_errors_total",
                "REST API errors during migration",
                &["method", "endpoint", "status_code"]
            )?,

            feature_flag_evaluations: register_counter!(
                "migration_feature_flag_evaluations_total",
                "Feature flag evaluations during migration",
                &["flag_name", "result"]
            )?,
            
            feature_flag_cache_hits: register_counter!(
                "migration_feature_flag_cache_hits_total",
                "Feature flag cache hits"
            )?,
            
            feature_flag_cache_misses: register_counter!(
                "migration_feature_flag_cache_misses_total", 
                "Feature flag cache misses"
            )?,

            circuit_breaker_state: register_gauge!(
                "migration_circuit_breaker_state",
                "Circuit breaker state (0=closed, 0.5=half-open, 1=open)",
                &["endpoint", "state"]
            )?,
            
            circuit_breaker_trips: register_counter!(
                "migration_circuit_breaker_trips_total",
                "Circuit breaker trips during migration",
                &["endpoint", "reason"]
            )?,

            canary_deployments: register_counter!(
                "migration_canary_deployments_total",
                "Canary deployment events",
                &["flag_name", "event_type"]
            )?,
            
            canary_success_rate: register_gauge!(
                "migration_canary_success_rate",
                "Success rate of canary deployments",
                &["flag_name"]
            )?,

            migration_completion_percentage: register_gauge!(
                "migration_completion_percentage",
                "Overall migration completion percentage"
            )?,
            
            active_users_on_graphql: register_int_gauge!(
                "migration_active_users_graphql",
                "Number of active users using GraphQL endpoints"
            )?,
            
            data_consistency_checks: register_counter!(
                "migration_data_consistency_checks_total",
                "Data consistency checks between REST and GraphQL",
                &["check_type", "result"]
            )?,
        })
    }

    #[instrument(skip(self))]
    pub async fn record_request(&self, backend: &str, endpoint: &str, duration: Duration, success: bool) {
        self.response_time_histogram
            .with_label_values(&[backend, endpoint])
            .observe(duration.as_secs_f64());

        if !success {
            self.error_counter
                .with_label_values(&[backend, endpoint, "request_failed"])
                .inc();
        }
    }

    #[instrument(skip(self))]
    pub async fn record_feature_flag_evaluation(&self, flag_name: &str, enabled: bool, from_cache: bool) {
        self.feature_flag_evaluations
            .with_label_values(&[flag_name, if enabled { "enabled" } else { "disabled" }])
            .inc();

        if from_cache {
            self.feature_flag_cache_hits.inc();
        } else {
            self.feature_flag_cache_misses.inc();
        }
    }

    #[instrument(skip(self))]
    pub async fn update_migration_progress(&self, percentage: f64) {
        self.migration_completion_percentage.set(percentage);
        info!("Migration progress updated to {}%", percentage);
    }

    #[instrument(skip(self))]
    pub async fn record_data_consistency_check(&self, check_type: &str, consistent: bool) {
        self.data_consistency_checks
            .with_label_values(&[check_type, if consistent { "consistent" } else { "inconsistent" }])
            .inc();

        if !consistent {
            warn!("Data consistency check failed for type: {}", check_type);
        }
    }
}

/// Migration dashboard data collector
#[derive(Clone)]
pub struct MigrationDashboard {
    metrics: Arc<MigrationMetrics>,
}

impl MigrationDashboard {
    pub fn new(metrics: Arc<MigrationMetrics>) -> Self {
        Self { metrics }
    }

    #[instrument(skip(self))]
    pub async fn get_migration_status(&self) -> MigrationStatus {
        // This would collect real metrics from Prometheus
        // For now, return mock data
        MigrationStatus {
            overall_progress: 45.0,
            graphql_adoption_rate: 35.0,
            error_rate_comparison: ErrorRateComparison {
                graphql_error_rate: 0.02,
                rest_error_rate: 0.015,
            },
            performance_comparison: PerformanceComparison {
                graphql_avg_response_time: 120.0,
                rest_avg_response_time: 95.0,
            },
            active_feature_flags: vec![
                FeatureFlagStatus {
                    name: "graphql_reviews_read".to_string(),
                    enabled: true,
                    rollout_percentage: 25.0,
                    affected_users: 1250,
                },
                FeatureFlagStatus {
                    name: "graphql_reviews_write".to_string(),
                    enabled: true,
                    rollout_percentage: 10.0,
                    affected_users: 500,
                },
            ],
            circuit_breaker_status: vec![
                CircuitBreakerStatus {
                    endpoint: "/api/v1/reviews".to_string(),
                    state: "closed".to_string(),
                    error_rate: 0.01,
                    last_trip: None,
                },
            ],
        }
    }

    #[instrument(skip(self))]
    pub async fn get_real_time_metrics(&self) -> RealTimeMetrics {
        RealTimeMetrics {
            requests_per_second: RequestsPerSecond {
                graphql: 45.2,
                rest: 123.8,
                total: 169.0,
            },
            current_error_rates: CurrentErrorRates {
                graphql: 0.018,
                rest: 0.012,
            },
            response_times: ResponseTimes {
                graphql_p95: 180.0,
                rest_p95: 145.0,
                graphql_p99: 350.0,
                rest_p99: 280.0,
            },
            active_users: ActiveUsers {
                total: 5000,
                on_graphql: 1750,
                on_rest: 3250,
            },
        }
    }

    pub async fn start_background_collection(&self) {
        let metrics = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Collect and update migration progress
                let progress = calculate_migration_progress().await;
                metrics.update_migration_progress(progress).await;
                
                // Update active users count
                let active_users = count_active_graphql_users().await;
                metrics.active_users_on_graphql.set(active_users);
                
                // Perform data consistency checks
                perform_data_consistency_checks(&metrics).await;
            }
        });
    }
}

async fn calculate_migration_progress() -> f64 {
    // This would calculate actual migration progress based on various metrics
    // For now, return a mock value
    45.0
}

async fn count_active_graphql_users() -> i64 {
    // This would count actual active users on GraphQL endpoints
    // For now, return a mock value
    1750
}

async fn perform_data_consistency_checks(metrics: &Arc<MigrationMetrics>) {
    // Perform various consistency checks between REST and GraphQL responses
    
    // Mock consistency check
    let consistent = true; // In reality, this would perform actual checks
    metrics.record_data_consistency_check("review_data", consistent).await;
}

/// Alert manager for migration issues
#[derive(Clone)]
pub struct MigrationAlertManager {
    metrics: Arc<MigrationMetrics>,
    config: AlertConfig,
}

impl MigrationAlertManager {
    pub fn new(metrics: Arc<MigrationMetrics>, config: AlertConfig) -> Self {
        Self { metrics, config }
    }

    pub async fn start_monitoring(&self) {
        let metrics = Arc::clone(&self.metrics);
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                // Check error rate thresholds
                Self::check_error_rates(&metrics, &config).await;
                
                // Check performance degradation
                Self::check_performance_degradation(&metrics, &config).await;
                
                // Check circuit breaker states
                Self::check_circuit_breakers(&metrics, &config).await;
            }
        });
    }

    async fn check_error_rates(metrics: &Arc<MigrationMetrics>, config: &AlertConfig) {
        // This would check actual error rates from Prometheus
        let graphql_error_rate = 0.025; // Mock value
        
        if graphql_error_rate > config.error_rate_threshold {
            Self::send_alert(AlertType::HighErrorRate {
                backend: "graphql".to_string(),
                rate: graphql_error_rate,
                threshold: config.error_rate_threshold,
            }).await;
        }
    }

    async fn check_performance_degradation(metrics: &Arc<MigrationMetrics>, config: &AlertConfig) {
        // This would check actual performance metrics
        let graphql_p95 = 250.0; // Mock value in ms
        
        if graphql_p95 > config.response_time_threshold {
            Self::send_alert(AlertType::PerformanceDegradation {
                backend: "graphql".to_string(),
                p95_ms: graphql_p95,
                threshold_ms: config.response_time_threshold,
            }).await;
        }
    }

    async fn check_circuit_breakers(_metrics: &Arc<MigrationMetrics>, _config: &AlertConfig) {
        // This would check circuit breaker states
        // For now, no alerts
    }

    async fn send_alert(alert: AlertType) {
        match alert {
            AlertType::HighErrorRate { backend, rate, threshold } => {
                error!("ALERT: High error rate detected - Backend: {}, Rate: {:.3}, Threshold: {:.3}", 
                       backend, rate, threshold);
                // In production, this would send to Slack, PagerDuty, etc.
            }
            AlertType::PerformanceDegradation { backend, p95_ms, threshold_ms } => {
                error!("ALERT: Performance degradation detected - Backend: {}, P95: {:.1}ms, Threshold: {:.1}ms", 
                       backend, p95_ms, threshold_ms);
            }
            AlertType::CircuitBreakerOpen { endpoint } => {
                error!("ALERT: Circuit breaker opened for endpoint: {}", endpoint);
            }
        }
    }
}

// Data structures for monitoring
#[derive(Debug, Serialize)]
pub struct MigrationStatus {
    pub overall_progress: f64,
    pub graphql_adoption_rate: f64,
    pub error_rate_comparison: ErrorRateComparison,
    pub performance_comparison: PerformanceComparison,
    pub active_feature_flags: Vec<FeatureFlagStatus>,
    pub circuit_breaker_status: Vec<CircuitBreakerStatus>,
}

#[derive(Debug, Serialize)]
pub struct ErrorRateComparison {
    pub graphql_error_rate: f64,
    pub rest_error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct PerformanceComparison {
    pub graphql_avg_response_time: f64,
    pub rest_avg_response_time: f64,
}

#[derive(Debug, Serialize)]
pub struct FeatureFlagStatus {
    pub name: String,
    pub enabled: bool,
    pub rollout_percentage: f64,
    pub affected_users: u64,
}

#[derive(Debug, Serialize)]
pub struct CircuitBreakerStatus {
    pub endpoint: String,
    pub state: String,
    pub error_rate: f64,
    pub last_trip: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RealTimeMetrics {
    pub requests_per_second: RequestsPerSecond,
    pub current_error_rates: CurrentErrorRates,
    pub response_times: ResponseTimes,
    pub active_users: ActiveUsers,
}

#[derive(Debug, Serialize)]
pub struct RequestsPerSecond {
    pub graphql: f64,
    pub rest: f64,
    pub total: f64,
}

#[derive(Debug, Serialize)]
pub struct CurrentErrorRates {
    pub graphql: f64,
    pub rest: f64,
}

#[derive(Debug, Serialize)]
pub struct ResponseTimes {
    pub graphql_p95: f64,
    pub rest_p95: f64,
    pub graphql_p99: f64,
    pub rest_p99: f64,
}

#[derive(Debug, Serialize)]
pub struct ActiveUsers {
    pub total: u64,
    pub on_graphql: u64,
    pub on_rest: u64,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub error_rate_threshold: f64,
    pub response_time_threshold: f64,
    pub enable_alerts: bool,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            error_rate_threshold: 0.05, // 5%
            response_time_threshold: 500.0, // 500ms
            enable_alerts: true,
        }
    }
}

#[derive(Debug)]
pub enum AlertType {
    HighErrorRate {
        backend: String,
        rate: f64,
        threshold: f64,
    },
    PerformanceDegradation {
        backend: String,
        p95_ms: f64,
        threshold_ms: f64,
    },
    CircuitBreakerOpen {
        endpoint: String,
    },
}