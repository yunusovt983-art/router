use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};
use std::process;
use tokio;

#[derive(Parser)]
#[command(name = "migration-cli")]
#[command(about = "CLI tool for managing GraphQL migration feature flags")]
struct Cli {
    #[arg(short, long, default_value = "http://localhost:4001")]
    base_url: String,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all feature flags
    List,
    
    /// Get details of a specific feature flag
    Get {
        /// Name of the feature flag
        flag_name: String,
    },
    
    /// Enable a feature flag
    Enable {
        /// Name of the feature flag
        flag_name: String,
    },
    
    /// Disable a feature flag
    Disable {
        /// Name of the feature flag
        flag_name: String,
    },
    
    /// Set rollout percentage for a feature flag
    Rollout {
        /// Name of the feature flag
        flag_name: String,
        /// Rollout percentage (0-100)
        percentage: f64,
    },
    
    /// Enable flag for a specific user
    EnableUser {
        /// Name of the feature flag
        flag_name: String,
        /// User ID
        user_id: String,
    },
    
    /// Disable flag for a specific user
    DisableUser {
        /// Name of the feature flag
        flag_name: String,
        /// User ID
        user_id: String,
    },
    
    /// Start canary deployment
    StartCanary {
        /// Name of the feature flag
        flag_name: String,
    },
    
    /// Promote canary deployment
    PromoteCanary {
        /// Name of the feature flag
        flag_name: String,
        /// Target percentage
        percentage: f64,
    },
    
    /// Rollback canary deployment
    RollbackCanary {
        /// Name of the feature flag
        flag_name: String,
    },
    
    /// Emergency rollback all flags
    EmergencyRollback {
        /// Reason for rollback
        reason: String,
    },
    
    /// Get migration status
    Status,
    
    /// Get migration metrics
    Metrics,
    
    /// Create A/B test
    CreateAbTest {
        /// Name of the A/B test
        test_name: String,
        /// Description
        description: String,
        /// Traffic percentage
        #[arg(default_value = "50.0")]
        traffic_percentage: f64,
    },
    
    /// Assign user to A/B test variant
    AssignUser {
        /// Name of the A/B test
        test_name: String,
        /// User ID
        user_id: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = Client::new();

    let result = match cli.command {
        Commands::List => list_flags(&client, &cli.base_url).await,
        Commands::Get { flag_name } => get_flag(&client, &cli.base_url, &flag_name).await,
        Commands::Enable { flag_name } => enable_flag(&client, &cli.base_url, &flag_name).await,
        Commands::Disable { flag_name } => disable_flag(&client, &cli.base_url, &flag_name).await,
        Commands::Rollout { flag_name, percentage } => {
            set_rollout(&client, &cli.base_url, &flag_name, percentage).await
        }
        Commands::EnableUser { flag_name, user_id } => {
            enable_flag_for_user(&client, &cli.base_url, &flag_name, &user_id).await
        }
        Commands::DisableUser { flag_name, user_id } => {
            disable_flag_for_user(&client, &cli.base_url, &flag_name, &user_id).await
        }
        Commands::StartCanary { flag_name } => {
            start_canary(&client, &cli.base_url, &flag_name).await
        }
        Commands::PromoteCanary { flag_name, percentage } => {
            promote_canary(&client, &cli.base_url, &flag_name, percentage).await
        }
        Commands::RollbackCanary { flag_name } => {
            rollback_canary(&client, &cli.base_url, &flag_name).await
        }
        Commands::EmergencyRollback { reason } => {
            emergency_rollback(&client, &cli.base_url, &reason).await
        }
        Commands::Status => get_status(&client, &cli.base_url).await,
        Commands::Metrics => get_metrics(&client, &cli.base_url).await,
        Commands::CreateAbTest { test_name, description, traffic_percentage } => {
            create_ab_test(&client, &cli.base_url, &test_name, &description, traffic_percentage).await
        }
        Commands::AssignUser { test_name, user_id } => {
            assign_user_to_variant(&client, &cli.base_url, &test_name, &user_id).await
        }
    };

    match result {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

async fn list_flags(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/api/migration/flags", base_url))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if let Some(flags) = data["flags"].as_array() {
        println!("Feature Flags:");
        println!("{:<30} {:<10} {:<10} {}", "Name", "Enabled", "Rollout%", "Description");
        println!("{}", "-".repeat(80));
        
        for flag in flags {
            let name = flag["name"].as_str().unwrap_or("N/A");
            let enabled = flag["enabled"].as_bool().unwrap_or(false);
            let rollout = flag["rollout_percentage"].as_f64().unwrap_or(0.0);
            let description = flag["description"].as_str().unwrap_or("");
            
            println!("{:<30} {:<10} {:<10.1} {}", name, enabled, rollout, description);
        }
    }

    Ok(())
}

async fn get_flag(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/api/migration/flags/{}", base_url, flag_name))
        .send()
        .await?;

    if response.status().is_success() {
        let data: Value = response.json().await?;
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else {
        println!("Flag '{}' not found", flag_name);
    }

    Ok(())
}

async fn enable_flag(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/flags/{}/enable", base_url, flag_name))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Flag enabled"));
    } else {
        println!("✗ Failed to enable flag");
    }

    Ok(())
}

async fn disable_flag(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/flags/{}/disable", base_url, flag_name))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Flag disabled"));
    } else {
        println!("✗ Failed to disable flag");
    }

    Ok(())
}

async fn set_rollout(client: &Client, base_url: &str, flag_name: &str, percentage: f64) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .put(&format!("{}/api/migration/flags/{}/rollout", base_url, flag_name))
        .json(&json!({ "percentage": percentage }))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Rollout percentage updated"));
    } else {
        println!("✗ Failed to update rollout percentage");
    }

    Ok(())
}

async fn enable_flag_for_user(client: &Client, base_url: &str, flag_name: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/flags/{}/users/{}/enable", base_url, flag_name, user_id))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Flag enabled for user"));
    } else {
        println!("✗ Failed to enable flag for user");
    }

    Ok(())
}

async fn disable_flag_for_user(client: &Client, base_url: &str, flag_name: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/flags/{}/users/{}/disable", base_url, flag_name, user_id))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Flag disabled for user"));
    } else {
        println!("✗ Failed to disable flag for user");
    }

    Ok(())
}

async fn start_canary(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/canary/{}/start", base_url, flag_name))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Canary deployment started"));
    } else {
        println!("✗ Failed to start canary deployment");
    }

    Ok(())
}

async fn promote_canary(client: &Client, base_url: &str, flag_name: &str, percentage: f64) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/canary/{}/promote", base_url, flag_name))
        .json(&json!({ "target_percentage": percentage }))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Canary promoted"));
    } else {
        println!("✗ Failed to promote canary");
    }

    Ok(())
}

async fn rollback_canary(client: &Client, base_url: &str, flag_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/canary/{}/rollback", base_url, flag_name))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Canary rolled back"));
    } else {
        println!("✗ Failed to rollback canary");
    }

    Ok(())
}

async fn emergency_rollback(client: &Client, base_url: &str, reason: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/emergency/rollback", base_url))
        .json(&json!({ "reason": reason }))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ {}", data["message"].as_str().unwrap_or("Emergency rollback completed"));
    } else {
        println!("✗ Failed to perform emergency rollback");
    }

    Ok(())
}

async fn get_status(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/api/migration/status", base_url))
        .send()
        .await?;

    let data: Value = response.json().await?;
    println!("Migration Status:");
    println!("{}", serde_json::to_string_pretty(&data)?);

    Ok(())
}

async fn get_metrics(client: &Client, base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/api/migration/metrics", base_url))
        .send()
        .await?;

    let data: Value = response.json().await?;
    println!("Migration Metrics:");
    println!("{}", serde_json::to_string_pretty(&data)?);

    Ok(())
}

async fn create_ab_test(client: &Client, base_url: &str, test_name: &str, description: &str, traffic_percentage: f64) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/api/migration/ab-tests", base_url))
        .json(&json!({
            "name": test_name,
            "description": description,
            "traffic_percentage": traffic_percentage
        }))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    if data["success"].as_bool().unwrap_or(false) {
        println!("✓ A/B test '{}' created successfully", test_name);
        println!("Flag name: {}", data["flag_name"].as_str().unwrap_or("N/A"));
    } else {
        println!("✗ Failed to create A/B test");
    }

    Ok(())
}

async fn assign_user_to_variant(client: &Client, base_url: &str, test_name: &str, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/api/migration/ab-tests/{}/assign/{}", base_url, test_name, user_id))
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    println!("User {} assigned to variant: {}", 
             data["user_id"].as_str().unwrap_or("N/A"),
             data["variant"].as_str().unwrap_or("N/A"));

    Ok(())
}