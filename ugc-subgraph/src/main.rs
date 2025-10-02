use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{info, instrument, error};
use std::sync::Arc;

mod auth;
mod config;
mod database;
mod error;
mod graphql;
mod migration;
mod models;
mod repository;
mod service;
mod telemetry;

use auth::{AuthService, auth_middleware};
use config::Config;
use database::{create_database_pool, run_migrations, check_database_health};
use graphql::create_schema;
use migration::{
    FeatureFlagService, RestAdapter, TrafficRouter, TrafficRouterConfig,
    MigrationMetrics, MigrationDashboard, MigrationAlertManager, AlertConfig,
    MigrationManagementAPI, ConfigLoader,
};
use telemetry::{
    tracing::{init_tracing, shutdown_tracing, TracingConfig},
    metrics::{Metrics, create_metrics_router, http_metrics_middleware},
    logging::{correlation_middleware, BusinessEventLogger},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize distributed tracing
    let tracing_config = TracingConfig {
        service_name: "ugc-subgraph".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
        sample_rate: std::env::var("TRACE_SAMPLE_RATE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0),
        enable_console: std::env::var("ENABLE_CONSOLE_LOGS")
            .map(|s| s.to_lowercase() == "true")
            .unwrap_or(true),
    };
    
    init_tracing(tracing_config)?;
    info!("Distributed tracing initialized");

    // Initialize metrics
    let metrics = Arc::new(Metrics::new()?);
    info!("Metrics initialized");

    info!("Starting UGC Subgraph service");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Create database connection pool
    let pool = create_database_pool(&config.database_url).await?;
    info!("Database connection pool created");

    // Run migrations
    run_migrations(&pool).await?;
    info!("Database migrations completed");

    // Create authentication service
    let auth_service = std::sync::Arc::new(
        AuthService::new_with_secret(&config.jwt_secret)
            .with_issuer("auto-ru-federation".to_string())
            .with_audience("ugc-subgraph".to_string())
    );
    info!("Authentication service created");

    // Create external service client
    let external_service = service::ExternalServiceClient::new(
        config.users_service_url.clone(),
        config.offers_service_url.clone(),
    );

    // Create GraphQL schema with performance optimizations
    let schema = graphql::create_enhanced_schema(pool.clone(), external_service, &config).await?;
    info!("Enhanced GraphQL schema created with performance optimizations");

    // Build application router
    let app = create_app(schema, pool, auth_service, config.clone(), metrics.clone()).await?;

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Server starting on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    
    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
        shutdown_tracing();
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    Ok(())
}

async fn create_app(
    schema: graphql::Schema,
    pool: PgPool,
    auth_service: std::sync::Arc<AuthService>,
    config: Config,
    metrics: Arc<Metrics>,
) -> Result<Router> {
    // Initialize migration components
    let migration_metrics = Arc::new(MigrationMetrics::new()?);
    
    // Load migration configuration
    let config_loader = ConfigLoader::new()
        .with_config_path("feature-flags.yaml".to_string());
    let migration_config = config_loader.load().await?;
    
    // Initialize feature flags service
    let feature_flags = Arc::new(
        if config.redis.enabled {
            FeatureFlagService::new().with_redis(&config.redis.url)?
        } else {
            FeatureFlagService::new()
        }
    );
    
    // Apply migration configuration to feature flags service
    migration_config.apply_to_service(&feature_flags).await?;
    
    // Start configuration file watcher
    config_loader.watch_and_reload(feature_flags.clone()).await?;
    
    // Initialize traffic router
    let traffic_router = Arc::new(TrafficRouter::new(
        feature_flags.clone(),
        migration_metrics.clone(),
        TrafficRouterConfig::default(),
    ));
    
    // Initialize REST adapter for backward compatibility
    let rest_adapter = RestAdapter::new(
        schema.clone(),
        feature_flags.clone(),
        migration_metrics.clone(),
    );
    
    // Initialize migration dashboard
    let migration_dashboard = MigrationDashboard::new(migration_metrics.clone());
    migration_dashboard.start_background_collection().await;
    
    // Initialize alert manager
    let alert_manager = MigrationAlertManager::new(
        migration_metrics.clone(),
        AlertConfig::default(),
    );
    alert_manager.start_monitoring().await;
    
    // Initialize migration management API
    let management_api = MigrationManagementAPI::new(
        feature_flags.clone(),
        migration_metrics.clone(),
    );
    
    // Create metrics router
    let metrics_router = create_metrics_router(metrics.clone());
    
    let app = Router::new()
        // GraphQL endpoints
        .route("/graphql", post(graphql::graphql_handler))
        .route("/", get(graphql::graphql_playground))
        
        // Health and readiness checks
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/.well-known/jwks.json", get(jwks_endpoint))
        
        // Migration management endpoints
        .route("/migration/status", get(migration_status))
        .route("/migration/metrics", get(migration_metrics_endpoint))
        .route("/migration/flags", get(list_feature_flags))
        .route("/migration/flags/:flag_name", post(update_feature_flag))
        .route("/migration/rollback/:flag_name", post(emergency_rollback))
        
        // REST API compatibility layer
        .nest("/", rest_adapter.router())
        
        // Migration management API
        .nest("/", management_api.router())
        
        // Metrics and monitoring
        .nest("/", metrics_router)
        
        .with_state((schema, pool, feature_flags.clone(), migration_dashboard))
        .layer(axum::middleware::from_fn_with_state(
            traffic_router,
            migration::traffic_routing_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            metrics.clone(),
            http_metrics_middleware,
        ))
        .layer(axum::middleware::from_fn(correlation_middleware))
        .layer(axum::middleware::from_fn_with_state(
            auth_service.clone(),
            auth_middleware,
        ))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_headers(Any)
                        .allow_methods(Any),
                )
        );

    // Start background task to update business metrics
    let metrics_clone = metrics.clone();
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            metrics_clone.update_business_metrics(&pool_clone).await;
        }
    });

    // Start background task for rate limit cleanup
    let rate_limiter = Arc::new(service::QueryRateLimiter::new());
    let rate_limiter_clone = Arc::clone(&rate_limiter);
    tokio::spawn(async move {
        service::start_rate_limit_cleanup_task(rate_limiter_clone).await;
    });
    info!("Background tasks started");

    Ok(app)
}

#[instrument]
async fn health_check() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "ugc-subgraph",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

#[instrument]
async fn readiness_check(
    State((_, pool)): State<(graphql::Schema, PgPool)>,
) -> Result<Json<Value>, StatusCode> {
    // Check database connectivity
    let db_status = match check_database_health(&pool).await {
        Ok(_) => "ready",
        Err(e) => {
            error!("Database health check failed: {}", e);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    Ok(Json(json!({
        "status": "ready",
        "service": "ugc-subgraph",
        "database": db_status,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

#[instrument]
async fn jwks_endpoint() -> Json<Value> {
    // For development, return empty JWKS
    // In production, this would return the actual public keys
    Json(json!({
        "keys": []
    }))
}

#[instrument]
async fn migration_status(
    State((_, _, _, dashboard)): State<(graphql::Schema, PgPool, Arc<FeatureFlagService>, MigrationDashboard)>,
) -> Json<Value> {
    let status = dashboard.get_migration_status().await;
    Json(serde_json::to_value(status).unwrap_or_default())
}

#[instrument]
async fn migration_metrics_endpoint(
    State((_, _, _, dashboard)): State<(graphql::Schema, PgPool, Arc<FeatureFlagService>, MigrationDashboard)>,
) -> Json<Value> {
    let metrics = dashboard.get_real_time_metrics().await;
    Json(serde_json::to_value(metrics).unwrap_or_default())
}

#[instrument]
async fn list_feature_flags(
    State((_, _, feature_flags, _)): State<(graphql::Schema, PgPool, Arc<FeatureFlagService>, MigrationDashboard)>,
) -> Json<Value> {
    let flags = feature_flags.list_flags().await;
    Json(json!({
        "flags": flags
    }))
}

#[instrument]
async fn update_feature_flag(
    State((_, _, feature_flags, _)): State<(graphql::Schema, PgPool, Arc<FeatureFlagService>, MigrationDashboard)>,
    Path(flag_name): Path<String>,
    Json(payload): Json<migration::FeatureFlag>,
) -> Result<Json<Value>, StatusCode> {
    match feature_flags.update_flag(&flag_name, payload).await {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": format!("Feature flag '{}' updated successfully", flag_name)
        }))),
        Err(e) => {
            error!("Failed to update feature flag '{}': {}", flag_name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument]
async fn emergency_rollback(
    State((_, _, feature_flags, _)): State<(graphql::Schema, PgPool, Arc<FeatureFlagService>, MigrationDashboard)>,
    Path(flag_name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let rollback_service = migration::RollbackService::new(feature_flags);
    
    match rollback_service.emergency_rollback(&format!("Manual rollback of flag '{}'", flag_name)).await {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": "Emergency rollback completed successfully"
        }))),
        Err(e) => {
            error!("Emergency rollback failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

