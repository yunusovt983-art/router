use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::info;

pub async fn create_database_pool(database_url: &str) -> Result<PgPool> {
    // Log database URL with password masked
    let masked_url = if database_url.contains('@') {
        let parts: Vec<&str> = database_url.split('@').collect();
        if parts.len() == 2 {
            let user_part = parts[0];
            let host_part = parts[1];
            if let Some(colon_pos) = user_part.rfind(':') {
                format!("{}:***@{}", &user_part[..colon_pos], host_part)
            } else {
                database_url.to_string()
            }
        } else {
            database_url.to_string()
        }
    } else {
        database_url.to_string()
    };
    info!("Connecting to database: {}", masked_url);
    
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    info!("Database connection pool created successfully");
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    info!("Running database migrations...");
    
    // Create migrations table if it doesn't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Check which migrations have been applied
    let applied_migrations: Vec<i32> = sqlx::query_scalar("SELECT version FROM _migrations ORDER BY version")
        .fetch_all(pool)
        .await?;

    // Migration 001: Create reviews table
    if !applied_migrations.contains(&1) {
        info!("Applying migration 001: Create reviews table");
        let migration_sql = include_str!("../migrations/001_create_reviews_table.sql");
        sqlx::query(migration_sql).execute(pool).await?;
        
        sqlx::query("INSERT INTO _migrations (version, name) VALUES (1, 'create_reviews_table')")
            .execute(pool)
            .await?;
        info!("Migration 001 applied successfully");
    }

    // Migration 002: Create offer_ratings table
    if !applied_migrations.contains(&2) {
        info!("Applying migration 002: Create offer_ratings table");
        let migration_sql = include_str!("../migrations/002_create_offer_ratings_table.sql");
        sqlx::query(migration_sql).execute(pool).await?;
        
        sqlx::query("INSERT INTO _migrations (version, name) VALUES (2, 'create_offer_ratings_table')")
            .execute(pool)
            .await?;
        info!("Migration 002 applied successfully");
    }

    info!("All migrations completed successfully");
    Ok(())
}

pub async fn check_database_health(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1").fetch_one(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    
    #[test]
    fn test_mask_database_url() {
        let url = "postgresql://user:password@localhost:5432/db";
        let masked = if url.contains('@') {
            let parts: Vec<&str> = url.split('@').collect();
            if parts.len() == 2 {
                let user_part = parts[0];
                let host_part = parts[1];
                if let Some(colon_pos) = user_part.rfind(':') {
                    format!("{}:***@{}", &user_part[..colon_pos], host_part)
                } else {
                    url.to_string()
                }
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        };
        
        assert_eq!(masked, "postgresql://user:***@localhost:5432/db");
    }
    
    #[test]
    fn test_mask_database_url_no_password() {
        let url = "postgresql://localhost:5432/db";
        let masked = if url.contains('@') {
            let parts: Vec<&str> = url.split('@').collect();
            if parts.len() == 2 {
                let user_part = parts[0];
                let host_part = parts[1];
                if let Some(colon_pos) = user_part.rfind(':') {
                    format!("{}:***@{}", &user_part[..colon_pos], host_part)
                } else {
                    url.to_string()
                }
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        };
        
        assert_eq!(masked, "postgresql://localhost:5432/db");
    }
}