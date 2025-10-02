use async_graphql::{Context, EmptySubscription, Object, Result, Schema, SimpleObject, ID};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{info, instrument};

// Offer entity - this is the primary entity for this subgraph
#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Offer {
    pub id: ID,
    pub title: String,
    pub description: String,
    pub price: i32,
    pub currency: String,
    pub year: i32,
    pub mileage: i32,
    pub location: String,
    pub seller_id: ID,
    pub created_at: String, // Using String for simplicity in the stub
    pub updated_at: String, // Using String for simplicity in the stub
    pub is_active: bool,
}

#[async_graphql::ComplexObject]
impl Offer {
    // Reference to seller (User entity from Users subgraph)
    pub async fn seller(&self, _ctx: &Context<'_>) -> Result<User> {
        // In federation, this would resolve the User from Users subgraph
        Ok(User {
            id: self.seller_id.clone(),
        })
    }
}

// User reference type for federation - extends User from Users subgraph
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct User {
    pub id: ID,
}

#[async_graphql::ComplexObject]
impl User {
    /// Get offers created by this user
    pub async fn offers(&self, ctx: &Context<'_>) -> Result<Vec<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offers_by_seller(&self.id).await)
    }
}

// Mock data store
pub struct OfferService {
    // In a real implementation, this would be a database connection
    offers: Vec<Offer>,
}

impl OfferService {
    pub fn new() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        // Create some mock offers for testing
        let offers = vec![
            Offer {
                id: ID::from("offer-1"),
                title: "BMW X5 2020".to_string(),
                description: "Отличный внедорожник в идеальном состоянии".to_string(),
                price: 3500000,
                currency: "RUB".to_string(),
                year: 2020,
                mileage: 45000,
                location: "Москва".to_string(),
                seller_id: ID::from("user-1"),
                created_at: now.clone(),
                updated_at: now.clone(),
                is_active: true,
            },
            Offer {
                id: ID::from("offer-2"),
                title: "Toyota Camry 2019".to_string(),
                description: "Надежный седан для семьи".to_string(),
                price: 2200000,
                currency: "RUB".to_string(),
                year: 2019,
                mileage: 62000,
                location: "Санкт-Петербург".to_string(),
                seller_id: ID::from("user-2"),
                created_at: now.clone(),
                updated_at: now.clone(),
                is_active: true,
            },
            Offer {
                id: ID::from("offer-3"),
                title: "Mercedes-Benz C-Class 2021".to_string(),
                description: "Премиальный седан с полной комплектацией".to_string(),
                price: 4100000,
                currency: "RUB".to_string(),
                year: 2021,
                mileage: 28000,
                location: "Екатеринбург".to_string(),
                seller_id: ID::from("user-3"),
                created_at: now.clone(),
                updated_at: now,
                is_active: true,
            },
        ];

        Self { offers }
    }

    pub async fn get_offer_by_id(&self, id: &str) -> Option<Offer> {
        self.offers.iter().find(|o| o.id == id).cloned()
    }

    pub async fn get_offers(&self) -> Vec<Offer> {
        self.offers.clone()
    }

    pub async fn get_offers_by_seller(&self, seller_id: &str) -> Vec<Offer> {
        self.offers
            .iter()
            .filter(|o| o.seller_id == seller_id)
            .cloned()
            .collect()
    }
}

// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
    /// Get offer by ID
    #[instrument(skip(self, ctx))]
    pub async fn offer(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&id).await)
    }

    /// Get all offers (for testing purposes)
    #[instrument(skip(self, ctx))]
    pub async fn offers(&self, ctx: &Context<'_>) -> Result<Vec<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offers().await)
    }

    /// Get offers by seller
    #[instrument(skip(self, ctx))]
    pub async fn offers_by_seller(&self, ctx: &Context<'_>, seller_id: ID) -> Result<Vec<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offers_by_seller(&seller_id).await)
    }

    /// Federation entity resolver for Offer
    pub async fn find_offer_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&id).await)
    }
}

// GraphQL Mutation root (stub)
pub struct Mutation;

#[Object]
impl Mutation {
    /// Placeholder mutation
    pub async fn placeholder(&self) -> Result<String> {
        Ok("Offers subgraph mutations not implemented yet".to_string())
    }
}

// Application state
#[derive(Clone)]
pub struct AppState {
    pub schema: Schema<Query, Mutation, EmptySubscription>,
}

// GraphQL handler
pub async fn graphql_handler(
    State(state): State<AppState>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    state.schema.execute(req.into_inner()).await.into()
}

// Health check handler
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "offers-subgraph",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Offers Subgraph...");

    // Create services
    let offer_service = Arc::new(OfferService::new());

    // Build GraphQL schema with federation support
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(offer_service)
        .finish();

    // Create application state
    let state = AppState { schema };

    // Build router
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let listener = TcpListener::bind("0.0.0.0:4004").await?;
    info!("Offers Subgraph listening on http://0.0.0.0:4004");
    info!("GraphQL endpoint: http://0.0.0.0:4004/graphql");
    info!("Health check: http://0.0.0.0:4004/health");

    axum::serve(listener, app).await?;

    Ok(())
}