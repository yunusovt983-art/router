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

// Search result entity
#[derive(SimpleObject, Clone)]
pub struct SearchResult {
    pub id: ID,
    pub title: String,
    pub description: String,
    pub price: i32,
    pub location: String,
    pub image_url: Option<String>,
    pub relevance_score: f64,
    pub result_type: SearchResultType,
}

// Search result type enum
#[derive(async_graphql::Enum, Clone, Copy, PartialEq, Eq)]
pub enum SearchResultType {
    Offer,
    Brand,
    Model,
}

// Search filters input
#[derive(async_graphql::InputObject)]
pub struct SearchFilters {
    pub query: Option<String>,
    pub min_price: Option<i32>,
    pub max_price: Option<i32>,
    pub location: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
}

// Search connection for pagination
#[derive(SimpleObject)]
pub struct SearchConnection {
    pub edges: Vec<SearchEdge>,
    pub page_info: PageInfo,
    pub total_count: i32,
}

#[derive(SimpleObject)]
pub struct SearchEdge {
    pub node: SearchResult,
    pub cursor: String,
}

#[derive(SimpleObject)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

// Autocomplete suggestion
#[derive(SimpleObject, Clone)]
pub struct Suggestion {
    pub text: String,
    pub category: String,
    pub count: i32,
}

// Mock search service
pub struct SearchService {
    // In a real implementation, this would connect to Elasticsearch
    mock_results: Vec<SearchResult>,
    mock_suggestions: Vec<Suggestion>,
}

impl SearchService {
    pub fn new() -> Self {
        // Create some mock search results
        let mock_results = vec![
            SearchResult {
                id: ID::from("offer-1"),
                title: "BMW X5 2020".to_string(),
                description: "Отличный внедорожник в идеальном состоянии".to_string(),
                price: 3500000,
                location: "Москва".to_string(),
                image_url: Some("https://example.com/bmw-x5.jpg".to_string()),
                relevance_score: 0.95,
                result_type: SearchResultType::Offer,
            },
            SearchResult {
                id: ID::from("offer-2"),
                title: "Toyota Camry 2019".to_string(),
                description: "Надежный седан для семьи".to_string(),
                price: 2200000,
                location: "Санкт-Петербург".to_string(),
                image_url: Some("https://example.com/toyota-camry.jpg".to_string()),
                relevance_score: 0.87,
                result_type: SearchResultType::Offer,
            },
            SearchResult {
                id: ID::from("offer-3"),
                title: "Mercedes-Benz C-Class 2021".to_string(),
                description: "Премиальный седан с полной комплектацией".to_string(),
                price: 4100000,
                location: "Екатеринбург".to_string(),
                image_url: Some("https://example.com/mercedes-c-class.jpg".to_string()),
                relevance_score: 0.92,
                result_type: SearchResultType::Offer,
            },
        ];

        // Create some mock suggestions
        let mock_suggestions = vec![
            Suggestion {
                text: "BMW".to_string(),
                category: "brand".to_string(),
                count: 1250,
            },
            Suggestion {
                text: "BMW X5".to_string(),
                category: "model".to_string(),
                count: 340,
            },
            Suggestion {
                text: "Toyota Camry".to_string(),
                category: "model".to_string(),
                count: 890,
            },
            Suggestion {
                text: "Mercedes-Benz".to_string(),
                category: "brand".to_string(),
                count: 760,
            },
        ];

        Self {
            mock_results,
            mock_suggestions,
        }
    }

    pub async fn search(
        &self,
        filters: Option<SearchFilters>,
        first: Option<i32>,
        after: Option<String>,
    ) -> SearchConnection {
        // In a real implementation, this would query Elasticsearch
        let mut results = self.mock_results.clone();

        // Apply basic filtering
        if let Some(filters) = filters {
            if let Some(query) = filters.query {
                let query_lower = query.to_lowercase();
                results.retain(|r| {
                    r.title.to_lowercase().contains(&query_lower)
                        || r.description.to_lowercase().contains(&query_lower)
                });
            }

            if let Some(min_price) = filters.min_price {
                results.retain(|r| r.price >= min_price);
            }

            if let Some(max_price) = filters.max_price {
                results.retain(|r| r.price <= max_price);
            }

            if let Some(location) = filters.location {
                let location_lower = location.to_lowercase();
                results.retain(|r| r.location.to_lowercase().contains(&location_lower));
            }
        }

        // Sort by relevance score
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        // Apply pagination
        let limit = first.unwrap_or(10) as usize;
        let start_index = 0; // In a real implementation, parse the cursor
        let end_index = std::cmp::min(start_index + limit, results.len());
        let page_results = results[start_index..end_index].to_vec();

        let edges: Vec<SearchEdge> = page_results
            .into_iter()
            .enumerate()
            .map(|(i, result)| SearchEdge {
                cursor: format!("cursor_{}", start_index + i),
                node: result,
            })
            .collect();

        let has_next_page = end_index < results.len();
        let has_previous_page = start_index > 0;

        SearchConnection {
            edges,
            page_info: PageInfo {
                has_next_page,
                has_previous_page,
                start_cursor: if !edges.is_empty() {
                    Some(edges.first().unwrap().cursor.clone())
                } else {
                    None
                },
                end_cursor: if !edges.is_empty() {
                    Some(edges.last().unwrap().cursor.clone())
                } else {
                    None
                },
            },
            total_count: results.len() as i32,
        }
    }

    pub async fn autocomplete(&self, query: String) -> Vec<Suggestion> {
        let query_lower = query.to_lowercase();
        self.mock_suggestions
            .iter()
            .filter(|s| s.text.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }
}

// GraphQL Query root
pub struct Query;

#[Object]
impl Query {
    /// Search for offers, brands, models, etc.
    #[instrument(skip(self, ctx))]
    async fn search(
        &self,
        ctx: &Context<'_>,
        filters: Option<SearchFilters>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<SearchConnection> {
        let service = ctx.data::<Arc<SearchService>>()?;
        Ok(service.search(filters, first, after).await)
    }

    /// Get autocomplete suggestions
    #[instrument(skip(self, ctx))]
    async fn autocomplete(&self, ctx: &Context<'_>, query: String) -> Result<Vec<Suggestion>> {
        let service = ctx.data::<Arc<SearchService>>()?;
        Ok(service.autocomplete(query).await)
    }
}

// GraphQL Mutation root (stub)
pub struct Mutation;

#[Object]
impl Mutation {
    /// Placeholder mutation
    async fn placeholder(&self) -> Result<String> {
        Ok("Search subgraph mutations not implemented yet".to_string())
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
        "service": "search-subgraph",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Search Subgraph...");

    // Create services
    let search_service = Arc::new(SearchService::new());

    // Build GraphQL schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(search_service)
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
    let listener = TcpListener::bind("0.0.0.0:4005").await?;
    info!("Search Subgraph listening on http://0.0.0.0:4005");
    info!("GraphQL endpoint: http://0.0.0.0:4005/graphql");
    info!("Health check: http://0.0.0.0:4005/health");

    axum::serve(listener, app).await?;

    Ok(())
}