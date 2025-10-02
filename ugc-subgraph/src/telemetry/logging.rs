use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};
use uuid::Uuid;

/// Structured JSON formatter for logs
pub struct JsonFormatter;

impl<S, N> FormatEvent<S, N> for JsonFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        let metadata = event.metadata();
        let mut fields = HashMap::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        // Get correlation ID from current span
        let correlation_id = ctx
            .lookup_current()
            .and_then(|span| {
                span.extensions()
                    .get::<CorrelationId>()
                    .map(|id| id.0.to_string())
            })
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let log_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "module": metadata.module_path(),
            "file": metadata.file(),
            "line": metadata.line(),
            "correlation_id": correlation_id,
            "service": "ugc-subgraph",
            "version": env!("CARGO_PKG_VERSION"),
            "fields": fields,
        });

        writeln!(writer, "{}", log_entry)?;
        Ok(())
    }
}

/// Visitor for extracting fields from tracing events
struct JsonVisitor<'a>(&'a mut HashMap<String, Value>);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_string(), json!(format!("{:?}", value)));
    }
}

/// Correlation ID for request tracing
#[derive(Debug, Clone)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

/// Extract correlation ID from HTTP headers
pub fn extract_correlation_id(headers: &axum::http::HeaderMap) -> CorrelationId {
    headers
        .get("x-correlation-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| CorrelationId::from_string(s).ok())
        .unwrap_or_else(CorrelationId::new)
}

/// Middleware for adding correlation ID to requests
pub async fn correlation_middleware<B>(
    mut request: axum::extract::Request<B>,
    next: axum::middleware::Next<B>,
) -> axum::response::Response {
    use tracing::Span;
    
    let correlation_id = extract_correlation_id(request.headers());
    
    // Add correlation ID to request extensions
    request.extensions_mut().insert(correlation_id.clone());
    
    // Add correlation ID to current span
    let span = Span::current();
    span.record("correlation_id", &correlation_id.to_string());
    
    let mut response = next.run(request).await;
    
    // Add correlation ID to response headers
    response.headers_mut().insert(
        "x-correlation-id",
        correlation_id.to_string().parse().unwrap(),
    );
    
    response
}

/// Structured logging macros with context
#[macro_export]
macro_rules! log_info {
    ($($field:tt)*) => {
        tracing::info!($($field)*)
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($field:tt)*) => {
        tracing::warn!($($field)*)
    };
}

#[macro_export]
macro_rules! log_error {
    ($($field:tt)*) => {
        tracing::error!($($field)*)
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($field:tt)*) => {
        tracing::debug!($($field)*)
    };
}

/// Business event logging
pub struct BusinessEventLogger;

impl BusinessEventLogger {
    pub fn review_created(review_id: Uuid, offer_id: Uuid, author_id: Uuid, rating: i32) {
        tracing::info!(
            event_type = "review_created",
            review_id = %review_id,
            offer_id = %offer_id,
            author_id = %author_id,
            rating = rating,
            "Review created successfully"
        );
    }

    pub fn review_updated(review_id: Uuid, author_id: Uuid, old_rating: i32, new_rating: i32) {
        tracing::info!(
            event_type = "review_updated",
            review_id = %review_id,
            author_id = %author_id,
            old_rating = old_rating,
            new_rating = new_rating,
            "Review updated successfully"
        );
    }

    pub fn review_deleted(review_id: Uuid, author_id: Uuid) {
        tracing::info!(
            event_type = "review_deleted",
            review_id = %review_id,
            author_id = %author_id,
            "Review deleted successfully"
        );
    }

    pub fn review_moderated(review_id: Uuid, moderator_id: Uuid, status: &str) {
        tracing::info!(
            event_type = "review_moderated",
            review_id = %review_id,
            moderator_id = %moderator_id,
            moderation_status = status,
            "Review moderated"
        );
    }

    pub fn authentication_failed(user_id: Option<Uuid>, reason: &str) {
        tracing::warn!(
            event_type = "authentication_failed",
            user_id = ?user_id,
            reason = reason,
            "Authentication failed"
        );
    }

    pub fn authorization_failed(user_id: Uuid, resource: &str, action: &str) {
        tracing::warn!(
            event_type = "authorization_failed",
            user_id = %user_id,
            resource = resource,
            action = action,
            "Authorization failed"
        );
    }

    pub fn external_service_error(service: &str, error: &str, duration_ms: u64) {
        tracing::error!(
            event_type = "external_service_error",
            service = service,
            error = error,
            duration_ms = duration_ms,
            "External service call failed"
        );
    }

    pub fn database_error(operation: &str, error: &str, duration_ms: u64) {
        tracing::error!(
            event_type = "database_error",
            operation = operation,
            error = error,
            duration_ms = duration_ms,
            "Database operation failed"
        );
    }

    pub fn circuit_breaker_opened(service: &str, failure_count: u32) {
        tracing::warn!(
            event_type = "circuit_breaker_opened",
            service = service,
            failure_count = failure_count,
            "Circuit breaker opened due to failures"
        );
    }

    pub fn circuit_breaker_closed(service: &str) {
        tracing::info!(
            event_type = "circuit_breaker_closed",
            service = service,
            "Circuit breaker closed - service recovered"
        );
    }
}

/// Security event logging
pub struct SecurityEventLogger;

impl SecurityEventLogger {
    pub fn suspicious_activity(user_id: Option<Uuid>, activity: &str, details: &str) {
        tracing::warn!(
            event_type = "suspicious_activity",
            user_id = ?user_id,
            activity = activity,
            details = details,
            "Suspicious activity detected"
        );
    }

    pub fn rate_limit_exceeded(user_id: Option<Uuid>, endpoint: &str, limit: u32) {
        tracing::warn!(
            event_type = "rate_limit_exceeded",
            user_id = ?user_id,
            endpoint = endpoint,
            limit = limit,
            "Rate limit exceeded"
        );
    }

    pub fn invalid_token(token_type: &str, reason: &str) {
        tracing::warn!(
            event_type = "invalid_token",
            token_type = token_type,
            reason = reason,
            "Invalid token detected"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correlation_id_creation() {
        let id = CorrelationId::new();
        assert!(!id.to_string().is_empty());
    }

    #[test]
    fn test_correlation_id_from_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = CorrelationId::from_string(uuid_str).unwrap();
        assert_eq!(id.to_string(), uuid_str);
    }

    #[test]
    fn test_correlation_id_invalid_string() {
        let result = CorrelationId::from_string("invalid-uuid");
        assert!(result.is_err());
    }
}