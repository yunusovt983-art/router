use async_graphql::{Context, EmptySubscription, Object, Result, Schema, SimpleObject, ID};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tracing::{info, instrument};
use uuid::Uuid;

// Car Brand entity
#[derive(SimpleObject, Clone)]
pub struct Brand {
    pub id: ID,
    pub name: String,
    pub country: String,
    pub logo_url: Option<String>,
    pub founded_year: Option<i32>,
}

// Car Model entity
#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Model {
    pub id: ID,
    pub name: String,
    pub brand_id: ID,
    pub body_type: String,
    pub fuel_type: String,
    pub transmission: String,
    pub drive_type: String,
    pub engine_volume: Option<f64>,
    pub power_hp: Option<i32>,
    pub production_start_year: Option<i32>,
    pub production_end_year: Option<i32>,
}

#[async_graphql::ComplexObject]
impl Model {
    // Reference to brand
    async fn brand(&self, ctx: &Context<'_>) -> Result<Option<Brand>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_brand_by_id(&self.brand_id).await)
    }
}

// Mock data store
pub struct CatalogService {
    brands: Vec<Brand>,
    models: Vec<Model>,
}

impl CatalogService {
    pub fn new() -> Self {
        // Create some mock brands
        let brands = vec![
            Brand {
                id: ID::from("brand-1"),
                name: "BMW".to_string(),
                country: "Germany".to_string(),
                logo_url: Some("https://example.com/bmw-logo.png".to_string()),
                founded_year: Some(1916),
            },
            Brand {
                id: ID::from("brand-2"),
                name: "Toyota".to_string(),
                country: "Japan".to_string(),
                logo_url: Some("https://example.com/toyota-logo.png".to_string()),
                founded_year: Some(1937),
            },
            Brand {
                id: ID::from("brand-3"),
                name: "Mercedes-Benz".to_string(),
                country: "Germany".to_string(),
                logo_url: Some("https://example.com/mercedes-logo.png".to_string()),
                founded_year: Some(1926),
            },
        ];

        // Create some mock models
        let models = vec![
            Model {
                id: ID::from("model-1"),
                name: "X5".to_string(),
                brand_id: ID::from("brand-1"),
                body_type: "SUV".to_string(),
                fuel_type: "Gasoline".to_string(),
                transmission: "Automatic".to_string(),
                drive_type: "AWD".to_string(),
                engine_volume: Some(3.0),
                power_hp: Some(340),
                production_start_year: Some(1999),
                production_end_year: None,
            },
            Model {
                id: ID::from("model-2"),
                name: "Camry".to_string(),
                brand_id: ID::from("brand-2"),
                body_type: "Sedan".to_string(),
                fuel_type: "Gasoline".to_string(),
                transmission: "Automatic".to_string(),
                drive_type: "FWD".to_string(),
                engine_volume: Some(2.5),
                power_hp: Some(203),
                production_start_year: Some(1982),
                production_end_year: None,
            },
            Model {
                id: ID::from("model-3"),
                name: "C-Class".to_string(),
                brand_id: ID::from("brand-3"),
                body_type: "Sedan".to_string(),
                fuel_type: "Gasoline".to_string(),
                transmission: "Automatic".to_string(),
                drive_type: "RWD".to_string(),
                engine_volume: Some(2.0),
                power_hp: Some(255),
                production_start_year: Some(1993),
                production_end_year: None,
            },
        ];

        Self { brands, models }
    }

    pub async fn get_brand_by_id(&self, id: &str) -> Option<Brand> {
        self.brands.iter().find(|b| b.id == id).cloned()
    }

    pub async fn get_brands(&self) -> Vec<Brand> {
        self.brands.clone()
    }

    pub async fn get_model_by_id(&self, id: &str) -> Option<Model> {
        self.models.iter().find(|m| m.id == id).cloned()
    }

    pub async fn get_models(&self) -> Vec<Model> {
        self.models.clone()
    }

    pub async fn get_models_by_brand(&self, brand_id: &str) -> Vec<Model> {
        self.models
            .iter()
            .filter(|m| m.brand_id == brand_id)
            .cloned()
            .collect()
    }
}

// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
    /// Get brand by ID
    #[instrument(skip(self, ctx))]
    async fn brand(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Brand>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_brand_by_id(&id).await)
    }

    /// Get all brands
    #[instrument(skip(self, ctx))]
    async fn brands(&self, ctx: &Context<'_>) -> Result<Vec<Brand>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_brands().await)
    }

    /// Get model by ID
    #[instrument(skip(self, ctx))]
    async fn model(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Model>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_model_by_id(&id).await)
    }

    /// Get all models
    #[instrument(skip(self, ctx))]
    async fn models(&self, ctx: &Context<'_>) -> Result<Vec<Model>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_models().await)
    }

    /// Get models by brand
    #[instrument(skip(self, ctx))]
    async fn models_by_brand(&self, ctx: &Context<'_>, brand_id: ID) -> Result<Vec<Model>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_models_by_brand(&brand_id).await)
    }

    /// Federation entity resolvers
    #[graphql(entity)]
    async fn find_brand_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Brand>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_brand_by_id(&id).await)
    }

    #[graphql(entity)]
    async fn find_model_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Model>> {
        let service = ctx.data::<Arc<CatalogService>>()?;
        Ok(service.get_model_by_id(&id).await)
    }
}

// GraphQL Mutation root (stub)
pub struct Mutation;

#[Object]
impl Mutation {
    /// Placeholder mutation
    async fn placeholder(&self) -> Result<String> {
        Ok("Catalog subgraph mutations not implemented yet".to_string())
    }
}

// Application state
#[derive(Clone)]
pub struct AppState {
    pub schema: Schema<Query, Mutation, EmptySubscription>,
}

// GraphQL handler
async fn graphql_handler(
    State(state): State<AppState>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    state.schema.execute(req.into_inner()).await.into()
}

// Health check handler
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "catalog-subgraph",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Catalog Subgraph...");

    // Create services
    let catalog_service = Arc::new(CatalogService::new());

    // Build GraphQL schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(catalog_service)
        .enable_federation()
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
    let listener = TcpListener::bind("0.0.0.0:4003").await?;
    info!("Catalog Subgraph listening on http://0.0.0.0:4003");
    info!("GraphQL endpoint: http://0.0.0.0:4003/graphql");
    info!("Health check: http://0.0.0.0:4003/health");

    axum::serve(listener, app).await?;

    Ok(())
}