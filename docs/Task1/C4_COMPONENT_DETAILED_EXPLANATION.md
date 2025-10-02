# Task 1: Component Diagram - Подробное объяснение компонентов инфраструктуры

## 🎯 Цель диаграммы

Component диаграмма Task 1 демонстрирует **детальную внутреннюю структуру инфраструктурных компонентов** федеративной GraphQL системы Auto.ru. Диаграмма служит мостом между архитектурным дизайном контейнеров и конкретной реализацией файлов, конфигураций и модулей, показывая как каждый компонент воплощается в исполняемый код.

## 🏗️ Архитектурные компоненты и их файловая реализация

### 1. Cargo Workspace - Система управления проектом

#### Workspace Config - Центральная конфигурация
**Архитектурная функция**: Управление структурой проекта и зависимостями
**Файловая реализация**:`
``toml
# Cargo.toml - Архитектурная структура workspace
[workspace]
members = [
    "crates/apollo-router",    # Федеративный роутер
    "crates/ugc-subgraph",     # Домен пользовательского контента
    "crates/users-subgraph",   # Домен пользователей
    "crates/offers-subgraph",  # Домен объявлений
    "crates/shared",           # Общие компоненты
]
resolver = "2"

# Общие зависимости для всех крейтов (DRY принцип)
[workspace.dependencies]
async-graphql = { version = "7.0", features = ["tracing", "apollo_tracing"] }
tokio = { version = "1.0", features = ["full"] }
axum = { version = "0.7", features = ["tracing"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono"] }
redis = { version = "0.24", features = ["tokio-comp"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
prometheus = "0.13"

# Профили сборки для разных окружений
[profile.dev]
debug = true
opt-level = 0
incremental = true

[profile.release]
debug = false
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
```

**Архитектурные принципы в реализации**:
- **Модульность**: Разделение на независимые крейты по доменам
- **Переиспользование**: Общие зависимости в workspace.dependencies
- **Оптимизация**: Разные профили для dev/release окружений

### 2. Shared Crate - Общие компоненты

#### Common Types - Типизированные идентификаторы
**Архитектурная функция**: Type safety и предотвращение ошибок смешивания ID
**Реализация в коде**:
```rust
// crates/shared/src/types.rs
use async_graphql::*;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

/// Типизированный ID пользователя для предотвращения путаницы с другими ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub uuid::Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
    
    pub fn from_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[Scalar]
impl ScalarType for UserId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                Self::from_str(&s).map_err(|_| InputValueError::custom("Invalid UUID format"))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

/// Аналогично для других доменных ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OfferId(pub uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReviewId(pub uuid::Uuid);

// Реализация аналогичных трейтов для OfferId и ReviewId...
```

#### Auth Utils - Утилиты аутентификации
**Архитектурная функция**: Централизованная логика безопасности
**Реализация в коде**:
```rust
// crates/shared/src/auth.rs
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // Subject (user ID)
    pub exp: usize,         // Expiration time
    pub iat: usize,         // Issued at
    pub roles: Vec<String>, // User roles
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtService {
    pub fn new(secret: &str) -> Self {
        let encoding_key = EncodingKey::from_secret(secret.as_ref());
        let decoding_key = DecodingKey::from_secret(secret.as_ref());
        let validation = Validation::default();
        
        Self {
            encoding_key,
            decoding_key,
            validation,
        }
    }
    
    pub fn create_token(&self, user_id: &str, roles: Vec<String>) -> Result<String, AuthError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as usize;
        
        let claims = Claims {
            sub: user_id.to_string(),
            exp: now + 3600, // 1 hour
            iat: now,
            roles,
        };
        
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(AuthError::JwtError)
    }
    
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(AuthError::JwtError)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("System time error: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
}
```

#### Error Handling - Централизованная обработка ошибок
**Архитектурная функция**: Единообразная обработка ошибок во всех сервисах
**Реализация в коде**:
```rust
// crates/shared/src/errors.rs
use async_graphql::{ErrorExtensions, Result as GraphQLResult};
use thiserror::Error;

/// Централизованная система ошибок для всех подграфов
#[derive(Error, Debug)]
pub enum UgcError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("Authentication error: {0}")]
    Auth(#[from] crate::auth::AuthError),
    
    #[error("Validation error: {message}")]
    Validation { message: String },
    
    #[error("Not found: {resource}")]
    NotFound { resource: String },
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Internal server error")]
    Internal,
}

impl ErrorExtensions for UgcError {
    fn extend(&self) -> async_graphql::Error {
        let mut error = async_graphql::Error::new(self.to_string());
        
        match self {
            UgcError::Validation { .. } => {
                error = error.extend_with(|_, e| e.set("code", "VALIDATION_ERROR"));
            }
            UgcError::NotFound { .. } => {
                error = error.extend_with(|_, e| e.set("code", "NOT_FOUND"));
            }
            UgcError::PermissionDenied => {
                error = error.extend_with(|_, e| e.set("code", "PERMISSION_DENIED"));
            }
            UgcError::Auth(_) => {
                error = error.extend_with(|_, e| e.set("code", "AUTHENTICATION_ERROR"));
            }
            _ => {
                error = error.extend_with(|_, e| e.set("code", "INTERNAL_ERROR"));
            }
        }
        
        error
    }
}

/// Конвертер для использования в GraphQL резолверах
pub type Result<T> = std::result::Result<T, UgcError>;
```

### 3. Apollo Router Crate - Федеративный роутер

#### Router Config - Конфигурация федерации
**Архитектурная функция**: Определение топологии федеративной сети
**Файловая реализация**:
```yaml
# router.yaml - Архитектурная конфигурация роутера
supergraph:
  listen: 0.0.0.0:4000
  introspection: true
  
# CORS конфигурация для веб-клиентов
cors:
  origins:
    - http://localhost:3000
    - https://auto.ru
  allow_credentials: true
  allow_headers:
    - authorization
    - content-type
    - x-apollo-tracing

# Телеметрия и мониторинг
telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
      default_attribute_requirement_level: required
  exporters:
    metrics:
      prometheus:
        enabled: true
        listen: 0.0.0.0:9090
        path: /metrics
    tracing:
      jaeger:
        endpoint: http://jaeger:14268/api/traces
        batch_processor:
          max_export_batch_size: 512
          max_queue_size: 2048

# Конфигурация подграфов с retry политиками
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
      retry_percent: 0.2
    timeout: 30s
    
  users:
    routing_url: http://users-subgraph:4002/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
      retry_percent: 0.2
    timeout: 30s
    
  offers:
    routing_url: http://offers-subgraph:4003/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
      retry_percent: 0.2
    timeout: 30s

# Кеширование запросов
query_planning:
  cache:
    in_memory:
      limit: 512
```

#### Supergraph Schema - Композитная схема
**Архитектурная функция**: Единая GraphQL схема из всех подграфов
**Генерация схемы**:
```bash
#!/bin/bash
# scripts/generate-supergraph.sh - Автоматическая композиция схемы

set -e

echo "🔧 Generating supergraph schema..."

# Ожидание готовности всех подграфов
wait_for_subgraph() {
    local name=$1
    local url=$2
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if curl -f -s "$url" > /dev/null 2>&1; then
            echo "✅ $name is ready"
            return 0
        fi
        echo "⏳ Waiting for $name (attempt $attempt/$max_attempts)..."
        sleep 2
        ((attempt++))
    done
    
    echo "❌ $name failed to start"
    return 1
}

# Проверка готовности всех подграфов
wait_for_subgraph "UGC Subgraph" "http://localhost:4001/health"
wait_for_subgraph "Users Subgraph" "http://localhost:4002/health"
wait_for_subgraph "Offers Subgraph" "http://localhost:4003/health"

# Создание конфигурации для rover
cat > supergraph-config.yaml << EOF
federation_version: =2.5.7
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    schema:
      subgraph_url: http://localhost:4001/graphql
  users:
    routing_url: http://users-subgraph:4002/graphql
    schema:
      subgraph_url: http://localhost:4002/graphql
  offers:
    routing_url: http://offers-subgraph:4003/graphql
    schema:
      subgraph_url: http://localhost:4003/graphql
EOF

# Композиция супер-схемы с помощью Apollo Rover
rover supergraph compose --config supergraph-config.yaml > supergraph.graphql

echo "✅ Supergraph schema generated successfully"
echo "📄 Schema saved to supergraph.graphql"

# Валидация схемы
rover graph check --schema supergraph.graphql || {
    echo "⚠️  Schema validation warnings (non-critical)"
}

echo "🚀 Ready to start Apollo Router"
```

### 4. Docker Infrastructure - Контейнеризация

#### Docker Compose - Оркестрация сервисов
**Архитектурная функция**: Управление жизненным циклом всех компонентов
**Файловая реализация**:
```yaml
# docker-compose.yml - Полная оркестрация системы
version: '3.8'

# Определение сетей для сегментации трафика
networks:
  federation-network:
    driver: bridge
    name: autoru-federation
  data-network:
    driver: bridge
    internal: true
    name: autoru-data
  monitoring-network:
    driver: bridge
    name: autoru-monitoring

# Персистентные тома для данных
volumes:
  postgres_data:
    driver: local
    name: autoru_postgres_data
  redis_data:
    driver: local
    name: autoru_redis_data
  prometheus_data:
    driver: local
    name: autoru_prometheus_data

services:
  # Базы данных (запускаются первыми)
  postgres:
    image: postgres:15-alpine
    container_name: autoru_postgres
    environment:
      POSTGRES_DB: ${POSTGRES_DB:-autoru}
      POSTGRES_USER: ${POSTGRES_USER:-postgres}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-password}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./migrations:/docker-entrypoint-initdb.d:ro
    networks:
      - data-network
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-postgres} -d ${POSTGRES_DB:-autoru}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 10s
    restart: unless-stopped

  redis:
    image: redis:7-alpine
    container_name: autoru_redis
    command: redis-server --requirepass ${REDIS_PASSWORD:-redis_password}
    volumes:
      - redis_data:/data
    networks:
      - data-network
    healthcheck:
      test: ["CMD", "redis-cli", "--no-auth-warning", "-a", "${REDIS_PASSWORD:-redis_password}", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3
    restart: unless-stopped

  # Подграфы (зависят от БД)
  ugc-subgraph:
    build:
      context: .
      dockerfile: crates/ugc-subgraph/Dockerfile
      target: runtime
    container_name: autoru_ugc_subgraph
    environment:
      DATABASE_URL: postgresql://${POSTGRES_USER:-postgres}:${POSTGRES_PASSWORD:-password}@postgres:5432/${POSTGRES_DB:-autoru}
      REDIS_URL: redis://:${REDIS_PASSWORD:-redis_password}@redis:6379
      RUST_LOG: ${RUST_LOG:-info}
      JAEGER_ENDPOINT: http://jaeger:14268/api/traces
    networks:
      - federation-network
      - data-network
      - monitoring-network
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    restart: unless-stopped

  # Apollo Router (зависит от всех подграфов)
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
    container_name: autoru_apollo_router
    ports:
      - "${APOLLO_ROUTER_PORT:-4000}:4000"
      - "${APOLLO_ROUTER_METRICS_PORT:-9090}:9090"
    environment:
      APOLLO_ROUTER_CONFIG_PATH: /app/router.yaml
      APOLLO_ROUTER_SUPERGRAPH_PATH: /app/supergraph.graphql
      RUST_LOG: ${RUST_LOG:-info}
    volumes:
      - ./router.yaml:/app/router.yaml:ro
      - ./supergraph.graphql:/app/supergraph.graphql:ro
    networks:
      - federation-network
      - monitoring-network
    depends_on:
      ugc-subgraph:
        condition: service_healthy
      users-subgraph:
        condition: service_healthy
      offers-subgraph:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
    restart: unless-stopped

  # Мониторинг
  prometheus:
    image: prom/prometheus:latest
    container_name: autoru_prometheus
    ports:
      - "${PROMETHEUS_PORT:-9091}:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus_data:/prometheus
    networks:
      - monitoring-network
      - federation-network
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      - '--storage.tsdb.retention.time=200h'
      - '--web.enable-lifecycle'
    restart: unless-stopped

  jaeger:
    image: jaegertracing/all-in-one:latest
    container_name: autoru_jaeger
    ports:
      - "${JAEGER_UI_PORT:-16686}:16686"
    environment:
      COLLECTOR_OTLP_ENABLED: true
    networks:
      - monitoring-network
      - federation-network
    restart: unless-stopped
```

### 5. Development Tools - Автоматизация разработки

#### Makefile - Команды автоматизации
**Архитектурная функция**: Упрощение повседневных задач разработки
**Файловая реализация**:
```makefile
# Makefile - Автоматизация команд разработки
.PHONY: help dev build test clean docker-build docker-up docker-down logs

# Переменные окружения
RUST_LOG ?= info
COMPOSE_FILE ?= docker-compose.yml

help: ## Показать справку по командам
	@echo "Auto.ru GraphQL Federation - Development Commands"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

dev: ## Запустить среду разработки
	@echo "🚀 Starting development environment..."
	@./scripts/dev-setup.sh

build: ## Собрать все Rust проекты
	@echo "🔨 Building Rust workspace..."
	@cargo build --workspace

test: ## Запустить все тесты
	@echo "🧪 Running tests..."
	@cargo test --workspace

clean: ## Очистить артефакты сборки
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@docker system prune -f

docker-build: ## Собрать Docker образы
	@echo "🐳 Building Docker images..."
	@docker-compose -f $(COMPOSE_FILE) build

docker-up: ## Запустить Docker контейнеры
	@echo "🚀 Starting Docker containers..."
	@docker-compose -f $(COMPOSE_FILE) up -d

docker-down: ## Остановить Docker контейнеры
	@echo "🛑 Stopping Docker containers..."
	@docker-compose -f $(COMPOSE_FILE) down

logs: ## Показать логи всех сервисов
	@docker-compose -f $(COMPOSE_FILE) logs -f

# Специфичные команды для разработки
generate-schema: ## Генерировать supergraph схему
	@echo "📋 Generating supergraph schema..."
	@./scripts/generate-supergraph.sh

migrate: ## Запустить миграции БД
	@echo "🗄️ Running database migrations..."
	@docker-compose exec postgres psql -U postgres -d autoru -f /docker-entrypoint-initdb.d/001_initial_schema.sql

reset-db: ## Сбросить базу данных
	@echo "🔄 Resetting database..."
	@docker-compose down postgres
	@docker volume rm autoru_postgres_data || true
	@docker-compose up -d postgres

health-check: ## Проверить здоровье всех сервисов
	@echo "🏥 Checking system health..."
	@./scripts/health-check.sh

# Команды для тестирования
integration-test: ## Запустить интеграционные тесты
	@echo "🔗 Running integration tests..."
	@cargo test --test integration_tests

load-test: ## Запустить нагрузочные тесты
	@echo "⚡ Running load tests..."
	@./scripts/load-test.sh

# Команды для production
prod-build: ## Собрать для production
	@echo "🏭 Building for production..."
	@cargo build --release --workspace

prod-docker: ## Собрать production Docker образы
	@echo "🐳 Building production Docker images..."
	@docker-compose -f docker-compose.prod.yml build
```

#### Environment Config - Конфигурация окружения
**Архитектурная функция**: Управление переменными окружения
**Файловая реализация**:
```bash
# .env.example - Шаблон конфигурации окружения
# Скопируйте в .env и настройте под свое окружение

# === Database Configuration ===
POSTGRES_DB=autoru
POSTGRES_USER=postgres
POSTGRES_PASSWORD=secure_password_here
DATABASE_URL=postgresql://postgres:secure_password_here@localhost:5432/autoru

# === Redis Configuration ===
REDIS_PASSWORD=redis_secure_password
REDIS_URL=redis://:redis_secure_password@localhost:6379

# === JWT Configuration ===
JWT_SECRET=your_super_secret_jwt_key_here_minimum_32_characters
JWT_EXPIRATION=3600

# === Service Ports ===
APOLLO_ROUTER_PORT=4000
APOLLO_ROUTER_METRICS_PORT=9090
UGC_SUBGRAPH_PORT=4001
USERS_SUBGRAPH_PORT=4002
OFFERS_SUBGRAPH_PORT=4003

# === Monitoring Ports ===
PROMETHEUS_PORT=9091
JAEGER_UI_PORT=16686

# === Logging Configuration ===
RUST_LOG=info
# Для отладки используйте: RUST_LOG=debug
# Для production: RUST_LOG=warn

# === Development Settings ===
RUST_BACKTRACE=1
CARGO_INCREMENTAL=1

# === Feature Flags ===
ENABLE_INTROSPECTION=true
ENABLE_PLAYGROUND=true
ENABLE_TRACING=true
ENABLE_METRICS=true

# === External Services ===
ELASTICSEARCH_URL=http://localhost:9200
KIBANA_URL=http://localhost:5601

# === Security Settings ===
CORS_ORIGINS=http://localhost:3000,https://auto.ru
RATE_LIMIT_REQUESTS_PER_MINUTE=100
SESSION_TIMEOUT_MINUTES=60
```

## 🔄 Компонентные взаимодействия и зависимости

### Граф зависимостей компонентов
```mermaid
graph TD
    A[Workspace Config] --> B[Shared Crate]
    A --> C[Apollo Router Crate]
    A --> D[Subgraph Crates]
    
    B --> E[Common Types]
    B --> F[Auth Utils]
    B --> G[Error Handling]
    B --> H[Common Utils]
    
    C --> I[Router Config]
    C --> J[Supergraph Schema]
    C --> K[Router Dockerfile]
    
    D --> L[UGC Dockerfile]
    D --> M[Users Dockerfile]
    D --> N[Offers Dockerfile]
    
    O[Docker Compose] --> P[PostgreSQL Service]
    O --> Q[Redis Service]
    O --> C
    O --> D
    
    R[Makefile] --> S[Dev Setup Script]
    R --> T[Health Check Script]
    R --> O
    
    U[Environment Config] --> O
    U --> C
    U --> D
```

### Файловая структура проекта
```
auto-ru-graphql-federation/
├── Cargo.toml                 # Workspace конфигурация
├── docker-compose.yml         # Оркестрация сервисов
├── Makefile                   # Автоматизация команд
├── .env.example              # Шаблон конфигурации
├── router.yaml               # Конфигурация Apollo Router
├── supergraph.graphql        # Композитная схема
├── prometheus.yml            # Конфигурация мониторинга
├── crates/
│   ├── shared/               # Общие компоненты
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs        # Точка входа
│   │       ├── types.rs      # Типизированные ID
│   │       ├── auth.rs       # JWT утилиты
│   │       ├── errors.rs     # Обработка ошибок
│   │       └── utils.rs      # Вспомогательные функции
│   ├── apollo-router/        # Федеративный роутер
│   │   ├── Dockerfile
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── ugc-subgraph/         # UGC подграф
│   │   ├── Dockerfile
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── users-subgraph/       # Users подграф
│   └── offers-subgraph/      # Offers подграф
├── scripts/
│   ├── dev-setup.sh          # Настройка среды
│   ├── dev-stop.sh           # Остановка сервисов
│   ├── generate-supergraph.sh # Генерация схемы
│   └── health-check.sh       # Проверка здоровья
└── migrations/
    └── 001_initial_schema.sql # Схема БД
```

Эта Component диаграмма демонстрирует как архитектурные решения Task 1 воплощаются в конкретные файлы, конфигурации и модули, обеспечивая модульность, переиспользование кода и автоматизацию процессов разработки.