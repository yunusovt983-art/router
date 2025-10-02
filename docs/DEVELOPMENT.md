# Development Guide

This guide provides comprehensive information for developers working on the Auto.ru GraphQL Federation project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Adding New Features](#adding-new-features)
- [Testing Strategy](#testing-strategy)
- [Code Quality](#code-quality)
- [Debugging](#debugging)
- [Performance Optimization](#performance-optimization)
- [Best Practices](#best-practices)

## Getting Started

### Prerequisites

- **Rust**: 1.75+ (install via [rustup](https://rustup.rs/))
- **Docker**: 20.10+ with Docker Compose 2.0+
- **Node.js**: 18+ (for tooling and schema validation)
- **Git**: Latest version
- **IDE**: VS Code with Rust Analyzer extension (recommended)

### Initial Setup

```bash
# Clone the repository
git clone <repository-url>
cd auto-ru-graphql-federation

# Install Rust toolchain and components
rustup component add rustfmt clippy

# Install development tools
cargo install cargo-watch cargo-audit sqlx-cli

# Setup development environment
make setup

# Verify installation
make check
```

### IDE Configuration

#### VS Code Extensions
- **rust-analyzer**: Rust language server
- **GraphQL**: GraphQL syntax highlighting
- **Docker**: Docker support
- **YAML**: YAML language support
- **GitLens**: Git integration

#### VS Code Settings (.vscode/settings.json)
```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.inlayHints.enable": true,
  "files.watcherExclude": {
    "**/target/**": true
  },
  "graphql.useSchemaFileForIntrospection": true
}
```

## Development Environment

### Environment Modes

#### 1. Full Docker Development
```bash
# Start everything in Docker
make start-dev

# View logs
make logs-ugc
make logs-router
```

#### 2. Hybrid Development (Recommended)
```bash
# Start infrastructure in Docker
make start-infra

# Run subgraphs locally for hot reload
make dev-ugc    # Terminal 1
make dev-users  # Terminal 2
make dev-offers # Terminal 3

# Start router
make start-router
```

#### 3. Local Development
```bash
# Start databases manually
docker-compose up -d postgres redis elasticsearch

# Set environment variables
export DATABASE_URL="postgresql://postgres:password@localhost:5432/ugc_db"
export REDIS_URL="redis://localhost:6379"

# Run services
cd ugc-subgraph && cargo run
```

### Environment Variables

Create a `.env` file in the project root:

```bash
# Database connections
UGC_DATABASE_URL=postgresql://postgres:password@localhost:5432/ugc_db
USERS_DATABASE_URL=postgresql://postgres:password@localhost:5433/users_db
OFFERS_DATABASE_URL=postgresql://postgres:password@localhost:5434/offers_db
CATALOG_DATABASE_URL=postgresql://postgres:password@localhost:5435/catalog_db

# External services
REDIS_URL=redis://localhost:6379
ELASTICSEARCH_URL=http://localhost:9200

# Security
JWT_SECRET=dev-secret-key-change-in-production

# Logging
RUST_LOG=debug
RUST_BACKTRACE=1

# Development flags
ENVIRONMENT=development
```

## Project Structure

```
auto-ru-graphql-federation/
├── Cargo.toml                 # Workspace configuration
├── Makefile                   # Development commands
├── docker-compose.yml         # Development infrastructure
├── router.yaml               # Apollo Router configuration
├── .github/workflows/        # CI/CD pipelines
├── docs/                     # Documentation
├── scripts/                  # Utility scripts
├── schemas/                  # GraphQL schemas
│
├── ugc-subgraph/             # User Generated Content service
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── src/
│   │   ├── main.rs           # Application entry point
│   │   ├── config.rs         # Configuration management
│   │   ├── database.rs       # Database connection
│   │   ├── error.rs          # Error types
│   │   ├── graphql/          # GraphQL schema and resolvers
│   │   ├── models/           # Data models
│   │   ├── repository/       # Data access layer
│   │   └── service/          # Business logic
│   ├── migrations/           # Database migrations
│   └── tests/               # Test files
│
├── users-subgraph/          # User management service
├── offers-subgraph/         # Car listings service
├── catalog-subgraph/        # Car catalog service
└── search-subgraph/         # Search service
```

### Subgraph Architecture

Each subgraph follows a layered architecture:

```
src/
├── main.rs                   # Application bootstrap
├── config.rs                 # Configuration
├── database.rs               # Database setup
├── error.rs                  # Error handling
├── health.rs                 # Health checks
├── telemetry.rs             # Observability
│
├── graphql/
│   ├── mod.rs               # GraphQL schema
│   ├── query.rs             # Query resolvers
│   ├── mutation.rs          # Mutation resolvers
│   ├── subscription.rs      # Subscription resolvers
│   └── types.rs             # GraphQL types
│
├── models/
│   ├── mod.rs               # Data models
│   ├── review.rs            # Review model
│   └── user.rs              # User model
│
├── repository/
│   ├── mod.rs               # Repository trait
│   ├── review.rs            # Review repository
│   └── user.rs              # User repository
│
└── service/
    ├── mod.rs               # Service layer
    ├── review.rs            # Review service
    └── auth.rs              # Authentication service
```

## Development Workflow

### Daily Development

```bash
# 1. Start development environment
make start-infra

# 2. Run your subgraph with hot reload
cd ugc-subgraph
cargo watch -x run

# 3. Make changes and test
# Files are automatically recompiled on save

# 4. Run tests
cargo test

# 5. Check code quality
make clippy
make fmt
```

### Feature Development

```bash
# 1. Create feature branch
git checkout -b feature/new-review-system

# 2. Implement changes
# ... make your changes ...

# 3. Run comprehensive tests
make test
make check

# 4. Update documentation
# ... update relevant docs ...

# 5. Commit and push
git add .
git commit -m "feat: implement new review system"
git push origin feature/new-review-system

# 6. Create pull request
```

### Database Migrations

```bash
# Create new migration
cd ugc-subgraph
sqlx migrate add create_reviews_table

# Edit the migration file
# migrations/001_create_reviews_table.sql

# Run migration
sqlx migrate run --database-url $UGC_DATABASE_URL

# Revert migration (if needed)
sqlx migrate revert --database-url $UGC_DATABASE_URL
```

## Adding New Features

### Adding a New Subgraph

1. **Create subgraph directory**:
```bash
mkdir new-subgraph
cd new-subgraph
```

2. **Create Cargo.toml**:
```toml
[package]
name = "new-subgraph"
version = "0.1.0"
edition = "2021"

[dependencies]
async-graphql = "6.0"
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono"] }
# ... other dependencies
```

3. **Add to workspace** (root Cargo.toml):
```toml
[workspace]
members = [
    "ugc-subgraph",
    "users-subgraph",
    "new-subgraph",  # Add here
]
```

4. **Create basic structure**:
```bash
mkdir -p src/{graphql,models,repository,service}
touch src/main.rs src/config.rs src/database.rs
```

5. **Implement GraphQL schema**:
```rust
// src/graphql/mod.rs
use async_graphql::{Schema, Object, Result};

pub struct Query;

#[Object]
impl Query {
    async fn hello(&self) -> &str {
        "Hello from new subgraph!"
    }
}

pub type AppSchema = Schema<Query, EmptyMutation, EmptySubscription>;
```

6. **Add to Docker Compose**:
```yaml
new-subgraph:
  build:
    context: .
    dockerfile: new-subgraph/Dockerfile
  ports:
    - "4006:4006"
  environment:
    - DATABASE_URL=postgresql://postgres:password@postgres:5432/new_db
```

7. **Update router configuration**:
```yaml
# router.yaml
subgraphs:
  new:
    routing_url: http://new-subgraph:4006/graphql
```

### Adding a New GraphQL Type

1. **Define the model**:
```rust
// src/models/product.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

2. **Create GraphQL type**:
```rust
// src/graphql/types.rs
use async_graphql::{Object, ID};
use crate::models::Product as ProductModel;

#[Object]
impl ProductModel {
    async fn id(&self) -> ID {
        ID(self.id.to_string())
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    async fn price(&self) -> i32 {
        self.price
    }
}
```

3. **Add repository methods**:
```rust
// src/repository/product.rs
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::Product;

#[async_trait]
pub trait ProductRepository {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Product>>;
    async fn create(&self, product: CreateProductInput) -> Result<Product>;
}

pub struct PostgresProductRepository {
    pool: PgPool,
}

#[async_trait]
impl ProductRepository for PostgresProductRepository {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Product>> {
        let product = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(product)
    }
}
```

4. **Add resolvers**:
```rust
// src/graphql/query.rs
#[Object]
impl Query {
    async fn product(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Product>> {
        let repository = ctx.data::<Arc<dyn ProductRepository>>()?;
        let product_id = Uuid::parse_str(&id)?;
        repository.get_by_id(product_id).await
    }
}
```

### Adding Federation Directives

1. **Entity with key**:
```rust
use async_graphql::{Object, ID};

#[derive(async_graphql::SimpleObject)]
#[graphql(complex)]
pub struct Product {
    #[graphql(external)]
    pub id: ID,
    pub name: String,
}

#[ComplexObject]
impl Product {
    #[graphql(entity)]
    async fn find_by_id(ctx: &Context<'_>, id: ID) -> Result<Option<Product>> {
        // Reference resolver implementation
    }
}
```

2. **Extending external types**:
```rust
#[derive(async_graphql::SimpleObject)]
#[graphql(extends)]
pub struct User {
    #[graphql(external)]
    pub id: ID,
    
    // New fields added by this subgraph
    pub products: Vec<Product>,
}
```

## Testing Strategy

### Unit Tests

```rust
// src/service/review.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_create_review() {
        let mut mock_repo = MockReviewRepository::new();
        mock_repo
            .expect_create()
            .with(eq(CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "Great!".to_string(),
            }))
            .times(1)
            .returning(|_| Ok(Review { /* ... */ }));

        let service = ReviewService::new(Arc::new(mock_repo));
        let result = service.create_review(input, user_id).await;
        
        assert!(result.is_ok());
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs
use testcontainers::*;
use sqlx::PgPool;

#[tokio::test]
async fn test_review_creation_flow() {
    let docker = clients::Cli::default();
    let postgres = docker.run(images::postgres::Postgres::default());
    
    let database_url = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        postgres.get_host_port_ipv4(5432)
    );
    
    let pool = PgPool::connect(&database_url).await.unwrap();
    
    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    
    // Test your service
    let service = ReviewService::new(pool);
    let result = service.create_review(input, user_id).await;
    
    assert!(result.is_ok());
}
```

### GraphQL Tests

```rust
// tests/graphql_test.rs
use async_graphql::*;

#[tokio::test]
async fn test_review_query() {
    let schema = create_test_schema().await;
    
    let query = r#"
        query GetReview($id: ID!) {
            review(id: $id) {
                id
                rating
                text
                author {
                    name
                }
            }
        }
    "#;
    
    let result = schema
        .execute(Request::new(query).variables(variables! {
            "id": "test-review-id"
        }))
        .await;
    
    assert!(result.errors.is_empty());
    assert!(result.data.is_some());
}
```

### End-to-End Tests

```bash
# tests/e2e/test_federation.sh
#!/bin/bash

# Start services
docker-compose up -d

# Wait for services to be ready
./scripts/wait-for-services.sh

# Run GraphQL queries
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "{ offers(first: 1) { edges { node { id reviews(first: 1) { edges { node { rating } } } } } } }"
  }' | jq '.errors // empty' | grep -q null

echo "E2E tests passed!"
```

## Code Quality

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Linting

```bash
# Run clippy
cargo clippy -- -D warnings

# Run clippy with all features
cargo clippy --all-features -- -D warnings
```

### Security Audit

```bash
# Install cargo-audit
cargo install cargo-audit

# Run security audit
cargo audit

# Check for known vulnerabilities
cargo audit --deny warnings
```

### Code Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/
```

## Debugging

### Logging

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self), fields(user_id = %user_id))]
pub async fn create_review(&self, input: CreateReviewInput, user_id: Uuid) -> Result<Review> {
    info!("Creating review for offer {}", input.offer_id);
    
    // Implementation
    debug!("Validating review input");
    
    match self.repository.create(input, user_id).await {
        Ok(review) => {
            info!("Review created successfully: {}", review.id);
            Ok(review)
        }
        Err(e) => {
            error!("Failed to create review: {}", e);
            Err(e)
        }
    }
}
```

### Debug Configuration

```bash
# Enable debug logging
export RUST_LOG=debug

# Enable backtraces
export RUST_BACKTRACE=1

# Enable full backtraces
export RUST_BACKTRACE=full
```

### Database Debugging

```rust
// Enable SQL query logging
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

// Log all SQL queries
sqlx::query!("SELECT * FROM reviews WHERE id = $1", review_id)
    .fetch_one(&pool)
    .await?;
```

### GraphQL Debugging

```rust
use async_graphql::{Schema, extensions::Logger};

let schema = Schema::build(Query, Mutation, Subscription)
    .extension(Logger) // Add query logging
    .finish();
```

## Performance Optimization

### Database Optimization

1. **Use prepared statements**:
```rust
// Good: Uses prepared statement
sqlx::query_as!(Review, "SELECT * FROM reviews WHERE offer_id = $1", offer_id)
    .fetch_all(&pool)
    .await?;

// Avoid: Dynamic query construction
let query = format!("SELECT * FROM reviews WHERE offer_id = '{}'", offer_id);
```

2. **Add database indexes**:
```sql
-- migrations/003_add_indexes.sql
CREATE INDEX idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX idx_reviews_author_id ON reviews(author_id);
CREATE INDEX idx_reviews_created_at ON reviews(created_at DESC);
```

3. **Use connection pooling**:
```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .acquire_timeout(Duration::from_secs(30))
    .connect(&database_url)
    .await?;
```

### GraphQL Optimization

1. **Implement DataLoader**:
```rust
use async_graphql::dataloader::*;

pub struct UserLoader {
    pool: PgPool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for UserLoader {
    type Value = User;
    type Error = sqlx::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let users = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ANY($1)",
            keys
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users.into_iter().map(|user| (user.id, user)).collect())
    }
}
```

2. **Query complexity limiting**:
```rust
use async_graphql::extensions::analyzer::*;

let schema = Schema::build(Query, Mutation, Subscription)
    .extension(Analyzer::new().depth_limit(10).complexity_limit(1000))
    .finish();
```

### Caching

1. **Redis caching**:
```rust
use redis::AsyncCommands;

pub struct CacheService {
    client: redis::Client,
}

impl CacheService {
    pub async fn get_review(&self, id: Uuid) -> Result<Option<Review>> {
        let mut conn = self.client.get_async_connection().await?;
        let cached: Option<String> = conn.get(format!("review:{}", id)).await?;
        
        match cached {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub async fn set_review(&self, review: &Review) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        let data = serde_json::to_string(review)?;
        conn.set_ex(format!("review:{}", review.id), data, 3600).await?;
        Ok(())
    }
}
```

## Best Practices

### Error Handling

1. **Use typed errors**:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReviewError {
    #[error("Review not found: {id}")]
    NotFound { id: Uuid },
    
    #[error("Unauthorized access to review {id}")]
    Unauthorized { id: Uuid },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

2. **Convert to GraphQL errors**:
```rust
impl From<ReviewError> for async_graphql::Error {
    fn from(err: ReviewError) -> Self {
        match err {
            ReviewError::NotFound { id } => {
                async_graphql::Error::new("Review not found")
                    .extend_with(|_, e| e.set("code", "NOT_FOUND"))
                    .extend_with(|_, e| e.set("reviewId", id.to_string()))
            }
            // ... other variants
        }
    }
}
```

### Security

1. **Input validation**:
```rust
use validator::{Validate, ValidationError};

#[derive(Validate)]
pub struct CreateReviewInput {
    pub offer_id: Uuid,
    
    #[validate(range(min = 1, max = 5))]
    pub rating: i32,
    
    #[validate(length(min = 10, max = 1000))]
    pub text: String,
}
```

2. **Authorization guards**:
```rust
use async_graphql::{Guard, Context, Result};

pub struct RequireAuth;

#[async_trait::async_trait]
impl Guard for RequireAuth {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        match ctx.data_opt::<UserContext>() {
            Some(_) => Ok(()),
            None => Err("Authentication required".into()),
        }
    }
}
```

### Configuration

1. **Environment-based config**:
```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::from_env()
    }
}
```

### Documentation

1. **GraphQL schema documentation**:
```rust
#[Object]
impl Query {
    /// Get a review by ID
    /// 
    /// Returns the review if found and accessible by the current user.
    /// Requires authentication for private reviews.
    async fn review(
        &self,
        ctx: &Context<'_>,
        /// The unique identifier of the review
        id: ID,
    ) -> Result<Option<Review>> {
        // Implementation
    }
}
```

2. **Code documentation**:
```rust
/// Service for managing user-generated reviews and ratings.
/// 
/// This service handles CRUD operations for reviews, including
/// validation, authorization, and cache management.
pub struct ReviewService {
    repository: Arc<dyn ReviewRepository>,
    cache: Arc<CacheService>,
}

impl ReviewService {
    /// Creates a new review for the specified offer.
    /// 
    /// # Arguments
    /// 
    /// * `input` - The review data to create
    /// * `user_id` - The ID of the user creating the review
    /// 
    /// # Returns
    /// 
    /// Returns the created review or an error if creation fails.
    /// 
    /// # Errors
    /// 
    /// This function will return an error if:
    /// - The user is not authorized to create reviews
    /// - The offer does not exist
    /// - The input validation fails
    pub async fn create_review(
        &self,
        input: CreateReviewInput,
        user_id: Uuid,
    ) -> Result<Review, ReviewError> {
        // Implementation
    }
}
```

This development guide provides comprehensive coverage of the development process, from initial setup to advanced optimization techniques, ensuring developers can effectively contribute to the project.