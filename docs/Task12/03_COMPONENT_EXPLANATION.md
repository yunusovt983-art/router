# Task 12: Component Level Architecture Explanation
## Настройка среды разработки и деплоя - Компонентная диаграмма

### Обзор компонентной архитектуры

Компонентная диаграмма Task 12 детализирует внутреннюю структуру каждого контейнера, показывая конкретные компоненты (файлы конфигурации, скрипты, модули) и их взаимодействие в рамках системы разработки и деплоя.

### Docker Configuration Components (Task 12.1)

#### Multi-stage Dockerfiles

##### 1. UGC Dockerfile Component
**Файл:** `ugc-subgraph/Dockerfile`
**Тип:** Multi-stage Docker configuration
**Назначение:** Оптимизированная сборка UGC сервиса

**Структура компонента:**
```dockerfile
# ===== BUILDER STAGE =====
FROM rust:1.75-slim as builder
WORKDIR /app

# Системные зависимости для компиляции
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Копирование манифестов для кеширования зависимостей
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
COPY shared/Cargo.toml ./shared/

# Создание dummy файлов для сборки зависимостей
RUN mkdir -p ugc-subgraph/src shared/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs && \
    echo "// dummy" > shared/src/lib.rs

# Сборка зависимостей (кешируется Docker)
RUN cargo build --release --package ugc-subgraph

# Копирование реального исходного кода
COPY . .

# Пересборка только приложения
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# ===== RUNTIME STAGE =====
FROM debian:bookworm-slim

# Runtime зависимости
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Создание non-root пользователя
RUN useradd -r -s /bin/false ugc

# Копирование бинарного файла
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/

# Security и operational настройки
USER ugc
EXPOSE 4001

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```

**Ключевые особенности:**
- **Dependency caching:** Отдельная сборка зависимостей для ускорения rebuild
- **Security:** Non-root пользователь, minimal runtime image
- **Health monitoring:** Встроенный health check
- **Size optimization:** Multi-stage build исключает build tools

##### 2. Users/Offers Dockerfile Components
**Файлы:** `users-subgraph/Dockerfile`, `offers-subgraph/Dockerfile`
**Структура:** Аналогичная UGC с адаптацией портов и package names

#### Docker Compose Configurations

##### 1. Main Compose Configuration
**Файл:** `docker-compose.yml`
**Тип:** YAML Configuration
**Назначение:** Основная оркестрация сервисов

**Структура компонента:**
```yaml
version: '3.8'

services:
  # ===== FEDERATION GATEWAY =====
  apollo-router:
    build:
      context: .
      dockerfile: apollo-router/Dockerfile
    ports:
      - "4000:4000"
    volumes:
      - ./router.yaml:/app/router.yaml:ro
      - ./supergraph.graphql:/app/supergraph.graphql:ro
    depends_on:
      - ugc-subgraph
      - users-subgraph
      - offers-subgraph
    environment:
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
      - RUST_LOG=info
    networks:
      - federation-network
    restart: unless-stopped

  # ===== UGC SUBGRAPH =====
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
      - RUST_LOG=info
      - JWT_SECRET=${JWT_SECRET:-development-secret}
    networks:
      - federation-network
    restart: unless-stopped

  # ===== DATABASE SERVICES =====
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

volumes:
  postgres_data:
  redis_data:

networks:
  federation-network:
    driver: bridge
```

##### 2. Development Overrides
**Файл:** `docker-compose.dev.yml`
**Тип:** Development Configuration
**Назначение:** Development-specific настройки

**Структура компонента:**
```yaml
version: '3.8'

services:
  ugc-subgraph:
    build:
      target: builder  # Остановка на builder stage для hot reload
    volumes:
      # Hot reload исходного кода
      - ./ugc-subgraph/src:/app/ugc-subgraph/src:ro
      - ./shared/src:/app/shared/src:ro
      # Cargo cache для ускорения компиляции
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
      - CARGO_INCREMENTAL=1
    command: |
      sh -c "
        cargo install cargo-watch &&
        cargo watch -x 'run --package ugc-subgraph'
      "

  users-subgraph:
    volumes:
      - ./users-subgraph/src:/app/users-subgraph/src:ro
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1

  # Development tools
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"
      - "14268:14268"
    environment:
      - COLLECTOR_OTLP_ENABLED=true
    networks:
      - federation-network

volumes:
  cargo_cache:
  target_cache:
```

##### 3. Production Overrides
**Файл:** `docker-compose.prod.yml`
**Тип:** Production Configuration
**Назначение:** Production optimizations

**Структура компонента:**
```yaml
version: '3.8'

services:
  ugc-subgraph:
    environment:
      - RUST_LOG=warn
      - RUST_BACKTRACE=0
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 256M
      restart_policy:
        condition: on-failure
        delay: 5s
        max_attempts: 3

  apollo-router:
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
      restart_policy:
        condition: on-failure

  # Production monitoring
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
    networks:
      - federation-network
```

##### 4. Docker Ignore Configuration
**Файл:** `.dockerignore`
**Тип:** Docker Ignore Rules
**Назначение:** Оптимизация build context

**Содержимое:**
```dockerignore
# Git и IDE файлы
.git
.gitignore
.vscode/
.idea/

# Rust build artifacts
target/
Cargo.lock

# Documentation
docs/
*.md
!README.md

# CI/CD файлы
.github/
.gitlab-ci.yml

# Docker файлы
Dockerfile*
docker-compose*.yml

# Logs и temporary файлы
*.log
tmp/
.env.local

# OS specific
.DS_Store
Thumbs.db
```

### CI/CD Configuration Components (Task 12.2)

#### GitHub Actions Workflows

##### 1. Main CI Workflow
**Файл:** `.github/workflows/ci.yml`
**Тип:** GitHub Actions Workflow
**Назначение:** Основной CI pipeline

**Структура компонента:**
```yaml
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
  # ===== CODE QUALITY CHECKS =====
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

  # ===== UNIT TESTS =====
  test:
    name: Unit Tests
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
      
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379

    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-test-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        run: cargo test --all-features --verbose
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
          REDIS_URL: redis://localhost:6379

  # ===== DOCKER BUILD MATRIX =====
  build:
    name: Build Docker Images
    runs-on: ubuntu-latest
    needs: [lint, test]
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
      
      - name: Build Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./${{ matrix.service }}/Dockerfile
          push: false
          tags: ${{ matrix.service }}:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
          platforms: linux/amd64,linux/arm64

  # ===== SCHEMA VALIDATION =====
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
      
      - name: Start services
        run: |
          docker-compose up -d
          sleep 30  # Wait for services to be ready
      
      - name: Validate schema composition
        run: |
          rover supergraph compose --config supergraph.yaml > composed-schema.graphql
          echo "✅ Schema composition successful"
      
      - name: Test federation queries
        run: |
          # Test basic federation query
          curl -X POST http://localhost:4000/graphql \
            -H "Content-Type: application/json" \
            -d '{
              "query": "query { __schema { types { name } } }"
            }' | jq .
```

##### 2. Staging Deployment Workflow
**Файл:** `.github/workflows/deploy-staging.yml`
**Тип:** Deployment Automation
**Назначение:** Автоматический деплой в staging

**Структура компонента:**
```yaml
name: Deploy to Staging

on:
  push:
    branches: [develop]
  workflow_dispatch:
    inputs:
      force_deploy:
        description: 'Force deployment even if tests fail'
        required: false
        default: 'false'

jobs:
  deploy:
    name: Deploy to Staging
    runs-on: ubuntu-latest
    environment: staging
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          aws-region: us-east-1
      
      - name: Login to Amazon ECR
        id: login-ecr
        uses: aws-actions/amazon-ecr-login@v2
      
      - name: Build and push Docker images
        env:
          ECR_REGISTRY: ${{ steps.login-ecr.outputs.registry }}
          ECR_REPOSITORY: auto-ru-federation
          IMAGE_TAG: ${{ github.sha }}
        run: |
          services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "apollo-router")
          
          for service in "${services[@]}"; do
            echo "Building $service..."
            docker build \
              -f $service/Dockerfile \
              -t $ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG \
              .
            
            echo "Pushing $service..."
            docker push $ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG
          done
      
      - name: Deploy to EKS
        env:
          ECR_REGISTRY: ${{ steps.login-ecr.outputs.registry }}
          ECR_REPOSITORY: auto-ru-federation
          IMAGE_TAG: ${{ github.sha }}
        run: |
          # Update kubeconfig
          aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
          
          # Update deployment images
          services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "apollo-router")
          
          for service in "${services[@]}"; do
            kubectl set image deployment/$service \
              $service=$ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG \
              -n staging
          done
          
          # Wait for rollout completion
          for service in "${services[@]}"; do
            kubectl rollout status deployment/$service -n staging --timeout=300s
          done
      
      - name: Run smoke tests
        run: |
          # Wait for services to be ready
          sleep 60
          
          # Get staging URL
          STAGING_URL=$(kubectl get service apollo-router -n staging -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
          
          # Test GraphQL endpoint
          curl -X POST http://$STAGING_URL/graphql \
            -H "Content-Type: application/json" \
            -d '{"query": "query { __typename }"}' \
            --fail-with-body
```

##### 3. Security Scanning Workflow
**Файл:** `.github/workflows/security.yml`
**Тип:** Security Automation
**Назначение:** Автоматическое сканирование безопасности

**Структура компонента:**
```yaml
name: Security Scan

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 2 * * 1'  # Weekly on Monday at 2 AM

jobs:
  # ===== DEPENDENCY AUDIT =====
  audit:
    name: Dependency Audit
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Install cargo-audit
        run: cargo install cargo-audit
      
      - name: Run cargo audit
        run: cargo audit --deny warnings

  # ===== CONTAINER SECURITY SCAN =====
  container-scan:
    name: Container Security Scan
    runs-on: ubuntu-latest
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Build Docker image
        run: |
          docker build -f ${{ matrix.service }}/Dockerfile -t ${{ matrix.service }}:scan .
      
      - name: Run Trivy vulnerability scanner
        uses: aquasecurity/trivy-action@master
        with:
          image-ref: '${{ matrix.service }}:scan'
          format: 'sarif'
          output: 'trivy-results-${{ matrix.service }}.sarif'
      
      - name: Upload Trivy scan results to GitHub Security tab
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: 'trivy-results-${{ matrix.service }}.sarif'

  # ===== SECRET SCANNING =====
  secret-scan:
    name: Secret Scanning
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - name: Run gitleaks
        uses: gitleaks/gitleaks-action@v2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Documentation Components (Task 12.3)

#### Core Documentation Files

##### 1. Main README Component
**Файл:** `README.md`
**Тип:** Markdown Documentation
**Назначение:** Основная документация проекта

**Структура компонента:**
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
   # Health endpoints
   curl http://localhost:4001/health  # UGC Subgraph
   curl http://localhost:4002/health  # Users Subgraph
   curl http://localhost:4004/health  # Offers Subgraph
   
   # GraphQL Playground
   open http://localhost:4000/graphql
   ```

## 🏗️ Architecture

### Subgraphs
- **UGC Subgraph** (4001) - Отзывы и рейтинги автомобилей
- **Users Subgraph** (4002) - Управление пользователями (заглушка)
- **Offers Subgraph** (4004) - Объявления о продаже (заглушка)

### Federation Gateway
- **Apollo Router** (4000) - Федеративный шлюз

## 🛠️ Development

### Environment Setup
```bash
# Development environment с hot reload
make dev

# Production-like environment
make prod

# Run tests
make test

# Check service health
make health
```

### Adding New Subgraph
1. Создать директорию подграфа
2. Добавить `Cargo.toml` с зависимостями
3. Создать `Dockerfile` по образцу существующих
4. Обновить `docker-compose.yml`
5. Добавить в `router.yaml` и `supergraph.yaml`

## 🧪 Testing

### Unit Tests
```bash
cargo test --package ugc-subgraph
cargo test --package users-subgraph
cargo test --package offers-subgraph
```

### Integration Tests
```bash
cargo test --test integration
```

### E2E Tests
```bash
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

## 🚢 Deployment

### Staging
Автоматический деплой при push в `develop` ветку через GitHub Actions.

### Production
Ручной деплой через GitHub Actions workflow:
```bash
gh workflow run deploy-production.yml
```

## 📊 Monitoring

### Local Development
- **Jaeger UI**: http://localhost:16686 (tracing)
- **Prometheus**: http://localhost:9090 (metrics)

### Production
- **Grafana Dashboards**: Мониторинг производительности
- **AlertManager**: Уведомления о проблемах

## 🔧 Troubleshooting

### Common Issues

#### Services not starting
```bash
# Check logs
docker-compose logs ugc-subgraph

# Restart specific service
docker-compose restart ugc-subgraph
```

#### Database connection issues
```bash
# Check PostgreSQL status
docker-compose exec ugc-postgres pg_isready -U ugc_user

# Reset database
docker-compose down -v
docker-compose up -d
```

#### Federation schema errors
```bash
# Validate schema composition
rover supergraph compose --config supergraph.yaml

# Check subgraph schemas
curl http://localhost:4001/graphql -d '{"query": "{ __schema { types { name } } }"}'
```
```

##### 2. API Documentation Component
**Файл:** `docs/API.md`
**Тип:** GraphQL API Reference
**Назначение:** Документация GraphQL API

**Структура компонента:**
```markdown
# GraphQL API Documentation

## 🔗 Endpoints

- **Federation Gateway**: http://localhost:4000/graphql
- **UGC Subgraph**: http://localhost:4001/graphql
- **Users Subgraph**: http://localhost:4002/graphql
- **Offers Subgraph**: http://localhost:4004/graphql

## 🌐 Federated Queries

### Get Review with User and Offer Details

Пример федеративного запроса, который объединяет данные из всех подграфов:

```graphql
query GetReviewWithDetails($reviewId: ID!) {
  review(id: $reviewId) {
    id
    content
    rating
    createdAt
    moderationStatus
    
    # Resolved from Users subgraph
    author {
      id
      name
      email
      registrationDate
    }
    
    # Resolved from Offers subgraph
    offer {
      id
      title
      price
      currency
      car {
        make
        model
        year
        mileage
      }
      seller {
        id
        name
        phone
      }
    }
  }
}
```

**Variables:**
```json
{
  "reviewId": "review_123"
}
```

**Response:**
```json
{
  "data": {
    "review": {
      "id": "review_123",
      "content": "Отличный автомобиль, рекомендую!",
      "rating": 5,
      "createdAt": "2024-01-15T10:30:00Z",
      "moderationStatus": "APPROVED",
      "author": {
        "id": "user_456",
        "name": "Иван Петров",
        "email": "ivan@example.com",
        "registrationDate": "2023-06-01T00:00:00Z"
      },
      "offer": {
        "id": "offer_789",
        "title": "Toyota Camry 2020",
        "price": 2500000,
        "currency": "RUB",
        "car": {
          "make": "Toyota",
          "model": "Camry",
          "year": 2020,
          "mileage": 45000
        },
        "seller": {
          "id": "user_101",
          "name": "Автосалон Премиум",
          "phone": "+7-495-123-45-67"
        }
      }
    }
  }
}
```

## ✏️ Mutations

### Create Review

```graphql
mutation CreateReview($input: CreateReviewInput!) {
  createReview(input: $input) {
    id
    content
    rating
    createdAt
    moderationStatus
  }
}
```

**Input Type:**
```graphql
input CreateReviewInput {
  content: String!
  rating: Int!
  offerId: ID!
}
```

### Update Review

```graphql
mutation UpdateReview($id: ID!, $input: UpdateReviewInput!) {
  updateReview(id: $id, input: $input) {
    id
    content
    rating
    updatedAt
  }
}
```

## 🔍 Queries

### Get Reviews with Pagination

```graphql
query GetReviews(
  $first: Int = 10
  $after: String
  $filter: ReviewFilter
) {
  reviews(first: $first, after: $after, filter: $filter) {
    edges {
      node {
        id
        content
        rating
        createdAt
      }
      cursor
    }
    pageInfo {
      hasNextPage
      hasPreviousPage
      startCursor
      endCursor
    }
    totalCount
  }
}
```

## 🔐 Authentication

### JWT Token

Все запросы должны включать JWT token в заголовке:

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

### Example with curl

```bash
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{
    "query": "query { reviews(first: 5) { edges { node { id content rating } } } }"
  }'
```

## 📝 Schema Types

### Core Types

```graphql
type Review @key(fields: "id") {
  id: ID!
  content: String!
  rating: Int!
  createdAt: DateTime!
  updatedAt: DateTime!
  moderationStatus: ModerationStatus!
  
  # Federation extensions
  author: User! @external
  offer: Offer! @external
}

enum ModerationStatus {
  PENDING
  APPROVED
  REJECTED
}

type User @key(fields: "id") @extends {
  id: ID! @external
  reviews: [Review!]!
}

type Offer @key(fields: "id") @extends {
  id: ID! @external
  reviews: [Review!]!
  averageRating: Float
}
```
```

##### 3. Makefile Automation Component
**Файл:** `Makefile`
**Тип:** Build Automation
**Назначение:** Автоматизация команд разработки

**Структура компонента:**
```makefile
# ===== CONFIGURATION =====
.DEFAULT_GOAL := help
.PHONY: help build test clean dev prod logs health schema lint format

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[1;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

# ===== HELP =====
help: ## Show this help message
	@echo "$(BLUE)Auto.ru GraphQL Federation - Development Commands$(NC)"
	@echo ""
	@echo "$(YELLOW)Usage:$(NC) make [target]"
	@echo ""
	@echo "$(YELLOW)Available targets:$(NC)"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# ===== DEVELOPMENT =====
dev: ## Start development environment with hot reload
	@echo "$(BLUE)Starting development environment...$(NC)"
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "$(GREEN)✅ Development environment started$(NC)"
	@echo "$(YELLOW)GraphQL Playground: http://localhost:4000/graphql$(NC)"

prod: ## Start production-like environment
	@echo "$(BLUE)Starting production environment...$(NC)"
	docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
	@echo "$(GREEN)✅ Production environment started$(NC)"

stop: ## Stop all services
	@echo "$(BLUE)Stopping all services...$(NC)"
	docker-compose down
	@echo "$(GREEN)✅ All services stopped$(NC)"

restart: stop dev ## Restart development environment

# ===== BUILDING =====
build: ## Build all Docker images
	@echo "$(BLUE)Building all Docker images...$(NC)"
	docker-compose build --parallel
	@echo "$(GREEN)✅ All images built successfully$(NC)"

build-no-cache: ## Build all Docker images without cache
	@echo "$(BLUE)Building all Docker images (no cache)...$(NC)"
	docker-compose build --no-cache --parallel
	@echo "$(GREEN)✅ All images built successfully$(NC)"

# ===== TESTING =====
test: ## Run all tests
	@echo "$(BLUE)Running all tests...$(NC)"
	cargo test --all-features --verbose
	@echo "$(GREEN)✅ All tests passed$(NC)"

test-unit: ## Run unit tests only
	@echo "$(BLUE)Running unit tests...$(NC)"
	cargo test --lib --all-features
	@echo "$(GREEN)✅ Unit tests passed$(NC)"

test-integration: ## Run integration tests
	@echo "$(BLUE)Running integration tests...$(NC)"
	cargo test --test integration --all-features
	@echo "$(GREEN)✅ Integration tests passed$(NC)"

test-e2e: ## Run end-to-end tests
	@echo "$(BLUE)Running E2E tests...$(NC)"
	docker-compose -f docker-compose.test.yml up --build --abort-on-container-exit
	docker-compose -f docker-compose.test.yml down -v
	@echo "$(GREEN)✅ E2E tests passed$(NC)"

# ===== CODE QUALITY =====
lint: ## Run linting (clippy)
	@echo "$(BLUE)Running clippy...$(NC)"
	cargo clippy --all-targets --all-features -- -D warnings
	@echo "$(GREEN)✅ Linting passed$(NC)"

format: ## Format code
	@echo "$(BLUE)Formatting code...$(NC)"
	cargo fmt --all
	@echo "$(GREEN)✅ Code formatted$(NC)"

format-check: ## Check code formatting
	@echo "$(BLUE)Checking code formatting...$(NC)"
	cargo fmt --all -- --check
	@echo "$(GREEN)✅ Code formatting is correct$(NC)"

audit: ## Run security audit
	@echo "$(BLUE)Running security audit...$(NC)"
	cargo audit
	@echo "$(GREEN)✅ Security audit passed$(NC)"

# ===== MONITORING =====
logs: ## Show logs from all services
	docker-compose logs -f

logs-ugc: ## Show logs from UGC subgraph
	docker-compose logs -f ugc-subgraph

logs-users: ## Show logs from Users subgraph
	docker-compose logs -f users-subgraph

logs-offers: ## Show logs from Offers subgraph
	docker-compose logs -f offers-subgraph

logs-router: ## Show logs from Apollo Router
	docker-compose logs -f apollo-router

health: ## Check health of all services
	@echo "$(BLUE)Checking service health...$(NC)"
	@echo "$(YELLOW)UGC Subgraph:$(NC)"
	@curl -s http://localhost:4001/health | jq . || echo "$(RED)❌ UGC Subgraph not responding$(NC)"
	@echo "$(YELLOW)Users Subgraph:$(NC)"
	@curl -s http://localhost:4002/health | jq . || echo "$(RED)❌ Users Subgraph not responding$(NC)"
	@echo "$(YELLOW)Offers Subgraph:$(NC)"
	@curl -s http://localhost:4004/health | jq . || echo "$(RED)❌ Offers Subgraph not responding$(NC)"
	@echo "$(YELLOW)Apollo Router:$(NC)"
	@curl -s http://localhost:4000/health | jq . || echo "$(RED)❌ Apollo Router not responding$(NC)"

# ===== SCHEMA MANAGEMENT =====
schema: ## Compose and validate federation schema
	@echo "$(BLUE)Composing federation schema...$(NC)"
	rover supergraph compose --config supergraph.yaml > supergraph.graphql
	@echo "$(GREEN)✅ Schema composed successfully$(NC)"

schema-check: ## Check schema for breaking changes
	@echo "$(BLUE)Checking schema for breaking changes...$(NC)"
	rover graph check auto-ru-federation@staging --schema supergraph.graphql
	@echo "$(GREEN)✅ No breaking changes detected$(NC)"

introspect: ## Introspect subgraph schemas
	@echo "$(BLUE)Introspecting subgraph schemas...$(NC)"
	@echo "$(YELLOW)UGC Subgraph:$(NC)"
	rover subgraph introspect http://localhost:4001/graphql > ugc-schema.graphql
	@echo "$(YELLOW)Users Subgraph:$(NC)"
	rover subgraph introspect http://localhost:4002/graphql > users-schema.graphql
	@echo "$(YELLOW)Offers Subgraph:$(NC)"
	rover subgraph introspect http://localhost:4004/graphql > offers-schema.graphql
	@echo "$(GREEN)✅ All schemas introspected$(NC)"

# ===== CLEANUP =====
clean: ## Clean up containers, volumes, and images
	@echo "$(BLUE)Cleaning up Docker resources...$(NC)"
	docker-compose down -v --remove-orphans
	docker system prune -f
	@echo "$(GREEN)✅ Cleanup completed$(NC)"

clean-all: ## Clean up everything including images
	@echo "$(BLUE)Cleaning up all Docker resources...$(NC)"
	docker-compose down -v --remove-orphans --rmi all
	docker system prune -af --volumes
	@echo "$(GREEN)✅ Complete cleanup finished$(NC)"

# ===== DATABASE =====
db-reset: ## Reset database (drop and recreate)
	@echo "$(BLUE)Resetting database...$(NC)"
	docker-compose stop ugc-postgres
	docker-compose rm -f ugc-postgres
	docker volume rm auto-ru-graphql-federation_postgres_data || true
	docker-compose up -d ugc-postgres
	@echo "$(GREEN)✅ Database reset completed$(NC)"

db-migrate: ## Run database migrations
	@echo "$(BLUE)Running database migrations...$(NC)"
	docker-compose exec ugc-subgraph sqlx migrate run
	@echo "$(GREEN)✅ Migrations completed$(NC)"

# ===== UTILITIES =====
shell-ugc: ## Open shell in UGC subgraph container
	docker-compose exec ugc-subgraph /bin/bash

shell-postgres: ## Open PostgreSQL shell
	docker-compose exec ugc-postgres psql -U ugc_user -d ugc_db

benchmark: ## Run performance benchmarks
	@echo "$(BLUE)Running performance benchmarks...$(NC)"
	cargo bench
	@echo "$(GREEN)✅ Benchmarks completed$(NC)"

docs: ## Generate and serve documentation
	@echo "$(BLUE)Generating documentation...$(NC)"
	cargo doc --no-deps --open
	@echo "$(GREEN)✅ Documentation generated$(NC)"
```

### Взаимодействие компонентов

#### 1. Configuration Dependencies
```mermaid
graph TD
    A[docker-compose.yml] --> B[ugc-subgraph/Dockerfile]
    A --> C[users-subgraph/Dockerfile]
    A --> D[offers-subgraph/Dockerfile]
    A --> E[apollo-router/Dockerfile]
    
    F[docker-compose.dev.yml] --> A
    G[docker-compose.prod.yml] --> A
    
    H[Makefile] --> A
    H --> F
    H --> G
    
    I[.github/workflows/ci.yml] --> B
    I --> C
    I --> D
    I --> E
    
    J[router.yaml] --> E
    K[supergraph.yaml] --> J
```

#### 2. Build Process Flow
```mermaid
sequenceDiagram
    participant Dev as Developer
    participant Make as Makefile
    participant Compose as Docker Compose
    participant Docker as Docker Engine
    
    Dev->>Make: make dev
    Make->>Compose: docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
    Compose->>Docker: Build images from Dockerfiles
    Docker->>Docker: Multi-stage build process
    Docker->>Compose: Images ready
    Compose->>Make: Services started
    Make->>Dev: Development environment ready
```

#### 3. CI/CD Integration
```mermaid
graph LR
    A[Git Push] --> B[GitHub Actions]
    B --> C[Lint & Format]
    B --> D[Unit Tests]
    B --> E[Build Matrix]
    
    C --> F[Schema Validation]
    D --> F
    E --> F
    
    F --> G[Security Scan]
    G --> H[Deploy Staging]
    H --> I[E2E Tests]
    I --> J[Deploy Production]
```

### Заключение

Компонентная архитектура Task 12 обеспечивает:

- **Modularity:** Четкое разделение ответственности между компонентами
- **Reusability:** Переиспользование конфигураций между окружениями
- **Maintainability:** Простота обновления и модификации компонентов
- **Automation:** Полная автоматизация процессов разработки и деплоя
- **Quality Assurance:** Встроенные проверки качества и безопасности
- **Developer Experience:** Удобные инструменты для разработки