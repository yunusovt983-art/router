use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, instrument, error};
use uuid::Uuid;

use crate::migration::{
    FeatureFlagService, FeatureFlag, FlagCondition,
    ABTestService, ABTestVariant, RollbackService,
    MigrationMetrics, CanaryController,
};
use crate::auth::UserContext;

/// Management API for feature flags and migration control
#[derive(Clone)]
pub struct MigrationManagementAPI {
    feature_flags: Arc<FeatureFlagService>,
    ab_test_service: Arc<ABTestService>,
    rollback_service: Arc<RollbackService>,
    canary_controller: Arc<CanaryController>,
    metrics: Arc<MigrationMetrics>,
}

impl MigrationManagementAPI {
    pub fn new(
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
    ) -> Self {
        let ab_test_service = Arc::new(ABTestService::new(feature_flags.clone()));
        let rollback_service = Arc::new(RollbackService::new(feature_flags.clone()));
        let canary_controller = Arc::new(CanaryController::new(feature_flags.clone(), metrics.clone()));

        Self {
            feature_flags,
            ab_test_service,
            rollback_service,
            canary_controller,
            metrics,
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            // Feature flag management
            .route("/api/migration/flags", get(Self::list_flags).post(Self::create_flag))
            .route("/api/migration/flags/:flag_name", get(Self::get_flag).put(Self::update_flag).delete(Self::delete_flag))
            .route("/api/migration/flags/:flag_name/enable", post(Self::enable_flag))
            .route("/api/migration/flags/:flag_name/disable", post(Self::disable_flag))
            .route("/api/migration/flags/:flag_name/rollout", put(Self::set_rollout_percentage))
            
            // User-specific flag management
            .route("/api/migration/flags/:flag_name/users/:user_id/enable", post(Self::enable_flag_for_user))
            .route("/api/migration/flags/:flag_name/users/:user_id/disable", post(Self::disable_flag_for_user))
            
            // A/B Testing
            .route("/api/migration/ab-tests", get(Self::list_ab_tests).post(Self::create_ab_test))
            .route("/api/migration/ab-tests/:test_name", get(Self::get_ab_test))
            .route("/api/migration/ab-tests/:test_name/assign/:user_id", get(Self::assign_user_to_variant))
            .route("/api/migration/ab-tests/:test_name/track", post(Self::track_conversion))
            
            // Canary deployments
            .route("/api/migration/canary/:flag_name/start", post(Self::start_canary))
            .route("/api/migration/canary/:flag_name/promote", post(Self::promote_canary))
            .route("/api/migration/canary/:flag_name/rollback", post(Self::rollback_canary))
            
            // Emergency controls
            .route("/api/migration/emergency/rollback", post(Self::emergency_rollback))
            .route("/api/migration/emergency/disable-all", post(Self::disable_all_flags))
            
            // Monitoring and analytics
            .route("/api/migration/status", get(Self::get_migration_status))
            .route("/api/migration/metrics", get(Self::get_migration_metrics))
            .route("/api/migration/health", get(Self::health_check))
            
            .with_state(self.clone())
    }

    // Feature flag endpoints
    #[instrument(skip(api))]
    async fn list_flags(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        let flags = api.feature_flags.list_flags().await;
        Ok(Json(json!({
            "flags": flags,
            "total": flags.len()
        })))
    }

    #[instrument(skip(api))]
    async fn get_flag(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        if let Some(flag) = api.feature_flags.get_flag(&flag_name).await {
            Ok(Json(serde_json::to_value(flag).unwrap_or_default()))
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    #[instrument(skip(api))]
    async fn create_flag(
        State(api): State<MigrationManagementAPI>,
        Json(request): Json<CreateFlagRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        let flag = FeatureFlag {
            name: request.name.clone(),
            enabled: request.enabled.unwrap_or(false),
            rollout_percentage: request.rollout_percentage.unwrap_or(0.0),
            user_whitelist: request.user_whitelist.unwrap_or_default(),
            user_blacklist: request.user_blacklist.unwrap_or_default(),
            conditions: request.conditions.unwrap_or_default(),
            description: request.description.unwrap_or_default(),
        };

        match api.feature_flags.update_flag(&request.name, flag).await {
            Ok(_) => {
                info!("Created feature flag: {}", request.name);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Feature flag '{}' created successfully", request.name)
                })))
            }
            Err(e) => {
                error!("Failed to create feature flag '{}': {}", request.name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn update_flag(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
        Json(flag): Json<FeatureFlag>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.feature_flags.update_flag(&flag_name, flag).await {
            Ok(_) => {
                info!("Updated feature flag: {}", flag_name);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Feature flag '{}' updated successfully", flag_name)
                })))
            }
            Err(e) => {
                error!("Failed to update feature flag '{}': {}", flag_name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn delete_flag(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        // For safety, we don't actually delete flags, just disable them
        if let Some(mut flag) = api.feature_flags.get_flag(&flag_name).await {
            flag.enabled = false;
            flag.rollout_percentage = 0.0;
            
            match api.feature_flags.update_flag(&flag_name, flag).await {
                Ok(_) => {
                    warn!("Disabled feature flag (delete requested): {}", flag_name);
                    Ok(Json(json!({
                        "success": true,
                        "message": format!("Feature flag '{}' disabled (not deleted for safety)", flag_name)
                    })))
                }
                Err(e) => {
                    error!("Failed to disable feature flag '{}': {}", flag_name, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    #[instrument(skip(api))]
    async fn enable_flag(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        if let Some(mut flag) = api.feature_flags.get_flag(&flag_name).await {
            flag.enabled = true;
            
            match api.feature_flags.update_flag(&flag_name, flag).await {
                Ok(_) => {
                    info!("Enabled feature flag: {}", flag_name);
                    Ok(Json(json!({
                        "success": true,
                        "message": format!("Feature flag '{}' enabled", flag_name)
                    })))
                }
                Err(e) => {
                    error!("Failed to enable feature flag '{}': {}", flag_name, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    #[instrument(skip(api))]
    async fn disable_flag(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        if let Some(mut flag) = api.feature_flags.get_flag(&flag_name).await {
            flag.enabled = false;
            
            match api.feature_flags.update_flag(&flag_name, flag).await {
                Ok(_) => {
                    warn!("Disabled feature flag: {}", flag_name);
                    Ok(Json(json!({
                        "success": true,
                        "message": format!("Feature flag '{}' disabled", flag_name)
                    })))
                }
                Err(e) => {
                    error!("Failed to disable feature flag '{}': {}", flag_name, e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    #[instrument(skip(api))]
    async fn set_rollout_percentage(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
        Json(request): Json<RolloutRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.feature_flags.set_rollout_percentage(&flag_name, request.percentage).await {
            Ok(_) => {
                info!("Set rollout percentage for flag '{}' to {}%", flag_name, request.percentage);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Rollout percentage set to {}%", request.percentage)
                })))
            }
            Err(e) => {
                error!("Failed to set rollout percentage for flag '{}': {}", flag_name, e);
                Err(StatusCode::BAD_REQUEST)
            }
        }
    }

    #[instrument(skip(api))]
    async fn enable_flag_for_user(
        State(api): State<MigrationManagementAPI>,
        Path((flag_name, user_id)): Path<(String, String)>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.feature_flags.enable_flag_for_user(&flag_name, &user_id).await {
            Ok(_) => {
                info!("Enabled flag '{}' for user '{}'", flag_name, user_id);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Flag '{}' enabled for user '{}'", flag_name, user_id)
                })))
            }
            Err(e) => {
                error!("Failed to enable flag '{}' for user '{}': {}", flag_name, user_id, e);
                Err(StatusCode::BAD_REQUEST)
            }
        }
    }

    #[instrument(skip(api))]
    async fn disable_flag_for_user(
        State(api): State<MigrationManagementAPI>,
        Path((flag_name, user_id)): Path<(String, String)>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.feature_flags.disable_flag_for_user(&flag_name, &user_id).await {
            Ok(_) => {
                warn!("Disabled flag '{}' for user '{}'", flag_name, user_id);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Flag '{}' disabled for user '{}'", flag_name, user_id)
                })))
            }
            Err(e) => {
                error!("Failed to disable flag '{}' for user '{}': {}", flag_name, user_id, e);
                Err(StatusCode::BAD_REQUEST)
            }
        }
    }

    // A/B Testing endpoints
    #[instrument(skip(api))]
    async fn list_ab_tests(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        // This would list active A/B tests
        // For now, return mock data
        Ok(Json(json!({
            "tests": [
                {
                    "name": "graphql_migration_test",
                    "description": "A/B test for GraphQL migration",
                    "status": "active",
                    "variants": ["control", "treatment"],
                    "traffic_split": {"control": 50, "treatment": 50}
                }
            ]
        })))
    }

    #[instrument(skip(api))]
    async fn create_ab_test(
        State(api): State<MigrationManagementAPI>,
        Json(request): Json<CreateABTestRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        // Create feature flag for A/B test
        let flag_name = format!("ab_test_{}", request.name);
        let flag = FeatureFlag {
            name: flag_name.clone(),
            enabled: true,
            rollout_percentage: request.traffic_percentage.unwrap_or(50.0),
            user_whitelist: vec![],
            user_blacklist: vec![],
            conditions: vec![],
            description: format!("A/B test: {}", request.description.unwrap_or_default()),
        };

        match api.feature_flags.update_flag(&flag_name, flag).await {
            Ok(_) => {
                info!("Created A/B test: {}", request.name);
                Ok(Json(json!({
                    "success": true,
                    "test_name": request.name,
                    "flag_name": flag_name,
                    "message": "A/B test created successfully"
                })))
            }
            Err(e) => {
                error!("Failed to create A/B test '{}': {}", request.name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn get_ab_test(
        State(api): State<MigrationManagementAPI>,
        Path(test_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        let flag_name = format!("ab_test_{}", test_name);
        
        if let Some(flag) = api.feature_flags.get_flag(&flag_name).await {
            Ok(Json(json!({
                "test_name": test_name,
                "flag_name": flag_name,
                "enabled": flag.enabled,
                "traffic_percentage": flag.rollout_percentage,
                "description": flag.description
            })))
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }

    #[instrument(skip(api))]
    async fn assign_user_to_variant(
        State(api): State<MigrationManagementAPI>,
        Path((test_name, user_id)): Path<(String, String)>,
    ) -> Result<Json<Value>, StatusCode> {
        let variant = api.ab_test_service.assign_user_to_variant(&test_name, &user_id).await;
        
        Ok(Json(json!({
            "test_name": test_name,
            "user_id": user_id,
            "variant": variant
        })))
    }

    #[instrument(skip(api))]
    async fn track_conversion(
        State(api): State<MigrationManagementAPI>,
        Path(test_name): Path<String>,
        Json(request): Json<TrackConversionRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        api.ab_test_service.track_conversion(&test_name, &request.user_id, &request.event).await;
        
        Ok(Json(json!({
            "success": true,
            "message": "Conversion tracked successfully"
        })))
    }

    // Canary deployment endpoints
    #[instrument(skip(api))]
    async fn start_canary(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.canary_controller.start_canary_deployment(&flag_name).await {
            Ok(_) => {
                info!("Started canary deployment for flag: {}", flag_name);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Canary deployment started for flag '{}'", flag_name)
                })))
            }
            Err(e) => {
                error!("Failed to start canary deployment for flag '{}': {}", flag_name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn promote_canary(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
        Json(request): Json<PromoteCanaryRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.canary_controller.promote_canary(&flag_name, request.target_percentage).await {
            Ok(_) => {
                info!("Promoted canary for flag '{}' to {}%", flag_name, request.target_percentage);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Canary promoted to {}%", request.target_percentage)
                })))
            }
            Err(e) => {
                error!("Failed to promote canary for flag '{}': {}", flag_name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn rollback_canary(
        State(api): State<MigrationManagementAPI>,
        Path(flag_name): Path<String>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.canary_controller.rollback_canary(&flag_name).await {
            Ok(_) => {
                warn!("Rolled back canary deployment for flag: {}", flag_name);
                Ok(Json(json!({
                    "success": true,
                    "message": format!("Canary deployment rolled back for flag '{}'", flag_name)
                })))
            }
            Err(e) => {
                error!("Failed to rollback canary for flag '{}': {}", flag_name, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    // Emergency controls
    #[instrument(skip(api))]
    async fn emergency_rollback(
        State(api): State<MigrationManagementAPI>,
        Json(request): Json<EmergencyRollbackRequest>,
    ) -> Result<Json<Value>, StatusCode> {
        match api.rollback_service.emergency_rollback(&request.reason).await {
            Ok(_) => {
                warn!("Emergency rollback completed: {}", request.reason);
                Ok(Json(json!({
                    "success": true,
                    "message": "Emergency rollback completed successfully"
                })))
            }
            Err(e) => {
                error!("Emergency rollback failed: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    #[instrument(skip(api))]
    async fn disable_all_flags(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        let flags = api.feature_flags.list_flags().await;
        let mut disabled_count = 0;

        for mut flag in flags {
            if flag.enabled {
                flag.enabled = false;
                flag.rollout_percentage = 0.0;
                
                if api.feature_flags.update_flag(&flag.name, flag).await.is_ok() {
                    disabled_count += 1;
                }
            }
        }

        warn!("Emergency: Disabled {} feature flags", disabled_count);
        Ok(Json(json!({
            "success": true,
            "disabled_flags": disabled_count,
            "message": format!("Disabled {} feature flags", disabled_count)
        })))
    }

    // Monitoring endpoints
    #[instrument(skip(api))]
    async fn get_migration_status(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        let flags = api.feature_flags.list_flags().await;
        let enabled_flags = flags.iter().filter(|f| f.enabled).count();
        let total_rollout: f64 = flags.iter().map(|f| f.rollout_percentage).sum();
        let avg_rollout = if !flags.is_empty() { total_rollout / flags.len() as f64 } else { 0.0 };

        Ok(Json(json!({
            "total_flags": flags.len(),
            "enabled_flags": enabled_flags,
            "average_rollout_percentage": avg_rollout,
            "migration_health": "healthy", // This would be calculated based on metrics
            "last_updated": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[instrument(skip(api))]
    async fn get_migration_metrics(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        // This would collect real metrics from Prometheus
        Ok(Json(json!({
            "requests_per_second": {
                "graphql": 45.2,
                "rest": 123.8
            },
            "error_rates": {
                "graphql": 0.018,
                "rest": 0.012
            },
            "response_times": {
                "graphql_p95": 180.0,
                "rest_p95": 145.0
            },
            "feature_flag_evaluations": 1250,
            "cache_hit_rate": 0.85
        })))
    }

    #[instrument(skip(api))]
    async fn health_check(
        State(api): State<MigrationManagementAPI>,
    ) -> Result<Json<Value>, StatusCode> {
        // Check if feature flag service is healthy
        let flags_healthy = !api.feature_flags.list_flags().await.is_empty();
        
        Ok(Json(json!({
            "status": if flags_healthy { "healthy" } else { "unhealthy" },
            "feature_flags_service": if flags_healthy { "up" } else { "down" },
            "timestamp": chrono::Utc::now().to_rfc3339()
        })))
    }
}

// Request/Response DTOs
#[derive(Debug, Deserialize)]
pub struct CreateFlagRequest {
    pub name: String,
    pub enabled: Option<bool>,
    pub rollout_percentage: Option<f64>,
    pub user_whitelist: Option<Vec<String>>,
    pub user_blacklist: Option<Vec<String>>,
    pub conditions: Option<Vec<FlagCondition>>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RolloutRequest {
    pub percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateABTestRequest {
    pub name: String,
    pub description: Option<String>,
    pub traffic_percentage: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TrackConversionRequest {
    pub user_id: String,
    pub event: String,
}

#[derive(Debug, Deserialize)]
pub struct PromoteCanaryRequest {
    pub target_percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct EmergencyRollbackRequest {
    pub reason: String,
}