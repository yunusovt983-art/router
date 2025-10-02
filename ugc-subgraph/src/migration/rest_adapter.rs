use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, instrument, error};
use uuid::Uuid;

use crate::graphql::Schema;
use crate::models::{Review, CreateReviewInput, UpdateReviewInput};
use crate::service::ReviewService;
use crate::auth::UserContext;
use crate::migration::feature_flags::FeatureFlagService;
use crate::migration::monitoring::MigrationMetrics;

/// REST API adapter that provides backward compatibility
/// while gradually migrating to GraphQL
#[derive(Clone)]
pub struct RestAdapter {
    schema: Schema,
    feature_flags: Arc<FeatureFlagService>,
    metrics: Arc<MigrationMetrics>,
}

impl RestAdapter {
    pub fn new(
        schema: Schema,
        feature_flags: Arc<FeatureFlagService>,
        metrics: Arc<MigrationMetrics>,
    ) -> Self {
        Self {
            schema,
            feature_flags,
            metrics,
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/api/v1/reviews", get(Self::get_reviews).post(Self::create_review))
            .route("/api/v1/reviews/:id", get(Self::get_review).put(Self::update_review).delete(Self::delete_review))
            .route("/api/v1/offers/:offer_id/reviews", get(Self::get_offer_reviews))
            .route("/api/v1/users/:user_id/reviews", get(Self::get_user_reviews))
            .route("/api/v1/offers/:offer_id/rating", get(Self::get_offer_rating))
            .with_state(self.clone())
    }

    #[instrument(skip(adapter))]
    async fn get_reviews(
        State(adapter): State<RestAdapter>,
        Query(params): Query<ReviewsQueryParams>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["GET", "/api/v1/reviews"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        // Check if GraphQL migration is enabled for this user
        if adapter.feature_flags.is_enabled("graphql_reviews_read", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["reviews", "read"]).inc();
            return Self::get_reviews_via_graphql(adapter, params, user_context).await;
        }

        // Fallback to legacy REST implementation
        adapter.metrics.legacy_rest_requests.with_label_values(&["reviews", "read"]).inc();
        Self::get_reviews_legacy(adapter, params, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn create_review(
        State(adapter): State<RestAdapter>,
        headers: HeaderMap,
        Json(payload): Json<CreateReviewRequest>,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["POST", "/api/v1/reviews"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        // Check if GraphQL migration is enabled for this user
        if adapter.feature_flags.is_enabled("graphql_reviews_write", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["reviews", "write"]).inc();
            return Self::create_review_via_graphql(adapter, payload, user_context).await;
        }

        // Fallback to legacy REST implementation
        adapter.metrics.legacy_rest_requests.with_label_values(&["reviews", "write"]).inc();
        Self::create_review_legacy(adapter, payload, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn get_review(
        State(adapter): State<RestAdapter>,
        Path(id): Path<Uuid>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["GET", "/api/v1/reviews/:id"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_read", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["review", "read"]).inc();
            return Self::get_review_via_graphql(adapter, id, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["review", "read"]).inc();
        Self::get_review_legacy(adapter, id, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn update_review(
        State(adapter): State<RestAdapter>,
        Path(id): Path<Uuid>,
        headers: HeaderMap,
        Json(payload): Json<UpdateReviewRequest>,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["PUT", "/api/v1/reviews/:id"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_write", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["review", "update"]).inc();
            return Self::update_review_via_graphql(adapter, id, payload, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["review", "update"]).inc();
        Self::update_review_legacy(adapter, id, payload, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn delete_review(
        State(adapter): State<RestAdapter>,
        Path(id): Path<Uuid>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<()>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["DELETE", "/api/v1/reviews/:id"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_write", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["review", "delete"]).inc();
            return Self::delete_review_via_graphql(adapter, id, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["review", "delete"]).inc();
        Self::delete_review_legacy(adapter, id, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn get_offer_reviews(
        State(adapter): State<RestAdapter>,
        Path(offer_id): Path<Uuid>,
        Query(params): Query<PaginationParams>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["GET", "/api/v1/offers/:offer_id/reviews"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_read", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["offer_reviews", "read"]).inc();
            return Self::get_offer_reviews_via_graphql(adapter, offer_id, params, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["offer_reviews", "read"]).inc();
        Self::get_offer_reviews_legacy(adapter, offer_id, params, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn get_user_reviews(
        State(adapter): State<RestAdapter>,
        Path(user_id): Path<Uuid>,
        Query(params): Query<PaginationParams>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["GET", "/api/v1/users/:user_id/reviews"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_read", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["user_reviews", "read"]).inc();
            return Self::get_user_reviews_via_graphql(adapter, user_id, params, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["user_reviews", "read"]).inc();
        Self::get_user_reviews_legacy(adapter, user_id, params, user_context).await
    }

    #[instrument(skip(adapter))]
    async fn get_offer_rating(
        State(adapter): State<RestAdapter>,
        Path(offer_id): Path<Uuid>,
        headers: HeaderMap,
    ) -> Result<Json<RestResponse<OfferRatingResponse>>, StatusCode> {
        adapter.metrics.rest_request_total.with_label_values(&["GET", "/api/v1/offers/:offer_id/rating"]).inc();
        
        let user_context = extract_user_context(&headers)?;
        
        if adapter.feature_flags.is_enabled("graphql_reviews_read", &user_context.user_id.to_string()).await {
            adapter.metrics.graphql_migration_requests.with_label_values(&["offer_rating", "read"]).inc();
            return Self::get_offer_rating_via_graphql(adapter, offer_id, user_context).await;
        }

        adapter.metrics.legacy_rest_requests.with_label_values(&["offer_rating", "read"]).inc();
        Self::get_offer_rating_legacy(adapter, offer_id, user_context).await
    }

    // GraphQL-based implementations
    async fn get_reviews_via_graphql(
        adapter: RestAdapter,
        params: ReviewsQueryParams,
        user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        let query = r#"
            query GetReviews($first: Int, $after: String, $offerId: ID, $authorId: ID) {
                reviews(first: $first, after: $after, offerId: $offerId, authorId: $authorId) {
                    edges {
                        node {
                            id
                            offerId
                            authorId
                            rating
                            text
                            createdAt
                            updatedAt
                            isModerated
                        }
                    }
                    pageInfo {
                        hasNextPage
                        endCursor
                    }
                }
            }
        "#;

        let variables = json!({
            "first": params.limit.unwrap_or(20),
            "after": params.cursor,
            "offerId": params.offer_id,
            "authorId": params.author_id
        });

        match execute_graphql_query(&adapter.schema, query, variables, user_context).await {
            Ok(result) => {
                let reviews: Vec<ReviewResponse> = result["data"]["reviews"]["edges"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|edge| ReviewResponse::from_graphql_node(&edge["node"]))
                    .collect();

                Ok(Json(RestResponse::success(reviews)))
            }
            Err(e) => {
                error!("GraphQL query failed: {}", e);
                adapter.metrics.graphql_errors.with_label_values(&["reviews", "read"]).inc();
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    async fn create_review_via_graphql(
        adapter: RestAdapter,
        payload: CreateReviewRequest,
        user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        let mutation = r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                    offerId
                    authorId
                    rating
                    text
                    createdAt
                    updatedAt
                    isModerated
                }
            }
        "#;

        let variables = json!({
            "input": {
                "offerId": payload.offer_id,
                "rating": payload.rating,
                "text": payload.text
            }
        });

        match execute_graphql_query(&adapter.schema, mutation, variables, user_context).await {
            Ok(result) => {
                let review = ReviewResponse::from_graphql_node(&result["data"]["createReview"]);
                Ok(Json(RestResponse::success(review)))
            }
            Err(e) => {
                error!("GraphQL mutation failed: {}", e);
                adapter.metrics.graphql_errors.with_label_values(&["review", "create"]).inc();
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    // Legacy REST implementations (stubs for backward compatibility)
    async fn get_reviews_legacy(
        _adapter: RestAdapter,
        _params: ReviewsQueryParams,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        warn!("Using legacy REST implementation for get_reviews");
        // This would contain the original REST logic
        Ok(Json(RestResponse::success(vec![])))
    }

    async fn create_review_legacy(
        _adapter: RestAdapter,
        _payload: CreateReviewRequest,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        warn!("Using legacy REST implementation for create_review");
        // This would contain the original REST logic
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn get_review_legacy(
        _adapter: RestAdapter,
        _id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        warn!("Using legacy REST implementation for get_review");
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn update_review_legacy(
        _adapter: RestAdapter,
        _id: Uuid,
        _payload: UpdateReviewRequest,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        warn!("Using legacy REST implementation for update_review");
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn delete_review_legacy(
        _adapter: RestAdapter,
        _id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<()>>, StatusCode> {
        warn!("Using legacy REST implementation for delete_review");
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn get_offer_reviews_legacy(
        _adapter: RestAdapter,
        _offer_id: Uuid,
        _params: PaginationParams,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        warn!("Using legacy REST implementation for get_offer_reviews");
        Ok(Json(RestResponse::success(vec![])))
    }

    async fn get_user_reviews_legacy(
        _adapter: RestAdapter,
        _user_id: Uuid,
        _params: PaginationParams,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        warn!("Using legacy REST implementation for get_user_reviews");
        Ok(Json(RestResponse::success(vec![])))
    }

    async fn get_offer_rating_legacy(
        _adapter: RestAdapter,
        _offer_id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<OfferRatingResponse>>, StatusCode> {
        warn!("Using legacy REST implementation for get_offer_rating");
        Ok(Json(RestResponse::success(OfferRatingResponse {
            offer_id: _offer_id,
            average_rating: 0.0,
            reviews_count: 0,
            rating_distribution: HashMap::new(),
        })))
    }

    // Additional GraphQL implementations for other endpoints...
    async fn get_review_via_graphql(
        _adapter: RestAdapter,
        _id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        // Implementation similar to get_reviews_via_graphql but for single review
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn update_review_via_graphql(
        _adapter: RestAdapter,
        _id: Uuid,
        _payload: UpdateReviewRequest,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<ReviewResponse>>, StatusCode> {
        // Implementation for update via GraphQL
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn delete_review_via_graphql(
        _adapter: RestAdapter,
        _id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<()>>, StatusCode> {
        // Implementation for delete via GraphQL
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn get_offer_reviews_via_graphql(
        _adapter: RestAdapter,
        _offer_id: Uuid,
        _params: PaginationParams,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        // Implementation for offer reviews via GraphQL
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn get_user_reviews_via_graphql(
        _adapter: RestAdapter,
        _user_id: Uuid,
        _params: PaginationParams,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<Vec<ReviewResponse>>>, StatusCode> {
        // Implementation for user reviews via GraphQL
        Err(StatusCode::NOT_IMPLEMENTED)
    }

    async fn get_offer_rating_via_graphql(
        _adapter: RestAdapter,
        _offer_id: Uuid,
        _user_context: UserContext,
    ) -> Result<Json<RestResponse<OfferRatingResponse>>, StatusCode> {
        // Implementation for offer rating via GraphQL
        Err(StatusCode::NOT_IMPLEMENTED)
    }
}

// Helper function to execute GraphQL queries
async fn execute_graphql_query(
    _schema: &Schema,
    _query: &str,
    _variables: Value,
    _user_context: UserContext,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // This would execute the GraphQL query using the schema
    // For now, return a mock response
    Ok(json!({
        "data": {
            "reviews": {
                "edges": [],
                "pageInfo": {
                    "hasNextPage": false,
                    "endCursor": null
                }
            }
        }
    }))
}

// Helper function to extract user context from headers
fn extract_user_context(headers: &HeaderMap) -> Result<UserContext, StatusCode> {
    // This would extract and validate the JWT token from headers
    // For now, return a mock user context
    Ok(UserContext {
        user_id: Uuid::new_v4(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        roles: vec!["user".to_string()],
    })
}

// Request/Response DTOs
#[derive(Debug, Deserialize)]
pub struct ReviewsQueryParams {
    pub limit: Option<i32>,
    pub cursor: Option<String>,
    pub offer_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<i32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReviewRequest {
    pub offer_id: Uuid,
    pub rating: i32,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReviewRequest {
    pub rating: Option<i32>,
    pub text: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    pub id: Uuid,
    pub offer_id: Uuid,
    pub author_id: Uuid,
    pub rating: i32,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_moderated: bool,
}

impl ReviewResponse {
    fn from_graphql_node(node: &Value) -> Self {
        Self {
            id: Uuid::parse_str(node["id"].as_str().unwrap_or("")).unwrap_or_default(),
            offer_id: Uuid::parse_str(node["offerId"].as_str().unwrap_or("")).unwrap_or_default(),
            author_id: Uuid::parse_str(node["authorId"].as_str().unwrap_or("")).unwrap_or_default(),
            rating: node["rating"].as_i64().unwrap_or(0) as i32,
            text: node["text"].as_str().unwrap_or("").to_string(),
            created_at: node["createdAt"].as_str().unwrap_or("").to_string(),
            updated_at: node["updatedAt"].as_str().unwrap_or("").to_string(),
            is_moderated: node["isModerated"].as_bool().unwrap_or(false),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OfferRatingResponse {
    pub offer_id: Uuid,
    pub average_rating: f64,
    pub reviews_count: i32,
    pub rating_distribution: HashMap<i32, i32>,
}

#[derive(Debug, Serialize)]
pub struct RestResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: String,
}

impl<T> RestResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}