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

// User entity - this is the primary entity for this subgraph
#[derive(SimpleObject, Clone)]
pub struct User {
    pub id: ID,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub created_at: String, // Using String for simplicity in the stub
    pub updated_at: String, // Using String for simplicity in the stub
}

// Mock data store
pub struct UserService {
    // In a real implementation, this would be a database connection
    users: Vec<User>,
}

impl UserService {
    pub fn new() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        // Create some mock users for testing
        let users = vec![
            User {
                id: ID::from("user-1"),
                name: "Иван Иванов".to_string(),
                email: "ivan@example.com".to_string(),
                phone: Some("+7-900-123-45-67".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            User {
                id: ID::from("user-2"),
                name: "Мария Петрова".to_string(),
                email: "maria@example.com".to_string(),
                phone: Some("+7-900-765-43-21".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            User {
                id: ID::from("user-3"),
                name: "Алексей Сидоров".to_string(),
                email: "alexey@example.com".to_string(),
                phone: None,
                created_at: now.clone(),
                updated_at: now,
            },
        ];

        Self { users }
    }

    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        self.users.iter().find(|u| u.id == id).cloned()
    }

    pub async fn get_users(&self) -> Vec<User> {
        self.users.clone()
    }
}

// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
    /// Get user by ID
    #[instrument(skip(self, ctx))]
    pub async fn user(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }

    /// Get all users (for testing purposes)
    #[instrument(skip(self, ctx))]
    pub async fn users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_users().await)
    }

    /// Federation entity resolver for User
    pub async fn find_user_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }
}

// GraphQL Mutation root (stub)
pub struct Mutation;

#[Object]
impl Mutation {
    /// Placeholder mutation
    pub async fn placeholder(&self) -> Result<String> {
        Ok("Users subgraph mutations not implemented yet".to_string())
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
        "service": "users-subgraph",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// JWKS endpoint (stub for JWT validation)
pub async fn jwks() -> Json<Value> {
    Json(json!({
        "keys": []
    }))
}

pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Users Subgraph...");

    // Create services
    let user_service = Arc::new(UserService::new());

    // Build GraphQL schema with federation support
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(user_service)
        .finish();

    // Create application state
    let state = AppState { schema };

    // Build router
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_check))
        .route("/.well-known/jwks.json", get(jwks))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let listener = TcpListener::bind("0.0.0.0:4002").await?;
    info!("Users Subgraph listening on http://0.0.0.0:4002");
    info!("GraphQL endpoint: http://0.0.0.0:4002/graphql");
    info!("Health check: http://0.0.0.0:4002/health");

    axum::serve(listener, app).await?;

    Ok(())
}