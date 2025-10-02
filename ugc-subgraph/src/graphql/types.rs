use async_graphql::{ComplexObject, Context, ErrorExtensions, InputObject, Result, SimpleObject, Union};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{
    offer_rating::{OfferRating, RatingDistribution},
    review::{CreateReviewInput as ModelCreateReviewInput, ModerationStatus, Review, UpdateReviewInput as ModelUpdateReviewInput},
};


// ============================================================================
// Core GraphQL Types
// ============================================================================

/// Review type with federation directives
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct ReviewType {
    pub id: Uuid,
    pub offer_id: Uuid,
    pub author_id: Uuid,
    pub rating: i32,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_moderated: bool,
    pub moderation_status: ModerationStatus,
}

impl ReviewType {
    /// Reference resolver for Review entity
    pub async fn find_by_id(ctx: &Context<'_>, id: Uuid) -> Result<Option<ReviewType>> {
        use crate::service::ReviewService;
        
        let service = ctx.data::<ReviewService>()?;
        
        match service.get_review_by_id(id).await {
            Ok(Some(review)) => Ok(Some(review.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(e.extend()),
        }
    }
}

#[ComplexObject]
impl ReviewType {
    /// Get the offer this review belongs to (federated)
    async fn offer(&self, _ctx: &Context<'_>) -> Result<OfferType> {
        Ok(OfferType {
            id: self.offer_id,
        })
    }

    /// Get the author of this review (federated)
    async fn author(&self, _ctx: &Context<'_>) -> Result<UserType> {
        Ok(UserType {
            id: self.author_id,
        })
    }
}

impl From<Review> for ReviewType {
    fn from(review: Review) -> Self {
        Self {
            id: review.id,
            offer_id: review.offer_id,
            author_id: review.author_id,
            rating: review.rating,
            text: review.text,
            created_at: review.created_at,
            updated_at: review.updated_at,
            is_moderated: review.is_moderated,
            moderation_status: review.moderation_status,
        }
    }
}

/// Federated User type (extended from Users subgraph)
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct UserType {
    pub id: Uuid,
}

impl UserType {
    /// Reference resolver for User entity
    pub async fn find_by_id(_ctx: &Context<'_>, id: Uuid) -> Result<UserType> {
        // This is a reference resolver - it creates a UserType with just the key
        // The actual user data will be resolved by the Users subgraph
        Ok(UserType { id })
    }
}

#[ComplexObject]
impl UserType {
    /// Get reviews written by this user with pagination
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        filter: Option<ReviewsFilterInput>,
    ) -> Result<ReviewConnection> {
        use crate::service::ReviewService;
        use crate::models::review::ReviewsFilter;

        let service = ctx.data::<ReviewService>()?;
        let limit = first.unwrap_or(10).min(100).max(1);

        // Add author_id to filter
        let mut filter: ReviewsFilter = filter.map(|f| f.into()).unwrap_or_default();
        filter.author_id = Some(self.id);

        let reviews = if let Some(cursor) = after {
            // Cursor-based pagination
            let (cursor_timestamp, cursor_id) = parse_cursor(&cursor)?;
            
            match service.get_reviews_after_cursor(Some(filter), cursor_timestamp, cursor_id, limit + 1).await {
                Ok(mut reviews) => {
                    let has_next_page = reviews.len() > limit as usize;
                    if has_next_page {
                        reviews.pop();
                    }

                    let edges: Vec<ReviewEdge> = reviews
                        .into_iter()
                        .map(|review| {
                            let cursor = generate_cursor(&review.id, &review.created_at);
                            ReviewEdge {
                                node: review.into(),
                                cursor,
                            }
                        })
                        .collect();

                    let start_cursor = edges.first().map(|e| e.cursor.clone());
                    let end_cursor = edges.last().map(|e| e.cursor.clone());

                    ReviewConnection {
                        edges,
                        page_info: PageInfo {
                            has_next_page,
                            has_previous_page: true,
                            start_cursor,
                            end_cursor,
                        },
                        total_count: 0,
                    }
                }
                Err(e) => return Err(e.extend()),
            }
        } else {
            // Offset-based pagination for first page
            match service.get_reviews_with_pagination(Some(filter), limit, 0).await {
                Ok((reviews, total_count)) => {
                    let has_next_page = reviews.len() == limit as usize && total_count > limit;

                    let edges: Vec<ReviewEdge> = reviews
                        .into_iter()
                        .map(|review| {
                            let cursor = generate_cursor(&review.id, &review.created_at);
                            ReviewEdge {
                                node: review.into(),
                                cursor,
                            }
                        })
                        .collect();

                    let start_cursor = edges.first().map(|e| e.cursor.clone());
                    let end_cursor = edges.last().map(|e| e.cursor.clone());

                    ReviewConnection {
                        edges,
                        page_info: PageInfo {
                            has_next_page,
                            has_previous_page: false,
                            start_cursor,
                            end_cursor,
                        },
                        total_count,
                    }
                }
                Err(e) => return Err(e.extend()),
            }
        };

        Ok(reviews)
    }
}

/// Federated Offer type (extended from Offers subgraph)
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct OfferType {
    pub id: Uuid,
}

impl OfferType {
    /// Reference resolver for Offer entity
    pub async fn find_by_id(_ctx: &Context<'_>, id: Uuid) -> Result<OfferType> {
        // This is a reference resolver - it creates an OfferType with just the key
        // The actual offer data will be resolved by the Offers subgraph
        Ok(OfferType { id })
    }
}

#[ComplexObject]
impl OfferType {
    /// Get reviews for this offer with pagination
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        filter: Option<ReviewsFilterInput>,
    ) -> Result<ReviewConnection> {
        use crate::service::ReviewService;
        use crate::models::review::ReviewsFilter;

        let service = ctx.data::<ReviewService>()?;
        let limit = first.unwrap_or(10).min(100).max(1);

        // Add offer_id to filter
        let mut filter: ReviewsFilter = filter.map(|f| f.into()).unwrap_or_default();
        filter.offer_id = Some(self.id);

        let reviews = if let Some(cursor) = after {
            // Cursor-based pagination
            let (cursor_timestamp, cursor_id) = parse_cursor(&cursor)?;
            
            match service.get_reviews_after_cursor(Some(filter), cursor_timestamp, cursor_id, limit + 1).await {
                Ok(mut reviews) => {
                    let has_next_page = reviews.len() > limit as usize;
                    if has_next_page {
                        reviews.pop();
                    }

                    let edges: Vec<ReviewEdge> = reviews
                        .into_iter()
                        .map(|review| {
                            let cursor = generate_cursor(&review.id, &review.created_at);
                            ReviewEdge {
                                node: review.into(),
                                cursor,
                            }
                        })
                        .collect();

                    let start_cursor = edges.first().map(|e| e.cursor.clone());
                    let end_cursor = edges.last().map(|e| e.cursor.clone());

                    ReviewConnection {
                        edges,
                        page_info: PageInfo {
                            has_next_page,
                            has_previous_page: true,
                            start_cursor,
                            end_cursor,
                        },
                        total_count: 0,
                    }
                }
                Err(e) => return Err(e.extend()),
            }
        } else {
            // Offset-based pagination for first page
            match service.get_reviews_with_pagination(Some(filter), limit, 0).await {
                Ok((reviews, total_count)) => {
                    let has_next_page = reviews.len() == limit as usize && total_count > limit;

                    let edges: Vec<ReviewEdge> = reviews
                        .into_iter()
                        .map(|review| {
                            let cursor = generate_cursor(&review.id, &review.created_at);
                            ReviewEdge {
                                node: review.into(),
                                cursor,
                            }
                        })
                        .collect();

                    let start_cursor = edges.first().map(|e| e.cursor.clone());
                    let end_cursor = edges.last().map(|e| e.cursor.clone());

                    ReviewConnection {
                        edges,
                        page_info: PageInfo {
                            has_next_page,
                            has_previous_page: false,
                            start_cursor,
                            end_cursor,
                        },
                        total_count,
                    }
                }
                Err(e) => return Err(e.extend()),
            }
        };

        Ok(reviews)
    }

    /// Get average rating for this offer
    async fn average_rating(&self, ctx: &Context<'_>) -> Result<Option<f64>> {
        use crate::service::ReviewService;

        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(self.id).await {
            Ok(Some(rating)) => Ok(Some(rating.average_rating_f64())),
            Ok(None) => {
                match service.update_offer_rating(self.id).await {
                    Ok(rating) => Ok(Some(rating.average_rating_f64())),
                    Err(_) => Ok(None),
                }
            }
            Err(e) => Err(e.extend()),
        }
    }

    /// Get total number of reviews for this offer
    async fn reviews_count(&self, ctx: &Context<'_>) -> Result<i32> {
        use crate::service::ReviewService;

        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(self.id).await {
            Ok(Some(rating)) => Ok(rating.reviews_count),
            Ok(None) => {
                match service.update_offer_rating(self.id).await {
                    Ok(rating) => Ok(rating.reviews_count),
                    Err(_) => Ok(0),
                }
            }
            Err(e) => Err(e.extend()),
        }
    }

    /// Get rating distribution for this offer
    async fn rating_distribution(&self, ctx: &Context<'_>) -> Result<Option<RatingDistributionType>> {
        use crate::service::ReviewService;

        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(self.id).await {
            Ok(Some(rating)) => {
                match rating.get_rating_distribution() {
                    Ok(distribution) => Ok(Some(distribution.into())),
                    Err(_) => Ok(None),
                }
            }
            Ok(None) => {
                match service.update_offer_rating(self.id).await {
                    Ok(rating) => {
                        match rating.get_rating_distribution() {
                            Ok(distribution) => Ok(Some(distribution.into())),
                            Err(_) => Ok(None),
                        }
                    }
                    Err(_) => Ok(None),
                }
            }
            Err(e) => Err(e.extend()),
        }
    }
}

/// Rating distribution type
#[derive(SimpleObject)]
pub struct RatingDistributionType {
    pub rating_1: i32,
    pub rating_2: i32,
    pub rating_3: i32,
    pub rating_4: i32,
    pub rating_5: i32,
    pub total_reviews: i32,
}

impl From<RatingDistribution> for RatingDistributionType {
    fn from(distribution: RatingDistribution) -> Self {
        Self {
            rating_1: distribution.rating_1,
            rating_2: distribution.rating_2,
            rating_3: distribution.rating_3,
            rating_4: distribution.rating_4,
            rating_5: distribution.rating_5,
            total_reviews: distribution.total_reviews(),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a new review
#[derive(InputObject, Debug)]
pub struct CreateReviewInput {
    pub offer_id: Uuid,
    pub rating: i32,
    pub text: String,
}

impl From<CreateReviewInput> for ModelCreateReviewInput {
    fn from(input: CreateReviewInput) -> Self {
        Self {
            offer_id: input.offer_id,
            rating: input.rating,
            text: input.text,
        }
    }
}

/// Input for updating an existing review
#[derive(InputObject, Debug)]
pub struct UpdateReviewInput {
    pub rating: Option<i32>,
    pub text: Option<String>,
}

impl From<UpdateReviewInput> for ModelUpdateReviewInput {
    fn from(input: UpdateReviewInput) -> Self {
        Self {
            rating: input.rating,
            text: input.text,
        }
    }
}

/// Input for moderating a review (admin only)
#[derive(InputObject, Debug)]
pub struct ModerateReviewInput {
    pub review_id: Uuid,
    pub status: ModerationStatus,
    pub reason: Option<String>,
}

/// Filter input for querying reviews
#[derive(InputObject, Debug)]
pub struct ReviewsFilterInput {
    pub offer_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub min_rating: Option<i32>,
    pub max_rating: Option<i32>,
    pub moderated_only: Option<bool>,
    pub moderation_status: Option<ModerationStatus>,
}

impl From<ReviewsFilterInput> for crate::models::review::ReviewsFilter {
    fn from(input: ReviewsFilterInput) -> Self {
        Self {
            offer_id: input.offer_id,
            author_id: input.author_id,
            min_rating: input.min_rating,
            max_rating: input.max_rating,
            moderated_only: input.moderated_only,
            moderation_status: input.moderation_status,
        }
    }
}

// ============================================================================
// Connection Types for Relay-style Pagination
// ============================================================================

/// Connection type for paginated reviews
#[derive(SimpleObject)]
pub struct ReviewConnection {
    pub edges: Vec<ReviewEdge>,
    pub page_info: PageInfo,
    pub total_count: i32,
}

/// Edge type for review connections
#[derive(SimpleObject)]
pub struct ReviewEdge {
    pub node: ReviewType,
    pub cursor: String,
}

/// Page info for pagination
#[derive(SimpleObject)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Response type for review mutations
#[derive(SimpleObject)]
pub struct ReviewMutationResponse {
    pub success: bool,
    pub message: String,
    pub review: Option<ReviewType>,
}

/// Response type for moderation actions
#[derive(SimpleObject)]
pub struct ModerationResponse {
    pub success: bool,
    pub message: String,
    pub review: Option<ReviewType>,
}

// ============================================================================
// Aggregation Types
// ============================================================================

/// Aggregated rating statistics for an offer
#[derive(SimpleObject)]
pub struct OfferRatingStats {
    pub offer_id: Uuid,
    pub average_rating: f64,
    pub reviews_count: i32,
    pub rating_distribution: RatingDistributionType,
    pub updated_at: DateTime<Utc>,
}

impl From<OfferRating> for OfferRatingStats {
    fn from(rating: OfferRating) -> Self {
        let distribution = rating.get_rating_distribution().unwrap_or_default();
        
        Self {
            offer_id: rating.offer_id,
            average_rating: rating.average_rating_f64(),
            reviews_count: rating.reviews_count,
            rating_distribution: distribution.into(),
            updated_at: rating.updated_at,
        }
    }
}

// ============================================================================
// Union Types
// ============================================================================

/// Union type for different types of review responses
#[derive(Union)]
pub enum ReviewResult {
    Success(ReviewMutationResponse),
    Error(ErrorResponse),
}

/// Error response type
#[derive(SimpleObject)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Generate cursor for pagination
pub fn generate_cursor(id: &Uuid, created_at: &DateTime<Utc>) -> String {
    use base64::{engine::general_purpose, Engine as _};
    let cursor_data = format!("{}:{}", created_at.timestamp_millis(), id);
    general_purpose::STANDARD.encode(cursor_data)
}

/// Parse cursor for pagination
pub fn parse_cursor(cursor: &str) -> Result<(i64, Uuid)> {
    use base64::{engine::general_purpose, Engine as _};
    
    let decoded = general_purpose::STANDARD
        .decode(cursor)
        .map_err(|_| async_graphql::Error::new("Invalid cursor format"))?;
    
    let cursor_str = String::from_utf8(decoded)
        .map_err(|_| async_graphql::Error::new("Invalid cursor encoding"))?;
    
    let parts: Vec<&str> = cursor_str.split(':').collect();
    if parts.len() != 2 {
        return Err(async_graphql::Error::new("Invalid cursor structure"));
    }
    
    let timestamp = parts[0]
        .parse::<i64>()
        .map_err(|_| async_graphql::Error::new("Invalid cursor timestamp"))?;
    
    let id = parts[1]
        .parse::<Uuid>()
        .map_err(|_| async_graphql::Error::new("Invalid cursor ID"))?;
    
    Ok((timestamp, id))
}