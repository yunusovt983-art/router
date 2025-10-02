use async_graphql::{Context, Object, Result};
use tracing::instrument;
use uuid::Uuid;

use crate::auth::UserContext;
use crate::graphql::guards::{RequireAuth, RequireModerator, RequireAdmin};
use crate::graphql::types::{
    CreateReviewInput, ModerationResponse, ModerateReviewInput, ReviewMutationResponse,
    UpdateReviewInput,
};
use crate::models::review::ModerationStatus;
use crate::service::ReviewService;

pub struct Mutation;

#[Object]
impl Mutation {
    #[instrument(skip(self, _ctx))]
    async fn ping(&self, _ctx: &Context<'_>) -> Result<String> {
        Ok("pong".to_string())
    }

    /// Create a new review
    #[instrument(skip(self, ctx))]
    #[graphql(guard = "RequireAuth")]
    async fn create_review(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> Result<ReviewMutationResponse> {
        let service = ctx.data::<ReviewService>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        match service.create_review(input.into(), user_context.user_id).await {
            Ok(review) => Ok(ReviewMutationResponse {
                success: true,
                message: "Review created successfully".to_string(),
                review: Some(review.into()),
            }),
            Err(e) => Ok(ReviewMutationResponse {
                success: false,
                message: e.to_string(),
                review: None,
            }),
        }
    }

    /// Update an existing review
    #[instrument(skip(self, ctx))]
    async fn update_review(
        &self,
        ctx: &Context<'_>,
        review_id: Uuid,
        input: UpdateReviewInput,
    ) -> Result<ReviewMutationResponse> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Extract user_id from authentication context
        let user_id = Uuid::new_v4(); // This should come from JWT token
        
        match service.update_review(review_id, input.into(), user_id).await {
            Ok(review) => Ok(ReviewMutationResponse {
                success: true,
                message: "Review updated successfully".to_string(),
                review: Some(review.into()),
            }),
            Err(e) => Ok(ReviewMutationResponse {
                success: false,
                message: e.to_string(),
                review: None,
            }),
        }
    }

    /// Delete a review
    #[instrument(skip(self, ctx))]
    async fn delete_review(
        &self,
        ctx: &Context<'_>,
        review_id: Uuid,
    ) -> Result<ReviewMutationResponse> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Extract user_id from authentication context
        let user_id = Uuid::new_v4(); // This should come from JWT token
        
        match service.delete_review(review_id, user_id).await {
            Ok(_) => Ok(ReviewMutationResponse {
                success: true,
                message: "Review deleted successfully".to_string(),
                review: None,
            }),
            Err(e) => Ok(ReviewMutationResponse {
                success: false,
                message: e.to_string(),
                review: None,
            }),
        }
    }

    /// Moderate a review (admin only)
    #[instrument(skip(self, ctx))]
    async fn moderate_review(
        &self,
        ctx: &Context<'_>,
        input: ModerateReviewInput,
    ) -> Result<ModerationResponse> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Check if user has moderator role
        // For now, allowing all users - this will be implemented in authorization task
        
        match service.moderate_review(input.review_id, input.status).await {
            Ok(review) => {
                let message = match input.status {
                    ModerationStatus::Approved => "Review approved successfully",
                    ModerationStatus::Rejected => "Review rejected successfully",
                    ModerationStatus::Flagged => "Review flagged for further review",
                    ModerationStatus::Pending => "Review marked as pending",
                };
                
                Ok(ModerationResponse {
                    success: true,
                    message: message.to_string(),
                    review: Some(review.into()),
                })
            }
            Err(e) => Ok(ModerationResponse {
                success: false,
                message: e.to_string(),
                review: None,
            }),
        }
    }

    /// Bulk approve reviews (admin only)
    #[instrument(skip(self, ctx))]
    async fn bulk_approve_reviews(
        &self,
        ctx: &Context<'_>,
        review_ids: Vec<Uuid>,
    ) -> Result<Vec<ModerationResponse>> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Check if user has moderator role
        
        let mut responses = Vec::new();
        
        for review_id in review_ids {
            let response = match service.moderate_review(review_id, ModerationStatus::Approved).await {
                Ok(review) => ModerationResponse {
                    success: true,
                    message: "Review approved successfully".to_string(),
                    review: Some(review.into()),
                },
                Err(e) => ModerationResponse {
                    success: false,
                    message: e.to_string(),
                    review: None,
                },
            };
            responses.push(response);
        }
        
        Ok(responses)
    }

    /// Bulk reject reviews (admin only)
    #[instrument(skip(self, ctx))]
    async fn bulk_reject_reviews(
        &self,
        ctx: &Context<'_>,
        review_ids: Vec<Uuid>,
    ) -> Result<Vec<ModerationResponse>> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Check if user has moderator role
        
        let mut responses = Vec::new();
        
        for review_id in review_ids {
            let response = match service.moderate_review(review_id, ModerationStatus::Rejected).await {
                Ok(review) => ModerationResponse {
                    success: true,
                    message: "Review rejected successfully".to_string(),
                    review: Some(review.into()),
                },
                Err(e) => ModerationResponse {
                    success: false,
                    message: e.to_string(),
                    review: None,
                },
            };
            responses.push(response);
        }
        
        Ok(responses)
    }

    /// Update offer rating statistics manually (admin only)
    #[instrument(skip(self, ctx))]
    async fn refresh_offer_rating(
        &self,
        ctx: &Context<'_>,
        offer_id: Uuid,
    ) -> Result<ReviewMutationResponse> {
        let service = ctx.data::<ReviewService>()?;
        
        // TODO: Check if user has admin role
        
        match service.update_offer_rating(offer_id).await {
            Ok(_) => Ok(ReviewMutationResponse {
                success: true,
                message: "Offer rating statistics refreshed successfully".to_string(),
                review: None,
            }),
            Err(e) => Ok(ReviewMutationResponse {
                success: false,
                message: e.to_string(),
                review: None,
            }),
        }
    }
}