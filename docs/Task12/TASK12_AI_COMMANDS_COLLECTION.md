# Task 12: AI Commands Collection
## Настройка среды разработки и деплоя

### Обзор Task 12
**Задача:** Настройка среды разработки и деплоя
- 12.1 Создать Docker конфигурацию
- 12.2 Настроить CI/CD pipeline
- 12.3 Создать документацию

### Команды AI для выполнения Task 12

#### 1. Анализ и планирование инфраструктуры

##### Команда: `readFile` (анализ спецификации)
```bash
# Эквивалент: cat .kiro/specs/auto-ru-graphql-federation/tasks.md
```

**Назначение:** Чтение спецификации для понимания требований Task 12
**Параметры:**
- `path`: ".kiro/specs/auto-ru-graphql-federation/tasks.md"
- `start_line`: 180, `end_line`: 210
- `explanation`: "Читаю полный Task 12 с заголовком"

**Результат:** Получение детального описания всех подзадач инфраструктуры

##### Команда: `listDirectory` (анализ структуры проекта)
```bash
# Эквивалент: ls -la .
```

**Назначение:** Изучение структуры проекта для планирования Docker конфигураций
**Параметры:**
- `path`: "."
- `depth`: 1
- `explanation`: "Проверяю структуру проекта для понимания Docker конфигураций"

#### 2. Task 12.1: Создание Docker конфигурации

##### Команда: `fsWrite` (создание Dockerfile для UGC подграфа)
```bash
# Эквивалент: cat > ugc-subgraph/Dockerfile << 'EOF'
```

**Назначение:** Создание multi-stage Dockerfile для UGC подграфа
**Параметры:**
- `path`: "ugc-subgraph/Dockerfile"
- `text`: Multi-stage Docker build с Rust компиляцией

**Содержимое:**
```dockerfile
# Multi-stage build для оптимизации размера образа
FROM rust:1.75-slim as builder

WORKDIR /app

# Установка системных зависимостей
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Копирование Cargo файлов для кеширования зависимостей
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
COPY shared/Cargo.toml ./shared/

# Создание dummy файлов для сборки зависимостей
RUN mkdir -p ugc-subgraph/src shared/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs && \
    echo "// dummy" > shared/src/lib.rs

# Сборка зависимостей (будет закешировано)
RUN cargo build --release --package ugc-subgraph

# Копирование исходного кода
COPY . .

# Пересборка только приложения
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# Runtime stage
FROM debian:bookworm-slim

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Создание пользователя без root прав
RUN useradd -r -s /bin/false ugc

# Копирование бинарного файла
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/

# Настройка безопасности
USER ugc
EXPOSE 4001

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```

##### Команда: `fsWrite` (создание Dockerfile для Users подграфа)
```bash
# Эквивалент: cat > users-subgraph/Dockerfile << 'EOF'
```

**Назначение:** Dockerfile для Users подграфа (заглушка)
**Параметры:**
- `path`: "users-subgraph/Dockerfile"
- `text`: Аналогичная структура с портом 4002

##### Команда: `fsWrite` (создание Dockerfile для Offers подграфа)
```bash
# Эквивалент: cat > offers-subgraph/Dockerfile << 'EOF'
```

**Назначение:** Dockerfile для Offers подграфа (заглушка)
**Параметры:**
- `path`: "offers-subgraph/Dockerfile"
- `text`: Аналогичная структура с портом 4004##### Кома
нда: `fsWrite` (создание docker-compose.yml)
```bash
# Эквивалент: cat > docker-compose.yml << 'EOF'
```

**Назначение:** Основная конфигурация Docker Compose для всех сервисов
**Параметры:**
- `path`: "docker-compose.yml"
- `text`: Полная конфигурация всех подграфов и зависимостей

**Ключевые компоненты:**
```yaml
version: '3.8'

services:
  # Apollo Router - Federation Gateway
  apollo-router:
    build:
      context: .
      dockerfile: apollo-router/Dockerfile
    ports:
      - "4000:4000"
    volumes:
      - ./router.yaml:/app/router.yaml
      - ./supergraph.graphql:/app/supergraph.graphql
    depends_on:
      - ugc-subgraph
      - users-subgraph
      - offers-subgraph
    environment:
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
    networks:
      - federation-network

  # UGC Subgraph
  ugc-subgraph:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
    ports:
      - "4001:4001"
    depends_on:
      - ugc-postgres
      - redis
    environment:
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    networks:
      - federation-network

  # Users Subgraph (Stub)
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

  # Offers Subgraph (Stub)
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

networks:
  federation-network:
    driver: bridge
```

##### Команда: `fsWrite` (создание docker-compose.dev.yml)
```bash
# Эквивалент: cat > docker-compose.dev.yml << 'EOF'
```

**Назначение:** Development конфигурация с hot reload и отладкой
**Параметры:**
- `path`: "docker-compose.dev.yml"
- `text`: Development-specific настройки

**Development особенности:**
```yaml
version: '3.8'

services:
  ugc-subgraph:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
      target: builder  # Остановка на builder stage
    volumes:
      - ./ugc-subgraph/src:/app/ugc-subgraph/src:ro
      - ./shared/src:/app/shared/src:ro
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
    command: cargo run --package ugc-subgraph
    # Hot reload через cargo watch
```

##### Команда: `fsWrite` (создание docker-compose.prod.yml)
```bash
# Эквивалент: cat > docker-compose.prod.yml << 'EOF'
```

**Назначение:** Production конфигурация с оптимизациями
**Параметры:**
- `path`: "docker-compose.prod.yml"
- `text`: Production-ready настройки

#### 3. Task 12.2: Настройка CI/CD pipeline

##### Команда: `fsWrite` (создание GitHub Actions CI)
```bash
# Эквивалент: cat > .github/workflows/ci.yml << 'EOF'
```

**Назначение:** Основной CI pipeline для тестирования и сборки
**Параметры:**
- `path`: ".github/workflows/ci.yml"
- `text`: Comprehensive CI workflow

**CI Pipeline структура:**
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
      - uses: actions/checkout@v3

      - name: Setup Rust
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

      - name: Run tests
        run: cargo test --all-features
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db

  build-docker:
    runs-on: ubuntu-latest
    needs: test
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph]
    
    steps:
      - uses: actions/checkout@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2

      - name: Build Docker image
        uses: docker/build-push-action@v3
        with:
          context: .
          file: ./${{ matrix.service }}/Dockerfile
          target: runtime
          push: false
          tags: ${{ matrix.service }}:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  schema-validation:
    runs-on: ubuntu-latest
    needs: build-docker
    
    steps:
      - uses: actions/checkout@v3

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
          echo "Schema composition successful"

      - name: Run federation tests
        run: |
          # Test cross-subgraph queries
          curl -X POST http://localhost:4000/graphql \
            -H "Content-Type: application/json" \
            -d '{"query": "query { __schema { types { name } } }"}'
```

##### Команда: `fsWrite` (создание deployment workflow)
```bash
# Эквивалент: cat > .github/workflows/deploy-staging.yml << 'EOF'
```

**Назначение:** Автоматический деплой в staging окружение
**Параметры:**
- `path`: ".github/workflows/deploy-staging.yml"
- `text`: Staging deployment pipeline

**Deployment особенности:**
```yaml
name: Deploy to Staging

on:
  push:
    branches: [develop]
  workflow_dispatch:

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment: staging
    
    steps:
      - uses: actions/checkout@v3

      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v2
        with:
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          aws-region: us-east-1

      - name: Login to Amazon ECR
        id: login-ecr
        uses: aws-actions/amazon-ecr-login@v1

      - name: Build and push Docker images
        env:
          ECR_REGISTRY: ${{ steps.login-ecr.outputs.registry }}
          ECR_REPOSITORY: auto-ru-federation
        run: |
          services=("ugc-subgraph" "users-subgraph" "offers-subgraph")
          for service in "${services[@]}"; do
            docker build -f $service/Dockerfile -t $ECR_REGISTRY/$ECR_REPOSITORY:$service-$GITHUB_SHA .
            docker push $ECR_REGISTRY/$ECR_REPOSITORY:$service-$GITHUB_SHA
          done

      - name: Deploy to EKS
        run: |
          aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
          
          # Update deployment images
          kubectl set image deployment/ugc-subgraph ugc-subgraph=$ECR_REGISTRY/$ECR_REPOSITORY:ugc-subgraph-$GITHUB_SHA -n staging
          kubectl set image deployment/users-subgraph users-subgraph=$ECR_REGISTRY/$ECR_REPOSITORY:users-subgraph-$GITHUB_SHA -n staging
          kubectl set image deployment/offers-subgraph offers-subgraph=$ECR_REGISTRY/$ECR_REPOSITORY:offers-subgraph-$GITHUB_SHA -n staging
          
          # Wait for rollout
          kubectl rollout status deployment/ugc-subgraph -n staging --timeout=300s
          kubectl rollout status deployment/users-subgraph -n staging --timeout=300s
          kubectl rollout status deployment/offers-subgraph -n staging --timeout=300s
```

##### Команда: `fsWrite` (создание security workflow)
```bash
# Эквивалент: cat > .github/workflows/security.yml << 'EOF'
```

**Назначение:** Security scanning и vulnerability checks
**Параметры:**
- `path`: ".github/workflows/security.yml"
- `text`: Security automation pipeline

#### 4. Task 12.3: Создание документации

##### Команда: `fsWrite` (создание основного README)
```bash
# Эквивалент: cat > README.md << 'EOF'
```

**Назначение:** Главная документация проекта с инструкциями
**Параметры:**
- `path`: "README.md"
- `text`: Comprehensive project documentation

**README структура:**
```markdown
# Auto.ru GraphQL Federation

Федеративная GraphQL архитектура для Auto.ru с микросервисами.

## Быстрый старт

### Предварительные требования
- Docker и Docker Compose
- Rust 1.75+
- Node.js 18+ (для Apollo CLI)

### Локальная разработка

1. Клонирование репозитория:
```bash
git clone <repository-url>
cd auto-ru-graphql-federation
```

2. Запуск всех сервисов:
```bash
docker-compose up -d
```

3. Проверка состояния:
```bash
# Проверка health endpoints
curl http://localhost:4001/health  # UGC Subgraph
curl http://localhost:4002/health  # Users Subgraph
curl http://localhost:4004/health  # Offers Subgraph

# GraphQL Playground
open http://localhost:4000/graphql
```

## Архитектура

### Подграфы
- **UGC Subgraph** (4001) - Отзывы и рейтинги
- **Users Subgraph** (4002) - Пользователи (заглушка)
- **Offers Subgraph** (4004) - Объявления (заглушка)

### Federation Gateway
- **Apollo Router** (4000) - Федеративный шлюз

## Разработка

### Добавление нового подграфа
1. Создать директорию подграфа
2. Добавить Cargo.toml
3. Создать Dockerfile
4. Обновить docker-compose.yml
5. Добавить в router.yaml

### Тестирование
```bash
# Unit тесты
cargo test

# Integration тесты
cargo test --test integration

# E2E тесты с Docker
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

## Деплой

### Staging
Автоматический деплой при push в `develop` ветку.

### Production
Ручной деплой через GitHub Actions workflow.
```

##### Команда: `fsWrite` (создание API документации)
```bash
# Эквивалент: cat > docs/API.md << 'EOF'
```

**Назначение:** Документация GraphQL API с примерами
**Параметры:**
- `path`: "docs/API.md"
- `text`: GraphQL API documentation

**API документация:**
```markdown
# GraphQL API Documentation

## Federated Queries

### Получение отзыва с данными пользователя и объявления

```graphql
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

### Создание отзыва

```graphql
mutation CreateReview($input: CreateReviewInput!) {
  createReview(input: $input) {
    id
    content
    rating
    createdAt
  }
}
```

## Endpoints

- **Federation Gateway**: http://localhost:4000/graphql
- **UGC Subgraph**: http://localhost:4001/graphql
- **Users Subgraph**: http://localhost:4002/graphql
- **Offers Subgraph**: http://localhost:4004/graphql
```

##### Команда: `strReplace` (обновление workspace Cargo.toml)
```bash
# Эквивалент: sed -i '/members = \[/a\    "users-subgraph",' Cargo.toml
```

**Назначение:** Добавление новых подграфов в workspace
**Параметры:**
- `path`: "Cargo.toml"
- `oldStr`: Существующий список members
- `newStr`: Обновленный список с новыми подграфами

##### Команда: `fsWrite` (создание Makefile)
```bash
# Эквивалент: cat > Makefile << 'EOF'
```

**Назначение:** Автоматизация часто используемых команд
**Параметры:**
- `path`: "Makefile"
- `text`: Development automation commands

**Makefile содержимое:**
```makefile
.PHONY: help build test clean dev prod logs

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build all Docker images
	docker-compose build

test: ## Run all tests
	cargo test --all-features

clean: ## Clean up containers and images
	docker-compose down -v
	docker system prune -f

dev: ## Start development environment
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

prod: ## Start production environment
	docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d

logs: ## Show logs from all services
	docker-compose logs -f

schema: ## Compose and validate federation schema
	rover supergraph compose --config supergraph.yaml

health: ## Check health of all services
	@echo "Checking service health..."
	@curl -s http://localhost:4001/health | jq .
	@curl -s http://localhost:4002/health | jq .
	@curl -s http://localhost:4004/health | jq .
```

#### 5. Конфигурация инфраструктуры

##### Команда: `fsWrite` (создание .dockerignore)
```bash
# Эквивалент: cat > .dockerignore << 'EOF'
```

**Назначение:** Оптимизация Docker build context
**Параметры:**
- `path`: ".dockerignore"
- `text`: Исключения для Docker build

##### Команда: `fsWrite` (создание router.yaml)
```bash
# Эквивалент: cat > router.yaml << 'EOF'
```

**Назначение:** Конфигурация Apollo Router для федерации
**Параметры:**
- `path`: "router.yaml"
- `text`: Apollo Router configuration

##### Команда: `fsWrite` (создание supergraph.yaml)
```bash
# Эквивалент: cat > supergraph.yaml << 'EOF'
```

**Назначение:** Конфигурация композиции федеративной схемы
**Параметры:**
- `path`: "supergraph.yaml"
- `text`: Supergraph composition config

### Итоговая статистика команд Task 12

#### Количество команд по типам:
- **fsWrite**: 15 команд (создание конфигурационных файлов)
- **strReplace**: 3 команды (обновление существующих файлов)
- **readFile**: 2 команды (анализ спецификации)
- **listDirectory**: 2 команды (изучение структуры)
- **fileSearch**: 2 команды (поиск существующих конфигураций)

#### Созданная инфраструктура:

1. **Docker Configuration** (12.1):
   - Multi-stage Dockerfiles для всех подграфов
   - Docker Compose конфигурации (dev, prod, logging)
   - Оптимизированные образы с security best practices
   - Health checks и monitoring

2. **CI/CD Pipeline** (12.2):
   - GitHub Actions workflows (CI, deployment, security)
   - Automated testing и schema validation
   - Multi-environment deployment (staging, production)
   - Docker image building и registry integration

3. **Documentation** (12.3):
   - Comprehensive README с quick start
   - API documentation с GraphQL примерами
   - Architecture documentation
   - Development guides и troubleshooting

#### Ключевые технологии:
- **Docker**: Multi-stage builds, compose orchestration
- **GitHub Actions**: CI/CD automation, matrix builds
- **Apollo Federation**: Schema composition, gateway configuration
- **Rust**: Workspace management, dependency optimization
- **AWS**: EKS deployment, ECR registry
- **Monitoring**: Health checks, logging, metrics

### Проверка выполнения Task 12

Для проверки успешного выполнения Task 12 можно использовать следующие команды:

```bash
# Проверка Docker конфигурации
docker-compose config
docker-compose build

# Проверка CI/CD
git push origin feature-branch  # Triggers CI
gh workflow run deploy-staging.yml  # Manual deployment

# Проверка документации
make help
make dev
make health
```