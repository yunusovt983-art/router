# Task 12: Code Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_CODE.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Код-уровневая диаграмма Task 12 показывает конкретные файлы конфигурации, их содержимое и взаимосвязи на уровне исходного кода. Каждый компонент диаграммы представляет реальный файл с конкретным кодом, который можно выполнить и протестировать.

### Docker Configuration Files - Код реализация

#### UGC Dockerfile Component
**PlantUML элемент:**
```plantuml
Component(dockerfile_ugc_code, "ugc-subgraph/Dockerfile", "Multi-stage Dockerfile", "...")
```

**Файловая структура:**
```
ugc-subgraph/
├── Dockerfile                    # ← Этот файл
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── review.rs
│   │   └── rating.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── review_service.rs
│   │   └── auth_service.rs
│   └── graphql/
│       ├── mod.rs
│       ├── query.rs
│       ├── mutation.rs
│       └── types.rs
└── migrations/
    ├── 001_initial.sql
    └── 002_add_indexes.sql
```

**Полный код Dockerfile:**
```dockerfile
# ugc-subgraph/Dockerfile - Production-ready multi-stage build
# Соответствует архитектурным требованиям безопасности и производительности

# ===== BUILDER STAGE =====
FROM rust:1.75-slim as builder

# Metadata для трассируемости
LABEL maintainer="Auto.ru DevOps Team <devops@auto.ru>"
LABEL service="ugc-subgraph"
LABEL stage="builder"
LABEL version="1.0.0"

# Set working directory
WORKDIR /app

# Install system dependencies required for compilation
# Соответствует архитектурным требованиям для PostgreSQL и SSL
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    build-essential \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create app user for security
RUN groupadd -r app && useradd -r -g app app

# Copy dependency manifests first for better caching
# Архитектурное решение для оптимизации Docker layer caching
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
COPY shared/Cargo.toml ./shared/

# Create dummy source files to build dependencies
RUN mkdir -p ugc-subgraph/src shared/src && \
    echo "fn main() {println!(\"Dummy main for dependency caching\");}" > ugc-subgraph/src/main.rs && \
    echo "// Dummy lib for dependency caching" > shared/src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --package ugc-subgraph

# Remove dummy files
RUN rm -rf ugc-subgraph/src shared/src

# Copy actual source code
COPY . .

# Build the actual application
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# ===== RUNTIME STAGE =====
FROM debian:bookworm-slim as runtime

# Metadata
LABEL maintainer="Auto.ru DevOps Team <devops@auto.ru>"
LABEL service="ugc-subgraph"
LABEL stage="runtime"
LABEL version="1.0.0"

# Install only runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd -r ugc && useradd -r -g ugc -s /bin/false ugc

# Copy binary from builder stage
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/ugc-subgraph
RUN chmod +x /usr/local/bin/ugc-subgraph

# Create necessary directories
RUN mkdir -p /app/logs /app/cache && \
    chown -R ugc:ugc /app

# Switch to non-root user
USER ugc
WORKDIR /app

# Expose port - соответствует архитектурной диаграмме
EXPOSE 4001

# Health check - соответствует архитектурным требованиям observability
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

# Set default command
CMD ["ugc-subgraph"]
```*
*Связь с Rust кодом:**
```rust
// ugc-subgraph/src/main.rs - приложение, собираемое в Dockerfile
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use async_graphql::{Schema, EmptySubscription};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod models;
mod services;
mod graphql;

use graphql::{Query, Mutation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing - соответствует архитектурным требованиям observability
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "ugc_subgraph=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting UGC Subgraph service");

    // Database connection - соответствует архитектурной диаграмме
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set");
    
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    // Redis connection
    let redis_url = std::env::var("REDIS_URL")
        .expect("REDIS_URL environment variable must be set");
    
    let redis_client = redis::Client::open(redis_url)
        .expect("Failed to create Redis client");

    // GraphQL schema
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(pool.clone())
        .data(redis_client.clone())
        .finish();

    // HTTP server - порт соответствует Dockerfile EXPOSE 4001
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(Extension(schema))
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 4001));
    tracing::info!("UGC Subgraph listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// Health handler - используется в Dockerfile HEALTHCHECK
async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ugc-subgraph",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
```

#### Docker Compose Main Configuration
**PlantUML элемент:**
```plantuml
Component(compose_main_code, "docker-compose.yml", "Docker Compose Configuration", "...")
```

**Полный код docker-compose.yml:**
```yaml
# docker-compose.yml - Основная оркестрация сервисов
# Соответствует архитектурным требованиям контейнеризации

version: '3.8'

# Переменные окружения для переиспользования
x-common-variables: &common-variables
  RUST_LOG: ${RUST_LOG:-info}
  RUST_BACKTRACE: ${RUST_BACKTRACE:-0}
  OTEL_EXPORTER_JAEGER_ENDPOINT: http://jaeger:14268/api/traces

x-restart-policy: &restart-policy
  restart: unless-stopped

x-healthcheck-defaults: &healthcheck-defaults
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 40s

services:
  # ===== FEDERATION GATEWAY =====
  apollo-router:
    build:
      context: .
      dockerfile: apollo-router/Dockerfile
      args:
        - BUILDKIT_INLINE_CACHE=1
    container_name: auto-ru-apollo-router
    ports:
      - "${ROUTER_PORT:-4000}:4000"
    volumes:
      - ./router.yaml:/app/router.yaml:ro
      - ./supergraph.graphql:/app/supergraph.graphql:ro
    depends_on:
      ugc-subgraph:
        condition: service_healthy
      users-subgraph:
        condition: service_healthy
      offers-subgraph:
        condition: service_healthy
    environment:
      <<: *common-variables
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      <<: *healthcheck-defaults
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]

  # ===== UGC SUBGRAPH =====
  ugc-subgraph:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
      target: runtime
    container_name: auto-ru-ugc-subgraph
    ports:
      - "${UGC_PORT:-4001}:4001"
    depends_on:
      ugc-postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    environment:
      <<: *common-variables
      - DATABASE_URL=postgresql://${POSTGRES_USER:-ugc_user}:${POSTGRES_PASSWORD:-ugc_password}@ugc-postgres:5432/${POSTGRES_DB:-ugc_db}
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=${JWT_SECRET:-development-secret}
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      <<: *healthcheck-defaults
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]

networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
```

### CI/CD Configuration Files

#### GitHub Actions CI Workflow
**PlantUML элемент:**
```plantuml
Component(ci_workflow_code, ".github/workflows/ci.yml", "GitHub Actions CI", "...")
```

**Код CI workflow:**
```yaml
# .github/workflows/ci.yml - Основной CI pipeline
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
  test:
    runs-on: ubuntu-latest
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
      
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all-features
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
```

### Documentation Files

#### README Documentation
**PlantUML элемент:**
```plantuml
Component(readme_code, "README.md", "Main Documentation", "...")
```

**Код README.md:**
```markdown
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
   docker-compose up -d
   ```

3. **Проверка состояния:**
   ```bash
   curl http://localhost:4001/health  # UGC Subgraph
   curl http://localhost:4000/graphql # GraphQL Playground
   ```

## 🏗️ Architecture

### Service Ports
- **Apollo Router** (4000) - Federation gateway
- **UGC Subgraph** (4001) - Reviews and ratings
- **Users Subgraph** (4002) - User management
- **Offers Subgraph** (4004) - Car offers

### Development Commands
```bash
make dev      # Start development environment
make test     # Run all tests
make health   # Check service health
```
```

#### Makefile Automation
**PlantUML элемент:**
```plantuml
Component(makefile_code, "Makefile", "Development Automation", "...")
```

**Код Makefile:**
```makefile
.DEFAULT_GOAL := help
.PHONY: help build test dev health

GREEN := \033[0;32m
YELLOW := \033[1;33m
NC := \033[0m

help: ## Show available commands
	@echo "$(YELLOW)Auto.ru GraphQL Federation$(NC)"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Start development environment
	@echo "Starting development environment..."
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "✅ Development environment started"

test: ## Run all tests
	@echo "Running tests..."
	cargo test --all-features --verbose
	@echo "✅ Tests completed"

health: ## Check service health
	@echo "Checking service health..."
	@curl -s http://localhost:4001/health | jq . || echo "❌ UGC not responding"
	@curl -s http://localhost:4002/health | jq . || echo "❌ Users not responding"
```

### Infrastructure Configuration Files

#### Apollo Router Configuration
**PlantUML элемент:**
```plantuml
Component(router_config_code, "router.yaml", "Apollo Router Config", "...")
```

**Код router.yaml:**
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
  tracing:
    jaeger:
      enabled: true
      endpoint: http://jaeger:14268/api/traces

cors:
  allow_any_origin: true
```

### Заключение

Код-уровневая диаграмма Task 12 демонстрирует прямую связь между архитектурными компонентами и исполняемым кодом:

1. **Docker Files** → Конкретные Dockerfile с multi-stage builds
2. **Compose Config** → Полная оркестрация сервисов
3. **CI/CD Workflows** → Автоматизированные pipeline
4. **Documentation** → Исполняемые примеры и команды
5. **Infrastructure** → Конфигурационные файлы

Каждый файл можно запустить, протестировать и модифицировать, обеспечивая полную трассируемость от архитектуры к работающему коду.