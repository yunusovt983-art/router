use std::sync::Arc;
use uuid::Uuid;
use tracing::{info, warn, error, instrument, Span};

use crate::error::{Result, UgcError};
use crate::models::{
    offer_rating::OfferRating,
    review::{CreateReviewInput, ModerationStatus, Review, ReviewsFilter, UpdateReviewInput},
};
use crate::repository::{PostgresReviewRepository, ReviewRepository};
use crate::telemetry::{
    logging::BusinessEventLogger,
    tracing::add_span_attributes,
};

pub mod cache;
pub mod circuit_breaker;
pub mod external;
pub mod redis_cache;
pub mod dataloader;
pub mod query_limits;

#[cfg(test)]
mod tests;

pub use cache::{FallbackDataProvider, ServiceHealthMonitor, Cache, InMemoryCache};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, RetryConfig, RetryMechanism};
pub use external::{ExternalService, ExternalServiceClient, ServiceStatus};
pub use redis_cache::{CacheService, RedisCache, RedisCacheConfig, CacheStats};
pub use dataloader::{DataLoaderService, ReviewDataLoader, OfferRatingDataLoader, ReviewsByOfferDataLoader, ReviewsByAuthorDataLoader};
pub use query_limits::{QueryLimitsConfig, QueryLimitsExtensionFactory, QueryRateLimiter, UserLimits, start_rate_limit_cleanup_task};

pub struct ReviewService {
    repository: Arc<dyn ReviewRepository>,
    cache: Option<Arc<CacheService>>,
    dataloader: Option<Arc<DataLoaderService>>,
}

impl ReviewService {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        Self { 
            repository,
            cache: None,
            dataloader: None,
        }
    }

    pub fn with_cache(repository: Arc<dyn ReviewRepository>, cache: Arc<CacheService>) -> Self {
        Self { 
            repository,
            cache: Some(cache),
            dataloader: None,
        }
    }

    pub fn with_dataloader(repository: Arc<dyn ReviewRepository>, dataloader: Arc<DataLoaderService>) -> Self {
        Self { 
            repository,
            cache: None,
            dataloader: Some(dataloader),
        }
    }

    pub fn with_cache_and_dataloader(
        repository: Arc<dyn ReviewRepository>, 
        cache: Arc<CacheService>,
        dataloader: Arc<DataLoaderService>
    ) -> Self {
        Self { 
            repository,
            cache: Some(cache),
            dataloader: Some(dataloader),
        }
    }

    #[instrument(skip(self), fields(offer_id = %input.offer_id, author_id = %author_id, rating = input.rating))]
    pub async fn create_review(&self, input: CreateReviewInput, author_id: Uuid) -> Result<Review> {
        info!("Creating review for offer {}", input.offer_id);
        
        // Add span attributes
        add_span_attributes(vec![
            ("operation", "create_review".to_string()),
            ("offer_id", input.offer_id.to_string()),
            ("author_id", author_id.to_string()),
            ("rating", input.rating.to_string()),
        ]);

        // Validate input
        input.validate().map_err(|msg| {
            warn!("Review validation failed: {}", msg);
            UgcError::ValidationError { message: msg }
        })?;

        // Create review
        let review = self.repository.create_review(input, author_id).await
            .map_err(|e| {
                error!("Failed to create review: {}", e);
                e
            })?;

        // Log business event
        BusinessEventLogger::review_created(
            review.id,
            review.offer_id,
            review.author_id,
            review.rating,
        );

        // Update offer rating statistics
        if let Err(e) = self.repository.update_offer_rating(review.offer_id).await {
            warn!("Failed to update offer rating: {}", e);
        }

        // Cache the new review and invalidate related caches
        if let Some(cache) = &self.cache {
            if let Err(e) = cache.cache_review(&review).await {
                warn!("Failed to cache new review {}: {}", review.id, e);
            }
            if let Err(e) = cache.invalidate_offer_cache(review.offer_id).await {
                warn!("Failed to invalidate offer cache: {}", e);
            }
        }

        // Invalidate DataLoader caches
        if let Some(dataloader) = &self.dataloader {
            dataloader.invalidate_review(&review).await;
        }

        info!("Review created successfully: {}", review.id);
        Ok(review)
    }

    #[instrument(skip(self), fields(review_id = %id))]
    pub async fn get_review_by_id(&self, id: Uuid) -> Result<Option<Review>> {
        add_span_attributes(vec![
            ("operation", "get_review_by_id".to_string()),
            ("review_id", id.to_string()),
        ]);

        // Try cache first if available
        if let Some(cache) = &self.cache {
            if let Ok(Some(cached_review)) = cache.get_review(id).await {
                add_span_attributes(vec![("cache_hit", "true".to_string())]);
                return Ok(Some(cached_review));
            }
            add_span_attributes(vec![("cache_hit", "false".to_string())]);
        }

        // Try DataLoader if available (batches requests automatically)
        if let Some(dataloader) = &self.dataloader {
            if let Ok(review) = dataloader.reviews.load_review(id).await {
                add_span_attributes(vec![("dataloader_used", "true".to_string())]);
                
                // Cache the result if found and cache is available
                if let (Some(cache), Some(ref review)) = (&self.cache, &review) {
                    if let Err(e) = cache.cache_review(review).await {
                        warn!("Failed to cache review {}: {}", id, e);
                    }
                }
                
                return Ok(review);
            }
        }

        // Fallback to database
        let review = self.repository.get_review_by_id(id).await
            .map_err(|e| {
                error!("Failed to get review by id {}: {}", id, e);
                e
            })?;

        // Cache the result if found
        if let (Some(cache), Some(ref review)) = (&self.cache, &review) {
            if let Err(e) = cache.cache_review(review).await {
                warn!("Failed to cache review {}: {}", id, e);
            }
        }

        Ok(review)
    }

    #[instrument(skip(self), fields(review_id = %id, user_id = %user_id))]
    pub async fn update_review(&self, id: Uuid, input: UpdateReviewInput, user_id: Uuid) -> Result<Review> {
        info!("Updating review {}", id);
        
        add_span_attributes(vec![
            ("operation", "update_review".to_string()),
            ("review_id", id.to_string()),
            ("user_id", user_id.to_string()),
        ]);

        // Validate input
        input.validate().map_err(|msg| {
            warn!("Review update validation failed: {}", msg);
            UgcError::ValidationError { message: msg }
        })?;

        // Check if review exists and user has permission
        let existing_review = self.repository.get_review_by_id(id).await?
            .ok_or(UgcError::ReviewNotFound { id })?;

        if existing_review.author_id != user_id {
            warn!("Unauthorized review update attempt by user {}", user_id);
            return Err(UgcError::Unauthorized {
                user_id,
                review_id: id,
            });
        }

        let old_rating = existing_review.rating;

        // Update review
        let review = self.repository.update_review(id, input).await
            .map_err(|e| {
                error!("Failed to update review {}: {}", id, e);
                e
            })?;

        // Log business event
        BusinessEventLogger::review_updated(
            review.id,
            review.author_id,
            old_rating,
            review.rating,
        );

        // Update offer rating statistics
        if let Err(e) = self.repository.update_offer_rating(review.offer_id).await {
            warn!("Failed to update offer rating: {}", e);
        }

        // Invalidate caches
        if let Some(cache) = &self.cache {
            if let Err(e) = cache.invalidate_review_cache(&review).await {
                warn!("Failed to invalidate review cache: {}", e);
            }
        }

        // Invalidate DataLoader caches
        if let Some(dataloader) = &self.dataloader {
            dataloader.invalidate_review(&review).await;
        }

        info!("Review updated successfully: {}", review.id);
        Ok(review)
    }

    #[instrument(skip(self), fields(review_id = %id, user_id = %user_id))]
    pub async fn delete_review(&self, id: Uuid, user_id: Uuid) -> Result<()> {
        info!("Deleting review {}", id);
        
        add_span_attributes(vec![
            ("operation", "delete_review".to_string()),
            ("review_id", id.to_string()),
            ("user_id", user_id.to_string()),
        ]);

        // Check if review exists and user has permission
        let existing_review = self.repository.get_review_by_id(id).await?
            .ok_or(UgcError::ReviewNotFound { id })?;

        if existing_review.author_id != user_id {
            warn!("Unauthorized review deletion attempt by user {}", user_id);
            return Err(UgcError::Unauthorized {
                user_id,
                review_id: id,
            });
        }

        let offer_id = existing_review.offer_id;

        // Delete review
        self.repository.delete_review(id).await
            .map_err(|e| {
                error!("Failed to delete review {}: {}", id, e);
                e
            })?;

        // Log business event
        BusinessEventLogger::review_deleted(id, user_id);

        // Update offer rating statistics
        if let Err(e) = self.repository.update_offer_rating(offer_id).await {
            warn!("Failed to update offer rating: {}", e);
        }

        info!("Review deleted successfully: {}", id);
        Ok(())
    }

    pub async fn get_reviews_with_pagination(
        &self,
        filter: Option<ReviewsFilter>,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<Review>, i32)> {
        let limit = limit.min(100).max(1); // Limit between 1 and 100
        self.repository.get_reviews_with_pagination(filter, limit, offset).await
    }

    pub async fn get_reviews_after_cursor(
        &self,
        filter: Option<ReviewsFilter>,
        cursor_timestamp: i64,
        cursor_id: Uuid,
        limit: i32,
    ) -> Result<Vec<Review>> {
        let limit = limit.min(100).max(1); // Limit between 1 and 100
        self.repository.get_reviews_after_cursor(filter, cursor_timestamp, cursor_id, limit).await
    }

    #[instrument(skip(self), fields(review_id = %id, status = ?status))]
    pub async fn moderate_review(&self, id: Uuid, status: ModerationStatus) -> Result<Review> {
        info!("Moderating review {} with status {:?}", id, status);
        
        add_span_attributes(vec![
            ("operation", "moderate_review".to_string()),
            ("review_id", id.to_string()),
            ("moderation_status", format!("{:?}", status)),
        ]);

        let review = self.repository.moderate_review(id, status).await
            .map_err(|e| {
                error!("Failed to moderate review {}: {}", id, e);
                e
            })?;

        // Log business event (moderator_id would come from context in real implementation)
        BusinessEventLogger::review_moderated(
            id,
            Uuid::new_v4(), // TODO: Get actual moderator ID from context
            &format!("{:?}", status),
        );

        // Update offer rating statistics if approved/rejected
        if matches!(status, ModerationStatus::Approved | ModerationStatus::Rejected) {
            if let Err(e) = self.repository.update_offer_rating(review.offer_id).await {
                warn!("Failed to update offer rating: {}", e);
            }
        }

        info!("Review moderated successfully: {}", id);
        Ok(review)
    }

    pub async fn get_offer_rating(&self, offer_id: Uuid) -> Result<Option<OfferRating>> {
        // Try cache first if available
        if let Some(cache) = &self.cache {
            if let Ok(Some(cached_rating)) = cache.get_offer_rating(offer_id).await {
                return Ok(Some(cached_rating));
            }
        }

        // Try DataLoader if available (batches requests automatically)
        if let Some(dataloader) = &self.dataloader {
            if let Ok(rating) = dataloader.offer_ratings.load_offer_rating(offer_id).await {
                // Cache the result if found and cache is available
                if let (Some(cache), Some(ref rating)) = (&self.cache, &rating) {
                    if let Err(e) = cache.cache_offer_rating(rating).await {
                        warn!("Failed to cache offer rating for {}: {}", offer_id, e);
                    }
                }
                
                return Ok(rating);
            }
        }

        // Fallback to database
        let rating = self.repository.get_offer_rating(offer_id).await?;

        // Cache the result if found
        if let (Some(cache), Some(ref rating)) = (&self.cache, &rating) {
            if let Err(e) = cache.cache_offer_rating(rating).await {
                warn!("Failed to cache offer rating for {}: {}", offer_id, e);
            }
        }

        Ok(rating)
    }

    pub async fn update_offer_rating(&self, offer_id: Uuid) -> Result<OfferRating> {
        self.repository.update_offer_rating(offer_id).await
    }

    /// Get reviews for an offer using DataLoader for batching
    pub async fn get_reviews_for_offer(&self, offer_id: Uuid) -> Result<Vec<Review>> {
        if let Some(dataloader) = &self.dataloader {
            if let Ok(Some(reviews)) = dataloader.reviews_by_offer.load_reviews_for_offer(offer_id).await {
                return Ok(reviews);
            }
        }

        // Fallback to database with filter
        let filter = Some(crate::models::review::ReviewsFilter {
            offer_id: Some(offer_id),
            author_id: None,
            min_rating: None,
            max_rating: None,
            moderated_only: Some(true),
            moderation_status: None,
        });

        let (reviews, _) = self.repository.get_reviews_with_pagination(filter, 100, 0).await?;
        Ok(reviews)
    }

    /// Get reviews for an author using DataLoader for batching
    pub async fn get_reviews_for_author(&self, author_id: Uuid) -> Result<Vec<Review>> {
        if let Some(dataloader) = &self.dataloader {
            if let Ok(Some(reviews)) = dataloader.reviews_by_author.load_reviews_for_author(author_id).await {
                return Ok(reviews);
            }
        }

        // Fallback to database with filter
        let filter = Some(crate::models::review::ReviewsFilter {
            offer_id: None,
            author_id: Some(author_id),
            min_rating: None,
            max_rating: None,
            moderated_only: Some(true),
            moderation_status: None,
        });

        let (reviews, _) = self.repository.get_reviews_with_pagination(filter, 100, 0).await?;
        Ok(reviews)
    }

    /// Batch load multiple reviews by IDs (useful for GraphQL resolvers)
    pub async fn get_reviews_by_ids(&self, ids: Vec<Uuid>) -> Result<Vec<Option<Review>>> {
        if let Some(dataloader) = &self.dataloader {
            return dataloader.reviews.load_reviews(ids).await;
        }

        // Fallback to repository batch method
        self.repository.get_reviews_by_ids(ids).await
    }

    /// Batch load multiple offer ratings by IDs (useful for GraphQL resolvers)
    pub async fn get_offer_ratings_by_ids(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<OfferRating>>> {
        if let Some(dataloader) = &self.dataloader {
            return dataloader.offer_ratings.load_offer_ratings(offer_ids).await;
        }

        // Fallback to repository batch method
        self.repository.get_offer_ratings_by_ids(offer_ids).await
    }
}

impl Clone for ReviewService {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            cache: self.cache.as_ref().map(Arc::clone),
            dataloader: self.dataloader.as_ref().map(Arc::clone),
        }
    }
}

// Factory function to create service with PostgreSQL repository
pub fn create_review_service(pool: sqlx::PgPool) -> ReviewService {
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    ReviewService::new(repository)
}

// Factory function to create service with PostgreSQL repository and metrics
pub fn create_review_service_with_metrics(pool: sqlx::PgPool, metrics: Arc<crate::telemetry::metrics::Metrics>) -> ReviewService {
    let repository = Arc::new(PostgresReviewRepository::with_metrics(pool, metrics));
    ReviewService::new(repository)
}

// Factory function to create service with cache
pub async fn create_review_service_with_cache(
    pool: sqlx::PgPool, 
    cache_config: RedisCacheConfig
) -> Result<ReviewService> {
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let cache = Arc::new(CacheService::new(cache_config).await?);
    Ok(ReviewService::with_cache(repository, cache))
}

// Factory function to create service with cache and metrics
pub async fn create_review_service_with_cache_and_metrics(
    pool: sqlx::PgPool, 
    cache_config: RedisCacheConfig,
    metrics: Arc<crate::telemetry::metrics::Metrics>
) -> Result<ReviewService> {
    let repository = Arc::new(PostgresReviewRepository::with_metrics(pool, metrics));
    let cache = Arc::new(CacheService::new(cache_config).await?);
    Ok(ReviewService::with_cache(repository, cache))
}

// Factory function to create service with DataLoader
pub fn create_review_service_with_dataloader(pool: sqlx::PgPool) -> ReviewService {
    let repository = Arc::new(PostgresReviewRepository::new(pool));
    let dataloader = Arc::new(DataLoaderService::new(Arc::clone(&repository)));
    ReviewService::with_dataloader(repository, dataloader)
}

// Factory function to create service with cache, DataLoader and metrics
pub async fn create_review_service_full(
    pool: sqlx::PgPool, 
    cache_config: RedisCacheConfig,
    metrics: Arc<crate::telemetry::metrics::Metrics>
) -> Result<ReviewService> {
    let repository = Arc::new(PostgresReviewRepository::with_metrics(pool, metrics));
    let cache = Arc::new(CacheService::new(cache_config).await?);
    let dataloader = Arc::new(DataLoaderService::new(Arc::clone(&repository)));
    Ok(ReviewService::with_cache_and_dataloader(repository, cache, dataloader))
}

/// Background task for cache cleanup
pub async fn start_cache_cleanup_task(external_client: Arc<ExternalServiceClient>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
    
    loop {
        interval.tick().await;
        external_client.cleanup_expired_cache().await;
        tracing::debug!("Cache cleanup completed");
    }
}