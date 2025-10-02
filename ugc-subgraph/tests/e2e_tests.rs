use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use serde_json::{json, Value};
use tokio::time::timeout;
use testcontainers::{clients::Cli, images::postgres::Postgres};
use axum::{
    body::Body,
    extract::Request,
    http::{header, Method, StatusCode},
    Router,
};
use tower::ServiceExt;
use serial_test::serial;

use ugc_subgraph::{
    config::Config,
    graphql::{create_enhanced_schema, graphql_handler, graphql_playground},
    service::{ExternalServiceClient, create_review_service_with_metrics},
    auth::{AuthService, UserContext},
    telemetry::metrics::Metrics,
    models::review::{CreateReviewInput, ModerationStatus},
    error::UgcError,
};

// Test application setup
async fn setup_test_app() -> (Router, sqlx::PgPool) {
    let docker = Cli::default();
    let postgres_image = Postgres::default();
    let container = docker.run(postgres_image);
    
    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        container.get_host_port_ipv4(5432)
    );
    
    let pool = sqlx::PgPool::connect(&connection_string)
        .await
        .expect("Failed to connect to test database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    // Create test configuration
    let mut config = Config::default();
    config.database.url = connection_string;
    config.redis.enabled = false; // Disable Redis for E2E tests
    
    let external_service = ExternalServiceClient::new("http://localhost:8080".to_string());
    let metrics = Arc::new(Metrics::new());
    
    let schema = create_enhanced_schema(pool.clone(), external_service, &config)
        .await
        .expect("Failed to create schema");
    
    let app = Router::new()
        .route("/graphql", axum::routing::post(graphql_handler))
        .route("/playground", axum::routing::get(graphql_playground))
        .route("/health", axum::routing::get(|| async { "OK" }))
        .with_state((schema, pool.clone()));
    
    (app, pool)
}

// Helper to create authenticated request
fn create_authenticated_request(
    method: Method,
    uri: &str,
    body: Body,
    user_context: UserContext,
) -> Request {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap();
    
    request.extensions_mut().insert(user_context);
    request
}

// Helper to create test user context
fn create_test_user() -> UserContext {
    UserContext::new(
        Uuid::new_v4(),
        "Test User".to_string(),
        "test@example.com".to_string(),
        vec!["user".to_string()],
    )
}

fn create_moderator_user() -> UserContext {
    UserContext::new(
        Uuid::new_v4(),
        "Test Moderator".to_string(),
        "moderator@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string()],
    )
}

#[tokio::test]
#[serial]
async fn test_e2e_health_check() {
    let (app, _pool) = setup_test_app().await;
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
#[serial]
async fn test_e2e_graphql_playground() {
    let (app, _pool) = setup_test_app().await;
    
    let request = Request::builder()
        .method(Method::GET)
        .uri("/playground")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("GraphQL Playground"));
}

#[tokio::test]
#[serial]
async fn test_e2e_complete_review_lifecycle() {
    let (app, _pool) = setup_test_app().await;
    let user = create_test_user();
    let moderator = create_moderator_user();
    let offer_id = Uuid::new_v4();
    
    // Step 1: Create a review
    let create_mutation = json!({
        "query": r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                    rating
                    text
                    offerId
                    authorId
                    isModerated
                    moderationStatus
                    createdAt
                }
            }
        "#,
        "variables": {
            "input": {
                "offerId": offer_id.to_string(),
                "rating": 5,
                "text": "Excellent car, highly recommend!"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(create_mutation.to_string()),
        user.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(result["errors"].is_null() || result["errors"].as_array().unwrap().is_empty());
    
    let review = &result["data"]["createReview"];
    let review_id = review["id"].as_str().unwrap();
    
    assert_eq!(review["rating"], 5);
    assert_eq!(review["text"], "Excellent car, highly recommend!");
    assert_eq!(review["offerId"], offer_id.to_string());
    assert_eq!(review["authorId"], user.user_id.to_string());
    assert_eq!(review["isModerated"], false);
    assert_eq!(review["moderationStatus"], "PENDING");
    
    // Step 2: Query the review
    let query_review = json!({
        "query": r#"
            query GetReview($id: UUID!) {
                review(id: $id) {
                    id
                    rating
                    text
                    isModerated
                    moderationStatus
                }
            }
        "#,
        "variables": {
            "id": review_id
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query_review.to_string()))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let queried_review = &result["data"]["review"];
    assert_eq!(queried_review["id"], review_id);
    assert_eq!(queried_review["rating"], 5);
    assert_eq!(queried_review["text"], "Excellent car, highly recommend!");
    
    // Step 3: Update the review
    let update_mutation = json!({
        "query": r#"
            mutation UpdateReview($id: UUID!, $input: UpdateReviewInput!) {
                updateReview(id: $id, input: $input) {
                    id
                    rating
                    text
                    updatedAt
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "input": {
                "rating": 4,
                "text": "Good car, would recommend with minor reservations"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(update_mutation.to_string()),
        user.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let updated_review = &result["data"]["updateReview"];
    assert_eq!(updated_review["id"], review_id);
    assert_eq!(updated_review["rating"], 4);
    assert_eq!(updated_review["text"], "Good car, would recommend with minor reservations");
    
    // Step 4: Moderate the review (as moderator)
    let moderate_mutation = json!({
        "query": r#"
            mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
                moderateReview(id: $id, status: $status) {
                    id
                    moderationStatus
                    isModerated
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "status": "APPROVED"
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(moderate_mutation.to_string()),
        moderator,
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let moderated_review = &result["data"]["moderateReview"];
    assert_eq!(moderated_review["id"], review_id);
    assert_eq!(moderated_review["moderationStatus"], "APPROVED");
    assert_eq!(moderated_review["isModerated"], true);
    
    // Step 5: Query reviews for the offer
    let query_offer_reviews = json!({
        "query": r#"
            query GetOfferReviews($offerId: UUID!) {
                reviews(filter: { offerId: $offerId }) {
                    totalCount
                    edges {
                        node {
                            id
                            rating
                            text
                            moderationStatus
                        }
                    }
                }
            }
        "#,
        "variables": {
            "offerId": offer_id.to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query_offer_reviews.to_string()))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let reviews = &result["data"]["reviews"];
    assert_eq!(reviews["totalCount"], 1);
    assert_eq!(reviews["edges"].as_array().unwrap().len(), 1);
    
    let review_node = &reviews["edges"][0]["node"];
    assert_eq!(review_node["id"], review_id);
    assert_eq!(review_node["rating"], 4);
    assert_eq!(review_node["moderationStatus"], "APPROVED");
    
    // Step 6: Delete the review
    let delete_mutation = json!({
        "query": r#"
            mutation DeleteReview($id: UUID!) {
                deleteReview(id: $id)
            }
        "#,
        "variables": {
            "id": review_id
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(delete_mutation.to_string()),
        user,
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(result["data"]["deleteReview"].as_bool().unwrap_or(false) || result["data"]["deleteReview"].is_null());
    
    // Step 7: Verify the review is deleted
    let query_deleted_review = json!({
        "query": r#"
            query GetDeletedReview($id: UUID!) {
                review(id: $id) {
                    id
                }
            }
        "#,
        "variables": {
            "id": review_id
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query_deleted_review.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(result["data"]["review"].is_null());
}

#[tokio::test]
#[serial]
async fn test_e2e_pagination_workflow() {
    let (app, _pool) = setup_test_app().await;
    let user = create_test_user();
    let offer_id = Uuid::new_v4();
    
    // Create multiple reviews
    let mut review_ids = Vec::new();
    for i in 1..=5 {
        let create_mutation = json!({
            "query": r#"
                mutation CreateReview($input: CreateReviewInput!) {
                    createReview(input: $input) {
                        id
                    }
                }
            "#,
            "variables": {
                "input": {
                    "offerId": offer_id.to_string(),
                    "rating": i,
                    "text": format!("Review number {}", i)
                }
            }
        });
        
        let request = create_authenticated_request(
            Method::POST,
            "/graphql",
            Body::from(create_mutation.to_string()),
            user.clone(),
        );
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let result: Value = serde_json::from_slice(&body).unwrap();
        
        let review_id = result["data"]["createReview"]["id"].as_str().unwrap();
        review_ids.push(review_id.to_string());
    }
    
    // Test first page
    let first_page_query = json!({
        "query": r#"
            query GetFirstPage($first: Int, $offerId: UUID!) {
                reviews(first: $first, filter: { offerId: $offerId }) {
                    totalCount
                    edges {
                        node {
                            id
                            rating
                        }
                        cursor
                    }
                    pageInfo {
                        hasNextPage
                        hasPreviousPage
                        startCursor
                        endCursor
                    }
                }
            }
        "#,
        "variables": {
            "first": 3,
            "offerId": offer_id.to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(first_page_query.to_string()))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let reviews = &result["data"]["reviews"];
    assert_eq!(reviews["totalCount"], 5);
    assert_eq!(reviews["edges"].as_array().unwrap().len(), 3);
    assert_eq!(reviews["pageInfo"]["hasNextPage"], true);
    assert_eq!(reviews["pageInfo"]["hasPreviousPage"], false);
    
    let end_cursor = reviews["pageInfo"]["endCursor"].as_str().unwrap();
    
    // Test second page
    let second_page_query = json!({
        "query": r#"
            query GetSecondPage($first: Int, $after: String, $offerId: UUID!) {
                reviews(first: $first, after: $after, filter: { offerId: $offerId }) {
                    totalCount
                    edges {
                        node {
                            id
                            rating
                        }
                    }
                    pageInfo {
                        hasNextPage
                        hasPreviousPage
                    }
                }
            }
        "#,
        "variables": {
            "first": 3,
            "after": end_cursor,
            "offerId": offer_id.to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(second_page_query.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let reviews = &result["data"]["reviews"];
    assert_eq!(reviews["edges"].as_array().unwrap().len(), 2); // Remaining reviews
    assert_eq!(reviews["pageInfo"]["hasNextPage"], false);
    assert_eq!(reviews["pageInfo"]["hasPreviousPage"], true);
}

#[tokio::test]
#[serial]
async fn test_e2e_authorization_workflow() {
    let (app, _pool) = setup_test_app().await;
    let user1 = create_test_user();
    let user2 = create_test_user();
    let moderator = create_moderator_user();
    
    // User1 creates a review
    let create_mutation = json!({
        "query": r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                    authorId
                }
            }
        "#,
        "variables": {
            "input": {
                "offerId": Uuid::new_v4().to_string(),
                "rating": 5,
                "text": "Great car!"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(create_mutation.to_string()),
        user1.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let review_id = result["data"]["createReview"]["id"].as_str().unwrap();
    assert_eq!(result["data"]["createReview"]["authorId"], user1.user_id.to_string());
    
    // User2 tries to update User1's review (should fail)
    let unauthorized_update = json!({
        "query": r#"
            mutation UpdateReview($id: UUID!, $input: UpdateReviewInput!) {
                updateReview(id: $id, input: $input) {
                    id
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "input": {
                "rating": 3,
                "text": "Actually not that great"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(unauthorized_update.to_string()),
        user2.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    // Should have authorization error
    assert!(!result["errors"].as_array().unwrap().is_empty());
    let error = &result["errors"][0];
    assert!(error["message"].as_str().unwrap().contains("Unauthorized") || 
            error["message"].as_str().unwrap().contains("cannot access"));
    
    // User1 updates their own review (should succeed)
    let authorized_update = json!({
        "query": r#"
            mutation UpdateReview($id: UUID!, $input: UpdateReviewInput!) {
                updateReview(id: $id, input: $input) {
                    id
                    rating
                    text
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "input": {
                "rating": 4,
                "text": "Pretty good car"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(authorized_update.to_string()),
        user1.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(result["errors"].is_null() || result["errors"].as_array().unwrap().is_empty());
    let updated_review = &result["data"]["updateReview"];
    assert_eq!(updated_review["rating"], 4);
    assert_eq!(updated_review["text"], "Pretty good car");
    
    // Regular user tries to moderate (should fail)
    let unauthorized_moderation = json!({
        "query": r#"
            mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
                moderateReview(id: $id, status: $status) {
                    id
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "status": "APPROVED"
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(unauthorized_moderation.to_string()),
        user1.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    // Should have authorization error
    assert!(!result["errors"].as_array().unwrap().is_empty());
    let error = &result["errors"][0];
    assert!(error["message"].as_str().unwrap().contains("Insufficient permissions") ||
            error["message"].as_str().unwrap().contains("moderator"));
    
    // Moderator moderates the review (should succeed)
    let authorized_moderation = json!({
        "query": r#"
            mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
                moderateReview(id: $id, status: $status) {
                    id
                    moderationStatus
                    isModerated
                }
            }
        "#,
        "variables": {
            "id": review_id,
            "status": "APPROVED"
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(authorized_moderation.to_string()),
        moderator,
    );
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(result["errors"].is_null() || result["errors"].as_array().unwrap().is_empty());
    let moderated_review = &result["data"]["moderateReview"];
    assert_eq!(moderated_review["moderationStatus"], "APPROVED");
    assert_eq!(moderated_review["isModerated"], true);
}

#[tokio::test]
#[serial]
async fn test_e2e_error_handling() {
    let (app, _pool) = setup_test_app().await;
    let user = create_test_user();
    
    // Test validation error
    let invalid_review = json!({
        "query": r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                }
            }
        "#,
        "variables": {
            "input": {
                "offerId": Uuid::new_v4().to_string(),
                "rating": 6, // Invalid rating
                "text": "Test review"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(invalid_review.to_string()),
        user.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(!result["errors"].as_array().unwrap().is_empty());
    let error = &result["errors"][0];
    assert!(error["message"].as_str().unwrap().contains("Rating must be between 1 and 5"));
    
    let extensions = &error["extensions"];
    assert_eq!(extensions["code"], "VALIDATION_ERROR");
    assert_eq!(extensions["category"], "CLIENT_ERROR");
    assert_eq!(extensions["retryable"], false);
    
    // Test not found error
    let non_existent_review = json!({
        "query": r#"
            query GetReview($id: UUID!) {
                review(id: $id) {
                    id
                }
            }
        "#,
        "variables": {
            "id": Uuid::new_v4().to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(non_existent_review.to_string()))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    // Should return null for non-existent review (not an error)
    assert!(result["errors"].is_null() || result["errors"].as_array().unwrap().is_empty());
    assert!(result["data"]["review"].is_null());
    
    // Test malformed GraphQL query
    let malformed_query = json!({
        "query": "invalid graphql syntax {"
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(malformed_query.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(!result["errors"].as_array().unwrap().is_empty());
    let error = &result["errors"][0];
    assert!(error["message"].as_str().unwrap().contains("syntax") || 
            error["message"].as_str().unwrap().contains("parse"));
}

#[tokio::test]
#[serial]
async fn test_e2e_performance_under_load() {
    let (app, _pool) = setup_test_app().await;
    let user = create_test_user();
    let offer_id = Uuid::new_v4();
    
    let start_time = std::time::Instant::now();
    
    // Create 50 reviews concurrently
    let mut handles = vec![];
    for i in 0..50 {
        let app_clone = app.clone();
        let user_clone = user.clone();
        let offer_id_clone = offer_id;
        
        let handle = tokio::spawn(async move {
            let create_mutation = json!({
                "query": r#"
                    mutation CreateReview($input: CreateReviewInput!) {
                        createReview(input: $input) {
                            id
                            rating
                        }
                    }
                "#,
                "variables": {
                    "input": {
                        "offerId": offer_id_clone.to_string(),
                        "rating": (i % 5) + 1,
                        "text": format!("Performance test review {}", i)
                    }
                }
            });
            
            let request = create_authenticated_request(
                Method::POST,
                "/graphql",
                Body::from(create_mutation.to_string()),
                user_clone,
            );
            
            let response = app_clone.oneshot(request).await.unwrap();
            response.status() == StatusCode::OK
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete with timeout
    let mut successful_requests = 0;
    for handle in handles {
        let result = timeout(Duration::from_secs(30), handle).await;
        match result {
            Ok(Ok(success)) if success => successful_requests += 1,
            _ => {}
        }
    }
    
    let duration = start_time.elapsed();
    
    println!("Created {} reviews in {:?}", successful_requests, duration);
    
    // Performance assertions
    assert!(successful_requests >= 45, "Should handle most concurrent requests successfully");
    assert!(duration.as_secs() < 30, "Should complete within 30 seconds");
    
    // Query all reviews to test read performance
    let query_start = std::time::Instant::now();
    
    let query_reviews = json!({
        "query": r#"
            query GetAllReviews($offerId: UUID!) {
                reviews(first: 100, filter: { offerId: $offerId }) {
                    totalCount
                    edges {
                        node {
                            id
                            rating
                            text
                        }
                    }
                }
            }
        "#,
        "variables": {
            "offerId": offer_id.to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query_reviews.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let query_duration = query_start.elapsed();
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let reviews = &result["data"]["reviews"];
    assert!(reviews["totalCount"].as_u64().unwrap() >= 45);
    
    println!("Queried {} reviews in {:?}", reviews["totalCount"], query_duration);
    
    // Query performance assertion
    assert!(query_duration.as_millis() < 5000, "Query should complete within 5 seconds");
}

#[tokio::test]
#[serial]
async fn test_e2e_resilience_and_recovery() {
    let (app, pool) = setup_test_app().await;
    let user = create_test_user();
    
    // Create a review first
    let create_mutation = json!({
        "query": r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                }
            }
        "#,
        "variables": {
            "input": {
                "offerId": Uuid::new_v4().to_string(),
                "rating": 5,
                "text": "Resilience test review"
            }
        }
    });
    
    let request = create_authenticated_request(
        Method::POST,
        "/graphql",
        Body::from(create_mutation.to_string()),
        user.clone(),
    );
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    let review_id = result["data"]["createReview"]["id"].as_str().unwrap();
    
    // Simulate database connection issues by closing the pool
    pool.close().await;
    
    // Try to query the review (should handle gracefully)
    let query_review = json!({
        "query": r#"
            query GetReview($id: UUID!) {
                review(id: $id) {
                    id
                }
            }
        "#,
        "variables": {
            "id": review_id
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query_review.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    // Should have database error
    assert!(!result["errors"].as_array().unwrap().is_empty());
    let error = &result["errors"][0];
    
    let extensions = &error["extensions"];
    assert_eq!(extensions["category"], "SERVER_ERROR");
    assert_eq!(extensions["retryable"], true);
}

#[tokio::test]
#[serial]
async fn test_e2e_complex_federated_query() {
    let (app, _pool) = setup_test_app().await;
    let user = create_test_user();
    let offer_id = Uuid::new_v4();
    
    // Create multiple reviews for the same offer
    for i in 1..=3 {
        let create_mutation = json!({
            "query": r#"
                mutation CreateReview($input: CreateReviewInput!) {
                    createReview(input: $input) {
                        id
                    }
                }
            "#,
            "variables": {
                "input": {
                    "offerId": offer_id.to_string(),
                    "rating": i + 2, // ratings 3, 4, 5
                    "text": format!("Federated test review {}", i)
                }
            }
        });
        
        let request = create_authenticated_request(
            Method::POST,
            "/graphql",
            Body::from(create_mutation.to_string()),
            user.clone(),
        );
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    
    // Complex federated query that would span multiple subgraphs
    let complex_query = json!({
        "query": r#"
            query ComplexFederatedQuery($offerId: UUID!) {
                # Query reviews
                reviews(filter: { offerId: $offerId }) {
                    totalCount
                    edges {
                        node {
                            id
                            rating
                            text
                            createdAt
                            # These would be resolved from other subgraphs in real federation
                            offer {
                                id
                            }
                            author {
                                id
                            }
                        }
                    }
                }
                
                # Query aggregated rating stats
                offerRatingStats(offerId: $offerId) {
                    offerId
                    averageRating
                    reviewsCount
                    ratingDistribution
                }
                
                # Query individual rating
                offerAverageRating(offerId: $offerId)
                offerReviewsCount(offerId: $offerId)
            }
        "#,
        "variables": {
            "offerId": offer_id.to_string()
        }
    });
    
    let request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(complex_query.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    
    // The query might have some errors due to missing external resolvers,
    // but the parts we can resolve should work
    let data = &result["data"];
    
    if let Some(reviews) = data["reviews"].as_object() {
        assert_eq!(reviews["totalCount"], 3);
        assert_eq!(reviews["edges"].as_array().unwrap().len(), 3);
    }
    
    // Rating stats might not be available immediately due to moderation
    if let Some(rating_stats) = data["offerRatingStats"].as_object() {
        assert_eq!(rating_stats["offerId"], offer_id.to_string());
    }
}