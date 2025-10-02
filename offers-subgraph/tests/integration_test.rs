use async_graphql::{Schema, EmptySubscription, Value};
use offers_subgraph::{Query, Mutation, OfferService};
use std::sync::Arc;

#[tokio::test]
async fn test_offers_graphql_schema() {
    // Create the service
    let offer_service = Arc::new(OfferService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(offer_service)
        .finish();
    
    // Test a simple query
    let query = r#"
        query {
            offers {
                id
                title
                price
                currency
                year
                location
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::List(offers)) = data.get("offers") {
            // Should have 3 mock offers
            assert_eq!(offers.len(), 3);
            
            // Check first offer
            if let Value::Object(first_offer) = &offers[0] {
                if let Value::String(id) = first_offer.get("id").unwrap() {
                    assert_eq!(id, "offer-1");
                }
                if let Value::String(title) = first_offer.get("title").unwrap() {
                    assert_eq!(title, "BMW X5 2020");
                }
                if let Value::Number(price) = first_offer.get("price").unwrap() {
                    assert_eq!(price.as_i64().unwrap(), 3500000);
                }
                if let Value::String(currency) = first_offer.get("currency").unwrap() {
                    assert_eq!(currency, "RUB");
                }
            } else {
                panic!("First offer is not an object");
            }
        } else {
            panic!("Offers field is not a list");
        }
    } else {
        panic!("Result data is not an object");
    }
}

#[tokio::test]
async fn test_offer_by_id_query() {
    // Create the service
    let offer_service = Arc::new(OfferService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(offer_service)
        .finish();
    
    // Test query for specific offer
    let query = r#"
        query {
            offer(id: "offer-2") {
                id
                title
                description
                price
                year
                mileage
                location
                sellerId
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::Object(offer)) = data.get("offer") {
            if let Value::String(id) = offer.get("id").unwrap() {
                assert_eq!(id, "offer-2");
            }
            if let Value::String(title) = offer.get("title").unwrap() {
                assert_eq!(title, "Toyota Camry 2019");
            }
            if let Value::String(description) = offer.get("description").unwrap() {
                assert_eq!(description, "Надежный седан для семьи");
            }
            if let Value::Number(price) = offer.get("price").unwrap() {
                assert_eq!(price.as_i64().unwrap(), 2200000);
            }
            if let Value::Number(year) = offer.get("year").unwrap() {
                assert_eq!(year.as_i64().unwrap(), 2019);
            }
            if let Value::String(seller_id) = offer.get("sellerId").unwrap() {
                assert_eq!(seller_id, "user-2");
            }
        } else {
            panic!("Offer field is not an object");
        }
    } else {
        panic!("Result data is not an object");
    }
}

#[tokio::test]
async fn test_offers_by_seller_query() {
    // Create the service
    let offer_service = Arc::new(OfferService::new());
    
    // Build the schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(offer_service)
        .finish();
    
    // Test query for offers by seller
    let query = r#"
        query {
            offersBySeller(sellerId: "user-1") {
                id
                title
                sellerId
            }
        }
    "#;
    
    let result = schema.execute(query).await;
    
    // Should not have errors
    assert!(result.errors.is_empty(), "GraphQL errors: {:?}", result.errors);
    
    // Should have data
    assert!(!matches!(result.data, Value::Null));
    
    if let Value::Object(data) = result.data {
        if let Some(Value::List(offers)) = data.get("offersBySeller") {
            // Should have 1 offer for user-1
            assert_eq!(offers.len(), 1);
            
            // Check the offer
            if let Value::Object(offer) = &offers[0] {
                if let Value::String(id) = offer.get("id").unwrap() {
                    assert_eq!(id, "offer-1");
                }
                if let Value::String(seller_id) = offer.get("sellerId").unwrap() {
                    assert_eq!(seller_id, "user-1");
                }
            } else {
                panic!("Offer is not an object");
            }
        } else {
            panic!("offersBySeller field is not a list");
        }
    } else {
        panic!("Result data is not an object");
    }
}