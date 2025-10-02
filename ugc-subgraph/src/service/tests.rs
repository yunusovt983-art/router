use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use mockall::predicate::*;
use tokio_test;

use crate::error::{Result, UgcError};
use crate::models::{
    offer_rating::OfferRating,
    review::{CreateReviewInput, ModerationStatus, Review, ReviewsFilter, UpdateReviewInput},
};
use crate::repository::{MockReviewRepository, ReviewRepository};
use crate::service::ReviewService;

// Helper function to create a test review
fn create_test_review() -> Review {
    Review {
        id: Uuid::new_v4(),
        offer_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        rating: 5,
        text: "Great car!".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        is_moderated: true,
        moderation_status: ModerationStatus::Approved,
    }
}

// Helper function to create a test offer rating
fn create_test_offer_rating(offer_id: Uuid) -> OfferRating {
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
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_create_review_success() {
    let mut mock_repo = MockReviewRepository::new();
    let test_review = create_test_review();
    let author_id = test_review.author_id;
    let offer_id = test_review.offer_id;
    
    let input = CreateReviewInput {
        offer_id,
        rating: 5,
        text: "Great car!".to_string(),
    };

    // Set up expectations
    mock_repo
        .expect_create_review()
        .with(eq(input.clone()), eq(author_id))
        .times(1)
        .returning(move |_, _| Ok(test_review.clone()));

    mock_repo
        .expect_update_offer_rating()
        .with(eq(offer_id))
        .times(1)
        .returning(|_| Ok(create_test_offer_rating(offer_id)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.create_review(input, author_id).await;

    assert!(result.is_ok());
    let review = result.unwrap();
    assert_eq!(review.rating, 5);
    assert_eq!(review.text, "Great car!");
    assert_eq!(review.author_id, author_id);
}

#[tokio::test]
async fn test_create_review_validation_error() {
    let mock_repo = MockReviewRepository::new();
    let service = ReviewService::new(Arc::new(mock_repo));
    
    let invalid_input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 6, // Invalid rating
        text: "Great car!".to_string(),
    };

    let result = service.create_review(invalid_input, Uuid::new_v4()).await;
    
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UgcError::ValidationError { .. }));
}

#[tokio::test]
async fn test_get_review_by_id_found() {
    let mut mock_repo = MockReviewRepository::new();
    let test_review = create_test_review();
    let review_id = test_review.id;

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(move |_| Ok(Some(test_review.clone())));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_review_by_id(review_id).await;

    assert!(result.is_ok());
    let review = result.unwrap();
    assert!(review.is_some());
    assert_eq!(review.unwrap().id, review_id);
}

#[tokio::test]
async fn test_get_review_by_id_not_found() {
    let mut mock_repo = MockReviewRepository::new();
    let review_id = Uuid::new_v4();

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(|_| Ok(None));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_review_by_id(review_id).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_update_review_success() {
    let mut mock_repo = MockReviewRepository::new();
    let mut test_review = create_test_review();
    let review_id = test_review.id;
    let user_id = test_review.author_id;
    
    let input = UpdateReviewInput {
        rating: Some(4),
        text: Some("Updated review".to_string()),
    };

    // Update the test review with new values
    test_review.rating = 4;
    test_review.text = "Updated review".to_string();

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(move |_| Ok(Some(test_review.clone())));

    mock_repo
        .expect_update_review()
        .with(eq(review_id), eq(input.clone()))
        .times(1)
        .returning(move |_, _| Ok(test_review.clone()));

    mock_repo
        .expect_update_offer_rating()
        .with(eq(test_review.offer_id))
        .times(1)
        .returning(move |offer_id| Ok(create_test_offer_rating(offer_id)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.update_review(review_id, input, user_id).await;

    assert!(result.is_ok());
    let review = result.unwrap();
    assert_eq!(review.rating, 4);
    assert_eq!(review.text, "Updated review");
}

#[tokio::test]
async fn test_update_review_unauthorized() {
    let mut mock_repo = MockReviewRepository::new();
    let test_review = create_test_review();
    let review_id = test_review.id;
    let wrong_user_id = Uuid::new_v4(); // Different from author_id
    
    let input = UpdateReviewInput {
        rating: Some(4),
        text: Some("Updated review".to_string()),
    };

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(move |_| Ok(Some(test_review.clone())));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.update_review(review_id, input, wrong_user_id).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UgcError::Unauthorized { .. }));
}

#[tokio::test]
async fn test_update_review_not_found() {
    let mut mock_repo = MockReviewRepository::new();
    let review_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let input = UpdateReviewInput {
        rating: Some(4),
        text: Some("Updated review".to_string()),
    };

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(|_| Ok(None));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.update_review(review_id, input, user_id).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UgcError::ReviewNotFound { .. }));
}

#[tokio::test]
async fn test_delete_review_success() {
    let mut mock_repo = MockReviewRepository::new();
    let test_review = create_test_review();
    let review_id = test_review.id;
    let user_id = test_review.author_id;
    let offer_id = test_review.offer_id;

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(move |_| Ok(Some(test_review.clone())));

    mock_repo
        .expect_delete_review()
        .with(eq(review_id))
        .times(1)
        .returning(|_| Ok(()));

    mock_repo
        .expect_update_offer_rating()
        .with(eq(offer_id))
        .times(1)
        .returning(move |offer_id| Ok(create_test_offer_rating(offer_id)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.delete_review(review_id, user_id).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_review_unauthorized() {
    let mut mock_repo = MockReviewRepository::new();
    let test_review = create_test_review();
    let review_id = test_review.id;
    let wrong_user_id = Uuid::new_v4(); // Different from author_id

    mock_repo
        .expect_get_review_by_id()
        .with(eq(review_id))
        .times(1)
        .returning(move |_| Ok(Some(test_review.clone())));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.delete_review(review_id, wrong_user_id).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UgcError::Unauthorized { .. }));
}

#[tokio::test]
async fn test_moderate_review_success() {
    let mut mock_repo = MockReviewRepository::new();
    let mut test_review = create_test_review();
    let review_id = test_review.id;
    let offer_id = test_review.offer_id;
    
    test_review.moderation_status = ModerationStatus::Approved;
    test_review.is_moderated = true;

    mock_repo
        .expect_moderate_review()
        .with(eq(review_id), eq(ModerationStatus::Approved))
        .times(1)
        .returning(move |_, _| Ok(test_review.clone()));

    mock_repo
        .expect_update_offer_rating()
        .with(eq(offer_id))
        .times(1)
        .returning(move |offer_id| Ok(create_test_offer_rating(offer_id)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.moderate_review(review_id, ModerationStatus::Approved).await;

    assert!(result.is_ok());
    let review = result.unwrap();
    assert_eq!(review.moderation_status, ModerationStatus::Approved);
    assert!(review.is_moderated);
}

#[tokio::test]
async fn test_get_offer_rating_success() {
    let mut mock_repo = MockReviewRepository::new();
    let offer_id = Uuid::new_v4();
    let test_rating = create_test_offer_rating(offer_id);

    mock_repo
        .expect_get_offer_rating()
        .with(eq(offer_id))
        .times(1)
        .returning(move |_| Ok(Some(test_rating.clone())));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_offer_rating(offer_id).await;

    assert!(result.is_ok());
    let rating = result.unwrap();
    assert!(rating.is_some());
    let rating = rating.unwrap();
    assert_eq!(rating.offer_id, offer_id);
    assert_eq!(rating.reviews_count, 10);
}

#[tokio::test]
async fn test_get_reviews_with_pagination() {
    let mut mock_repo = MockReviewRepository::new();
    let test_reviews = vec![create_test_review(), create_test_review()];
    let total_count = 2;

    mock_repo
        .expect_get_reviews_with_pagination()
        .with(eq(None), eq(10), eq(0))
        .times(1)
        .returning(move |_, _, _| Ok((test_reviews.clone(), total_count)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_with_pagination(None, 10, 0).await;

    assert!(result.is_ok());
    let (reviews, count) = result.unwrap();
    assert_eq!(reviews.len(), 2);
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_get_reviews_with_filter() {
    let mut mock_repo = MockReviewRepository::new();
    let offer_id = Uuid::new_v4();
    let test_reviews = vec![create_test_review()];
    let total_count = 1;

    let filter = Some(ReviewsFilter {
        offer_id: Some(offer_id),
        author_id: None,
        min_rating: None,
        max_rating: None,
        moderated_only: Some(true),
        moderation_status: None,
    });

    mock_repo
        .expect_get_reviews_with_pagination()
        .with(eq(filter.clone()), eq(10), eq(0))
        .times(1)
        .returning(move |_, _, _| Ok((test_reviews.clone(), total_count)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_with_pagination(filter, 10, 0).await;

    assert!(result.is_ok());
    let (reviews, count) = result.unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_pagination_limits() {
    let mut mock_repo = MockReviewRepository::new();
    let test_reviews = vec![];
    let total_count = 0;

    // Test that limit is capped at 100
    mock_repo
        .expect_get_reviews_with_pagination()
        .with(eq(None), eq(100), eq(0))
        .times(1)
        .returning(move |_, _, _| Ok((test_reviews.clone(), total_count)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_with_pagination(None, 200, 0).await; // Request 200, should be capped at 100

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_reviews_for_offer() {
    let mut mock_repo = MockReviewRepository::new();
    let offer_id = Uuid::new_v4();
    let test_reviews = vec![create_test_review()];
    let total_count = 1;

    let expected_filter = Some(ReviewsFilter {
        offer_id: Some(offer_id),
        author_id: None,
        min_rating: None,
        max_rating: None,
        moderated_only: Some(true),
        moderation_status: None,
    });

    mock_repo
        .expect_get_reviews_with_pagination()
        .with(eq(expected_filter), eq(100), eq(0))
        .times(1)
        .returning(move |_, _, _| Ok((test_reviews.clone(), total_count)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_for_offer(offer_id).await;

    assert!(result.is_ok());
    let reviews = result.unwrap();
    assert_eq!(reviews.len(), 1);
}

#[tokio::test]
async fn test_get_reviews_for_author() {
    let mut mock_repo = MockReviewRepository::new();
    let author_id = Uuid::new_v4();
    let test_reviews = vec![create_test_review()];
    let total_count = 1;

    let expected_filter = Some(ReviewsFilter {
        offer_id: None,
        author_id: Some(author_id),
        min_rating: None,
        max_rating: None,
        moderated_only: Some(true),
        moderation_status: None,
    });

    mock_repo
        .expect_get_reviews_with_pagination()
        .with(eq(expected_filter), eq(100), eq(0))
        .times(1)
        .returning(move |_, _, _| Ok((test_reviews.clone(), total_count)));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_for_author(author_id).await;

    assert!(result.is_ok());
    let reviews = result.unwrap();
    assert_eq!(reviews.len(), 1);
}

#[tokio::test]
async fn test_get_reviews_by_ids() {
    let mut mock_repo = MockReviewRepository::new();
    let review1 = create_test_review();
    let review2 = create_test_review();
    let ids = vec![review1.id, review2.id];
    let expected_results = vec![Some(review1.clone()), Some(review2.clone())];

    mock_repo
        .expect_get_reviews_by_ids()
        .with(eq(ids.clone()))
        .times(1)
        .returning(move |_| Ok(expected_results.clone()));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_reviews_by_ids(ids).await;

    assert!(result.is_ok());
    let reviews = result.unwrap();
    assert_eq!(reviews.len(), 2);
    assert!(reviews[0].is_some());
    assert!(reviews[1].is_some());
}

#[tokio::test]
async fn test_get_offer_ratings_by_ids() {
    let mut mock_repo = MockReviewRepository::new();
    let offer_id1 = Uuid::new_v4();
    let offer_id2 = Uuid::new_v4();
    let rating1 = create_test_offer_rating(offer_id1);
    let rating2 = create_test_offer_rating(offer_id2);
    let ids = vec![offer_id1, offer_id2];
    let expected_results = vec![Some(rating1.clone()), Some(rating2.clone())];

    mock_repo
        .expect_get_offer_ratings_by_ids()
        .with(eq(ids.clone()))
        .times(1)
        .returning(move |_| Ok(expected_results.clone()));

    let service = ReviewService::new(Arc::new(mock_repo));
    let result = service.get_offer_ratings_by_ids(ids).await;

    assert!(result.is_ok());
    let ratings = result.unwrap();
    assert_eq!(ratings.len(), 2);
    assert!(ratings[0].is_some());
    assert!(ratings[1].is_some());
}

// Property-based tests using proptest
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_rating_validation_property(rating in 1i32..=5) {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating,
                text: "Test review".to_string(),
            };
            
            prop_assert!(input.validate().is_ok());
        }

        #[test]
        fn test_invalid_rating_validation_property(rating in i32::MIN..0i32) {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating,
                text: "Test review".to_string(),
            };
            
            prop_assert!(input.validate().is_err());
        }

        #[test]
        fn test_text_length_validation_property(text_len in 1usize..=5000) {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "a".repeat(text_len),
            };
            
            prop_assert!(input.validate().is_ok());
        }

        #[test]
        fn test_text_too_long_validation_property(text_len in 5001usize..10000) {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "a".repeat(text_len),
            };
            
            prop_assert!(input.validate().is_err());
        }
    }
}

// Benchmark tests using criterion
#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{black_box, Criterion};

    pub fn benchmark_create_review_validation(c: &mut Criterion) {
        c.bench_function("create_review_validation", |b| {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "This is a test review with some reasonable length".to_string(),
            };
            
            b.iter(|| {
                black_box(input.validate())
            })
        });
    }

    pub fn benchmark_moderation_status_parsing(c: &mut Criterion) {
        c.bench_function("moderation_status_parsing", |b| {
            b.iter(|| {
                black_box("approved".parse::<ModerationStatus>())
            })
        });
    }
}