# Task 12: Container Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_CONTAINER.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Контейнерная диаграмма Task 12 детализирует внутреннюю структуру систем, показывая конкретные контейнеры (приложения, сервисы, базы данных) и их взаимодействие. Каждый контейнер имеет прямое отражение в Docker конфигурациях и Kubernetes манифестах.

### Local Development Environment - Код реализации

#### Docker Compose Orchestration
**PlantUML элемент:**
```plantuml
Container(docker_compose, "Docker Compose", "YAML Configuration", "Orchestrates all services locally")
```

**Реализация в коде:**
```yaml
# docker-compose.yml - основная оркестрация
version: '3.8'

services:
  # Определение всех сервисов
  ugc-subgraph:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
    ports:
      - "4001:4001"
    depends_on:
      ugc-postgres:
        condition: service_healthy
      redis:
        condition: service_started
    environment:
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
      - REDIS_URL=redis://redis:6379
    networks:
      - federation-network
    restart: unless-stopped

networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

**Связь архитектуры с кодом:**
- **Orchestration logic** → `docker-compose.yml` service definitions
- **Service dependencies** → `depends_on` с health checks
- **Network isolation** → `networks` конфигурация
- **Environment management** → `environment` variables

#### Application Containers

##### UGC Subgraph Container
**PlantUML элемент:**
```plantuml
Container(ugc_container, "UGC Subgraph Container", "Docker/Rust", "Containerized UGC service")
```

**Dockerfile реализация:**
```dockerfile
# ugc-subgraph/Dockerfile - multi-stage build
FROM rust:1.75-slim as builder
WORKDIR /app

# Системные зависимости для компиляции
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Dependency caching optimization
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
RUN mkdir -p ugc-subgraph/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs
RUN cargo build --release --package ugc-subgraph

# Actual source compilation
COPY . .
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates libpq5 libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false ugc
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/
USER ugc
EXPOSE 4001

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```

**Rust application код:**
```rust
// ugc-subgraph/src/main.rs - основное приложение
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use async_graphql::{Schema, EmptySubscription};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация логирования
    tracing_subscriber::init();
    
    // Подключение к базе данных
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&database_url).await?;
    
    // Создание GraphQL схемы
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(pool.clone())
        .finish();
    
    // HTTP сервер
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))
        .layer(Extension(schema));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001").await?;
    tracing::info!("UGC Subgraph listening on port 4001");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn graphql_handler(
    schema: Extension<Schema<Query, Mutation, EmptySubscription>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ugc-subgraph",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
```

##### Apollo Router Container
**PlantUML элемент:**
```plantuml
Container(router_container, "Apollo Router Container", "Docker/Node.js", "Federation gateway")
```

**Dockerfile реализация:**
```dockerfile
# apollo-router/Dockerfile
FROM node:18-alpine as builder
WORKDIR /app

# Install Apollo Router
RUN npm install -g @apollo/rover
RUN curl -sSL https://router.apollo.dev/download/nix/latest | sh

FROM node:18-alpine
WORKDIR /app

# Copy router binary
COPY --from=builder /root/.rover/bin/router /usr/local/bin/
COPY --from=builder /usr/local/bin/rover /usr/local/bin/

# Configuration files
COPY router.yaml ./
COPY supergraph.graphql ./

EXPOSE 4000

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4000/health || exit 1

CMD ["router", "--config", "router.yaml", "--supergraph", "supergraph.graphql"]
```

**Router configuration:**
```yaml
# router.yaml - Apollo Router конфигурация
listen: 0.0.0.0:4000

supergraph:
  path: ./supergraph.graphql

subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    timeout: 30s
  users:
    routing_url: http://users-subgraph:4002/graphql
    timeout: 30s
  offers:
    routing_url: http://offers-subgraph:4004/graphql
    timeout: 30s

telemetry:
  metrics:
    prometheus:
      enabled: true
      listen: 0.0.0.0:9090
  tracing:
    jaeger:
      enabled: true
      endpoint: http://jaeger:14268/api/traces

cors:
  allow_any_origin: true
  allow_credentials: true
```

#### Infrastructure Containers

##### PostgreSQL Container
**PlantUML элемент:**
```plantuml
Container(postgres_container, "PostgreSQL Container", "Docker/PostgreSQL", "Database for development")
```

**Docker Compose конфигурация:**
```yaml
# docker-compose.yml - PostgreSQL service
ugc-postgres:
  image: postgres:14-alpine
  environment:
    - POSTGRES_DB=ugc_db
    - POSTGRES_USER=ugc_user
    - POSTGRES_PASSWORD=ugc_password
  volumes:
    - postgres_data:/var/lib/postgresql/data
    - ./ugc-subgraph/migrations:/docker-entrypoint-initdb.d:ro
  ports:
    - "5432:5432"
  networks:
    - federation-network
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U ugc_user -d ugc_db"]
    interval: 10s
    timeout: 5s
    retries: 5
  restart: unless-stopped
```

**Database migrations:**
```sql
-- ugc-subgraph/migrations/001_initial.sql
CREATE TABLE IF NOT EXISTS reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content TEXT NOT NULL,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    offer_id VARCHAR(255) NOT NULL,
    author_id VARCHAR(255) NOT NULL,
    moderation_status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX idx_reviews_author_id ON reviews(author_id);
CREATE INDEX idx_reviews_created_at ON reviews(created_at);

-- Trigger для автоматического обновления updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_reviews_updated_at 
    BEFORE UPDATE ON reviews 
    FOR EACH ROW 
    EXECUTE FUNCTION update_updated_at_column();
```

##### Redis Container
**PlantUML элемент:**
```plantuml
Container(redis_container, "Redis Container", "Docker/Redis", "Cache for development")
```

**Docker Compose конфигурация:**
```yaml
redis:
  image: redis:7-alpine
  ports:
    - "6379:6379"
  volumes:
    - redis_data:/data
  command: redis-server --appendonly yes --maxmemory 256mb --maxmemory-policy allkeys-lru
  networks:
    - federation-network
  healthcheck:
    test: ["CMD", "redis-cli", "ping"]
    interval: 10s
    timeout: 3s
    retries: 3
  restart: unless-stopped
```

### CI/CD System - Код реализация

#### GitHub Actions Container
**PlantUML элемент:**
```plantuml
Container(github_actions, "GitHub Actions", "YAML Workflows", "Automated CI/CD pipeline")
```

**Workflow реализация:**
```yaml
# .github/workflows/ci.yml - основной CI pipeline
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  # Lint и форматирование
  lint:
    name: Lint and Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  # Unit тесты
  test:
    name: Unit Tests
    runs-on: ubuntu-latest
    needs: lint
    
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: test_password
          POSTGRES_USER: test_user
          POSTGRES_DB: test_db
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432

    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Run tests
        run: cargo test --all-features --verbose
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
```

#### Docker Builder Container
**PlantUML элемент:**
```plantuml
Container(docker_builder, "Docker Builder", "Docker Buildx", "Builds multi-arch container images")
```

**Build workflow реализация:**
```yaml
# .github/workflows/ci.yml - Docker build matrix
build:
  name: Build Docker Images
  runs-on: ubuntu-latest
  needs: [lint, test]
  
  strategy:
    matrix:
      service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
      platform: [linux/amd64, linux/arm64]
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Set up Docker Buildx
      uses: docker/setup-buildx-action@v3
      with:
        platforms: linux/amd64,linux/arm64
    
    - name: Build multi-arch image
      uses: docker/build-push-action@v5
      with:
        context: .
        file: ./${{ matrix.service }}/Dockerfile
        platforms: ${{ matrix.platform }}
        push: false
        tags: |
          ${{ matrix.service }}:${{ github.sha }}
          ${{ matrix.service }}:latest
        cache-from: type=gha
        cache-to: type=gha,mode=max
        build-args: |
          BUILDKIT_INLINE_CACHE=1
```

#### Schema Validator Container
**PlantUML элемент:**
```plantuml
Container(schema_validator, "Schema Validator", "Apollo Rover", "Validates GraphQL federation schema")
```

**Schema validation реализация:**
```yaml
# .github/workflows/ci.yml - Schema validation job
schema-validation:
  name: GraphQL Schema Validation
  runs-on: ubuntu-latest
  needs: build
  
  steps:
    - uses: actions/checkout@v4
    
    - name: Install Apollo CLI
      run: |
        curl -sSL https://rover.apollo.dev/nix/latest | sh
        echo "$HOME/.rover/bin" >> $GITHUB_PATH
    
    - name: Start services for introspection
      run: |
        docker-compose up -d
        sleep 30  # Wait for services to be ready
    
    - name: Introspect subgraph schemas
      run: |
        # Introspect each subgraph
        rover subgraph introspect http://localhost:4001/graphql > ugc-schema.graphql
        rover subgraph introspect http://localhost:4002/graphql > users-schema.graphql
        rover subgraph introspect http://localhost:4004/graphql > offers-schema.graphql
    
    - name: Validate schema composition
      run: |
        # Compose supergraph schema
        rover supergraph compose --config supergraph.yaml > composed-schema.graphql
        echo "✅ Schema composition successful"
        
        # Validate federation compatibility
        rover graph check auto-ru-federation@staging --schema composed-schema.graphql
    
    - name: Test federation queries
      run: |
        # Test basic introspection
        curl -X POST http://localhost:4000/graphql \
          -H "Content-Type: application/json" \
          -d '{"query": "query { __schema { types { name } } }"}' | jq .
        
        # Test cross-subgraph query
        curl -X POST http://localhost:4000/graphql \
          -H "Content-Type: application/json" \
          -d '{
            "query": "query { reviews(first: 1) { edges { node { id author { name } offer { title } } } } }"
          }' | jq .
```

**Supergraph composition config:**
```yaml
# supergraph.yaml - Federation schema composition
federation_version: 2

subgraphs:
  ugc:
    routing_url: http://localhost:4001/graphql
    schema:
      file: ./ugc-schema.graphql
  
  users:
    routing_url: http://localhost:4002/graphql
    schema:
      file: ./users-schema.graphql
  
  offers:
    routing_url: http://localhost:4004/graphql
    schema:
      file: ./offers-schema.graphql
```

### Documentation System - Код реализация

#### README Documentation Container
**PlantUML элемент:**
```plantuml
Container(readme_docs, "README Documentation", "Markdown", "Project setup and usage guide")
```

**Markdown реализация:**
```markdown
# README.md - структурированная документация
# Auto.ru GraphQL Federation

Федеративная GraphQL архитектура для Auto.ru с микросервисами на Rust.

## 🚀 Quick Start

### Prerequisites
- Docker и Docker Compose
- Rust 1.75+
- Node.js 18+ (для Apollo CLI)

### Local Development

1. **Клонирование репозитория:**
   ```bash
   git clone <repository-url>
   cd auto-ru-graphql-federation
   ```

2. **Запуск всех сервисов:**
   ```bash
   # Используем Makefile для упрощения
   make dev
   
   # Или напрямую через Docker Compose
   docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
   ```

3. **Проверка состояния:**
   ```bash
   # Автоматическая проверка через Makefile
   make health
   
   # Или ручная проверка
   curl http://localhost:4001/health  # UGC Subgraph
   curl http://localhost:4002/health  # Users Subgraph
   curl http://localhost:4004/health  # Offers Subgraph
   ```

## 🏗️ Architecture

### Service Ports
- **Apollo Router** (4000) - Federation gateway
- **UGC Subgraph** (4001) - Reviews and ratings
- **Users Subgraph** (4002) - User management (stub)
- **Offers Subgraph** (4004) - Car offers (stub)
- **PostgreSQL** (5432) - Primary database
- **Redis** (6379) - Caching layer

### Development Commands
```bash
make dev      # Start development environment
make test     # Run all tests
make build    # Build Docker images
make clean    # Clean up resources
make logs     # View service logs
make health   # Check service health
```
```

#### Makefile Container
**PlantUML элемент:**
```plantuml
Container(makefile, "Makefile", "Make", "Development automation commands")
```

**Makefile реализация:**
```makefile
# Makefile - автоматизация команд разработки
.DEFAULT_GOAL := help
.PHONY: help build test clean dev prod logs health

# Colors for better UX
GREEN := \033[0;32m
YELLOW := \033[1;33m
BLUE := \033[0;34m
NC := \033[0m

help: ## Show available commands
	@echo "$(BLUE)Auto.ru GraphQL Federation - Development Commands$(NC)"
	@echo ""
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Start development environment with hot reload
	@echo "$(BLUE)Starting development environment...$(NC)"
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "$(GREEN)✅ Development environment started$(NC)"
	@echo "$(YELLOW)GraphQL Playground: http://localhost:4000/graphql$(NC)"

prod: ## Start production-like environment
	@echo "$(BLUE)Starting production environment...$(NC)"
	docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
	@echo "$(GREEN)✅ Production environment started$(NC)"

test: ## Run all tests
	@echo "$(BLUE)Running tests...$(NC)"
	cargo test --all-features --verbose
	@echo "$(GREEN)✅ Tests completed$(NC)"

build: ## Build all Docker images
	@echo "$(BLUE)Building Docker images...$(NC)"
	docker-compose build --parallel
	@echo "$(GREEN)✅ Build completed$(NC)"

health: ## Check health of all services
	@echo "$(BLUE)Checking service health...$(NC)"
	@echo "$(YELLOW)UGC Subgraph:$(NC)"
	@curl -s http://localhost:4001/health | jq . || echo "❌ Not responding"
	@echo "$(YELLOW)Users Subgraph:$(NC)"
	@curl -s http://localhost:4002/health | jq . || echo "❌ Not responding"
	@echo "$(YELLOW)Offers Subgraph:$(NC)"
	@curl -s http://localhost:4004/health | jq . || echo "❌ Not responding"
	@echo "$(YELLOW)Apollo Router:$(NC)"
	@curl -s http://localhost:4000/health | jq . || echo "❌ Not responding"

logs: ## Show logs from all services
	docker-compose logs -f

clean: ## Clean up containers and volumes
	@echo "$(BLUE)Cleaning up...$(NC)"
	docker-compose down -v --remove-orphans
	docker system prune -f
	@echo "$(GREEN)✅ Cleanup completed$(NC)"

schema: ## Validate and compose GraphQL schema
	@echo "$(BLUE)Composing federation schema...$(NC)"
	rover supergraph compose --config supergraph.yaml > supergraph.graphql
	@echo "$(GREEN)✅ Schema composition successful$(NC)"
```

### Container Relationships - Network Implementation

#### Service Discovery
**PlantUML связи:**
```plantuml
Rel(router_container, ugc_container, "Routes to", "HTTP/GraphQL")
Rel(router_container, users_container, "Routes to", "HTTP/GraphQL")
Rel(router_container, offers_container, "Routes to", "HTTP/GraphQL")
```

**Network код реализация:**
```yaml
# docker-compose.yml - network configuration
networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1

# Service discovery через DNS
services:
  apollo-router:
    networks:
      - federation-network
    environment:
      - UGC_ENDPOINT=http://ugc-subgraph:4001/graphql
      - USERS_ENDPOINT=http://users-subgraph:4002/graphql
      - OFFERS_ENDPOINT=http://offers-subgraph:4004/graphql
```

#### Database Connections
**PlantUML связи:**
```plantuml
Rel(ugc_container, postgres_container, "Connects to", "PostgreSQL")
Rel(ugc_container, redis_container, "Connects to", "Redis")
```

**Connection код реализация:**
```rust
// ugc-subgraph/src/database.rs - database connections
use sqlx::{PgPool, Pool, Postgres};
use redis::Client as RedisClient;

pub struct DatabaseConnections {
    pub postgres: PgPool,
    pub redis: RedisClient,
}

impl DatabaseConnections {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // PostgreSQL connection
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");
        
        let postgres = PgPool::connect(&database_url)
            .await
            .expect("Failed to connect to PostgreSQL");
        
        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&postgres)
            .await
            .expect("Failed to run migrations");
        
        // Redis connection
        let redis_url = std::env::var("REDIS_URL")
            .expect("REDIS_URL must be set");
        
        let redis = RedisClient::open(redis_url)
            .expect("Failed to create Redis client");
        
        // Test Redis connection
        let mut conn = redis.get_connection()
            .expect("Failed to connect to Redis");
        redis::cmd("PING").execute(&mut conn);
        
        Ok(Self { postgres, redis })
    }
}
```

### Заключение

Контейнерная диаграмма Task 12 обеспечивает прямую трассируемость между архитектурными решениями и кодом:

1. **Docker Containers** → Dockerfile и docker-compose.yml конфигурации
2. **Service Communication** → Network configuration и service discovery
3. **CI/CD Automation** → GitHub Actions workflows и build scripts
4. **Documentation** → Markdown файлы и automation scripts
5. **Database Integration** → Connection pools и migration scripts

Каждый контейнер в диаграмме имеет конкретную реализацию в коде, что обеспечивает полную согласованность между архитектурным дизайном и работающей системой.