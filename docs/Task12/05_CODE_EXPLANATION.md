# Task 12: Code Level Architecture Explanation
## Настройка среды разработки и деплоя - Код-уровневая диаграмма

### Обзор кодовой архитектуры

Код-уровневая диаграмма Task 12 детализирует конкретные файлы конфигурации, их содержимое и взаимосвязи в системе разработки и деплоя. Каждый компонент представлен с реальным кодом и объяснением его назначения.

### Docker Configuration Files

#### Multi-stage Dockerfile для UGC Subgraph

**Файл:** `ugc-subgraph/Dockerfile`

**Структура и назначение:**
```dockerfile
# ===== BUILDER STAGE =====
# Используем официальный Rust образ для компиляции
FROM rust:1.75-slim as builder
WORKDIR /app

# Установка системных зависимостей для компиляции
RUN apt-get update && apt-get install -y \
    pkg-config \      # Для поиска системных библиотек
    libssl-dev \      # OpenSSL headers для TLS
    libpq-dev \       # PostgreSQL client library
    && rm -rf /var/lib/apt/lists/*  # Очистка кеша apt

# Копирование манифестов для кеширования зависимостей
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
COPY shared/Cargo.toml ./shared/

# Создание dummy файлов для сборки зависимостей
RUN mkdir -p ugc-subgraph/src shared/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs && \
    echo "// dummy" > shared/src/lib.rs

# Сборка зависимостей (будет закешировано Docker)
RUN cargo build --release --package ugc-subgraph

# Копирование реального исходного кода
COPY . .

# Пересборка только приложения (зависимости уже собраны)
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# ===== RUNTIME STAGE =====
# Минимальный образ для production
FROM debian:bookworm-slim

# Runtime зависимости (только необходимые библиотеки)
RUN apt-get update && apt-get install -y \
    ca-certificates \  # SSL сертификаты
    libpq5 \          # PostgreSQL runtime library
    libssl3 \         # OpenSSL runtime
    curl \            # Для health checks
    && rm -rf /var/lib/apt/lists/*

# Создание non-root пользователя для безопасности
RUN useradd -r -s /bin/false ugc

# Копирование только бинарного файла из builder stage
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/

# Настройка безопасности и операционных параметров
USER ugc
EXPOSE 4001

# Health check для мониторинга состояния
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```**Ключ
евые особенности Dockerfile:**

1. **Multi-stage build:** Разделение на builder и runtime stages для оптимизации размера
2. **Dependency caching:** Отдельная сборка зависимостей для ускорения rebuild
3. **Security:** Non-root пользователь и minimal runtime image
4. **Health monitoring:** Встроенный health check endpoint
5. **Size optimization:** Исключение build tools из финального образа

#### Docker Compose Main Configuration

**Файл:** `docker-compose.yml`

**Основная оркестрация сервисов:**
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
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

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
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # ===== USERS SUBGRAPH (STUB) =====
  users-subgraph:
    build:
      context: .
      dockerfile: users-subgraph/Dockerfile
    ports:
      - "4002:4002"
    environment:
      - RUST_LOG=info
    networks:
      - federation-network
    restart: unless-stopped

  # ===== OFFERS SUBGRAPH (STUB) =====
  offers-subgraph:
    build:
      context: .
      dockerfile: offers-subgraph/Dockerfile
    ports:
      - "4004:4004"
    environment:
      - RUST_LOG=info
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

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes
    networks:
      - federation-network
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3
    restart: unless-stopped

# ===== PERSISTENT VOLUMES =====
volumes:
  postgres_data:
    driver: local
  redis_data:
    driver: local

# ===== NETWORK CONFIGURATION =====
networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
```

**Ключевые особенности Compose конфигурации:**

1. **Service dependencies:** Правильный порядок запуска с health checks
2. **Network isolation:** Dedicated network для federation сервисов
3. **Volume management:** Persistent storage для данных
4. **Health monitoring:** Health checks для всех критических сервисов
5. **Environment configuration:** Централизованное управление переменными

#### Development Overrides

**Файл:** `docker-compose.dev.yml`

**Development-specific настройки:**
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
    build:
      target: builder
    volumes:
      - ./users-subgraph/src:/app/users-subgraph/src:ro
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
    command: |
      sh -c "
        cargo install cargo-watch &&
        cargo watch -x 'run --package users-subgraph'
      "

  offers-subgraph:
    build:
      target: builder
    volumes:
      - ./offers-subgraph/src:/app/offers-subgraph/src:ro
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
    command: |
      sh -c "
        cargo install cargo-watch &&
        cargo watch -x 'run --package offers-subgraph'
      "

  # ===== DEVELOPMENT TOOLS =====
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # HTTP collector
    environment:
      - COLLECTOR_OTLP_ENABLED=true
    networks:
      - federation-network

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus-dev.yml:/etc/prometheus/prometheus.yml:ro
    networks:
      - federation-network

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
    networks:
      - federation-network

volumes:
  cargo_cache:
  target_cache:
  grafana_data:
```

### CI/CD Configuration Files

#### Main CI Workflow

**Файл:** `.github/workflows/ci.yml`

**Comprehensive CI pipeline:**
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
          restore-keys: |
            ${{ runner.os }}-cargo-
      
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
          
          # Test cross-subgraph query
          curl -X POST http://localhost:4000/graphql \
            -H "Content-Type: application/json" \
            -d '{
              "query": "query { reviews(first: 1) { edges { node { id author { name } } } } }"
            }' | jq .
      
      - name: Cleanup
        if: always()
        run: docker-compose down -v
```

#### Staging Deployment Workflow

**Файл:** `.github/workflows/deploy-staging.yml`

**Automated staging deployment:**
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
            echo "🔨 Building $service..."
            docker build \
              -f $service/Dockerfile \
              -t $ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG \
              .
            
            echo "📤 Pushing $service..."
            docker push $ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG
            
            echo "✅ $service pushed successfully"
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
            echo "🚀 Deploying $service to staging..."
            kubectl set image deployment/$service \
              $service=$ECR_REGISTRY/$ECR_REPOSITORY:$service-$IMAGE_TAG \
              -n staging
          done
          
          # Wait for rollout completion
          for service in "${services[@]}"; do
            echo "⏳ Waiting for $service rollout..."
            kubectl rollout status deployment/$service -n staging --timeout=300s
            echo "✅ $service deployed successfully"
          done
      
      - name: Run smoke tests
        run: |
          # Wait for services to be ready
          echo "⏳ Waiting for services to be ready..."
          sleep 60
          
          # Get staging URL
          STAGING_URL=$(kubectl get service apollo-router -n staging -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
          echo "🌐 Staging URL: $STAGING_URL"
          
          # Test GraphQL endpoint
          echo "🧪 Running smoke tests..."
          curl -X POST http://$STAGING_URL/graphql \
            -H "Content-Type: application/json" \
            -d '{"query": "query { __typename }"}' \
            --fail-with-body
          
          echo "✅ Smoke tests passed"
      
      - name: Notify deployment status
        if: always()
        uses: 8398a7/action-slack@v3
        with:
          status: ${{ job.status }}
          channel: '#deployments'
          webhook_url: ${{ secrets.SLACK_WEBHOOK }}
```

### Documentation Files

#### Main README

**Файл:** `README.md`

**Comprehensive project documentation:**
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
- **Grafana**: http://localhost:3000 (dashboards)

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

#### Makefile Automation

**Файл:** `Makefile`

**Development task automation:**
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

# ===== MONITORING =====
logs: ## Show logs from all services
	docker-compose logs -f

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

# ===== CLEANUP =====
clean: ## Clean up containers, volumes, and images
	@echo "$(BLUE)Cleaning up Docker resources...$(NC)"
	docker-compose down -v --remove-orphans
	docker system prune -f
	@echo "$(GREEN)✅ Cleanup completed$(NC)"
```

### Infrastructure Configuration Files

#### Apollo Router Configuration

**Файл:** `router.yaml`

**Federation gateway configuration:**
```yaml
# Apollo Router Configuration
listen: 0.0.0.0:4000

# Supergraph schema location
supergraph:
  path: ./supergraph.graphql

# Subgraph routing configuration
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
  
  users:
    routing_url: http://users-subgraph:4002/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
  
  offers:
    routing_url: http://offers-subgraph:4004/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s

# Telemetry configuration
telemetry:
  metrics:
    prometheus:
      enabled: true
      listen: 0.0.0.0:9090
      path: /metrics
  
  tracing:
    jaeger:
      enabled: true
      endpoint: http://jaeger:14268/api/traces
      batch_processor:
        max_export_batch_size: 512
        max_export_timeout: 30s

# CORS configuration
cors:
  allow_any_origin: true
  allow_credentials: true
  allow_headers:
    - Content-Type
    - Authorization
  expose_headers:
    - X-Custom-Header

# Security headers
headers:
  all:
    request:
      - propagate:
          named: "authorization"
    response:
      - insert:
          name: "X-Content-Type-Options"
          value: "nosniff"
      - insert:
          name: "X-Frame-Options"
          value: "DENY"

# Query planning
query_planning:
  cache:
    in_memory:
      limit: 512

# Rate limiting
traffic_shaping:
  all:
    global_rate_limit:
      capacity: 1000
      interval: 60s
  
  per_operation:
    timeout: 30s
    experimental_retry:
      min_per_sec: 10
      ttl: 10s
```

#### Supergraph Composition Configuration

**Файл:** `supergraph.yaml`

**Federation schema composition:**
```yaml
federation_version: 2

subgraphs:
  ugc:
    routing_url: http://localhost:4001/graphql
    schema:
      file: ./ugc-subgraph/schema.graphql
    
  users:
    routing_url: http://localhost:4002/graphql
    schema:
      file: ./users-subgraph/schema.graphql
    
  offers:
    routing_url: http://localhost:4004/graphql
    schema:
      file: ./offers-subgraph/schema.graphql
```

### Заключение

Код-уровневая архитектура Task 12 демонстрирует:

- **Configuration as Code:** Все настройки инфраструктуры в виде кода
- **Automation:** Полная автоматизация процессов через скрипты и workflows
- **Consistency:** Единообразные паттерны во всех конфигурационных файлах
- **Maintainability:** Четкая структура и документирование кода
- **Security:** Встроенные практики безопасности в каждом компоненте
- **Observability:** Comprehensive мониторинг и логирование
- **Developer Experience:** Удобные инструменты для разработки и отладки