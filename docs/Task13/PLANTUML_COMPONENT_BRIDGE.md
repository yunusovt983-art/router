# Task 13: Component Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_COMPONENT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Компонентная диаграмма Task 13 показывает детальную внутреннюю структуру системы миграции на уровне компонентов. Каждый компонент имеет прямое отражение в конкретных модулях, функциях и структурах данных в коде.

### Migration Service Components - Код реализация

#### 1. REST Endpoints Component
**PlantUML элемент:**
```plantuml
Component(rest_endpoints, "REST Endpoints", "Axum Routes", "Legacy REST API endpoints")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/rest_adapter.rs
use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put, delete},
    Router,
};

pub fn rest_endpoints_router() -> Router {
    Router::new()
        // Reviews endpoints
        .route("/reviews", 
            get(get_reviews_handler)
            .post(create_review_handler)
        )
        .route("/reviews/:id", 
            get(get_review_handler)
            .put(update_review_handler)
            .delete(delete_review_handler)
        )
        // Nested resource endpoints
        .route("/offers/:offer_id/reviews", get(get_offer_reviews_handler))
        .route("/users/:user_id/reviews", get(get_user_reviews_handler))
        .route("/offers/:offer_id/rating", get(get_offer_rating_handler))
}

// Handler implementations
async fn get_reviews_handler(
    State(adapter): State<RestAdapter>,
    Query(params): Query<ReviewsQueryParams>,
    headers: HeaderMap,
) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
    // Metrics recording
    adapter.metrics.rest_request_total
        .with_label_values(&["GET", "/api/v1/reviews"])
        .inc();
    
    let start_time = Instant::now();
    
    // User context extraction
    let user_context = extract_user_context(&headers)?;
    
    // Migration middleware integration
    let result = adapter.migration_middleware
        .process_request("get_reviews", &user_context, params)
        .await?;
    
    // Performance metrics
    let duration = start_time.elapsed().as_secs_f64();
    adapter.metrics.request_duration
        .with_label_values(&["rest", "get_reviews"])
        .observe(duration);
    
    Ok(Json(result))
}
```

#### 2. Migration Middleware Component
**PlantUML элемент:**
```plantuml
Component(migration_middleware, "Migration Middleware", "Axum Middleware", "Request interception and routing")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/middleware.rs
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

pub struct MigrationMiddleware {
    feature_flags: Arc<FeatureFlagService>,
    traffic_router: Arc<TrafficRouter>,
    metrics: Arc<MigrationMetrics>,
}

impl MigrationMiddleware {
    pub async fn process_request<T>(
        &self,
        operation: &str,
        user_context: &UserContext,
        params: T,
    ) -> Result<RestResponse<T::Output>, MigrationError>
    where
        T: RequestParams,
    {
        // 1. Feature flag evaluation
        let routing_decision = self.evaluate_routing_decision(operation, user_context).await?;
        
        // 2. Request routing
        match routing_decision.backend {
            Backend::GraphQL => {
                self.metrics.graphql_migration_requests
                    .with_label_values(&[operation, "routed"])
                    .inc();
                
                self.route_to_graphql(operation, params, user_context).await
            }
            Backend::LegacyRest => {
                self.metrics.legacy_rest_requests
                    .with_label_values(&[operation, "routed"])
                    .inc();
                
                self.route_to_legacy_rest(operation, params, user_context).await
            }
        }
    }
    
    async fn evaluate_routing_decision(
        &self,
        operation: &str,
        user_context: &UserContext,
    ) -> Result<RoutingDecision, MigrationError> {
        let flag_name = self.get_flag_name_for_operation(operation);
        
        let use_graphql = self.feature_flags
            .is_enabled(&flag_name, &user_context.user_id.to_string())
            .await;
        
        Ok(RoutingDecision {
            backend: if use_graphql { Backend::GraphQL } else { Backend::LegacyRest },
            flag_name,
            user_id: user_context.user_id.clone(),
            operation: operation.to_string(),
        })
    }
}

// Axum middleware integration
pub async fn migration_middleware_layer(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract user context from request
    let user_context = extract_user_context_from_request(&request)?;
    
    // Add user context to request extensions
    let mut request = request;
    request.extensions_mut().insert(user_context);
    
    // Continue to next middleware/handler
    let response = next.run(request).await;
    
    Ok(response)
}
```

#### 3. Feature Flag Engine Component
**PlantUML элемент:**
```plantuml
Component(feature_flag_engine, "Feature Flag Engine", "Rust Service", "Flag evaluation and caching")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/feature_flags.rs
pub struct FeatureFlagEngine {
    flags: Arc<RwLock<HashMap<String, FeatureFlag>>>,
    cache: Arc<FlagCache>,
    evaluator: Arc<FlagEvaluator>,
    metrics: Arc<FlagMetrics>,
}

impl FeatureFlagEngine {
    pub async fn evaluate_flag(
        &self,
        flag_name: &str,
        user_id: &str,
        context: &EvaluationContext,
    ) -> FlagEvaluationResult {
        let start_time = Instant::now();
        
        // 1. Check cache first
        if let Some(cached_result) = self.cache.get(flag_name, user_id).await {
            self.metrics.cache_hits
                .with_label_values(&[flag_name])
                .inc();
            
            return FlagEvaluationResult {
                enabled: cached_result,
                source: EvaluationSource::Cache,
                duration: start_time.elapsed(),
            };
        }
        
        // 2. Load flag configuration
        let flags = self.flags.read().await;
        let flag = match flags.get(flag_name) {
            Some(flag) => flag,
            None => {
                self.metrics.flag_not_found
                    .with_label_values(&[flag_name])
                    .inc();
                
                return FlagEvaluationResult::disabled(EvaluationSource::NotFound);
            }
        };
        
        // 3. Evaluate flag conditions
        let result = self.evaluator.evaluate(flag, user_id, context).await;
        
        // 4. Cache result
        self.cache.set(flag_name, user_id, result.enabled, flag.cache_ttl).await;
        
        // 5. Record metrics
        self.metrics.evaluations
            .with_label_values(&[flag_name, &result.enabled.to_string()])
            .inc();
        
        self.metrics.evaluation_duration
            .with_label_values(&[flag_name])
            .observe(start_time.elapsed().as_secs_f64());
        
        FlagEvaluationResult {
            enabled: result.enabled,
            source: EvaluationSource::Evaluated,
            duration: start_time.elapsed(),
        }
    }
}

// Flag evaluator implementation
pub struct FlagEvaluator;

impl FlagEvaluator {
    pub async fn evaluate(
        &self,
        flag: &FeatureFlag,
        user_id: &str,
        context: &EvaluationContext,
    ) -> EvaluationResult {
        // 1. Global enable check
        if !flag.enabled {
            return EvaluationResult::disabled("flag_globally_disabled");
        }
        
        // 2. Blacklist check
        if flag.user_blacklist.contains(&user_id.to_string()) {
            return EvaluationResult::disabled("user_blacklisted");
        }
        
        // 3. Whitelist check
        if flag.user_whitelist.contains(&user_id.to_string()) {
            return EvaluationResult::enabled("user_whitelisted");
        }
        
        // 4. Condition evaluation
        for condition in &flag.conditions {
            if !self.evaluate_condition(condition, user_id, context).await {
                return EvaluationResult::disabled("condition_failed");
            }
        }
        
        // 5. Rollout percentage check using consistent hashing
        let user_hash = self.hash_user_id(user_id);
        let user_percentage = (user_hash % 100) as f64;
        
        if user_percentage < flag.rollout_percentage {
            EvaluationResult::enabled("rollout_percentage")
        } else {
            EvaluationResult::disabled("rollout_percentage")
        }
    }
    
    fn hash_user_id(&self, user_id: &str) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        hasher.finish() as u32
    }
}
```

#### 4. User Targeting Component
**PlantUML элемент:**
```plantuml
Component(user_targeting, "User Targeting", "Rust Module", "User segmentation and targeting logic")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/user_targeting.rs
pub struct UserTargeting {
    segments: Arc<RwLock<HashMap<String, UserSegment>>>,
    targeting_rules: Arc<RwLock<Vec<TargetingRule>>>,
}

impl UserTargeting {
    pub async fn evaluate_user_eligibility(
        &self,
        user_id: &str,
        user_attributes: &UserAttributes,
        flag_name: &str,
    ) -> TargetingResult {
        // 1. Load user segments
        let segments = self.segments.read().await;
        let user_segments = self.get_user_segments(user_id, user_attributes, &segments).await;
        
        // 2. Apply targeting rules
        let rules = self.targeting_rules.read().await;
        for rule in rules.iter() {
            if rule.flag_name == flag_name {
                if self.evaluate_targeting_rule(rule, &user_segments, user_attributes).await {
                    return TargetingResult::Eligible {
                        rule_id: rule.id.clone(),
                        segments: user_segments,
                    };
                }
            }
        }
        
        TargetingResult::NotEligible
    }
    
    async fn get_user_segments(
        &self,
        user_id: &str,
        attributes: &UserAttributes,
        segments: &HashMap<String, UserSegment>,
    ) -> Vec<String> {
        let mut user_segments = Vec::new();
        
        for (segment_name, segment) in segments {
            if self.user_matches_segment(user_id, attributes, segment).await {
                user_segments.push(segment_name.clone());
            }
        }
        
        user_segments
    }
    
    async fn user_matches_segment(
        &self,
        user_id: &str,
        attributes: &UserAttributes,
        segment: &UserSegment,
    ) -> bool {
        match &segment.criteria {
            SegmentCriteria::UserIdPattern(pattern) => {
                user_id.matches(pattern)
            }
            SegmentCriteria::AttributeMatch { attribute, value } => {
                attributes.get(attribute) == Some(value)
            }
            SegmentCriteria::AttributeRange { attribute, min, max } => {
                if let Some(attr_value) = attributes.get(attribute) {
                    if let Ok(numeric_value) = attr_value.parse::<f64>() {
                        return numeric_value >= *min && numeric_value <= *max;
                    }
                }
                false
            }
            SegmentCriteria::Composite { operator, criteria } => {
                match operator {
                    LogicalOperator::And => {
                        for criterion in criteria {
                            if !self.evaluate_criterion(user_id, attributes, criterion).await {
                                return false;
                            }
                        }
                        true
                    }
                    LogicalOperator::Or => {
                        for criterion in criteria {
                            if self.evaluate_criterion(user_id, attributes, criterion).await {
                                return true;
                            }
                        }
                        false
                    }
                }
            }
        }
    }
}

// Data structures
#[derive(Debug, Clone)]
pub struct UserSegment {
    pub name: String,
    pub description: String,
    pub criteria: SegmentCriteria,
}

#[derive(Debug, Clone)]
pub enum SegmentCriteria {
    UserIdPattern(String),
    AttributeMatch { attribute: String, value: String },
    AttributeRange { attribute: String, min: f64, max: f64 },
    Composite { operator: LogicalOperator, criteria: Vec<SegmentCriteria> },
}

#[derive(Debug, Clone)]
pub struct TargetingRule {
    pub id: String,
    pub flag_name: String,
    pub segments: Vec<String>,
    pub percentage: f64,
    pub priority: i32,
}
```

#### 5. A/B Test Engine Component
**PlantUML элемент:**
```plantuml
Component(ab_test_engine, "A/B Test Engine", "Rust Service", "Experiment management and variant assignment")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/ab_testing.rs
pub struct ABTestEngine {
    experiments: Arc<RwLock<HashMap<String, Experiment>>>,
    assignments: Arc<RwLock<HashMap<String, VariantAssignment>>>,
    metrics: Arc<ABTestMetrics>,
}

impl ABTestEngine {
    pub async fn assign_variant(
        &self,
        experiment_name: &str,
        user_id: &str,
        user_attributes: &UserAttributes,
    ) -> VariantAssignment {
        // 1. Check existing assignment
        let assignments = self.assignments.read().await;
        let assignment_key = format!("{}:{}", experiment_name, user_id);
        
        if let Some(existing_assignment) = assignments.get(&assignment_key) {
            return existing_assignment.clone();
        }
        drop(assignments);
        
        // 2. Load experiment configuration
        let experiments = self.experiments.read().await;
        let experiment = match experiments.get(experiment_name) {
            Some(exp) => exp,
            None => {
                return VariantAssignment::not_enrolled("experiment_not_found");
            }
        };
        
        // 3. Check experiment eligibility
        if !self.is_user_eligible(experiment, user_id, user_attributes).await {
            return VariantAssignment::not_enrolled("not_eligible");
        }
        
        // 4. Assign variant using consistent hashing
        let variant = self.assign_variant_by_hash(experiment, user_id).await;
        
        // 5. Store assignment
        let assignment = VariantAssignment {
            experiment_name: experiment_name.to_string(),
            user_id: user_id.to_string(),
            variant: variant.clone(),
            assigned_at: chrono::Utc::now(),
            enrollment_status: EnrollmentStatus::Enrolled,
        };
        
        let mut assignments = self.assignments.write().await;
        assignments.insert(assignment_key, assignment.clone());
        
        // 6. Record metrics
        self.metrics.assignments
            .with_label_values(&[experiment_name, &variant.name])
            .inc();
        
        assignment
    }
    
    async fn assign_variant_by_hash(&self, experiment: &Experiment, user_id: &str) -> Variant {
        let user_hash = self.hash_user_id(user_id);
        let hash_percentage = (user_hash % 100) as f64;
        
        let mut cumulative_percentage = 0.0;
        for variant in &experiment.variants {
            cumulative_percentage += variant.traffic_percentage;
            if hash_percentage < cumulative_percentage {
                return variant.clone();
            }
        }
        
        // Fallback to control variant
        experiment.variants.first().unwrap().clone()
    }
    
    pub async fn track_conversion(
        &self,
        experiment_name: &str,
        user_id: &str,
        event_name: &str,
        event_value: Option<f64>,
    ) -> Result<(), ABTestError> {
        // 1. Get user's variant assignment
        let assignment = self.get_assignment(experiment_name, user_id).await?;
        
        // 2. Record conversion event
        self.metrics.conversions
            .with_label_values(&[
                experiment_name,
                &assignment.variant.name,
                event_name
            ])
            .inc();
        
        // 3. Record conversion value if provided
        if let Some(value) = event_value {
            self.metrics.conversion_value
                .with_label_values(&[
                    experiment_name,
                    &assignment.variant.name,
                    event_name
                ])
                .observe(value);
        }
        
        Ok(())
    }
}

// Data structures
#[derive(Debug, Clone)]
pub struct Experiment {
    pub name: String,
    pub description: String,
    pub variants: Vec<Variant>,
    pub traffic_percentage: f64,
    pub eligibility_criteria: Vec<EligibilityCriterion>,
    pub success_metrics: Vec<String>,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub description: String,
    pub traffic_percentage: f64,
    pub configuration: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct VariantAssignment {
    pub experiment_name: String,
    pub user_id: String,
    pub variant: Variant,
    pub assigned_at: chrono::DateTime<chrono::Utc>,
    pub enrollment_status: EnrollmentStatus,
}
```

#### 6. Canary Controller Component
**PlantUML элемент:**
```plantuml
Component(canary_controller, "Canary Controller", "Rust Service", "Canary deployment automation")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/canary.rs
pub struct CanaryController {
    feature_flags: Arc<FeatureFlagService>,
    health_checker: Arc<HealthChecker>,
    rollback_manager: Arc<RollbackManager>,
    metrics: Arc<CanaryMetrics>,
    config: CanaryConfig,
}

impl CanaryController {
    pub async fn start_canary(&self, flag_name: &str) -> Result<CanaryDeployment, CanaryError> {
        let config = self.get_canary_config(flag_name)?;
        
        // 1. Initialize canary deployment
        let deployment = CanaryDeployment {
            flag_name: flag_name.to_string(),
            current_percentage: config.initial_percentage,
            target_percentage: config.promotion_steps[0],
            status: CanaryStatus::Starting,
            started_at: chrono::Utc::now(),
            last_promotion_at: None,
            health_checks: Vec::new(),
        };
        
        // 2. Set initial rollout percentage
        self.feature_flags
            .set_rollout_percentage(flag_name, config.initial_percentage)
            .await?;
        
        // 3. Start health monitoring
        self.start_health_monitoring(flag_name).await?;
        
        // 4. Record metrics
        self.metrics.canary_started
            .with_label_values(&[flag_name])
            .inc();
        
        info!("Canary deployment started for {} at {}%", 
              flag_name, config.initial_percentage);
        
        Ok(deployment)
    }
    
    pub async fn promote_canary(&self, flag_name: &str) -> Result<PromotionResult, CanaryError> {
        let current_percentage = self.get_current_percentage(flag_name).await?;
        let config = self.get_canary_config(flag_name)?;
        
        // 1. Find next promotion step
        let next_step = self.get_next_promotion_step(current_percentage, &config.promotion_steps)?;
        
        // 2. Check promotion criteria
        let health_check = self.health_checker.check_promotion_criteria(flag_name).await?;
        
        if !health_check.can_promote {
            warn!("Promotion criteria not met for {}: {:?}", flag_name, health_check.failures);
            
            // Check if rollback is needed
            if health_check.should_rollback {
                return self.trigger_rollback(flag_name, "Health check failed").await;
            }
            
            return Ok(PromotionResult::Paused {
                reason: "Health criteria not met".to_string(),
                failures: health_check.failures,
            });
        }
        
        // 3. Promote to next step
        self.feature_flags
            .set_rollout_percentage(flag_name, next_step)
            .await?;
        
        // 4. Record promotion
        self.metrics.canary_promoted
            .with_label_values(&[flag_name, &next_step.to_string()])
            .inc();
        
        info!("Canary promoted for {} from {}% to {}%", 
              flag_name, current_percentage, next_step);
        
        // 5. Check if deployment is complete
        if next_step >= 100.0 {
            self.complete_canary_deployment(flag_name).await?;
            Ok(PromotionResult::Completed)
        } else {
            Ok(PromotionResult::Promoted { new_percentage: next_step })
        }
    }
    
    async fn trigger_rollback(&self, flag_name: &str, reason: &str) -> Result<PromotionResult, CanaryError> {
        warn!("Triggering rollback for {}: {}", flag_name, reason);
        
        // 1. Rollback to 0%
        self.feature_flags
            .set_rollout_percentage(flag_name, 0.0)
            .await?;
        
        // 2. Record rollback event
        self.metrics.canary_rollback
            .with_label_values(&[flag_name, reason])
            .inc();
        
        // 3. Notify stakeholders
        self.rollback_manager
            .notify_rollback(flag_name, reason)
            .await?;
        
        Ok(PromotionResult::RolledBack { reason: reason.to_string() })
    }
}

// Health checker implementation
pub struct HealthChecker {
    metrics_client: Arc<MetricsClient>,
    thresholds: HealthThresholds,
}

impl HealthChecker {
    pub async fn check_promotion_criteria(&self, flag_name: &str) -> Result<HealthCheckResult, HealthError> {
        let mut failures = Vec::new();
        
        // 1. Check error rate
        let error_rate = self.metrics_client.get_error_rate(flag_name).await?;
        if error_rate > self.thresholds.max_error_rate {
            failures.push(HealthFailure::HighErrorRate { 
                current: error_rate, 
                threshold: self.thresholds.max_error_rate 
            });
        }
        
        // 2. Check response time
        let p95_latency = self.metrics_client.get_p95_latency(flag_name).await?;
        if p95_latency > self.thresholds.max_response_time {
            failures.push(HealthFailure::HighLatency { 
                current: p95_latency, 
                threshold: self.thresholds.max_response_time 
            });
        }
        
        // 3. Check success rate
        let success_rate = self.metrics_client.get_success_rate(flag_name).await?;
        if success_rate < self.thresholds.min_success_rate {
            failures.push(HealthFailure::LowSuccessRate { 
                current: success_rate, 
                threshold: self.thresholds.min_success_rate 
            });
        }
        
        Ok(HealthCheckResult {
            can_promote: failures.is_empty(),
            should_rollback: failures.iter().any(|f| f.is_critical()),
            failures,
            checked_at: chrono::Utc::now(),
        })
    }
}
```

### Configuration и Data Structures

#### Configuration Loader Component
**PlantUML элемент:**
```plantuml
Component(config_loader, "Configuration Loader", "Rust Module", "YAML configuration management")
```

**Код реализации:**
```rust
// ugc-subgraph/src/migration/config_loader.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct MigrationConfig {
    pub feature_flags: HashMap<String, FeatureFlagConfig>,
    pub ab_tests: HashMap<String, ABTestConfig>,
    pub canary_deployments: HashMap<String, CanaryConfig>,
    pub migration_phases: HashMap<String, MigrationPhase>,
}

impl MigrationConfig {
    pub fn load_from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::FileRead { path: path.to_string(), error: e })?;
        
        let config: MigrationConfig = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::YamlParse { error: e })?;
        
        config.validate()?;
        Ok(config)
    }
    
    pub fn load_from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();
        
        // Override with environment variables
        if let Ok(enabled) = std::env::var("FF_GRAPHQL_REVIEWS_READ_ENABLED") {
            if let Some(flag_config) = config.feature_flags.get_mut("graphql_reviews_read") {
                flag_config.enabled = enabled.parse().unwrap_or(false);
            }
        }
        
        if let Ok(rollout) = std::env::var("FF_GRAPHQL_REVIEWS_READ_ROLLOUT") {
            if let Some(flag_config) = config.feature_flags.get_mut("graphql_reviews_read") {
                flag_config.rollout_percentage = rollout.parse().unwrap_or(0.0);
            }
        }
        
        config.validate()?;
        Ok(config)
    }
    
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate feature flags
        for (name, flag) in &self.feature_flags {
            if flag.rollout_percentage < 0.0 || flag.rollout_percentage > 100.0 {
                return Err(ConfigError::InvalidRolloutPercentage { 
                    flag: name.clone(), 
                    percentage: flag.rollout_percentage 
                });
            }
        }
        
        // Validate A/B tests
        for (name, test) in &self.ab_tests {
            let total_percentage: f64 = test.variants.values().sum();
            if (total_percentage - 100.0).abs() > 0.01 {
                return Err(ConfigError::InvalidVariantDistribution { 
                    test: name.clone(), 
                    total: total_percentage 
                });
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct FeatureFlagConfig {
    pub enabled: bool,
    pub rollout_percentage: f64,
    pub description: String,
    pub user_whitelist: Vec<String>,
    pub user_blacklist: Vec<String>,
    pub conditions: Vec<FlagConditionConfig>,
}

#[derive(Debug, Deserialize)]
pub struct CanaryConfig {
    pub initial_percentage: f64,
    pub promotion_steps: Vec<f64>,
    pub step_duration_minutes: u64,
    pub success_criteria: SuccessCriteria,
    pub rollback_criteria: RollbackCriteria,
}

#[derive(Debug, Deserialize)]
pub struct SuccessCriteria {
    pub max_error_rate: f64,
    pub max_response_time_p95: f64,
    pub min_success_rate: f64,
}
```

### Заключение

Компонентная диаграмма Task 13 обеспечивает детальную трассируемость между архитектурными компонентами и их реализацией в коде:

1. **REST Endpoints** → Axum route handlers с middleware integration
2. **Feature Flag Engine** → Comprehensive flag evaluation с caching
3. **A/B Test Engine** → Experiment management с consistent hashing
4. **Canary Controller** → Automated deployment с health monitoring
5. **Configuration Management** → YAML loading с validation
6. **Metrics Collection** → Prometheus integration с business metrics

Каждый компонент имеет четко определенную реализацию с proper error handling, metrics collection, и comprehensive testing capabilities.