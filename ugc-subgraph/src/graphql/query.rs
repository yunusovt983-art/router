use async_graphql::{Context, ErrorExtensions, Object, Result};
use tracing::instrument;
use uuid::Uuid;

use crate::graphql::types::{
    generate_cursor, parse_cursor, OfferRatingStats, PageInfo, ReviewConnection, ReviewEdge,
    ReviewType, ReviewsFilterInput,
};
use crate::service::ReviewService;

pub struct Query;

#[Object]
impl Query {
    #[instrument(skip(self, _ctx))]
    async fn health(&self, _ctx: &Context<'_>) -> Result<String> {
        Ok("UGC Subgraph is healthy".to_string())
    }

    #[instrument(skip(self, _ctx))]
    async fn version(&self, _ctx: &Context<'_>) -> Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }

    /// Get a review by ID
    #[instrument(skip(self, ctx))]
    async fn review(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<ReviewType>> {
        let service = ctx.data::<ReviewService>()?;
        
        match service.get_review_by_id(id).await {
            Ok(Some(review)) => Ok(Some(review.into())),
            Ok(None) => Ok(None),
            Err(e) => Err(e.extend()),
        }
    }

    /// Get reviews with Relay-style pagination
    #[instrument(skip(self, ctx))]
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        filter: Option<ReviewsFilterInput>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let limit = first.unwrap_or(10).min(100).max(1);

        let reviews = if let Some(cursor) = after {
            // Cursor-based pagination
            let (cursor_timestamp, cursor_id) = parse_cursor(&cursor)?;
            let filter = filter.map(|f| f.into());
            
            match service.get_reviews_after_cursor(filter, cursor_timestamp, cursor_id, limit + 1).await {
                Ok(mut reviews) => {
                    let has_next_page = reviews.len() > limit as usize;
                    if has_next_page {
                        reviews.pop(); // Remove the extra item used for pagination check
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
                            has_previous_page: true, // If we have a cursor, there are previous pages
                            start_cursor,
                            end_cursor,
                        },
                        total_count: 0, // Not calculated for cursor-based pagination
                    }
                }
                Err(e) => return Err(e.extend()),
            }
        } else {
            // Offset-based pagination for first page
            let filter = filter.map(|f| f.into());
            
            match service.get_reviews_with_pagination(filter, limit, 0).await {
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

    /// Get aggregated rating statistics for an offer
    #[instrument(skip(self, ctx))]
    async fn offer_rating_stats(&self, ctx: &Context<'_>, offer_id: Uuid) -> Result<Option<OfferRatingStats>> {
        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(offer_id).await {
            Ok(Some(rating)) => Ok(Some(rating.into())),
            Ok(None) => {
                // If no cached rating exists, calculate it
                match service.update_offer_rating(offer_id).await {
                    Ok(rating) => Ok(Some(rating.into())),
                    Err(_) => Ok(None), // Return None if offer has no reviews
                }
            }
            Err(e) => Err(e.extend()),
        }
    }

    /// Get average rating for an offer
    #[instrument(skip(self, ctx))]
    async fn offer_average_rating(&self, ctx: &Context<'_>, offer_id: Uuid) -> Result<Option<f64>> {
        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(offer_id).await {
            Ok(Some(rating)) => Ok(Some(rating.average_rating_f64())),
            Ok(None) => {
                // If no cached rating exists, calculate it
                match service.update_offer_rating(offer_id).await {
                    Ok(rating) => Ok(Some(rating.average_rating_f64())),
                    Err(_) => Ok(None),
                }
            }
            Err(e) => Err(e.extend()),
        }
    }

    /// Get total reviews count for an offer
    #[instrument(skip(self, ctx))]
    async fn offer_reviews_count(&self, ctx: &Context<'_>, offer_id: Uuid) -> Result<i32> {
        let service = ctx.data::<ReviewService>()?;
        
        match service.get_offer_rating(offer_id).await {
            Ok(Some(rating)) => Ok(rating.reviews_count),
            Ok(None) => {
                // If no cached rating exists, calculate it
                match service.update_offer_rating(offer_id).await {
                    Ok(rating) => Ok(rating.reviews_count),
                    Err(_) => Ok(0),
                }
            }
            Err(e) => Err(e.extend()),
        }
    }
}