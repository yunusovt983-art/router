use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Registry, TextEncoder, Encoder,
    register_counter_with_registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
    HistogramOpts, Opts,
};
use std::sync::Arc;
use tracing::{info, error};

/// Metrics registry and collectors
#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,
    
    // HTTP metrics
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_requests_in_flight: IntGauge,
    
    // GraphQL metrics
    pub graphql_requests_total: IntCounter,
    pub graphql_request_duration: Histogram,
    pub graphql_errors_total: IntCounter,
    pub graphql_query_complexity: Histogram,
    
    // Business metrics
    pub reviews_created_total: IntCounter,
    pub reviews_updated_total: IntCounter,
    pub reviews_deleted_total: IntCounter,
    pub reviews_moderated_total: IntCounter,
    pub active_reviews_gauge: IntGauge,
    pub average_rating_gauge: prometheus::Gauge,
    
    // Database metrics
    pub db_connections_active: IntGauge,
    pub db_connections_idle: IntGauge,
    pub db_query_duration: Histogram,
    pub db_queries_total: IntCounter,
    pub db_errors_total: IntCounter,
    
    // External service metrics
    pub external_requests_total: IntCounter,
    pub external_request_duration: Histogram,
    pub external_errors_total: IntCounter,
    pub circuit_breaker_state: IntGauge,
}

impl Metrics {
    /// Create new metrics instance with custom registry
    pub fn new() -> Result<Self> {
        let registry = Arc::new(Registry::new());
        
        // HTTP metrics
        let http_requests_total = register_int_counter_with_registry!(
            Opts::new("http_requests_total", "Total number of HTTP requests")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let http_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let http_requests_in_flight = register_int_gauge_with_registry!(
            Opts::new("http_requests_in_flight", "Number of HTTP requests currently being processed")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        // GraphQL metrics
        let graphql_requests_total = register_int_counter_with_registry!(
            Opts::new("graphql_requests_total", "Total number of GraphQL requests")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let graphql_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("graphql_request_duration_seconds", "GraphQL request duration in seconds")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let graphql_errors_total = register_int_counter_with_registry!(
            Opts::new("graphql_errors_total", "Total number of GraphQL errors")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let graphql_query_complexity = register_histogram_with_registry!(
            HistogramOpts::new("graphql_query_complexity", "GraphQL query complexity score")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        // Business metrics
        let reviews_created_total = register_int_counter_with_registry!(
            Opts::new("reviews_created_total", "Total number of reviews created")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let reviews_updated_total = register_int_counter_with_registry!(
            Opts::new("reviews_updated_total", "Total number of reviews updated")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let reviews_deleted_total = register_int_counter_with_registry!(
            Opts::new("reviews_deleted_total", "Total number of reviews deleted")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let reviews_moderated_total = register_int_counter_with_registry!(
            Opts::new("reviews_moderated_total", "Total number of reviews moderated")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let active_reviews_gauge = register_int_gauge_with_registry!(
            Opts::new("active_reviews_total", "Current number of active reviews")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let average_rating_gauge = prometheus::register_gauge_with_registry!(
            Opts::new("average_rating", "Current average rating across all reviews")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        // Database metrics
        let db_connections_active = register_int_gauge_with_registry!(
            Opts::new("db_connections_active", "Number of active database connections")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let db_connections_idle = register_int_gauge_with_registry!(
            Opts::new("db_connections_idle", "Number of idle database connections")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let db_query_duration = register_histogram_with_registry!(
            HistogramOpts::new("db_query_duration_seconds", "Database query duration in seconds")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let db_queries_total = register_int_counter_with_registry!(
            Opts::new("db_queries_total", "Total number of database queries")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let db_errors_total = register_int_counter_with_registry!(
            Opts::new("db_errors_total", "Total number of database errors")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        // External service metrics
        let external_requests_total = register_int_counter_with_registry!(
            Opts::new("external_requests_total", "Total number of external service requests")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let external_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("external_request_duration_seconds", "External service request duration in seconds")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let external_errors_total = register_int_counter_with_registry!(
            Opts::new("external_errors_total", "Total number of external service errors")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        let circuit_breaker_state = register_int_gauge_with_registry!(
            Opts::new("circuit_breaker_state", "Circuit breaker state (0=closed, 1=open, 2=half-open)")
                .const_labels(prometheus::labels! {"service" => "ugc-subgraph"}),
            registry.clone()
        )?;
        
        Ok(Self {
            registry,
            http_requests_total,
            http_request_duration,
            http_requests_in_flight,
            graphql_requests_total,
            graphql_request_duration,
            graphql_errors_total,
            graphql_query_complexity,
            reviews_created_total,
            reviews_updated_total,
            reviews_deleted_total,
            reviews_moderated_total,
            active_reviews_gauge,
            average_rating_gauge,
            db_connections_active,
            db_connections_idle,
            db_query_duration,
            db_queries_total,
            db_errors_total,
            external_requests_total,
            external_request_duration,
            external_errors_total,
            circuit_breaker_state,
        })
    }
    
    /// Update database connection metrics
    pub fn update_db_connection_metrics(&self, active: i64, idle: i64) {
        self.db_connections_active.set(active);
        self.db_connections_idle.set(idle);
    }
    
    /// Update business metrics
    pub async fn update_business_metrics(&self, pool: &sqlx::PgPool) {
        // Update active reviews count
        if let Ok(count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reviews WHERE is_moderated = true")
            .fetch_one(pool)
            .await
        {
            self.active_reviews_gauge.set(count);
        }
        
        // Update average rating
        if let Ok(avg_rating) = sqlx::query_scalar::<_, Option<rust_decimal::Decimal>>(
            "SELECT AVG(rating) FROM reviews WHERE is_moderated = true"
        )
        .fetch_one(pool)
        .await
        {
            if let Some(rating) = avg_rating {
                self.average_rating_gauge.set(rating.to_f64().unwrap_or(0.0));
            }
        }
    }
}

/// Metrics endpoint handler
pub async fn metrics_handler(State(metrics): State<Arc<Metrics>>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => {
            (
                StatusCode::OK,
                [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                output,
            ).into_response()
        }
        Err(e) => {
            error!("Failed to encode metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode metrics").into_response()
        }
    }
}

/// Create metrics router
pub fn create_metrics_router(metrics: Arc<Metrics>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics)
}

/// Middleware for HTTP metrics collection
pub async fn http_metrics_middleware<B>(
    State(metrics): State<Arc<Metrics>>,
    request: axum::extract::Request<B>,
    next: axum::middleware::Next<B>,
) -> Response {
    let start = std::time::Instant::now();
    metrics.http_requests_in_flight.inc();
    metrics.http_requests_total.inc();
    
    let response = next.run(request).await;
    
    let duration = start.elapsed().as_secs_f64();
    metrics.http_request_duration.observe(duration);
    metrics.http_requests_in_flight.dec();
    
    response
}

/// Timer for measuring operation duration
pub struct MetricsTimer {
    histogram: Histogram,
    start: std::time::Instant,
}

impl MetricsTimer {
    pub fn new(histogram: Histogram) -> Self {
        Self {
            histogram,
            start: std::time::Instant::now(),
        }
    }
}

impl Drop for MetricsTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.observe(duration);
    }
}

/// Macro for timing operations
#[macro_export]
macro_rules! time_operation {
    ($metrics:expr, $histogram:ident, $operation:expr) => {{
        let _timer = $crate::telemetry::metrics::MetricsTimer::new($metrics.$histogram.clone());
        $operation
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        assert_eq!(metrics.http_requests_total.get(), 0);
        assert_eq!(metrics.reviews_created_total.get(), 0);
    }

    #[test]
    fn test_metrics_increment() {
        let metrics = Metrics::new().unwrap();
        metrics.http_requests_total.inc();
        assert_eq!(metrics.http_requests_total.get(), 1);
    }

    #[test]
    fn test_timer() {
        let metrics = Metrics::new().unwrap();
        {
            let _timer = MetricsTimer::new(metrics.http_request_duration.clone());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Timer should have recorded a measurement
        assert!(metrics.http_request_duration.get_sample_count() > 0);
    }
}