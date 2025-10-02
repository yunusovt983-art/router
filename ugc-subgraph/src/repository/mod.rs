use async_trait::async_trait;
use chrono::DateTime;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use tracing::{instrument, error, warn, info};
use std::sync::Arc;

use crate::error::{Result, UgcError};
use crate::models::{
    offer_rating::OfferRating,
    review::{CreateReviewInput, ModerationStatus, Review, ReviewsFilter, UpdateReviewInput},
};
use crate::telemetry::{
    metrics::Metrics,
    logging::BusinessEventLogger,
};

#[cfg(test)]
use mockall::automock;

#[async_trait]
#[cfg_attr(test, automock)]
pub trait ReviewRepository: Send + Sync {
    async fn create_review(&self, input: CreateReviewInput, author_id: Uuid) -> Result<Review>;
    async fn get_review_by_id(&self, id: Uuid) -> Result<Option<Review>>;
    async fn update_review(&self, id: Uuid, input: UpdateReviewInput) -> Result<Review>;
    async fn delete_review(&self, id: Uuid) -> Result<()>;
    async fn get_reviews_with_pagination(
        &self,
        filter: Option<ReviewsFilter>,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<Review>, i32)>;
    async fn get_reviews_after_cursor(
        &self,
        filter: Option<ReviewsFilter>,
        cursor_timestamp: i64,
        cursor_id: Uuid,
        limit: i32,
    ) -> Result<Vec<Review>>;
    async fn moderate_review(&self, id: Uuid, status: ModerationStatus) -> Result<Review>;
    async fn get_offer_rating(&self, offer_id: Uuid) -> Result<Option<OfferRating>>;
    async fn update_offer_rating(&self, offer_id: Uuid) -> Result<OfferRating>;
    
    // Batch loading methods for DataLoader optimization
    async fn get_reviews_by_ids(&self, ids: Vec<Uuid>) -> Result<Vec<Option<Review>>>;
    async fn get_offer_ratings_by_ids(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<OfferRating>>>;
    async fn get_reviews_by_offer_ids(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>>;
    async fn get_reviews_by_author_ids(&self, author_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>>;
}

pub struct PostgresReviewRepository {
    pool: PgPool,
    metrics: Option<Arc<Metrics>>,
}

impl PostgresReviewRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { 
            pool,
            metrics: None,
        }
    }
    
    pub fn with_metrics(pool: PgPool, metrics: Arc<Metrics>) -> Self {
        Self { 
            pool,
            metrics: Some(metrics),
        }
    }
    
    fn record_db_query(&self, operation: &str, duration: std::time::Duration, success: bool) {
        if let Some(metrics) = &self.metrics {
            metrics.db_queries_total.inc();
            metrics.db_query_duration.observe(duration.as_secs_f64());
            
            if !success {
                metrics.db_errors_total.inc();
            }
        }
    }
}

#[async_trait]
impl ReviewRepository for PostgresReviewRepository {
    #[instrument(skip(self), fields(offer_id = %input.offer_id, author_id = %author_id))]
    async fn create_review(&self, input: CreateReviewInput, author_id: Uuid) -> Result<Review> {
        let start_time = std::time::Instant::now();
        
        let result = sqlx::query_as::<_, Review>(
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
        .fetch_one(&self.pool)
        .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("create_review", duration, success);

        match result {
            Ok(review) => {
                info!("Review created in database: {}", review.id);
                if let Some(metrics) = &self.metrics {
                    metrics.reviews_created_total.inc();
                }
                Ok(review)
            }
            Err(e) => {
                error!("Failed to create review in database: {}", e);
                BusinessEventLogger::database_error(
                    "create_review",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }

    #[instrument(skip(self), fields(review_id = %id))]
    async fn get_review_by_id(&self, id: Uuid) -> Result<Option<Review>> {
        let start_time = std::time::Instant::now();
        
        let result = sqlx::query_as::<_, Review>(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            FROM reviews 
            WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("get_review_by_id", duration, success);

        match result {
            Ok(review) => Ok(review),
            Err(e) => {
                error!("Failed to get review by id {}: {}", id, e);
                BusinessEventLogger::database_error(
                    "get_review_by_id",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }

    async fn update_review(&self, id: Uuid, input: UpdateReviewInput) -> Result<Review> {
        let review = sqlx::query_as::<_, Review>(
            r#"
            UPDATE reviews 
            SET rating = COALESCE($2, rating),
                text = COALESCE($3, text),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            "#
        )
        .bind(id)
        .bind(input.rating)
        .bind(input.text)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(UgcError::ReviewNotFound { id })?;

        Ok(review)
    }

    async fn delete_review(&self, id: Uuid) -> Result<()> {
        let result = sqlx::query("DELETE FROM reviews WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(UgcError::ReviewNotFound { id });
        }

        Ok(())
    }

    async fn get_reviews_with_pagination(
        &self,
        filter: Option<ReviewsFilter>,
        limit: i32,
        offset: i32,
    ) -> Result<(Vec<Review>, i32)> {
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated,
                   moderation_status
            FROM reviews 
            WHERE 1=1
            "#
        );

        let mut count_builder = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM reviews WHERE 1=1");

        if let Some(filter) = &filter {
            if let Some(offer_id) = filter.offer_id {
                query_builder.push(" AND offer_id = ");
                query_builder.push_bind(offer_id);
                count_builder.push(" AND offer_id = ");
                count_builder.push_bind(offer_id);
            }

            if let Some(author_id) = filter.author_id {
                query_builder.push(" AND author_id = ");
                query_builder.push_bind(author_id);
                count_builder.push(" AND author_id = ");
                count_builder.push_bind(author_id);
            }

            if let Some(min_rating) = filter.min_rating {
                query_builder.push(" AND rating >= ");
                query_builder.push_bind(min_rating);
                count_builder.push(" AND rating >= ");
                count_builder.push_bind(min_rating);
            }

            if let Some(max_rating) = filter.max_rating {
                query_builder.push(" AND rating <= ");
                query_builder.push_bind(max_rating);
                count_builder.push(" AND rating <= ");
                count_builder.push_bind(max_rating);
            }

            if let Some(moderated_only) = filter.moderated_only {
                if moderated_only {
                    query_builder.push(" AND is_moderated = true");
                    count_builder.push(" AND is_moderated = true");
                }
            }

            if let Some(status) = &filter.moderation_status {
                query_builder.push(" AND moderation_status = ");
                query_builder.push_bind(status.to_string());
                count_builder.push(" AND moderation_status = ");
                count_builder.push_bind(status.to_string());
            }
        }

        query_builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        query_builder.push_bind(limit);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset);

        let reviews_query = query_builder.build();
        let count_query = count_builder.build();

        let reviews: Vec<Review> = reviews_query
            .map(|row: sqlx::postgres::PgRow| Review {
                id: row.get("id"),
                offer_id: row.get("offer_id"),
                author_id: row.get("author_id"),
                rating: row.get("rating"),
                text: row.get("text"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                is_moderated: row.get("is_moderated"),
                moderation_status: row.get::<String, _>("moderation_status").parse().unwrap_or_default(),
            })
            .fetch_all(&self.pool)
            .await?;

        let total_count: i64 = count_query.fetch_one(&self.pool).await?.get(0);

        Ok((reviews, total_count as i32))
    }

    async fn get_reviews_after_cursor(
        &self,
        filter: Option<ReviewsFilter>,
        cursor_timestamp: i64,
        cursor_id: Uuid,
        limit: i32,
    ) -> Result<Vec<Review>> {
        let cursor_datetime = DateTime::from_timestamp_millis(cursor_timestamp)
            .ok_or_else(|| UgcError::ValidationError {
                message: "Invalid cursor timestamp".to_string(),
            })?;

        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated,
                   moderation_status
            FROM reviews 
            WHERE (created_at, id) < (
            "#
        );

        query_builder.push_bind(cursor_datetime);
        query_builder.push(", ");
        query_builder.push_bind(cursor_id);
        query_builder.push(")");

        if let Some(filter) = &filter {
            if let Some(offer_id) = filter.offer_id {
                query_builder.push(" AND offer_id = ");
                query_builder.push_bind(offer_id);
            }

            if let Some(author_id) = filter.author_id {
                query_builder.push(" AND author_id = ");
                query_builder.push_bind(author_id);
            }

            if let Some(min_rating) = filter.min_rating {
                query_builder.push(" AND rating >= ");
                query_builder.push_bind(min_rating);
            }

            if let Some(max_rating) = filter.max_rating {
                query_builder.push(" AND rating <= ");
                query_builder.push_bind(max_rating);
            }

            if let Some(moderated_only) = filter.moderated_only {
                if moderated_only {
                    query_builder.push(" AND is_moderated = true");
                }
            }

            if let Some(status) = &filter.moderation_status {
                query_builder.push(" AND moderation_status = ");
                query_builder.push_bind(status.to_string());
            }
        }

        query_builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        query_builder.push_bind(limit);

        let reviews: Vec<Review> = query_builder
            .build()
            .map(|row: sqlx::postgres::PgRow| Review {
                id: row.get("id"),
                offer_id: row.get("offer_id"),
                author_id: row.get("author_id"),
                rating: row.get("rating"),
                text: row.get("text"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                is_moderated: row.get("is_moderated"),
                moderation_status: row.get::<String, _>("moderation_status").parse().unwrap_or_default(),
            })
            .fetch_all(&self.pool)
            .await?;

        Ok(reviews)
    }

    async fn moderate_review(&self, id: Uuid, status: ModerationStatus) -> Result<Review> {
        let review = sqlx::query_as::<_, Review>(
            r#"
            UPDATE reviews 
            SET moderation_status = $2,
                is_moderated = CASE WHEN $2 = 'approved' THEN true ELSE is_moderated END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            "#
        )
        .bind(id)
        .bind(status.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(UgcError::ReviewNotFound { id })?;

        Ok(review)
    }

    async fn get_offer_rating(&self, offer_id: Uuid) -> Result<Option<OfferRating>> {
        let rating = sqlx::query_as::<_, OfferRating>(
            "SELECT offer_id, average_rating, reviews_count, rating_distribution, updated_at FROM offer_ratings WHERE offer_id = $1"
        )
        .bind(offer_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(rating)
    }

    async fn update_offer_rating(&self, offer_id: Uuid) -> Result<OfferRating> {
        // Calculate new rating statistics
        let stats = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as reviews_count,
                AVG(rating::decimal) as average_rating,
                COUNT(CASE WHEN rating = 1 THEN 1 END) as rating_1,
                COUNT(CASE WHEN rating = 2 THEN 1 END) as rating_2,
                COUNT(CASE WHEN rating = 3 THEN 1 END) as rating_3,
                COUNT(CASE WHEN rating = 4 THEN 1 END) as rating_4,
                COUNT(CASE WHEN rating = 5 THEN 1 END) as rating_5
            FROM reviews 
            WHERE offer_id = $1 AND is_moderated = true
            "#
        )
        .bind(offer_id)
        .fetch_one(&self.pool)
        .await?;

        let reviews_count: i64 = stats.try_get("reviews_count").unwrap_or(0);
        let average_rating: Option<rust_decimal::Decimal> = stats.try_get("average_rating").ok();
        let average_rating = average_rating.unwrap_or_else(|| rust_decimal::Decimal::ZERO);
        
        let rating_1: i64 = stats.try_get("rating_1").unwrap_or(0);
        let rating_2: i64 = stats.try_get("rating_2").unwrap_or(0);
        let rating_3: i64 = stats.try_get("rating_3").unwrap_or(0);
        let rating_4: i64 = stats.try_get("rating_4").unwrap_or(0);
        let rating_5: i64 = stats.try_get("rating_5").unwrap_or(0);
        
        let rating_distribution = serde_json::json!({
            "1": rating_1,
            "2": rating_2,
            "3": rating_3,
            "4": rating_4,
            "5": rating_5,
        });

        // Upsert the rating record
        let rating = sqlx::query_as::<_, OfferRating>(
            r#"
            INSERT INTO offer_ratings (offer_id, average_rating, reviews_count, rating_distribution, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (offer_id) 
            DO UPDATE SET 
                average_rating = EXCLUDED.average_rating,
                reviews_count = EXCLUDED.reviews_count,
                rating_distribution = EXCLUDED.rating_distribution,
                updated_at = NOW()
            RETURNING offer_id, average_rating, reviews_count, rating_distribution, updated_at
            "#
        )
        .bind(offer_id)
        .bind(average_rating)
        .bind(reviews_count as i32)
        .bind(rating_distribution)
        .fetch_one(&self.pool)
        .await?;

        Ok(rating)
    }

    async fn get_reviews_by_ids(&self, ids: Vec<Uuid>) -> Result<Vec<Option<Review>>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let start_time = std::time::Instant::now();
        
        // Build query with IN clause
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            FROM reviews 
            WHERE id = ANY(
            "#
        );
        
        query_builder.push_bind(&ids[..]);
        query_builder.push(")");

        let result = query_builder
            .build()
            .map(|row: sqlx::postgres::PgRow| Review {
                id: row.get("id"),
                offer_id: row.get("offer_id"),
                author_id: row.get("author_id"),
                rating: row.get("rating"),
                text: row.get("text"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                is_moderated: row.get("is_moderated"),
                moderation_status: row.get::<String, _>("moderation_status").parse().unwrap_or_default(),
            })
            .fetch_all(&self.pool)
            .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("get_reviews_by_ids", duration, success);

        match result {
            Ok(reviews) => {
                // Create a map for O(1) lookup
                let mut review_map = std::collections::HashMap::new();
                for review in reviews {
                    review_map.insert(review.id, review);
                }

                // Return results in the same order as requested IDs
                let results = ids
                    .into_iter()
                    .map(|id| review_map.remove(&id))
                    .collect();

                Ok(results)
            }
            Err(e) => {
                error!("Failed to get reviews by ids: {}", e);
                BusinessEventLogger::database_error(
                    "get_reviews_by_ids",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }

    async fn get_offer_ratings_by_ids(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<OfferRating>>> {
        if offer_ids.is_empty() {
            return Ok(vec![]);
        }

        let start_time = std::time::Instant::now();
        
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT offer_id, average_rating, reviews_count, rating_distribution, updated_at FROM offer_ratings WHERE offer_id = ANY("
        );
        
        query_builder.push_bind(&offer_ids[..]);
        query_builder.push(")");

        let result = query_builder
            .build_as::<OfferRating>()
            .fetch_all(&self.pool)
            .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("get_offer_ratings_by_ids", duration, success);

        match result {
            Ok(ratings) => {
                // Create a map for O(1) lookup
                let mut rating_map = std::collections::HashMap::new();
                for rating in ratings {
                    rating_map.insert(rating.offer_id, rating);
                }

                // Return results in the same order as requested IDs
                let results = offer_ids
                    .into_iter()
                    .map(|id| rating_map.remove(&id))
                    .collect();

                Ok(results)
            }
            Err(e) => {
                error!("Failed to get offer ratings by ids: {}", e);
                BusinessEventLogger::database_error(
                    "get_offer_ratings_by_ids",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }

    async fn get_reviews_by_offer_ids(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>> {
        if offer_ids.is_empty() {
            return Ok(vec![]);
        }

        let start_time = std::time::Instant::now();
        
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            FROM reviews 
            WHERE offer_id = ANY(
            "#
        );
        
        query_builder.push_bind(&offer_ids[..]);
        query_builder.push(") AND is_moderated = true ORDER BY created_at DESC");

        let result = query_builder
            .build()
            .map(|row: sqlx::postgres::PgRow| Review {
                id: row.get("id"),
                offer_id: row.get("offer_id"),
                author_id: row.get("author_id"),
                rating: row.get("rating"),
                text: row.get("text"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                is_moderated: row.get("is_moderated"),
                moderation_status: row.get::<String, _>("moderation_status").parse().unwrap_or_default(),
            })
            .fetch_all(&self.pool)
            .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("get_reviews_by_offer_ids", duration, success);

        match result {
            Ok(reviews) => {
                // Group reviews by offer_id
                let mut reviews_by_offer = std::collections::HashMap::new();
                for review in reviews {
                    reviews_by_offer
                        .entry(review.offer_id)
                        .or_insert_with(Vec::new)
                        .push(review);
                }

                // Return results in the same order as requested IDs
                let results = offer_ids
                    .into_iter()
                    .map(|id| reviews_by_offer.remove(&id))
                    .collect();

                Ok(results)
            }
            Err(e) => {
                error!("Failed to get reviews by offer ids: {}", e);
                BusinessEventLogger::database_error(
                    "get_reviews_by_offer_ids",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }

    async fn get_reviews_by_author_ids(&self, author_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>> {
        if author_ids.is_empty() {
            return Ok(vec![]);
        }

        let start_time = std::time::Instant::now();
        
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
            SELECT id, offer_id, author_id, rating, text, created_at, updated_at, is_moderated, moderation_status
            FROM reviews 
            WHERE author_id = ANY(
            "#
        );
        
        query_builder.push_bind(&author_ids[..]);
        query_builder.push(") AND is_moderated = true ORDER BY created_at DESC");

        let result = query_builder
            .build()
            .map(|row: sqlx::postgres::PgRow| Review {
                id: row.get("id"),
                offer_id: row.get("offer_id"),
                author_id: row.get("author_id"),
                rating: row.get("rating"),
                text: row.get("text"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                is_moderated: row.get("is_moderated"),
                moderation_status: row.get::<String, _>("moderation_status").parse().unwrap_or_default(),
            })
            .fetch_all(&self.pool)
            .await;

        let duration = start_time.elapsed();
        let success = result.is_ok();
        self.record_db_query("get_reviews_by_author_ids", duration, success);

        match result {
            Ok(reviews) => {
                // Group reviews by author_id
                let mut reviews_by_author = std::collections::HashMap::new();
                for review in reviews {
                    reviews_by_author
                        .entry(review.author_id)
                        .or_insert_with(Vec::new)
                        .push(review);
                }

                // Return results in the same order as requested IDs
                let results = author_ids
                    .into_iter()
                    .map(|id| reviews_by_author.remove(&id))
                    .collect();

                Ok(results)
            }
            Err(e) => {
                error!("Failed to get reviews by author ids: {}", e);
                BusinessEventLogger::database_error(
                    "get_reviews_by_author_ids",
                    &e.to_string(),
                    duration.as_millis() as u64,
                );
                Err(e.into())
            }
        }
    }
}