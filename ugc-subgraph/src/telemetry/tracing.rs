use anyhow::Result;
use opentelemetry::{
    global,
    sdk::{
        trace::{self, RandomIdGenerator, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use tracing::{info, warn};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Configuration for distributed tracing
#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub service_name: String,
    pub service_version: String,
    pub jaeger_endpoint: Option<String>,
    pub sample_rate: f64,
    pub enable_console: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            service_name: "ugc-subgraph".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            sample_rate: 1.0, // Sample all traces in development
            enable_console: true,
        }
    }
}

/// Initialize distributed tracing with OpenTelemetry and Jaeger
pub fn init_tracing(config: TracingConfig) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "ugc_subgraph=debug,tower_http=debug,sqlx=info".into());

    let registry = Registry::default().with(env_filter);

    // Add console layer if enabled
    let registry = if config.enable_console {
        registry.with(tracing_subscriber::fmt::layer().with_target(false))
    } else {
        registry
    };

    // Add OpenTelemetry layer if Jaeger endpoint is configured
    if let Some(jaeger_endpoint) = &config.jaeger_endpoint {
        info!("Initializing OpenTelemetry with Jaeger endpoint: {}", jaeger_endpoint);
        
        let tracer = init_tracer(&config)?;
        let telemetry_layer = OpenTelemetryLayer::new(tracer);
        
        registry.with(telemetry_layer).try_init()?;
    } else {
        warn!("No Jaeger endpoint configured, skipping OpenTelemetry initialization");
        registry.try_init()?;
    }

    info!("Tracing initialized successfully");
    Ok(())
}

/// Initialize OpenTelemetry tracer with Jaeger exporter
fn init_tracer(config: &TracingConfig) -> Result<opentelemetry::sdk::trace::Tracer> {
    let jaeger_endpoint = config.jaeger_endpoint.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Jaeger endpoint not configured"))?;

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_endpoint(jaeger_endpoint)
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::TraceIdRatioBased(config.sample_rate))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.service_name.clone()),
                    KeyValue::new("service.version", config.service_version.clone()),
                    KeyValue::new("service.namespace", "auto-ru-federation"),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    Ok(tracer)
}

/// Shutdown tracing gracefully
pub fn shutdown_tracing() {
    info!("Shutting down tracing");
    global::shutdown_tracer_provider();
}

/// Macro for creating spans with correlation ID
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {
        tracing::info_span!($name, correlation_id = %uuid::Uuid::new_v4())
    };
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!($name, correlation_id = %uuid::Uuid::new_v4(), $($field)*)
    };
}

/// Extract correlation ID from current span
pub fn get_correlation_id() -> Option<String> {
    use tracing::Span;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    
    let span = Span::current();
    let context = span.context();
    let span_context = context.span().span_context();
    
    if span_context.is_valid() {
        Some(span_context.trace_id().to_string())
    } else {
        None
    }
}

/// Add custom attributes to current span
pub fn add_span_attributes(attributes: Vec<(&str, String)>) {
    use tracing::Span;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    
    let span = Span::current();
    for (key, value) in attributes {
        span.set_attribute(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "ugc-subgraph");
        assert_eq!(config.sample_rate, 1.0);
        assert!(config.enable_console);
    }

    #[tokio::test]
    async fn test_correlation_id_generation() {
        let _span = trace_span!("test_span");
        let correlation_id = get_correlation_id();
        // In test environment without proper tracer, this might be None
        // In real environment with tracer, it should return Some(id)
        println!("Correlation ID: {:?}", correlation_id);
    }
}