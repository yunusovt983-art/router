use std::collections::HashMap;
use serde_json::{json, Value};
use async_graphql::{Schema, EmptySubscription, parser::parse_query, validation::visit_all_rules};

use ugc_subgraph::{
    graphql::{Query, Mutation},
    service::ReviewService,
    repository::MockReviewRepository,
};

// Schema version for compatibility testing
const SCHEMA_VERSION: &str = "1.0.0";

// Expected schema structure for compatibility testing
#[derive(Debug, Clone)]
struct SchemaField {
    name: String,
    type_name: String,
    is_required: bool,
    is_list: bool,
    is_deprecated: bool,
}

#[derive(Debug, Clone)]
struct SchemaType {
    name: String,
    kind: String,
    fields: Vec<SchemaField>,
    is_deprecated: bool,
}

// Helper function to create test schema
fn create_test_schema() -> Schema<Query, Mutation, EmptySubscription> {
    let mut mock_repo = MockReviewRepository::new();
    mock_repo.expect_get_review_by_id()
        .returning(|_| Ok(None));
    
    let service = ReviewService::new(std::sync::Arc::new(mock_repo));
    Schema::build(Query, Mutation, EmptySubscription)
        .data(service)
        .finish()
}

// Extract schema information for compatibility testing
async fn extract_schema_info(schema: &Schema<Query, Mutation, EmptySubscription>) -> HashMap<String, SchemaType> {
    let introspection_query = r#"
        query IntrospectionQuery {
            __schema {
                types {
                    name
                    kind
                    fields {
                        name
                        type {
                            name
                            kind
                            ofType {
                                name
                                kind
                                ofType {
                                    name
                                    kind
                                }
                            }
                        }
                        isDeprecated
                        deprecationReason
                    }
                    isDeprecated: false
                }
            }
        }
    "#;
    
    let result = schema.execute(introspection_query).await;
    assert!(result.errors.is_empty(), "Introspection query failed: {:?}", result.errors);
    
    let data = result.data.into_json().unwrap();
    let types = data["__schema"]["types"].as_array().unwrap();
    
    let mut schema_types = HashMap::new();
    
    for type_info in types {
        let type_name = type_info["name"].as_str().unwrap().to_string();
        let type_kind = type_info["kind"].as_str().unwrap().to_string();
        
        // Skip built-in GraphQL types
        if type_name.starts_with("__") {
            continue;
        }
        
        let mut fields = Vec::new();
        
        if let Some(type_fields) = type_info["fields"].as_array() {
            for field_info in type_fields {
                let field_name = field_info["name"].as_str().unwrap().to_string();
                let is_deprecated = field_info["isDeprecated"].as_bool().unwrap_or(false);
                
                let (type_name, is_required, is_list) = extract_type_info(&field_info["type"]);
                
                fields.push(SchemaField {
                    name: field_name,
                    type_name,
                    is_required,
                    is_list,
                    is_deprecated,
                });
            }
        }
        
        schema_types.insert(type_name.clone(), SchemaType {
            name: type_name,
            kind: type_kind,
            fields,
            is_deprecated: false, // GraphQL doesn't support type deprecation yet
        });
    }
    
    schema_types
}

// Helper function to extract type information from GraphQL introspection
fn extract_type_info(type_info: &Value) -> (String, bool, bool) {
    let kind = type_info["kind"].as_str().unwrap();
    
    match kind {
        "NON_NULL" => {
            let of_type = &type_info["ofType"];
            let (inner_type, _, is_list) = extract_type_info(of_type);
            (inner_type, true, is_list)
        }
        "LIST" => {
            let of_type = &type_info["ofType"];
            let (inner_type, is_required, _) = extract_type_info(of_type);
            (inner_type, is_required, true)
        }
        _ => {
            let type_name = type_info["name"].as_str().unwrap_or("Unknown").to_string();
            (type_name, false, false)
        }
    }
}

#[tokio::test]
async fn test_schema_backward_compatibility() {
    let schema = create_test_schema();
    let current_schema = extract_schema_info(&schema).await;
    
    // Define expected schema structure for backward compatibility
    let expected_types = vec![
        "Query", "Mutation", "Review", "ReviewConnection", "ReviewEdge", 
        "PageInfo", "OfferRatingStats", "ModerationStatus", "CreateReviewInput",
        "UpdateReviewInput", "ReviewsFilterInput", "User", "Offer"
    ];
    
    // Check that all expected types exist
    for expected_type in expected_types {
        assert!(
            current_schema.contains_key(expected_type),
            "Expected type '{}' is missing from schema",
            expected_type
        );
    }
    
    // Check Review type structure
    if let Some(review_type) = current_schema.get("Review") {
        let required_fields = vec![
            "id", "offerId", "authorId", "rating", "text", 
            "createdAt", "updatedAt", "isModerated", "moderationStatus"
        ];
        
        let field_names: Vec<&str> = review_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_field in required_fields {
            assert!(
                field_names.contains(&required_field),
                "Required field '{}' is missing from Review type",
                required_field
            );
        }
        
        // Check that id field is non-null
        let id_field = review_type.fields
            .iter()
            .find(|f| f.name == "id")
            .expect("id field should exist");
        assert!(id_field.is_required, "id field should be non-null");
        assert_eq!(id_field.type_name, "UUID", "id field should be UUID type");
    }
    
    // Check Query type structure
    if let Some(query_type) = current_schema.get("Query") {
        let required_queries = vec![
            "health", "version", "review", "reviews", 
            "offerRatingStats", "offerAverageRating", "offerReviewsCount"
        ];
        
        let query_names: Vec<&str> = query_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_query in required_queries {
            assert!(
                query_names.contains(&required_query),
                "Required query '{}' is missing from Query type",
                required_query
            );
        }
    }
    
    // Check Mutation type structure
    if let Some(mutation_type) = current_schema.get("Mutation") {
        let required_mutations = vec![
            "createReview", "updateReview", "deleteReview", "moderateReview"
        ];
        
        let mutation_names: Vec<&str> = mutation_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_mutation in required_mutations {
            assert!(
                mutation_names.contains(&required_mutation),
                "Required mutation '{}' is missing from Mutation type",
                required_mutation
            );
        }
    }
}

#[tokio::test]
async fn test_schema_no_breaking_changes() {
    let schema = create_test_schema();
    let current_schema = extract_schema_info(&schema).await;
    
    // Check that no fields are deprecated
    for (type_name, schema_type) in &current_schema {
        for field in &schema_type.fields {
            assert!(
                !field.is_deprecated,
                "Field '{}.{}' is deprecated, which may be a breaking change",
                type_name,
                field.name
            );
        }
    }
    
    // Check that required fields haven't become optional
    let critical_required_fields = vec![
        ("Review", "id"),
        ("Review", "offerId"),
        ("Review", "authorId"),
        ("Review", "rating"),
        ("Review", "text"),
        ("CreateReviewInput", "offerId"),
        ("CreateReviewInput", "rating"),
        ("CreateReviewInput", "text"),
    ];
    
    for (type_name, field_name) in critical_required_fields {
        if let Some(schema_type) = current_schema.get(type_name) {
            if let Some(field) = schema_type.fields.iter().find(|f| f.name == field_name) {
                assert!(
                    field.is_required,
                    "Critical field '{}.{}' should be required",
                    type_name,
                    field_name
                );
            }
        }
    }
}

#[tokio::test]
async fn test_federation_schema_compatibility() {
    let schema = create_test_schema();
    
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
    
    // Check federation directives
    assert!(sdl.contains("@key"), "Schema should contain @key directive");
    assert!(sdl.contains("@extends"), "Schema should contain @extends directive");
    assert!(sdl.contains("@external"), "Schema should contain @external directive");
    
    // Check federated types
    assert!(sdl.contains("type Review @key"), "Review type should have @key directive");
    assert!(sdl.contains("extend type Offer"), "Should extend Offer type");
    assert!(sdl.contains("extend type User"), "Should extend User type");
    
    // Check that federation fields are present
    assert!(sdl.contains("reviews: [Review!]!"), "Offer should have reviews field");
    assert!(sdl.contains("averageRating: Float"), "Offer should have averageRating field");
    assert!(sdl.contains("reviewsCount: Int!"), "Offer should have reviewsCount field");
}

#[tokio::test]
async fn test_schema_query_validation() {
    let schema = create_test_schema();
    
    // Test valid queries
    let valid_queries = vec![
        r#"query { health }"#,
        r#"query { version }"#,
        r#"query GetReview($id: UUID!) { review(id: $id) { id rating } }"#,
        r#"query GetReviews { reviews(first: 10) { edges { node { id } } } }"#,
        r#"mutation CreateReview($input: CreateReviewInput!) { createReview(input: $input) { id } }"#,
    ];
    
    for query_str in valid_queries {
        let doc = parse_query(query_str).expect(&format!("Query should parse: {}", query_str));
        let validation_result = visit_all_rules(&schema, &doc);
        assert!(
            validation_result.is_empty(),
            "Query should be valid: {} - Errors: {:?}",
            query_str,
            validation_result
        );
    }
    
    // Test invalid queries
    let invalid_queries = vec![
        r#"query { nonExistentField }"#,
        r#"query GetReview { review { id } }"#, // Missing required argument
        r#"mutation { createReview { id } }"#, // Missing required input
    ];
    
    for query_str in invalid_queries {
        let doc = parse_query(query_str).expect(&format!("Query should parse: {}", query_str));
        let validation_result = visit_all_rules(&schema, &doc);
        assert!(
            !validation_result.is_empty(),
            "Query should be invalid: {}",
            query_str
        );
    }
}

#[tokio::test]
async fn test_schema_input_validation_compatibility() {
    let schema = create_test_schema();
    let current_schema = extract_schema_info(&schema).await;
    
    // Check CreateReviewInput structure
    if let Some(input_type) = current_schema.get("CreateReviewInput") {
        let required_fields = vec!["offerId", "rating", "text"];
        let field_names: Vec<&str> = input_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_field in required_fields {
            assert!(
                field_names.contains(&required_field),
                "Required input field '{}' is missing from CreateReviewInput",
                required_field
            );
        }
        
        // Check field types
        let offer_id_field = input_type.fields
            .iter()
            .find(|f| f.name == "offerId")
            .expect("offerId field should exist");
        assert_eq!(offer_id_field.type_name, "UUID");
        assert!(offer_id_field.is_required);
        
        let rating_field = input_type.fields
            .iter()
            .find(|f| f.name == "rating")
            .expect("rating field should exist");
        assert_eq!(rating_field.type_name, "Int");
        assert!(rating_field.is_required);
        
        let text_field = input_type.fields
            .iter()
            .find(|f| f.name == "text")
            .expect("text field should exist");
        assert_eq!(text_field.type_name, "String");
        assert!(text_field.is_required);
    }
    
    // Check UpdateReviewInput structure
    if let Some(input_type) = current_schema.get("UpdateReviewInput") {
        let optional_fields = vec!["rating", "text"];
        let field_names: Vec<&str> = input_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for optional_field in optional_fields {
            assert!(
                field_names.contains(&optional_field),
                "Optional input field '{}' is missing from UpdateReviewInput",
                optional_field
            );
        }
        
        // All fields in UpdateReviewInput should be optional
        for field in &input_type.fields {
            assert!(
                !field.is_required,
                "Field '{}' in UpdateReviewInput should be optional",
                field.name
            );
        }
    }
}

#[tokio::test]
async fn test_schema_enum_compatibility() {
    let schema = create_test_schema();
    let current_schema = extract_schema_info(&schema).await;
    
    // Check ModerationStatus enum
    if let Some(enum_type) = current_schema.get("ModerationStatus") {
        assert_eq!(enum_type.kind, "ENUM");
        
        // Note: Enum values are not captured in our simplified schema extraction
        // In a real implementation, you would also check enum values
    }
    
    // Test enum usage in queries
    let enum_query = r#"
        query {
            __type(name: "ModerationStatus") {
                name
                kind
                enumValues {
                    name
                    isDeprecated
                }
            }
        }
    "#;
    
    let result = schema.execute(enum_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let enum_type = &data["__type"];
    
    assert_eq!(enum_type["name"], "ModerationStatus");
    assert_eq!(enum_type["kind"], "ENUM");
    
    let enum_values = enum_type["enumValues"].as_array().unwrap();
    let expected_values = vec!["PENDING", "APPROVED", "REJECTED", "FLAGGED"];
    
    let actual_values: Vec<&str> = enum_values
        .iter()
        .filter_map(|v| v["name"].as_str())
        .collect();
    
    for expected_value in expected_values {
        assert!(
            actual_values.contains(&expected_value),
            "Expected enum value '{}' is missing from ModerationStatus",
            expected_value
        );
    }
    
    // Check that no enum values are deprecated
    for enum_value in enum_values {
        assert_eq!(
            enum_value["isDeprecated"], false,
            "Enum value '{}' should not be deprecated",
            enum_value["name"]
        );
    }
}

#[tokio::test]
async fn test_schema_connection_compatibility() {
    let schema = create_test_schema();
    let current_schema = extract_schema_info(&schema).await;
    
    // Check ReviewConnection structure (Relay-style pagination)
    if let Some(connection_type) = current_schema.get("ReviewConnection") {
        let required_fields = vec!["edges", "pageInfo", "totalCount"];
        let field_names: Vec<&str> = connection_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_field in required_fields {
            assert!(
                field_names.contains(&required_field),
                "Required connection field '{}' is missing from ReviewConnection",
                required_field
            );
        }
        
        // Check edges field
        let edges_field = connection_type.fields
            .iter()
            .find(|f| f.name == "edges")
            .expect("edges field should exist");
        assert!(edges_field.is_list);
        assert!(edges_field.is_required);
        
        // Check pageInfo field
        let page_info_field = connection_type.fields
            .iter()
            .find(|f| f.name == "pageInfo")
            .expect("pageInfo field should exist");
        assert_eq!(page_info_field.type_name, "PageInfo");
        assert!(page_info_field.is_required);
    }
    
    // Check ReviewEdge structure
    if let Some(edge_type) = current_schema.get("ReviewEdge") {
        let required_fields = vec!["node", "cursor"];
        let field_names: Vec<&str> = edge_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_field in required_fields {
            assert!(
                field_names.contains(&required_field),
                "Required edge field '{}' is missing from ReviewEdge",
                required_field
            );
        }
    }
    
    // Check PageInfo structure
    if let Some(page_info_type) = current_schema.get("PageInfo") {
        let required_fields = vec!["hasNextPage", "hasPreviousPage", "startCursor", "endCursor"];
        let field_names: Vec<&str> = page_info_type.fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        
        for required_field in required_fields {
            assert!(
                field_names.contains(&required_field),
                "Required PageInfo field '{}' is missing",
                required_field
            );
        }
    }
}

#[tokio::test]
async fn test_schema_version_metadata() {
    let schema = create_test_schema();
    
    // Test that we can query schema version information
    let version_query = r#"
        query {
            version
        }
    "#;
    
    let result = schema.execute(version_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let version = data["version"].as_str().unwrap();
    
    // Version should follow semantic versioning
    assert!(
        version.matches('.').count() >= 2,
        "Version should follow semantic versioning format: {}",
        version
    );
}

#[tokio::test]
async fn test_schema_directive_compatibility() {
    let schema = create_test_schema();
    
    // Test that schema supports required directives
    let directives_query = r#"
        query {
            __schema {
                directives {
                    name
                    locations
                    args {
                        name
                        type {
                            name
                        }
                    }
                }
            }
        }
    "#;
    
    let result = schema.execute(directives_query).await;
    assert!(result.errors.is_empty());
    
    let data = result.data.into_json().unwrap();
    let directives = data["__schema"]["directives"].as_array().unwrap();
    
    let directive_names: Vec<&str> = directives
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    
    // Check that standard GraphQL directives are present
    let required_directives = vec!["skip", "include", "deprecated"];
    for required_directive in required_directives {
        assert!(
            directive_names.contains(&required_directive),
            "Required directive '{}' is missing",
            required_directive
        );
    }
}