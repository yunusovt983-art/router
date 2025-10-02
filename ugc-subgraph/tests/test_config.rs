use std::sync::Once;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static INIT: Once = Once::new();

/// Initialize test logging - call this at the beginning of tests that need logging
pub fn init_test_logging() {
    INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "ugc_subgraph=debug,tower_http=debug".into()),
            )
            .with(tracing_subscriber::fmt::layer().with_test_writer())
            .init();
    });
}

/// Test configuration constants
pub mod constants {
    use std::time::Duration;
    
    pub const TEST_TIMEOUT: Duration = Duration::from_secs(30);
    pub const PERFORMANCE_TEST_TIMEOUT: Duration = Duration::from_secs(120);
    pub const E2E_TEST_TIMEOUT: Duration = Duration::from_secs(60);
    
    pub const DEFAULT_TEST_RATING: i32 = 5;
    pub const DEFAULT_TEST_TEXT: &str = "Test review text";
    
    pub const PERFORMANCE_TEST_REVIEW_COUNT: usize = 1000;
    pub const CONCURRENT_TEST_OPERATIONS: usize = 100;
    
    pub const MIN_ACCEPTABLE_THROUGHPUT: f64 = 10.0; // operations per second
    pub const MAX_ACCEPTABLE_LATENCY_MS: u128 = 1000; // milliseconds
}

/// Test utilities
pub mod utils {
    use uuid::Uuid;
    use ugc_subgraph::{
        models::review::{CreateReviewInput, UpdateReviewInput},
        auth::UserContext,
    };
    
    pub fn create_test_review_input(offer_id: Option<Uuid>) -> CreateReviewInput {
        CreateReviewInput {
            offer_id: offer_id.unwrap_or_else(Uuid::new_v4),
            rating: super::constants::DEFAULT_TEST_RATING,
            text: super::constants::DEFAULT_TEST_TEXT.to_string(),
        }
    }
    
    pub fn create_test_update_input() -> UpdateReviewInput {
        UpdateReviewInput {
            rating: Some(4),
            text: Some("Updated test review".to_string()),
        }
    }
    
    pub fn create_test_user_context() -> UserContext {
        UserContext::new(
            Uuid::new_v4(),
            "Test User".to_string(),
            "test@example.com".to_string(),
            vec!["user".to_string()],
        )
    }
    
    pub fn create_test_moderator_context() -> UserContext {
        UserContext::new(
            Uuid::new_v4(),
            "Test Moderator".to_string(),
            "moderator@example.com".to_string(),
            vec!["user".to_string(), "moderator".to_string()],
        )
    }
    
    pub fn create_test_admin_context() -> UserContext {
        UserContext::new(
            Uuid::new_v4(),
            "Test Admin".to_string(),
            "admin@example.com".to_string(),
            vec!["user".to_string(), "moderator".to_string(), "admin".to_string()],
        )
    }
}

/// Test assertions and helpers
pub mod assertions {
    use std::time::Duration;
    
    pub fn assert_performance_acceptable(duration: Duration, operation_name: &str) {
        assert!(
            duration.as_millis() < super::constants::MAX_ACCEPTABLE_LATENCY_MS,
            "{} took {:?}, which exceeds maximum acceptable latency of {:?}",
            operation_name,
            duration,
            Duration::from_millis(super::constants::MAX_ACCEPTABLE_LATENCY_MS as u64)
        );
    }
    
    pub fn assert_throughput_acceptable(operations: usize, duration: Duration, operation_name: &str) {
        let throughput = operations as f64 / duration.as_secs_f64();
        assert!(
            throughput >= super::constants::MIN_ACCEPTABLE_THROUGHPUT,
            "{} throughput of {:.2} ops/sec is below minimum acceptable throughput of {:.2} ops/sec",
            operation_name,
            throughput,
            super::constants::MIN_ACCEPTABLE_THROUGHPUT
        );
    }
    
    pub fn assert_success_rate_acceptable(successful: usize, total: usize, min_success_rate: f64) {
        let success_rate = successful as f64 / total as f64;
        assert!(
            success_rate >= min_success_rate,
            "Success rate of {:.2}% is below minimum acceptable rate of {:.2}%",
            success_rate * 100.0,
            min_success_rate * 100.0
        );
    }
}

/// Database test helpers
pub mod database {
    use sqlx::PgPool;
    use testcontainers::{clients::Cli, images::postgres::Postgres, Container};
    
    pub async fn setup_test_database() -> (PgPool, Container<'static, Postgres>) {
        let docker = Cli::default();
        let postgres_image = Postgres::default();
        let container = docker.run(postgres_image);
        
        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            container.get_host_port_ipv4(5432)
        );
        
        let pool = PgPool::connect(&connection_string)
            .await
            .expect("Failed to connect to test database");
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");
        
        (pool, container)
    }
    
    pub async fn cleanup_test_data(pool: &PgPool) {
        // Clean up test data in reverse dependency order
        let _ = sqlx::query("DELETE FROM offer_ratings").execute(pool).await;
        let _ = sqlx::query("DELETE FROM reviews").execute(pool).await;
    }
}

/// Mock helpers
pub mod mocks {
    use mockall::predicate::*;
    use uuid::Uuid;
    use ugc_subgraph::{
        repository::MockReviewRepository,
        models::{
            review::{Review, ModerationStatus},
            offer_rating::OfferRating,
        },
        error::UgcError,
    };
    
    pub fn create_mock_review() -> Review {
        Review {
            id: Uuid::new_v4(),
            offer_id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            rating: 5,
            text: "Mock review".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_moderated: true,
            moderation_status: ModerationStatus::Approved,
        }
    }
    
    pub fn create_mock_offer_rating(offer_id: Uuid) -> OfferRating {
        OfferRating {
            offer_id,
            average_rating: rust_decimal::Decimal::new(45, 1), // 4.5
            reviews_count: 10,
            rating_distribution: serde_json::json!({
                "1": 0,
                "2": 1,
                "3": 2,
                "4": 3,
                "5": 4
            }),
            updated_at: chrono::Utc::now(),
        }
    }
    
    pub fn setup_successful_mock_repository() -> MockReviewRepository {
        let mut mock_repo = MockReviewRepository::new();
        
        mock_repo
            .expect_get_review_by_id()
            .returning(|_| Ok(Some(create_mock_review())));
        
        mock_repo
            .expect_create_review()
            .returning(|_, _| Ok(create_mock_review()));
        
        mock_repo
            .expect_update_review()
            .returning(|_, _| Ok(create_mock_review()));
        
        mock_repo
            .expect_delete_review()
            .returning(|_| Ok(()));
        
        mock_repo
            .expect_get_reviews_with_pagination()
            .returning(|_, _, _| Ok((vec![create_mock_review()], 1)));
        
        mock_repo
    }
    
    pub fn setup_error_mock_repository() -> MockReviewRepository {
        let mut mock_repo = MockReviewRepository::new();
        
        mock_repo
            .expect_get_review_by_id()
            .returning(|_| Err(UgcError::DatabaseError("Mock database error".to_string())));
        
        mock_repo
            .expect_create_review()
            .returning(|_, _| Err(UgcError::ValidationError { 
                message: "Mock validation error".to_string() 
            }));
        
        mock_repo
    }
}

/// Test data generators
pub mod generators {
    use fake::{Fake, Faker};
    use uuid::Uuid;
    use ugc_subgraph::models::review::{CreateReviewInput, ModerationStatus};
    
    pub fn generate_random_review_input() -> CreateReviewInput {
        CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: (1..=5).fake(),
            text: Faker.fake::<String>(),
        }
    }
    
    pub fn generate_review_inputs(count: usize) -> Vec<CreateReviewInput> {
        (0..count).map(|_| generate_random_review_input()).collect()
    }
    
    pub fn generate_random_moderation_status() -> ModerationStatus {
        match (0..4).fake::<u8>() {
            0 => ModerationStatus::Pending,
            1 => ModerationStatus::Approved,
            2 => ModerationStatus::Rejected,
            _ => ModerationStatus::Flagged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_constants() {
        assert!(constants::TEST_TIMEOUT.as_secs() > 0);
        assert!(constants::DEFAULT_TEST_RATING >= 1 && constants::DEFAULT_TEST_RATING <= 5);
        assert!(!constants::DEFAULT_TEST_TEXT.is_empty());
    }
    
    #[test]
    fn test_utils() {
        let input = utils::create_test_review_input(None);
        assert_eq!(input.rating, constants::DEFAULT_TEST_RATING);
        assert_eq!(input.text, constants::DEFAULT_TEST_TEXT);
        
        let user = utils::create_test_user_context();
        assert!(user.has_role("user"));
        assert!(!user.has_role("moderator"));
        
        let moderator = utils::create_test_moderator_context();
        assert!(moderator.has_role("user"));
        assert!(moderator.has_role("moderator"));
        assert!(!moderator.has_role("admin"));
    }
    
    #[test]
    fn test_generators() {
        let input = generators::generate_random_review_input();
        assert!(input.rating >= 1 && input.rating <= 5);
        assert!(!input.text.is_empty());
        
        let inputs = generators::generate_review_inputs(5);
        assert_eq!(inputs.len(), 5);
        
        let status = generators::generate_random_moderation_status();
        assert!(matches!(status, 
            ModerationStatus::Pending | 
            ModerationStatus::Approved | 
            ModerationStatus::Rejected | 
            ModerationStatus::Flagged
        ));
    }
}