use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    EmptySubscription, Schema as GraphQLSchema,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use tracing::{instrument, info, warn, error, Span};
use std::sync::Arc;

pub mod guards;
pub mod query;
pub mod mutation;
pub mod types;

#[cfg(test)]
mod federation_test;

#[cfg(test)]
mod tests;

use query::Query;
use mutation::Mutation;

pub type Schema = GraphQLSchema<Query, Mutation, EmptySubscription>;

pub async fn create_schema(
    pool: sqlx::PgPool,
    external_service: crate::service::ExternalServiceClient,
    metrics: Option<Arc<crate::telemetry::metrics::Metrics>>,
    query_limits_config: Option<crate::service::QueryLimitsConfig>,
) -> anyhow::Result<Schema> {
    use crate::service::{create_review_service, create_review_service_with_metrics, QueryLimitsExtensionFactory};
    
    let review_service = if let Some(metrics) = metrics {
        create_review_service_with_metrics(pool.clone(), metrics)
    } else {
        create_review_service(pool.clone())
    };
    
    let mut schema_builder = Schema::build(Query, Mutation, EmptySubscription)
        .data(pool)
        .data(review_service)
        .data(external_service);

    // Add query limits extension if configured
    if let Some(limits_config) = query_limits_config {
        schema_builder = schema_builder.extension(QueryLimitsExtensionFactory::new(limits_config));
        info!("Query limits extension enabled");
    }

    let schema = schema_builder.finish();

    info!("GraphQL schema created successfully");
    Ok(schema)
}

/// Enhanced schema creation with full configuration support
pub async fn create_enhanced_schema(
    pool: sqlx::PgPool,
    external_service: crate::service::ExternalServiceClient,
    config: &crate::config::Config,
) -> anyhow::Result<Schema> {
    use crate::service::{
        create_review_service_full, 
        create_review_service_with_dataloader,
        QueryLimitsExtensionFactory
    };
    
    // Create review service with cache and DataLoader if Redis is enabled
    let review_service = if config.redis.enabled {
        let cache_config = config.to_redis_cache_config();
        let metrics = None; // TODO: Pass metrics if available
        
        match create_review_service_full(pool.clone(), cache_config, metrics.unwrap_or_else(|| {
            Arc::new(crate::telemetry::metrics::Metrics::new())
        })).await {
            Ok(service) => {
                info!("Review service created with Redis cache and DataLoader");
                service
            }
            Err(e) => {
                warn!("Failed to create service with Redis cache: {}. Falling back to DataLoader only", e);
                create_review_service_with_dataloader(pool.clone())
            }
        }
    } else {
        info!("Redis disabled, creating service with DataLoader only");
        create_review_service_with_dataloader(pool.clone())
    };
    
    let mut schema_builder = Schema::build(Query, Mutation, EmptySubscription)
        .data(pool)
        .data(review_service)
        .data(external_service);

    // Add query limits extension
    let limits_config = config.to_query_limits_config();
    schema_builder = schema_builder.extension(QueryLimitsExtensionFactory::new(limits_config));
    info!("Query limits extension enabled with max_depth={}, max_complexity={}", 
          config.query_limits.max_depth, config.query_limits.max_complexity);

    let schema = schema_builder.finish();

    info!("Enhanced GraphQL schema created successfully");
    Ok(schema)
}

use axum::extract::Request as HttpRequest;

#[instrument(skip(schema, req, http_req), fields(operation_name, query_complexity))]
pub async fn graphql_handler(
    State((schema, _pool)): State<(Schema, sqlx::PgPool)>,
    http_req: HttpRequest,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let start_time = std::time::Instant::now();
    let mut request = req.into_inner();
    
    // Extract operation name for tracing
    let operation_name = request.operation_name.clone().unwrap_or_else(|| "anonymous".to_string());
    Span::current().record("operation_name", &operation_name);
    
    info!("Processing GraphQL request: {}", operation_name);
    
    // Add correlation ID from HTTP request extensions
    if let Some(correlation_id) = http_req.extensions().get::<crate::telemetry::logging::CorrelationId>() {
        request = request.data(correlation_id.clone());
    }
    
    // Add user context from HTTP request extensions if available
    if let Some(user_context) = http_req.extensions().get::<crate::auth::UserContext>() {
        request = request.data(user_context.clone());
        info!("Request authenticated for user: {}", user_context.user_id);
    } else {
        info!("Unauthenticated GraphQL request");
    }
    
    // Execute the GraphQL request
    let response = schema.execute(request).await;
    let duration = start_time.elapsed();
    
    // Log response metrics
    if response.errors.is_empty() {
        info!("GraphQL request completed successfully in {:?}", duration);
    } else {
        warn!("GraphQL request completed with {} errors in {:?}", response.errors.len(), duration);
        for error in &response.errors {
            error!("GraphQL error: {}", error.message);
        }
    }
    
    response.into()
}

pub async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}