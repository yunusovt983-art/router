use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, warn, error};
use anyhow::{Result, Context};

use crate::migration::{FeatureFlag, FlagCondition, FeatureFlagService};

/// Configuration loader for feature flags and migration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    pub feature_flags: HashMap<String, FeatureFlagConfig>,
    pub ab_tests: HashMap<String, ABTestConfig>,
    pub canary_deployments: HashMap<String, CanaryConfig>,
    pub migration_phases: HashMap<String, MigrationPhase>,
    pub emergency_procedures: HashMap<String, EmergencyProcedure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagConfig {
    pub enabled: bool,
    pub rollout_percentage: f64,
    pub description: String,
    pub conditions: Vec<FlagConditionConfig>,
    pub user_whitelist: Vec<String>,
    pub user_blacklist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagConditionConfig {
    pub condition_type: String,
    pub value: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    pub name: String,
    pub description: String,
    pub variants: HashMap<String, ABTestVariant>,
    pub success_metrics: Vec<String>,
    pub duration_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestVariant {
    pub name: String,
    pub description: String,
    pub traffic_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryConfig {
    pub initial_percentage: f64,
    pub promotion_steps: Vec<f64>,
    pub step_duration_minutes: u64,
    pub success_criteria: SuccessCriteria,
    pub rollback_criteria: RollbackCriteria,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessCriteria {
    pub max_error_rate: f64,
    pub max_response_time_p95: u64,
    pub min_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackCriteria {
    pub max_error_rate: f64,
    pub max_response_time_p95: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPhase {
    pub name: String,
    pub description: String,
    pub flags: Vec<String>,
    pub target_completion_date: String,
    pub success_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyProcedure {
    pub threshold: f64,
    pub actions: Vec<String>,
}

impl MigrationConfig {
    /// Load configuration from YAML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        
        let config: MigrationConfig = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse YAML configuration")?;
        
        info!("Loaded migration configuration from {}", path.as_ref().display());
        Ok(config)
    }

    /// Load configuration from environment variables and defaults
    pub fn load_from_env() -> Self {
        info!("Loading migration configuration from environment variables");
        
        // Create default configuration
        let mut feature_flags = HashMap::new();
        
        // Add default feature flags
        feature_flags.insert("graphql_reviews_read".to_string(), FeatureFlagConfig {
            enabled: std::env::var("FF_GRAPHQL_REVIEWS_READ_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            rollout_percentage: std::env::var("FF_GRAPHQL_REVIEWS_READ_ROLLOUT")
                .unwrap_or_else(|_| "0.0".to_string())
                .parse()
                .unwrap_or(0.0),
            description: "Enable GraphQL for reading reviews".to_string(),
            conditions: vec![],
            user_whitelist: vec![],
            user_blacklist: vec![],
        });

        feature_flags.insert("graphql_reviews_write".to_string(), FeatureFlagConfig {
            enabled: std::env::var("FF_GRAPHQL_REVIEWS_WRITE_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            rollout_percentage: std::env::var("FF_GRAPHQL_REVIEWS_WRITE_ROLLOUT")
                .unwrap_or_else(|_| "0.0".to_string())
                .parse()
                .unwrap_or(0.0),
            description: "Enable GraphQL for writing reviews".to_string(),
            conditions: vec![],
            user_whitelist: vec![],
            user_blacklist: vec![],
        });

        Self {
            feature_flags,
            ab_tests: HashMap::new(),
            canary_deployments: HashMap::new(),
            migration_phases: HashMap::new(),
            emergency_procedures: HashMap::new(),
        }
    }

    /// Apply configuration to feature flag service
    pub async fn apply_to_service(&self, service: &FeatureFlagService) -> Result<()> {
        info!("Applying migration configuration to feature flag service");
        
        for (flag_name, flag_config) in &self.feature_flags {
            let conditions = flag_config.conditions.iter()
                .filter_map(|c| self.parse_condition(c))
                .collect();

            let flag = FeatureFlag {
                name: flag_name.clone(),
                enabled: flag_config.enabled,
                rollout_percentage: flag_config.rollout_percentage,
                user_whitelist: flag_config.user_whitelist.clone(),
                user_blacklist: flag_config.user_blacklist.clone(),
                conditions,
                description: flag_config.description.clone(),
            };

            if let Err(e) = service.update_flag(flag_name, flag).await {
                error!("Failed to apply flag '{}': {}", flag_name, e);
            } else {
                info!("Applied flag '{}': enabled={}, rollout={}%", 
                      flag_name, flag_config.enabled, flag_config.rollout_percentage);
            }
        }

        Ok(())
    }

    fn parse_condition(&self, config: &FlagConditionConfig) -> Option<FlagCondition> {
        match config.condition_type.as_str() {
            "user_id_starts_with" => Some(FlagCondition::UserIdStartsWith(config.value.clone())),
            "user_id_ends_with" => Some(FlagCondition::UserIdEndsWith(config.value.clone())),
            "user_id_matches" => Some(FlagCondition::UserIdMatches(config.value.clone())),
            "time_window" => {
                if let (Some(start_str), Some(end_str)) = (&config.start_time, &config.end_time) {
                    if let (Ok(start), Ok(end)) = (
                        chrono::DateTime::parse_from_rfc3339(start_str),
                        chrono::DateTime::parse_from_rfc3339(end_str)
                    ) {
                        Some(FlagCondition::TimeWindow {
                            start: start.with_timezone(&chrono::Utc),
                            end: end.with_timezone(&chrono::Utc),
                        })
                    } else {
                        warn!("Invalid time format in condition: {} - {}", start_str, end_str);
                        None
                    }
                } else {
                    warn!("Missing start_time or end_time for time_window condition");
                    None
                }
            }
            _ => {
                warn!("Unknown condition type: {}", config.condition_type);
                None
            }
        }
    }

    /// Get canary configuration for a flag
    pub fn get_canary_config(&self, flag_name: &str) -> Option<&CanaryConfig> {
        self.canary_deployments.get(flag_name)
    }

    /// Get A/B test configuration
    pub fn get_ab_test_config(&self, test_name: &str) -> Option<&ABTestConfig> {
        self.ab_tests.get(test_name)
    }

    /// Get migration phase information
    pub fn get_migration_phase(&self, phase_name: &str) -> Option<&MigrationPhase> {
        self.migration_phases.get(phase_name)
    }

    /// Get emergency procedure configuration
    pub fn get_emergency_procedure(&self, procedure_name: &str) -> Option<&EmergencyProcedure> {
        self.emergency_procedures.get(procedure_name)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        info!("Validating migration configuration");

        // Validate feature flags
        for (flag_name, flag_config) in &self.feature_flags {
            if flag_config.rollout_percentage < 0.0 || flag_config.rollout_percentage > 100.0 {
                return Err(anyhow::anyhow!(
                    "Invalid rollout percentage for flag '{}': {}",
                    flag_name,
                    flag_config.rollout_percentage
                ));
            }
        }

        // Validate A/B tests
        for (test_name, test_config) in &self.ab_tests {
            let total_traffic: f64 = test_config.variants.values()
                .map(|v| v.traffic_percentage)
                .sum();
            
            if (total_traffic - 100.0).abs() > 0.01 {
                return Err(anyhow::anyhow!(
                    "A/B test '{}' traffic percentages don't sum to 100%: {}",
                    test_name,
                    total_traffic
                ));
            }
        }

        // Validate canary deployments
        for (flag_name, canary_config) in &self.canary_deployments {
            if canary_config.promotion_steps.is_empty() {
                return Err(anyhow::anyhow!(
                    "Canary deployment '{}' has no promotion steps",
                    flag_name
                ));
            }

            let max_step = canary_config.promotion_steps.iter().max_by(|a, b| a.partial_cmp(b).unwrap());
            if let Some(max) = max_step {
                if *max > 100.0 {
                    return Err(anyhow::anyhow!(
                        "Canary deployment '{}' has promotion step > 100%: {}",
                        flag_name,
                        max
                    ));
                }
            }
        }

        info!("Migration configuration validation passed");
        Ok(())
    }

    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .with_context(|| "Failed to serialize configuration to YAML")?;
        
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;
        
        info!("Saved migration configuration to {}", path.as_ref().display());
        Ok(())
    }
}

/// Configuration loader service
pub struct ConfigLoader {
    config_path: Option<String>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            config_path: std::env::var("MIGRATION_CONFIG_PATH").ok(),
        }
    }

    pub fn with_config_path(mut self, path: String) -> Self {
        self.config_path = Some(path);
        self
    }

    /// Load configuration from file or environment
    pub async fn load(&self) -> Result<MigrationConfig> {
        if let Some(path) = &self.config_path {
            if Path::new(path).exists() {
                info!("Loading migration configuration from file: {}", path);
                let config = MigrationConfig::load_from_file(path)?;
                config.validate()?;
                return Ok(config);
            } else {
                warn!("Configuration file not found: {}, falling back to environment", path);
            }
        }

        info!("Loading migration configuration from environment variables");
        let config = MigrationConfig::load_from_env();
        config.validate()?;
        Ok(config)
    }

    /// Watch for configuration changes and reload
    pub async fn watch_and_reload(
        &self,
        service: std::sync::Arc<FeatureFlagService>,
    ) -> Result<()> {
        if let Some(path) = &self.config_path {
            info!("Starting configuration file watcher for: {}", path);
            
            // In a real implementation, this would use a file watcher like notify
            // For now, we'll just reload periodically
            let path = path.clone();
            let service = service.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
                
                loop {
                    interval.tick().await;
                    
                    if let Ok(config) = MigrationConfig::load_from_file(&path) {
                        if let Err(e) = config.apply_to_service(&service).await {
                            error!("Failed to apply reloaded configuration: {}", e);
                        } else {
                            info!("Successfully reloaded migration configuration");
                        }
                    }
                }
            });
        }

        Ok(())
    }
}