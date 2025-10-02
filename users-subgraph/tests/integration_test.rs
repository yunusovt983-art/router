use async_graphql::{Schema, EmptySubscription, Value};
use users_subgraph::{Query, Mutation, UserService};
use std::sync::Arc;

#[tokio::test]
async fn test_users_graphql_schema() {
    // Create the service
    let user_service = Arc::new(UserService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(user_service)
        .finish();
    
    // Test a simple query
    let query = r#"
        query {
            users {
                id
                name
                email
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::List(users)) = data.get("users") {
            // Should have 3 mock users
            assert_eq!(users.len(), 3);
            
            // Check first user
            if let Value::Object(first_user) = &users[0] {
                if let Value::String(id) = first_user.get("id").unwrap() {
                    assert_eq!(id, "user-1");
                }
                if let Value::String(name) = first_user.get("name").unwrap() {
                    assert_eq!(name, "Иван Иванов");
                }
                if let Value::String(email) = first_user.get("email").unwrap() {
                    assert_eq!(email, "ivan@example.com");
                }
            } else {
                panic!("First user is not an object");
            }
        } else {
            panic!("Users field is not a list");
        }
    } else {
        panic!("Result data is not an object");
    }
}

#[tokio::test]
async fn test_user_by_id_query() {
    // Create the service
    let user_service = Arc::new(UserService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(user_service)
        .finish();
    
    // Test query for specific user
    let query = r#"
        query {
            user(id: "user-2") {
                id
                name
                email
                phone
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::Object(user)) = data.get("user") {
            if let Value::String(id) = user.get("id").unwrap() {
                assert_eq!(id, "user-2");
            }
            if let Value::String(name) = user.get("name").unwrap() {
                assert_eq!(name, "Мария Петрова");
            }
            if let Value::String(email) = user.get("email").unwrap() {
                assert_eq!(email, "maria@example.com");
            }
            if let Value::String(phone) = user.get("phone").unwrap() {
                assert_eq!(phone, "+7-900-765-43-21");
            }
        } else {
            panic!("User field is not an object");
        }
    } else {
        panic!("Result data is not an object");
    }
}

#[tokio::test]
async fn test_federation_entity_resolver() {
    // Create the service
    let user_service = Arc::new(UserService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(user_service)
        .finish();
    
    // Test federation entity resolver
    let query = r#"
        query {
            findUserById(id: "user-3") {
                id
                name
                email
                phone
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::Object(user)) = data.get("findUserById") {
            if let Value::String(id) = user.get("id").unwrap() {
                assert_eq!(id, "user-3");
            }
            if let Value::String(name) = user.get("name").unwrap() {
                assert_eq!(name, "Алексей Сидоров");
            }
            if let Value::String(email) = user.get("email").unwrap() {
                assert_eq!(email, "alexey@example.com");
            }
            assert!(matches!(user.get("phone").unwrap(), Value::Null)); // This user has no phone
        } else {
            panic!("findUserById field is not an object");
        }
    } else {
        panic!("Result data is not an object");
    }
}