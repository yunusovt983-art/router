use std::sync::Arc;
use uuid::Uuid;
use serde_json::{json, Value};
use wiremock::{
    matchers::{method, path, header, body_json},
    Mock, MockServer, ResponseTemplate,
};
use async_graphql::{Schema, EmptySubscription, Request, Variables};

use ugc_subgraph::{
    service::{ExternalServiceClient, ReviewService},
    repository::MockReviewRepository,
    graphql::{Query, Mutation},
    models::{
        review::{CreateReviewInput, Review, ModerationStatus},
        offer_rating::OfferRating,
    },
    error::UgcError,
    auth::UserContext,
};

// Contract tests for external services
#[tokio::test]
async fn test_users_service_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    let expected_user = json!({
        "id": user_id.to_string(),
        "name": "John Doe",
        "email": "john@example.com",
        "isActive": true
    });
    
    // Set up the expected contract
    Mock::given(method("GET"))
        .and(path(format!("/users/{}", user_id)))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(&expected_user))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    // Test the contract
    let user = client.get_user(user_id).await
        .expect("Should get user from external service");
    
    assert_eq!(user.id, user_id);
    assert_eq!(user.name, "John Doe");
    assert_eq!(user.email, "john@example.com");
    assert!(user.is_active);
}

#[tokio::test]
async fn test_users_service_not_found_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    
    // Set up the expected contract for not found
    Mock::given(method("GET"))
        .and(path(format!("/users/{}", user_id)))
        .respond_with(ResponseTemplate::new(404)
            .set_body_json(json!({
                "error": "User not found",
                "code": "USER_NOT_FOUND"
            })))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    // Test the contract
    let result = client.get_user(user_id).await;
    assert!(result.is_err());
    
    match result.unwrap_err() {
        UgcError::ExternalServiceError { service, message } => {
            assert_eq!(service, "users");
            assert!(message.contains("not found") || message.contains("404"));
        }
        _ => panic!("Expected ExternalServiceError"),
    }
}

#[tokio::test]
async fn test_offers_service_contract() {
    let mock_server = MockServer::start().await;
    
    let offer_id = Uuid::new_v4();
    let expected_offer = json!({
        "id": offer_id.to_string(),
        "title": "2020 Toyota Camry",
        "price": 25000,
        "currency": "USD",
        "description": "Excellent condition, low mileage",
        "sellerId": Uuid::new_v4().to_string(),
        "isActive": true,
        "createdAt": "2023-01-01T00:00:00Z"
    });
    
    // Set up the expected contract
    Mock::given(method("GET"))
        .and(path(format!("/offers/{}", offer_id)))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(&expected_offer))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    // Test the contract
    let offer = client.get_offer(offer_id).await
        .expect("Should get offer from external service");
    
    assert_eq!(offer.id, offer_id);
    assert_eq!(offer.title, "2020 Toyota Camry");
    assert_eq!(offer.price, 25000);
    assert_eq!(offer.currency, "USD");
    assert!(offer.is_active);
}

#[tokio::test]
async fn test_offers_service_batch_contract() {
    let mock_server = MockServer::start().await;
    
    let offer_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let expected_offers = json!([
        {
            "id": offer_ids[0].to_string(),
            "title": "2020 Toyota Camry",
            "price": 25000,
            "currency": "USD",
            "isActive": true
        },
        {
            "id": offer_ids[1].to_string(),
            "title": "2019 Honda Civic",
            "price": 22000,
            "currency": "USD",
            "isActive": true
        },
        {
            "id": offer_ids[2].to_string(),
            "title": "2021 Ford F-150",
            "price": 35000,
            "currency": "USD",
            "isActive": false
        }
    ]);
    
    // Set up the expected contract for batch request
    Mock::given(method("POST"))
        .and(path("/offers/batch"))
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "ids": offer_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
        })))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(&expected_offers))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    // Test the contract
    let offers = client.get_offers_batch(offer_ids.clone()).await
        .expect("Should get offers from external service");
    
    assert_eq!(offers.len(), 3);
    assert_eq!(offers[0].id, offer_ids[0]);
    assert_eq!(offers[1].id, offer_ids[1]);
    assert_eq!(offers[2].id, offer_ids[2]);
    assert!(offers[0].is_active);
    assert!(offers[1].is_active);
    assert!(!offers[2].is_active);
}

#[tokio::test]
async fn test_external_service_timeout_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    
    // Set up a slow response to test timeout handling
    Mock::given(method("GET"))
        .and(path(format!("/users/{}", user_id)))
        .respond_with(ResponseTemplate::new(200)
            .set_delay(std::time::Duration::from_secs(10)) // 10 second delay
            .set_body_json(json!({
                "id": user_id.to_string(),
                "name": "Slow User"
            })))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri())
        .with_timeout(std::time::Duration::from_secs(1)); // 1 second timeout
    
    // Test the timeout contract
    let result = client.get_user(user_id).await;
    assert!(result.is_err());
    
    match result.unwrap_err() {
        UgcError::ServiceTimeout { service } => {
            assert_eq!(service, "users");
        }
        _ => panic!("Expected ServiceTimeout error"),
    }
}

#[tokio::test]
async fn test_external_service_error_response_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    
    // Set up error response contract
    Mock::given(method("GET"))
        .and(path(format!("/users/{}", user_id)))
        .respond_with(ResponseTemplate::new(500)
            .set_body_json(json!({
                "error": "Internal server error",
                "code": "INTERNAL_ERROR",
                "timestamp": "2023-01-01T00:00:00Z"
            })))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    // Test the error contract
    let result = client.get_user(user_id).await;
    assert!(result.is_err());
    
    match result.unwrap_err() {
        UgcError::ExternalServiceError { service, message } => {
            assert_eq!(service, "users");
            assert!(message.contains("500") || message.contains("Internal server error"));
        }
        _ => panic!("Expected ExternalServiceError"),
    }
}

// GraphQL Schema Contract Tests
#[tokio::test]
async fn test_graphql_schema_compatibility() {
    let mut mock_repo = MockReviewRepository::new();
    
    // Set up mock expectations (minimal for schema testing)
    mock_repo.expect_get_review_by_id()
        .returning(|_| Ok(None));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    // Test that required types exist in schema
    let introspection_query = r#"
        query {
            __schema {
                types {
                    name
                    kind
                    fields {
                        name
                        type {
                            name
                            kind
                        }
                    }
                }
            }
        }
    "#;
    
    let result = schema.execute(introspection_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let types = data["__schema"]["types"].as_array().unwrap();
    
    // Check that required types exist
    let type_names: Vec<&str> = types
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    
    // Core types
    assert!(type_names.contains(&"Review"));
    assert!(type_names.contains(&"ReviewConnection"));
    assert!(type_names.contains(&"ReviewEdge"));
    assert!(type_names.contains(&"PageInfo"));
    assert!(type_names.contains(&"OfferRatingStats"));
    assert!(type_names.contains(&"ModerationStatus"));
    
    // Input types
    assert!(type_names.contains(&"CreateReviewInput"));
    assert!(type_names.contains(&"UpdateReviewInput"));
    assert!(type_names.contains(&"ReviewsFilterInput"));
    
    // Federation types
    assert!(type_names.contains(&"_Entity"));
    assert!(type_names.contains(&"_Service"));
}

#[tokio::test]
async fn test_graphql_federation_directives() {
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_get_review_by_id()
        .returning(|_| Ok(None));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    // Test federation service SDL
    let service_query = r#"
        query {
            _service {
                sdl
            }
        }
    "#;
    
    let result = schema.execute(service_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let sdl = data["_service"]["sdl"].as_str().unwrap();
    
    // Check that federation directives are present
    assert!(sdl.contains("@key"));
    assert!(sdl.contains("@extends"));
    assert!(sdl.contains("@external"));
    
    // Check that our types are properly federated
    assert!(sdl.contains("type Review @key"));
    assert!(sdl.contains("extend type Offer"));
    assert!(sdl.contains("extend type User"));
}

#[tokio::test]
async fn test_graphql_query_complexity_limits() {
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_get_reviews_with_pagination()
        .returning(|_, _, _| Ok((vec![], 0)));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    // Test a complex nested query
    let complex_query = r#"
        query ComplexQuery {
            reviews(first: 100) {
                edges {
                    node {
                        id
                        rating
                        text
                        offer {
                            id
                            title
                            reviews(first: 100) {
                                edges {
                                    node {
                                        id
                                        rating
                                        author {
                                            id
                                            name
                                            reviews(first: 100) {
                                                edges {
                                                    node {
                                                        id
                                                        rating
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    "#;
    
    let result = schema.execute(complex_query).await;
    
    // This should either succeed or fail with complexity/depth limits
    // The exact behavior depends on the query limits configuration
    if !result.errors.is_empty() {
        let error = &result.errors[0];
        assert!(
            error.message.contains("complexity") || 
            error.message.contains("depth") ||
            error.message.contains("limit")
        );
    }
}

#[tokio::test]
async fn test_graphql_input_validation_contract() {
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_create_review()
        .returning(|_, _| Err(UgcError::ValidationError { 
            message: "Rating must be between 1 and 5".to_string() 
        }));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .data(UserContext::new(
            Uuid::new_v4(),
            "Test User".to_string(),
            "test@example.com".to_string(),
            vec!["user".to_string()],
        ))
        .finish();
    
    // Test input validation contract
    let mutation = r#"
        mutation CreateReview($input: CreateReviewInput!) {
            createReview(input: $input) {
                id
                rating
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "input": {
            "offerId": Uuid::new_v4().to_string(),
            "rating": 6, // Invalid rating
            "text": "Test review"
        }
    }));
    
    let request = Request::new(mutation).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(!result.errors.is_empty());
    let error = &result.errors[0];
    assert!(error.message.contains("Rating must be between 1 and 5"));
    
    // Check error extensions contract
    let extensions = &error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("VALIDATION_ERROR".to_string())));
    assert_eq!(extensions.get("category"), Some(&Value::String("CLIENT_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(false)));
}

#[tokio::test]
async fn test_graphql_pagination_contract() {
    let mut mock_repo = MockReviewRepository::new();
    
    // Mock data for pagination test
    let test_reviews = vec![
        create_mock_review(),
        create_mock_review(),
        create_mock_review(),
    ];
    
    mock_repo.expect_get_reviews_with_pagination()
        .returning(move |_, _, _| Ok((test_reviews.clone(), 10)));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    // Test pagination contract
    let query = r#"
        query GetReviews($first: Int, $after: String) {
            reviews(first: $first, after: $after) {
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
                totalCount
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "first": 5
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let reviews = &data["reviews"];
    
    // Verify pagination contract structure
    assert!(reviews["edges"].is_array());
    assert!(reviews["pageInfo"].is_object());
    assert!(reviews["totalCount"].is_number());
    
    let page_info = &reviews["pageInfo"];
    assert!(page_info["hasNextPage"].is_boolean());
    assert!(page_info["hasPreviousPage"].is_boolean());
    
    // Verify edge structure
    let edges = reviews["edges"].as_array().unwrap();
    if !edges.is_empty() {
        let first_edge = &edges[0];
        assert!(first_edge["node"].is_object());
        assert!(first_edge["cursor"].is_string());
    }
}

#[tokio::test]
async fn test_graphql_error_format_contract() {
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_get_review_by_id()
        .returning(|_| Err(UgcError::DatabaseError("Connection failed".to_string())));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    let query = r#"
        query GetReview($id: UUID!) {
            review(id: $id) {
                id
                rating
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "id": Uuid::new_v4().to_string()
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(!result.errors.is_empty());
    
    let error = &result.errors[0];
    
    // Verify error format contract
    assert!(!error.message.is_empty());
    assert!(error.path.is_some());
    
    // Verify error extensions contract
    let extensions = &error.extensions;
    assert!(extensions.contains_key("code"));
    assert!(extensions.contains_key("category"));
    assert!(extensions.contains_key("retryable"));
    
    assert_eq!(extensions.get("category"), Some(&Value::String("SERVER_ERROR".to_string())));
    assert_eq!(extensions.get("retryable"), Some(&Value::Bool(true)));
}

// Breaking change detection tests
#[tokio::test]
async fn test_schema_breaking_changes_detection() {
    // This test would typically compare against a stored schema version
    // For now, we'll test that required fields are present
    
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_get_review_by_id()
        .returning(|_| Ok(None));
    
    let service = ReviewService::new(Arc::new(mock_repo));
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish();
    
    let type_query = r#"
        query {
            __type(name: "Review") {
                name
                fields {
                    name
                    type {
                        name
                        kind
                        ofType {
                            name
                            kind
                        }
                    }
                    isDeprecated
                }
            }
        }
    "#;
    
    let result = schema.execute(type_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let review_type = &data["__type"];
    
    assert_eq!(review_type["name"], "Review");
    
    let fields = review_type["fields"].as_array().unwrap();
    let field_names: Vec<&str> = fields
        .iter()
        .filter_map(|f| f["name"].as_str())
        .collect();
    
    // Verify that required fields are present (breaking change if removed)
    let required_fields = vec![
        "id", "offerId", "authorId", "rating", "text", 
        "createdAt", "updatedAt", "isModerated", "moderationStatus"
    ];
    
    for required_field in required_fields {
        assert!(
            field_names.contains(&required_field),
            "Required field '{}' is missing from Review type",
            required_field
        );
    }
    
    // Check that no fields are deprecated (would be a potential breaking change)
    for field in fields {
        assert_eq!(field["isDeprecated"], false, 
                  "Field '{}' is deprecated", field["name"]);
    }
}

// Helper function to create mock review
fn create_mock_review() -> Review {
    Review {
        id: Uuid::new_v4(),
        offer_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        rating: 5,
        text: "Great car!".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_moderated: true,
        moderation_status: ModerationStatus::Approved,
    }
}

// Contract test for external service circuit breaker
#[tokio::test]
async fn test_circuit_breaker_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    
    // Set up multiple failing responses to trigger circuit breaker
    for _ in 0..5 {
        Mock::given(method("GET"))
            .and(path(format!("/users/{}", user_id)))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
    }
    
    let client = ExternalServiceClient::new(mock_server.uri())
        .with_circuit_breaker_config(
            3, // failure_threshold
            std::time::Duration::from_secs(60), // timeout
        );
    
    // Make requests until circuit breaker opens
    for i in 0..5 {
        let result = client.get_user(user_id).await;
        
        if i < 3 {
            // First few requests should fail with service error
            assert!(matches!(result, Err(UgcError::ExternalServiceError { .. })));
        } else {
            // Later requests should fail with circuit breaker error
            assert!(matches!(result, Err(UgcError::CircuitBreakerOpen { .. })));
        }
    }
}

// Contract test for rate limiting
#[tokio::test]
async fn test_rate_limiting_contract() {
    let mock_server = MockServer::start().await;
    
    let user_id = Uuid::new_v4();
    
    // Set up rate limit response
    Mock::given(method("GET"))
        .and(path(format!("/users/{}", user_id)))
        .respond_with(ResponseTemplate::new(429)
            .set_body_json(json!({
                "error": "Rate limit exceeded",
                "code": "RATE_LIMIT_EXCEEDED",
                "retryAfter": 60
            })))
        .mount(&mock_server)
        .await;
    
    let client = ExternalServiceClient::new(mock_server.uri());
    
    let result = client.get_user(user_id).await;
    assert!(result.is_err());
    
    match result.unwrap_err() {
        UgcError::ExternalServiceError { service, message } => {
            assert_eq!(service, "users");
            assert!(message.contains("429") || message.contains("Rate limit"));
        }
        _ => panic!("Expected ExternalServiceError for rate limit"),
    }
}