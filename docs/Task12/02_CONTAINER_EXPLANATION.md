# Task 12: Container Level Architecture Explanation
## Настройка среды разработки и деплоя - Контейнерная диаграмма

### Обзор контейнерной архитектуры

Контейнерная диаграмма Task 12 детализирует внутреннюю структуру системы разработки и деплоя, показывая конкретные контейнеры (приложения, сервисы, базы данных) и их взаимодействие в рамках каждой системы.

### Система Local Development Environment

#### Docker Compose Orchestration

**Docker Compose** - центральный оркестратор локальной среды разработки

**Конфигурация:**
```yaml
version: '3.8'

services:
  apollo-router:
    build: ./apollo-router
    ports: ["4000:4000"]
    depends_on: [ugc-subgraph, users-subgraph, offers-subgraph]
    volumes:
      - ./router.yaml:/app/router.yaml
    environment:
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml

  ugc-subgraph:
    build: ./ugc-subgraph
    ports: ["4001:4001"]
    depends_on: [ugc-postgres, redis]
    environment:
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info

networks:
  federation-network:
    driver: bridge
```

#### Application Containers

##### 1. UGC Subgraph Container
**Технология:** Docker/Rust
**Порт:** 4001
**Назначение:** Основной сервис для отзывов и рейтингов

**Dockerfile структура:**
```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
# Установка системных зависимостей
RUN apt-get update && apt-get install -y pkg-config libssl-dev libpq-dev

# Копирование и сборка зависимостей (кеширование)
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --package ugc-subgraph

# Копирование исходного кода и финальная сборка
COPY . .
RUN cargo build --release --package ugc-subgraph

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libpq5 libssl3
RUN useradd -r -s /bin/false ugc
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/
USER ugc
EXPOSE 4001
CMD ["ugc-subgraph"]
```

**Особенности:**
- Multi-stage build для оптимизации размера
- Non-root пользователь для безопасности
- Health check endpoint
- Structured logging

##### 2. Users Subgraph Container
**Технология:** Docker/Rust
**Порт:** 4002
**Назначение:** Заглушка для сервиса пользователей

**Конфигурация:**
```rust
// Simplified stub implementation
#[derive(SimpleObject)]
struct User {
    id: ID,
    name: String,
    email: String,
}

#[Object]
impl Query {
    async fn user(&self, id: ID) -> Result<User> {
        // Mock implementation for development
        Ok(User {
            id,
            name: "Mock User".to_string(),
            email: "mock@example.com".to_string(),
        })
    }
}
```

##### 3. Offers Subgraph Container
**Технология:** Docker/Rust
**Порт:** 4004
**Назначение:** Заглушка для сервиса объявлений

**Federation Schema:**
```graphql
type Offer @key(fields: "id") {
  id: ID!
  title: String!
  price: Float!
  car: Car!
}

type Car {
  make: String!
  model: String!
  year: Int!
}
```

##### 4. Apollo Router Container
**Технология:** Docker/Node.js
**Порт:** 4000
**Назначение:** Federation gateway

**Router Configuration:**
```yaml
listen: 0.0.0.0:4000

supergraph:
  path: ./supergraph.graphql

subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
  users:
    routing_url: http://users-subgraph:4002/graphql
  offers:
    routing_url: http://offers-subgraph:4004/graphql

telemetry:
  metrics:
    prometheus:
      enabled: true
  tracing:
    jaeger:
      enabled: true
```

#### Infrastructure Containers

##### 1. PostgreSQL Container
**Технология:** Docker/PostgreSQL
**Порт:** 5432
**Назначение:** База данных для UGC сервиса

**Конфигурация:**
```yaml
ugc-postgres:
  image: postgres:14
  environment:
    - POSTGRES_DB=ugc_db
    - POSTGRES_USER=ugc_user
    - POSTGRES_PASSWORD=ugc_password
  volumes:
    - postgres_data:/var/lib/postgresql/data
    - ./migrations:/docker-entrypoint-initdb.d
  ports:
    - "5432:5432"
```

##### 2. Redis Container
**Технология:** Docker/Redis
**Порт:** 6379
**Назначение:** Кеширование для UGC сервиса

**Конфигурация:**
```yaml
redis:
  image: redis:7-alpine
  ports:
    - "6379:6379"
  volumes:
    - redis_data:/data
  command: redis-server --appendonly yes
```

### Система CI/CD Pipeline

#### GitHub Actions Containers

##### 1. GitHub Actions Runner
**Технология:** YAML Workflows
**Назначение:** Автоматизированный CI/CD pipeline

**Workflow Structure:**
```yaml
name: CI
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: test_password
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all-features
```

##### 2. Test Runner
**Технология:** Rust/Cargo
**Назначение:** Выполнение unit и integration тестов

**Test Configuration:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;
    
    #[sqlx::test]
    async fn test_create_review(pool: PgPool) {
        let review_service = ReviewService::new(pool);
        let input = CreateReviewInput {
            content: "Great car!".to_string(),
            rating: 5,
            offer_id: "offer_1".to_string(),
        };
        
        let result = review_service.create_review(input).await;
        assert!(result.is_ok());
    }
}
```

##### 3. Docker Builder
**Технология:** Docker Buildx
**Назначение:** Сборка multi-arch контейнерных образов

**Build Matrix:**
```yaml
strategy:
  matrix:
    service: [ugc-subgraph, users-subgraph, offers-subgraph]

steps:
  - name: Set up Docker Buildx
    uses: docker/setup-buildx-action@v2
    
  - name: Build Docker image
    uses: docker/build-push-action@v3
    with:
      context: .
      file: ./${{ matrix.service }}/Dockerfile
      platforms: linux/amd64,linux/arm64
      push: false
      tags: ${{ matrix.service }}:${{ github.sha }}
      cache-from: type=gha
      cache-to: type=gha,mode=max
```

##### 4. Schema Validator
**Технология:** Apollo Rover
**Назначение:** Валидация GraphQL federation схемы

**Validation Process:**
```bash
# Install Apollo CLI
curl -sSL https://rover.apollo.dev/nix/latest | sh

# Start services for schema introspection
docker-compose up -d

# Compose supergraph schema
rover supergraph compose --config supergraph.yaml > composed-schema.graphql

# Validate federation compatibility
rover graph check auto-ru-federation@staging --schema composed-schema.graphql
```

##### 5. Security Scanner
**Технология:** Trivy/Snyk
**Назначение:** Сканирование уязвимостей

**Security Workflow:**
```yaml
security:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v3
    
    - name: Run Trivy vulnerability scanner
      uses: aquasecurity/trivy-action@master
      with:
        scan-type: 'fs'
        scan-ref: '.'
        format: 'sarif'
        output: 'trivy-results.sarif'
    
    - name: Upload Trivy scan results
      uses: github/codeql-action/upload-sarif@v2
      with:
        sarif_file: 'trivy-results.sarif'
```

### Система Documentation

#### Documentation Containers

##### 1. README Documentation
**Технология:** Markdown
**Назначение:** Основная документация проекта

**Структура:**
```markdown
# Auto.ru GraphQL Federation

## Quick Start
### Prerequisites
- Docker and Docker Compose
- Rust 1.75+
- Node.js 18+ (for Apollo CLI)

### Local Development
1. Clone repository
2. Start services: `docker-compose up -d`
3. Check health: `curl http://localhost:4001/health`

## Architecture
### Subgraphs
- UGC Subgraph (4001) - Reviews and ratings
- Users Subgraph (4002) - Users (stub)
- Offers Subgraph (4004) - Car offers (stub)

### Federation Gateway
- Apollo Router (4000) - Federation gateway
```

##### 2. API Documentation
**Технология:** Markdown/GraphQL
**Назначение:** GraphQL API reference

**API Examples:**
```graphql
# Federated query example
query GetReviewWithDetails($reviewId: ID!) {
  review(id: $reviewId) {
    id
    content
    rating
    author {          # Resolved from Users subgraph
      id
      name
      email
    }
    offer {           # Resolved from Offers subgraph
      id
      title
      price
      car {
        make
        model
        year
      }
    }
  }
}
```

##### 3. Architecture Documentation
**Технология:** Markdown/PlantUML
**Назначение:** Архитектурные диаграммы и описания

##### 4. Makefile
**Технология:** Make
**Назначение:** Автоматизация команд разработки

**Commands:**
```makefile
.PHONY: help build test clean dev prod logs

dev: ## Start development environment
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

test: ## Run all tests
	cargo test --all-features

build: ## Build all Docker images
	docker-compose build

health: ## Check health of all services
	@curl -s http://localhost:4001/health | jq .
	@curl -s http://localhost:4002/health | jq .
	@curl -s http://localhost:4004/health | jq .
```

### Взаимодействие между контейнерами

#### 1. Service Discovery
```yaml
# Docker Compose автоматически создает DNS записи
networks:
  federation-network:
    driver: bridge

# Сервисы могут обращаться друг к другу по имени
ugc-subgraph:
  environment:
    - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
```

#### 2. Federation Communication
```mermaid
graph TD
    A[Apollo Router :4000] --> B[UGC Subgraph :4001]
    A --> C[Users Subgraph :4002]
    A --> D[Offers Subgraph :4004]
    
    B --> E[PostgreSQL :5432]
    B --> F[Redis :6379]
```

#### 3. Development Workflow
```bash
# Developer commands
make dev          # Start all services
make test         # Run tests
make logs         # View logs
make health       # Check service health
make clean        # Cleanup resources
```

### Качественные атрибуты

#### 1. Performance
- **Container optimization:** Multi-stage builds, minimal base images
- **Resource limits:** Proper CPU/memory allocation
- **Caching:** Docker layer caching, dependency caching

#### 2. Reliability
- **Health checks:** Automated service health monitoring
- **Graceful shutdown:** Proper signal handling
- **Restart policies:** Automatic container restart on failure

#### 3. Security
- **Non-root users:** All containers run as non-root
- **Network isolation:** Dedicated Docker networks
- **Secret management:** Environment variables for sensitive data

#### 4. Maintainability
- **Consistent structure:** Standardized Dockerfile patterns
- **Documentation:** Comprehensive setup guides
- **Automation:** Makefile commands for common tasks

### Заключение

Контейнерная архитектура Task 12 обеспечивает:

- **Isolation:** Каждый сервис изолирован в своем контейнере
- **Consistency:** Одинаковое поведение в разных окружениях
- **Scalability:** Возможность независимого масштабирования сервисов
- **Maintainability:** Простота обновления и развертывания
- **Developer Experience:** Быстрый старт и удобная разработка