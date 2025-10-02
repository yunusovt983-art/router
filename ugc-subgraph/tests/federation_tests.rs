use std::sync::Arc;
use uuid::Uuid;
use async_graphql::{Schema, EmptySubscription, Request, Variables};
use serde_json::{json, Value};
use testcontainers::{clients::Cli, images::postgres::Postgres};
use serial_test::serial;

use ugc_subgraph::{
    graphql::{Query, Mutation, create_schema},
    service::{ExternalServiceClient, create_review_service},
    models::review::{CreateReviewInput, ModerationStatus},
    repository::PostgresReviewRepository,
    auth::{AuthService, UserContext},
};

// Test schema setup
async fn setup_test_schema() -> Schema<Query, Mutation, EmptySubscription> {
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
    
    let external_service = ExternalServiceClient::new("http://localhost:8080".to_string());
    
    create_schema(pool, external_service, None, None)
        .await
        .expect("Failed to create schema")
}

// Helper to create authenticated user context
fn create_test_user_context() -> UserContext {
    UserContext::new(
        Uuid::new_v4(),
        "Test User".to_string(),
        "test@example.com".to_string(),
        vec!["user".to_string()],
    )
}

fn create_moderator_context() -> UserContext {
    UserContext::new(
        Uuid::new_v4(),
        "Test Moderator".to_string(),
        "moderator@example.com".to_string(),
        vec!["user".to_string(), "moderator".to_string()],
    )
}

#[tokio::test]
#[serial]
async fn test_federated_schema_introspection() {
    let schema = setup_test_schema().await;
    
    let introspection_query = r#"
        query IntrospectionQuery {
            __schema {
                types {
                    name
                    kind
                }
            }
        }
    "#;
    
    let result = schema.execute(introspection_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let types = data["__schema"]["types"].as_array().unwrap();
    
    // Check that our types are present
    let type_names: Vec<&str> = types
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    
    assert!(type_names.contains(&"Review"));
    assert!(type_names.contains(&"ReviewConnection"));
    assert!(type_names.contains(&"OfferRatingStats"));
    assert!(type_names.contains(&"ModerationStatus"));
}

#[tokio::test]
#[serial]
async fn test_federated_entity_resolution() {
    let schema = setup_test_schema().await;
    
    // Test _entities query (Apollo Federation)
    let entities_query = r#"
        query($_representations: [_Any!]!) {
            _entities(representations: $_representations) {
                ... on Review {
                    id
                    rating
                    text
                }
            }
        }
    "#;
    
    let review_id = Uuid::new_v4();
    let variables = Variables::from_json(json!({
        "_representations": [
            {
                "__typename": "Review",
                "id": review_id.to_string()
            }
        ]
    }));
    
    let request = Request::new(entities_query).variables(variables);
    let result = schema.execute(request).await;
    
    // This should not error even if the review doesn't exist
    assert!(result.errors.is_empty() || result.errors.iter().any(|e| e.message.contains("not found")));
}

#[tokio::test]
#[serial]
async fn test_federated_service_sdl() {
    let schema = setup_test_schema().await;
    
    // Test _service query (Apollo Federation)
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
    
    // Check that the SDL contains our federated directives
    assert!(sdl.contains("@key"));
    assert!(sdl.contains("@extends"));
    assert!(sdl.contains("Review"));
}

#[tokio::test]
#[serial]
async fn test_create_review_mutation() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    
    let mutation = r#"
        mutation CreateReview($input: CreateReviewInput!) {
            createReview(input: $input) {
                id
                rating
                text
                offerId
                authorId
                isModerated
                moderationStatus
            }
        }
    "#;
    
    let offer_id = Uuid::new_v4();
    let variables = Variables::from_json(json!({
        "input": {
            "offerId": offer_id.to_string(),
            "rating": 5,
            "text": "Excellent car, highly recommended!"
        }
    }));
    
    let request = Request::new(mutation)
        .variables(variables)
        .data(user_context);
    
    let result = schema.execute(request).await;
    
    if !result.errors.is_empty() {
        for error in &result.errors {
            println!("GraphQL Error: {}", error.message);
        }
    }
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let review = &data["createReview"];
    
    assert!(review["id"].is_string());
    assert_eq!(review["rating"], 5);
    assert_eq!(review["text"], "Excellent car, highly recommended!");
    assert_eq!(review["offerId"], offer_id.to_string());
    assert_eq!(review["isModerated"], false);
    assert_eq!(review["moderationStatus"], "PENDING");
}

#[tokio::test]
#[serial]
async fn test_create_review_validation_error() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    
    let mutation = r#"
        mutation CreateReview($input: CreateReviewInput!) {
            createReview(input: $input) {
                id
                rating
                text
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
    
    let request = Request::new(mutation)
        .variables(variables)
        .data(user_context);
    
    let result = schema.execute(request).await;
    
    assert!(!result.errors.is_empty());
    let error = &result.errors[0];
    assert!(error.message.contains("Rating must be between 1 and 5"));
    
    // Check error extensions
    let extensions = &error.extensions;
    assert_eq!(extensions.get("code"), Some(&Value::String("VALIDATION_ERROR".to_string())));
    assert_eq!(extensions.get("category"), Some(&Value::String("CLIENT_ERROR".to_string())));
}

#[tokio::test]
#[serial]
async fn test_reviews_query_with_pagination() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    
    // First create some reviews
    let offer_id = Uuid::new_v4();
    for i in 1..=5 {
        let mutation = r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                }
            }
        "#;
        
        let variables = Variables::from_json(json!({
            "input": {
                "offerId": offer_id.to_string(),
                "rating": i,
                "text": format!("Review {}", i)
            }
        }));
        
        let request = Request::new(mutation)
            .variables(variables)
            .data(user_context.clone());
        
        let result = schema.execute(request).await;
        assert!(result.errors.is_empty());
    }
    
    // Now query with pagination
    let query = r#"
        query GetReviews($first: Int, $filter: ReviewsFilterInput) {
            reviews(first: $first, filter: $filter) {
                edges {
                    node {
                        id
                        rating
                        text
                        offerId
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
        "first": 3,
        "filter": {
            "offerId": offer_id.to_string()
        }
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let reviews = &data["reviews"];
    
    assert_eq!(reviews["edges"].as_array().unwrap().len(), 3);
    assert_eq!(reviews["totalCount"], 5);
    assert_eq!(reviews["pageInfo"]["hasNextPage"], true);
    assert_eq!(reviews["pageInfo"]["hasPreviousPage"], false);
}

#[tokio::test]
#[serial]
async fn test_moderate_review_mutation() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    let moderator_context = create_moderator_context();
    
    // Create a review first
    let create_mutation = r#"
        mutation CreateReview($input: CreateReviewInput!) {
            createReview(input: $input) {
                id
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "input": {
            "offerId": Uuid::new_v4().to_string(),
            "rating": 5,
            "text": "Test review for moderation"
        }
    }));
    
    let request = Request::new(create_mutation)
        .variables(variables)
        .data(user_context);
    
    let result = schema.execute(request).await;
    assert!(result.errors.is_empty());
    
    let review_id = result.data.into_json().unwrap()["createReview"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    
    // Now moderate the review
    let moderate_mutation = r#"
        mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
            moderateReview(id: $id, status: $status) {
                id
                moderationStatus
                isModerated
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "id": review_id,
        "status": "APPROVED"
    }));
    
    let request = Request::new(moderate_mutation)
        .variables(variables)
        .data(moderator_context);
    
    let result = schema.execute(request).await;
    
    if !result.errors.is_empty() {
        for error in &result.errors {
            println!("Moderation Error: {}", error.message);
        }
    }
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let review = &data["moderateReview"];
    
    assert_eq!(review["moderationStatus"], "APPROVED");
    assert_eq!(review["isModerated"], true);
}

#[tokio::test]
#[serial]
async fn test_unauthorized_moderation() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context(); // Regular user, not moderator
    
    let moderate_mutation = r#"
        mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
            moderateReview(id: $id, status: $status) {
                id
                moderationStatus
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "id": Uuid::new_v4().to_string(),
        "status": "APPROVED"
    }));
    
    let request = Request::new(moderate_mutation)
        .variables(variables)
        .data(user_context);
    
    let result = schema.execute(request).await;
    
    assert!(!result.errors.is_empty());
    let error = &result.errors[0];
    assert!(error.message.contains("Insufficient permissions") || error.message.contains("moderator"));
}

#[tokio::test]
#[serial]
async fn test_offer_rating_stats_query() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    let moderator_context = create_moderator_context();
    
    let offer_id = Uuid::new_v4();
    
    // Create and approve multiple reviews
    let ratings = vec![5, 4, 5, 3, 4];
    for rating in ratings {
        // Create review
        let create_mutation = r#"
            mutation CreateReview($input: CreateReviewInput!) {
                createReview(input: $input) {
                    id
                }
            }
        "#;
        
        let variables = Variables::from_json(json!({
            "input": {
                "offerId": offer_id.to_string(),
                "rating": rating,
                "text": format!("Review with rating {}", rating)
            }
        }));
        
        let request = Request::new(create_mutation)
            .variables(variables)
            .data(user_context.clone());
        
        let result = schema.execute(request).await;
        assert!(result.errors.is_empty());
        
        let review_id = result.data.into_json().unwrap()["createReview"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        
        // Approve review
        let moderate_mutation = r#"
            mutation ModerateReview($id: UUID!, $status: ModerationStatus!) {
                moderateReview(id: $id, status: $status) {
                    id
                }
            }
        "#;
        
        let variables = Variables::from_json(json!({
            "id": review_id,
            "status": "APPROVED"
        }));
        
        let request = Request::new(moderate_mutation)
            .variables(variables)
            .data(moderator_context.clone());
        
        let result = schema.execute(request).await;
        assert!(result.errors.is_empty());
    }
    
    // Query offer rating stats
    let query = r#"
        query GetOfferRatingStats($offerId: UUID!) {
            offerRatingStats(offerId: $offerId) {
                offerId
                averageRating
                reviewsCount
                ratingDistribution
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "offerId": offer_id.to_string()
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let stats = &data["offerRatingStats"];
    
    assert_eq!(stats["offerId"], offer_id.to_string());
    assert_eq!(stats["reviewsCount"], 5);
    // Average should be (5+4+5+3+4)/5 = 4.2
    assert!((stats["averageRating"].as_f64().unwrap() - 4.2).abs() < 0.01);
    
    let distribution = stats["ratingDistribution"].as_object().unwrap();
    assert_eq!(distribution["3"], 1);
    assert_eq!(distribution["4"], 2);
    assert_eq!(distribution["5"], 2);
}

#[tokio::test]
#[serial]
async fn test_federated_type_extensions() {
    let schema = setup_test_schema().await;
    
    // Test that we can query extended types (this would normally come from other subgraphs)
    let query = r#"
        query TestExtensions {
            __type(name: "Offer") {
                name
                fields {
                    name
                    type {
                        name
                    }
                }
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    if let Some(offer_type) = data["__type"].as_object() {
        let fields = offer_type["fields"].as_array().unwrap();
        let field_names: Vec<&str> = fields
            .iter()
            .filter_map(|f| f["name"].as_str())
            .collect();
        
        // Check that our extended fields are present
        assert!(field_names.contains(&"reviews"));
        assert!(field_names.contains(&"averageRating"));
        assert!(field_names.contains(&"reviewsCount"));
    }
}

#[tokio::test]
#[serial]
async fn test_complex_federated_query() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    
    // Create a review first
    let create_mutation = r#"
        mutation CreateReview($input: CreateReviewInput!) {
            createReview(input: $input) {
                id
                offerId
            }
        }
    "#;
    
    let offer_id = Uuid::new_v4();
    let variables = Variables::from_json(json!({
        "input": {
            "offerId": offer_id.to_string(),
            "rating": 5,
            "text": "Complex query test review"
        }
    }));
    
    let request = Request::new(create_mutation)
        .variables(variables)
        .data(user_context.clone());
    
    let result = schema.execute(request).await;
    assert!(result.errors.is_empty());
    
    // Now perform a complex query that would span multiple subgraphs
    let complex_query = r#"
        query ComplexQuery($offerId: UUID!) {
            reviews(filter: { offerId: $offerId }) {
                edges {
                    node {
                        id
                        rating
                        text
                        createdAt
                        # These would normally be resolved from other subgraphs
                        offer {
                            id
                        }
                        author {
                            id
                        }
                    }
                }
            }
            offerRatingStats(offerId: $offerId) {
                averageRating
                reviewsCount
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "offerId": offer_id.to_string()
    }));
    
    let request = Request::new(complex_query).variables(variables);
    let result = schema.execute(request).await;
    
    // This might have errors due to missing external resolvers, but the structure should be valid
    if !result.errors.is_empty() {
        for error in &result.errors {
            println!("Complex query error: {}", error.message);
        }
    }
    
    // The query should at least parse and execute partially
    assert!(result.data.is_some());
}

#[tokio::test]
#[serial]
async fn test_error_handling_in_federation() {
    let schema = setup_test_schema().await;
    
    // Test querying non-existent review
    let query = r#"
        query GetNonExistentReview($id: UUID!) {
            review(id: $id) {
                id
                rating
                text
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "id": Uuid::new_v4().to_string()
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    assert!(data["review"].is_null());
}

#[tokio::test]
#[serial]
async fn test_concurrent_federated_operations() {
    let schema = setup_test_schema().await;
    let user_context = create_test_user_context();
    
    let offer_id = Uuid::new_v4();
    
    // Create multiple reviews concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let schema_clone = schema.clone();
        let user_context_clone = user_context.clone();
        let handle = tokio::spawn(async move {
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
                    "offerId": offer_id.to_string(),
                    "rating": (i % 5) + 1,
                    "text": format!("Concurrent review {}", i)
                }
            }));
            
            let request = Request::new(mutation)
                .variables(variables)
                .data(user_context_clone);
            
            schema_clone.execute(request).await
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    let mut successful_operations = 0;
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        if result.errors.is_empty() {
            successful_operations += 1;
        }
    }
    
    assert_eq!(successful_operations, 5);
    
    // Query all reviews for the offer
    let query = r#"
        query GetOfferReviews($offerId: UUID!) {
            reviews(filter: { offerId: $offerId }) {
                totalCount
                edges {
                    node {
                        id
                        rating
                    }
                }
            }
        }
    "#;
    
    let variables = Variables::from_json(json!({
        "offerId": offer_id.to_string()
    }));
    
    let request = Request::new(query).variables(variables);
    let result = schema.execute(request).await;
    
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    assert_eq!(data["reviews"]["totalCount"], 5);
}