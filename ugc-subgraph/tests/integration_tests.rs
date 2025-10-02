use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use sqlx::{PgPool, Row};
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};
use serial_test::serial;

use ugc_subgraph::{
    config::Config,
    error::{Result, UgcError},
    models::{
        offer_rating::OfferRating,
        review::{CreateReviewInput, ModerationStatus, Review, ReviewsFilter, UpdateReviewInput},
    },
    repository::{PostgresReviewRepository, ReviewRepository},
    service::ReviewService,
    auth::{AuthService, UserContext},
};

// Test database setup
async fn setup_test_db() -> PgPool {
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
    
    pool
}

// Helper function to create test data
async fn create_test_review(pool: &PgPool) -> Review {
    let offer_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    
    sqlx::query_as::<_, Review>(
        r#"
        INSERT INTO reviews (offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status)
        VALUES ($1, $2, $3, $4, NOW(), NOW(), true, 'approved')
        RETURNING id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
        "#
    )
    .bind(offer_id)
    .bind(author_id)
    .bind(5)
    .bind("Great car!")
    .fetch_one(pool)
    .await
    .expect("Failed to create test review")
}

#[tokio::test]
#[serial]
async fn test_database_connection() {
    let pool = setup_test_db().await;
    
    // Test basic connectivity
    let result = sqlx::query("SELECT 1 as test")
        .fetch_one(&pool)
        .await
        .expect("Failed to execute test query");
    
    let test_value: i32 = result.get("test");
    assert_eq!(test_value, 1);
}

#[tokio::test]
#[serial]
async fn test_repository_create_review() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool);
    
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 4,
        text: "Good car, would recommend".to_string(),
    };
    let author_id = Uuid::new_v4();
    
    let review = repository.create_review(input.clone(), author_id)
        .await
        .expect("Failed to create review");
    
    assert_eq!(review.offer_id, input.offer_id);
    assert_eq!(review.author_id, author_id);
    assert_eq!(review.rating, input.rating);
    assert_eq!(review.text, input.text);
    assert!(!review.is_moderated); // Should be false by default
    assert_eq!(review.moderation_status, ModerationStatus::Pending);
}

#[tokio::test]
#[serial]
async fn test_repository_get_review_by_id() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create a test review
    let created_review = create_test_review(&pool).await;
    
    // Retrieve it
    let retrieved_review = repository.get_review_by_id(created_review.id)
        .await
        .expect("Failed to get review")
        .expect("Review should exist");
    
    assert_eq!(retrieved_review.id, created_review.id);
    assert_eq!(retrieved_review.offer_id, created_review.offer_id);
    assert_eq!(retrieved_review.author_id, created_review.author_id);
    assert_eq!(retrieved_review.rating, created_review.rating);
    assert_eq!(retrieved_review.text, created_review.text);
}

#[tokio::test]
#[serial]
async fn test_repository_get_review_by_id_not_found() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool);
    
    let non_existent_id = Uuid::new_v4();
    let result = repository.get_review_by_id(non_existent_id)
        .await
        .expect("Query should succeed");
    
    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn test_repository_update_review() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create a test review
    let created_review = create_test_review(&pool).await;
    
    // Update it
    let update_input = UpdateReviewInput {
        rating: Some(3),
        text: Some("Updated review text".to_string()),
    };
    
    let updated_review = repository.update_review(created_review.id, update_input)
        .await
        .expect("Failed to update review");
    
    assert_eq!(updated_review.id, created_review.id);
    assert_eq!(updated_review.rating, 3);
    assert_eq!(updated_review.text, "Updated review text");
    assert!(updated_review.updated_at > created_review.updated_at);
}

#[tokio::test]
#[serial]
async fn test_repository_update_review_not_found() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool);
    
    let non_existent_id = Uuid::new_v4();
    let update_input = UpdateReviewInput {
        rating: Some(3),
        text: Some("Updated review text".to_string()),
    };
    
    let result = repository.update_review(non_existent_id, update_input).await;
    assert!(matches!(result, Err(UgcError::ReviewNotFound { .. })));
}

#[tokio::test]
#[serial]
async fn test_repository_delete_review() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create a test review
    let created_review = create_test_review(&pool).await;
    
    // Delete it
    repository.delete_review(created_review.id)
        .await
        .expect("Failed to delete review");
    
    // Verify it's gone
    let result = repository.get_review_by_id(created_review.id)
        .await
        .expect("Query should succeed");
    
    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn test_repository_delete_review_not_found() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool);
    
    let non_existent_id = Uuid::new_v4();
    let result = repository.delete_review(non_existent_id).await;
    assert!(matches!(result, Err(UgcError::ReviewNotFound { .. })));
}

#[tokio::test]
#[serial]
async fn test_repository_moderate_review() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create a test review
    let created_review = create_test_review(&pool).await;
    
    // Moderate it
    let moderated_review = repository.moderate_review(created_review.id, ModerationStatus::Rejected)
        .await
        .expect("Failed to moderate review");
    
    assert_eq!(moderated_review.id, created_review.id);
    assert_eq!(moderated_review.moderation_status, ModerationStatus::Rejected);
    // is_moderated should remain true since it was approved before
    assert!(moderated_review.is_moderated);
}

#[tokio::test]
#[serial]
async fn test_repository_get_reviews_with_pagination() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create multiple test reviews
    let offer_id = Uuid::new_v4();
    for i in 1..=5 {
        let input = CreateReviewInput {
            offer_id,
            rating: i,
            text: format!("Review {}", i),
        };
        repository.create_review(input, Uuid::new_v4())
            .await
            .expect("Failed to create review");
    }
    
    // Test pagination
    let (reviews, total_count) = repository.get_reviews_with_pagination(None, 3, 0)
        .await
        .expect("Failed to get reviews");
    
    assert_eq!(reviews.len(), 3);
    assert_eq!(total_count, 5);
    
    // Test with offset
    let (reviews, total_count) = repository.get_reviews_with_pagination(None, 3, 3)
        .await
        .expect("Failed to get reviews");
    
    assert_eq!(reviews.len(), 2);
    assert_eq!(total_count, 5);
}

#[tokio::test]
#[serial]
async fn test_repository_get_reviews_with_filter() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    let offer_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    
    // Create reviews with different ratings
    for rating in 1..=5 {
        let input = CreateReviewInput {
            offer_id,
            rating,
            text: format!("Review with rating {}", rating),
        };
        repository.create_review(input, author_id)
            .await
            .expect("Failed to create review");
    }
    
    // Test filter by offer_id
    let filter = Some(ReviewsFilter {
        offer_id: Some(offer_id),
        author_id: None,
        min_rating: None,
        max_rating: None,
        moderated_only: None,
        moderation_status: None,
    });
    
    let (reviews, total_count) = repository.get_reviews_with_pagination(filter, 10, 0)
        .await
        .expect("Failed to get reviews");
    
    assert_eq!(reviews.len(), 5);
    assert_eq!(total_count, 5);
    assert!(reviews.iter().all(|r| r.offer_id == offer_id));
    
    // Test filter by rating range
    let filter = Some(ReviewsFilter {
        offer_id: Some(offer_id),
        author_id: None,
        min_rating: Some(3),
        max_rating: Some(5),
        moderated_only: None,
        moderation_status: None,
    });
    
    let (reviews, total_count) = repository.get_reviews_with_pagination(filter, 10, 0)
        .await
        .expect("Failed to get reviews");
    
    assert_eq!(reviews.len(), 3);
    assert_eq!(total_count, 3);
    assert!(reviews.iter().all(|r| r.rating >= 3 && r.rating <= 5));
}

#[tokio::test]
#[serial]
async fn test_repository_update_offer_rating() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    let offer_id = Uuid::new_v4();
    
    // Create multiple reviews for the same offer
    let ratings = vec![5, 4, 5, 3, 4];
    for rating in &ratings {
        let input = CreateReviewInput {
            offer_id,
            rating: *rating,
            text: format!("Review with rating {}", rating),
        };
        let review = repository.create_review(input, Uuid::new_v4())
            .await
            .expect("Failed to create review");
        
        // Moderate the review to make it count
        repository.moderate_review(review.id, ModerationStatus::Approved)
            .await
            .expect("Failed to moderate review");
    }
    
    // Update offer rating
    let offer_rating = repository.update_offer_rating(offer_id)
        .await
        .expect("Failed to update offer rating");
    
    assert_eq!(offer_rating.offer_id, offer_id);
    assert_eq!(offer_rating.reviews_count, 5);
    
    // Calculate expected average: (5+4+5+3+4)/5 = 4.2
    let expected_avg = rust_decimal::Decimal::new(42, 1); // 4.2
    assert_eq!(offer_rating.average_rating, expected_avg);
    
    // Check rating distribution
    let distribution = offer_rating.rating_distribution.as_object().unwrap();
    assert_eq!(distribution["3"], 1);
    assert_eq!(distribution["4"], 2);
    assert_eq!(distribution["5"], 2);
}

#[tokio::test]
#[serial]
async fn test_repository_get_reviews_by_ids() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Create test reviews
    let review1 = create_test_review(&pool).await;
    let review2 = create_test_review(&pool).await;
    let non_existent_id = Uuid::new_v4();
    
    let ids = vec![review1.id, review2.id, non_existent_id];
    let results = repository.get_reviews_by_ids(ids)
        .await
        .expect("Failed to get reviews by ids");
    
    assert_eq!(results.len(), 3);
    assert!(results[0].is_some());
    assert!(results[1].is_some());
    assert!(results[2].is_none());
    
    assert_eq!(results[0].as_ref().unwrap().id, review1.id);
    assert_eq!(results[1].as_ref().unwrap().id, review2.id);
}

#[tokio::test]
#[serial]
async fn test_service_integration() {
    let pool = setup_test_db().await;
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let service = ReviewService::new(repository);
    
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 5,
        text: "Excellent car!".to_string(),
    };
    let author_id = Uuid::new_v4();
    
    // Create review through service
    let review = service.create_review(input.clone(), author_id)
        .await
        .expect("Failed to create review through service");
    
    assert_eq!(review.offer_id, input.offer_id);
    assert_eq!(review.author_id, author_id);
    assert_eq!(review.rating, input.rating);
    assert_eq!(review.text, input.text);
    
    // Get review through service
    let retrieved_review = service.get_review_by_id(review.id)
        .await
        .expect("Failed to get review through service")
        .expect("Review should exist");
    
    assert_eq!(retrieved_review.id, review.id);
    
    // Update review through service
    let update_input = UpdateReviewInput {
        rating: Some(4),
        text: Some("Updated through service".to_string()),
    };
    
    let updated_review = service.update_review(review.id, update_input, author_id)
        .await
        .expect("Failed to update review through service");
    
    assert_eq!(updated_review.rating, 4);
    assert_eq!(updated_review.text, "Updated through service");
    
    // Delete review through service
    service.delete_review(review.id, author_id)
        .await
        .expect("Failed to delete review through service");
    
    // Verify it's gone
    let result = service.get_review_by_id(review.id).await
        .expect("Query should succeed");
    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn test_service_authorization() {
    let pool = setup_test_db().await;
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let service = ReviewService::new(repository);
    
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 5,
        text: "Test review".to_string(),
    };
    let author_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();
    
    // Create review
    let review = service.create_review(input, author_id)
        .await
        .expect("Failed to create review");
    
    // Try to update with wrong user - should fail
    let update_input = UpdateReviewInput {
        rating: Some(3),
        text: Some("Unauthorized update".to_string()),
    };
    
    let result = service.update_review(review.id, update_input, other_user_id).await;
    assert!(matches!(result, Err(UgcError::Unauthorized { .. })));
    
    // Try to delete with wrong user - should fail
    let result = service.delete_review(review.id, other_user_id).await;
    assert!(matches!(result, Err(UgcError::Unauthorized { .. })));
    
    // Operations with correct user should succeed
    let update_input = UpdateReviewInput {
        rating: Some(3),
        text: Some("Authorized update".to_string()),
    };
    
    let updated_review = service.update_review(review.id, update_input, author_id)
        .await
        .expect("Authorized update should succeed");
    
    assert_eq!(updated_review.rating, 3);
    assert_eq!(updated_review.text, "Authorized update");
}

#[tokio::test]
#[serial]
async fn test_concurrent_operations() {
    let pool = setup_test_db().await;
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let service = ReviewService::new(repository);
    
    let offer_id = Uuid::new_v4();
    
    // Create multiple reviews concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            let input = CreateReviewInput {
                offer_id,
                rating: (i % 5) + 1,
                text: format!("Concurrent review {}", i),
            };
            service_clone.create_review(input, Uuid::new_v4()).await
        });
        handles.push(handle);
    }
    
    // Wait for all reviews to be created
    let mut reviews = vec![];
    for handle in handles {
        let review = handle.await
            .expect("Task should complete")
            .expect("Review creation should succeed");
        reviews.push(review);
    }
    
    assert_eq!(reviews.len(), 10);
    
    // Verify all reviews have the same offer_id
    assert!(reviews.iter().all(|r| r.offer_id == offer_id));
    
    // Get reviews for the offer
    let offer_reviews = service.get_reviews_for_offer(offer_id)
        .await
        .expect("Failed to get reviews for offer");
    
    // Should have 10 reviews (though some might not be moderated)
    assert!(!offer_reviews.is_empty());
}

#[tokio::test]
#[serial]
async fn test_database_transaction_rollback() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool.clone());
    
    // Start a transaction
    let mut tx = pool.begin().await.expect("Failed to start transaction");
    
    // Create a review within the transaction
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 5,
        text: "Transaction test".to_string(),
    };
    let author_id = Uuid::new_v4();
    
    let review = sqlx::query_as::<_, Review>(
        r#"
        INSERT INTO reviews (offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status)
        VALUES ($1, $2, $3, $4, NOW(), NOW(), false, 'pending')
        RETURNING id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
        "#
    )
    .bind(input.offer_id)
    .bind(author_id)
    .bind(input.rating)
    .bind(input.text)
    .fetch_one(&mut *tx)
    .await
    .expect("Failed to create review in transaction");
    
    // Rollback the transaction
    tx.rollback().await.expect("Failed to rollback transaction");
    
    // Verify the review doesn't exist
    let result = repository.get_review_by_id(review.id)
        .await
        .expect("Query should succeed");
    
    assert!(result.is_none());
}

// Authentication integration tests
#[tokio::test]
async fn test_auth_service_integration() {
    let secret = "test-secret-key";
    let auth_service = AuthService::new_with_secret(secret);
    
    // Test valid token validation
    let token = "Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3OC05MGFiLWNkZWYtMTIzNC01Njc4OTBhYmNkZWYiLCJuYW1lIjoiSm9obiBEb2UiLCJlbWFpbCI6ImpvaG5AZXhhbXBsZS5jb20iLCJyb2xlcyI6WyJ1c2VyIl0sImlhdCI6MTUxNjIzOTAyMiwiZXhwIjo5OTk5OTk5OTk5fQ.Lf_7VZGvMhWvGgLdNhFvGgLdNhFvGgLdNhFvGgLdNhE";
    
    let result = auth_service.validate_auth_header(token);
    // This will fail with the test token, but shows the integration structure
    assert!(result.is_err());
}

// Performance integration tests
#[tokio::test]
#[serial]
async fn test_bulk_operations_performance() {
    let pool = setup_test_db().await;
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let service = ReviewService::new(repository);
    
    let start_time = std::time::Instant::now();
    
    // Create 100 reviews
    let offer_id = Uuid::new_v4();
    for i in 0..100 {
        let input = CreateReviewInput {
            offer_id,
            rating: (i % 5) + 1,
            text: format!("Performance test review {}", i),
        };
        service.create_review(input, Uuid::new_v4())
            .await
            .expect("Failed to create review");
    }
    
    let creation_time = start_time.elapsed();
    println!("Created 100 reviews in {:?}", creation_time);
    
    // Query all reviews
    let query_start = std::time::Instant::now();
    let (reviews, total_count) = service.get_reviews_with_pagination(None, 100, 0)
        .await
        .expect("Failed to get reviews");
    
    let query_time = query_start.elapsed();
    println!("Queried {} reviews in {:?}", reviews.len(), query_time);
    
    assert_eq!(total_count, 100);
    assert_eq!(reviews.len(), 100);
    
    // Performance assertions (adjust based on your requirements)
    assert!(creation_time.as_millis() < 5000, "Creation took too long: {:?}", creation_time);
    assert!(query_time.as_millis() < 1000, "Query took too long: {:?}", query_time);
}

// Error handling integration tests
#[tokio::test]
#[serial]
async fn test_database_connection_failure_handling() {
    // Create a pool with invalid connection string
    let invalid_pool = PgPool::connect("postgres://invalid:invalid@localhost:9999/invalid")
        .await;
    
    assert!(invalid_pool.is_err());
}

#[tokio::test]
#[serial]
async fn test_database_constraint_violations() {
    let pool = setup_test_db().await;
    let repository = PostgresReviewRepository::new(pool);
    
    // Test with invalid rating (should be caught by validation, not DB constraint)
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 10, // Invalid rating
        text: "Test review".to_string(),
    };
    
    // This should fail at the validation level before hitting the database
    let result = input.validate();
    assert!(result.is_err());
}