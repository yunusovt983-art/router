# Task 1: Component Diagram - Внутренняя архитектура компонентов

## Обзор

Component диаграмма Task 1 раскрывает **внутреннюю структуру системы на уровне компонентов**, показывая как архитектурные решения воплощаются в конкретные модули, файлы и конфигурации. Диаграмма служит детальным мостом между высокоуровневой архитектурой и реальной файловой структурой проекта.

## 🏗️ Cargo Workspace: Архитектурная модульность

### Workspace Configuration
```toml
# Cargo.toml - Корневая конфигурация workspace
[workspace]
members = [
    "crates/apollo-router",    # Федеративный роутер
    "crates/ugc-subgraph",     # Домен пользовательского контента
    "crates/users-subgraph",   # Домен пользователей
    "crates/offers-subgraph",  # Домен объявлений
    "crates/shared",           # Общие компоненты
]
resolver = "2"

# Общие зависимости для оптимизации сборки
[workspace.dependencies]
async-graphql = { version = "7.0", features = ["apollo_tracing", "dataloader"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono", "runtime-tokio-rustls"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
prometheus = "0.13"
redis = { version = "0.24", features = ["tokio-comp"] }
```

### Архитектурная структура проекта
```
auto-ru-graphql-federation/
├── Cargo.toml                 # Workspace конфигурация
├── docker-compose.yml         # Оркестрация контейнеров
├── Makefile                   # Автоматизация команд
├── .env.example              # Шаблон конфигурации
├── crates/                   # Rust крейты
│   ├── shared/               # Общие компоненты
│   ├── apollo-router/        # Федеративный роутер
│   ├── ugc-subgraph/         # UGC подграф
│   ├── users-subgraph/       # Users подграф
│   └── offers-subgraph/      # Offers подграф
├── scripts/                  # Автоматизация DevOps
├── migrations/               # Миграции БД
└── monitoring/               # Конфигурация мониторинга
```

## 📚 Shared Crate: Архитектурная основа

### Types Module - Типизированная архитектура
```rust
// crates/shared/src/types.rs
//! Общие типы данных для всей федеративной системы
//! Реализует архитектурное решение о type safety

use async_graphql::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Типизированный ID пользователя
/// Архитектурное решение: предотвращение путаницы между разными типами ID
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct UserId(pub uuid::Uuid);

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[Scalar]
impl ScalarType for UserId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                uuid::Uuid::parse_str(&s)
                    .map(UserId)
                    .map_err(|_| InputValueError::custom("Invalid UUID format"))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

/// Типизированный ID объявления
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct OfferId(pub uuid::Uuid);

/// Типизированный ID отзыва
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ReviewId(pub uuid::Uuid);

/// Пагинация для GraphQL Connection
#[derive(Debug, Clone, InputObject)]
pub struct PaginationInput {
    pub first: Option<i32>,
    pub after: Option<String>,
    pub last: Option<i32>,
    pub before: Option<String>,
}

impl PaginationInput {
    /// Валидация параметров пагинации согласно GraphQL Cursor Connections Specification
    pub fn validate(&self) -> Result<(), String> {
        if let Some(first) = self.first {
            if first < 0 || first > 100 {
                return Err("first must be between 0 and 100".to_string());
            }
        }
        
        if let Some(last) = self.last {
            if last < 0 || last > 100 {
                return Err("last must be between 0 and 100".to_string());
            }
        }
        
        Ok(())
    }
}
```

### Auth Module - Безопасность как архитектурный принцип
```rust
// crates/shared/src/auth.rs
//! Модуль аутентификации и авторизации
//! Реализует архитектурные требования безопасности

use async_graphql::{Context, Guard, Result as GraphQLResult};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Роли пользователей в системе
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Role {
    User,
    Moderator,
    Admin,
}

/// Разрешения в системе
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    ReadOffers,
    CreateOffer,
    UpdateOffer,
    DeleteOffer,
    ReadReviews,
    CreateReview,
    ModerateReview,
    ManageUsers,
}

/// Контекст пользователя для GraphQL запросов
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: UserId,
    pub roles: HashSet<Role>,
    pub permissions: HashSet<Permission>,
}

impl UserContext {
    /// Проверка наличия разрешения
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
    
    /// Проверка наличия роли
    pub fn has_role(&self, role: &Role) -> bool {
        self.roles.contains(role)
    }
}

/// JWT Claims структура
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // User ID
    pub roles: Vec<Role>,      // User roles
    pub exp: usize,           // Expiration time
    pub iat: usize,           // Issued at
}

/// Guard для проверки разрешений в GraphQL резолверах
pub struct RequirePermission {
    pub permission: Permission,
}

#[async_trait::async_trait]
impl Guard for RequirePermission {
    async fn check(&self, ctx: &Context<'_>) -> GraphQLResult<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User not authenticated")?;

        if user_context.has_permission(&self.permission) {
            Ok(())
        } else {
            Err(format!("Permission {:?} required", self.permission).into())
        }
    }
}

/// Сервис для работы с JWT токенами
pub struct JwtService {
    secret: String,
    validation: Validation,
}

impl JwtService {
    pub fn new(secret: String) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        
        Self { secret, validation }
    }
    
    /// Валидация JWT токена и извлечение claims
    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &self.validation,
        )?;
        
        Ok(token_data.claims)
    }
    
    /// Создание UserContext из JWT токена
    pub fn create_user_context(&self, token: &str) -> Result<UserContext, Box<dyn std::error::Error>> {
        let claims = self.validate_token(token)?;
        let user_id = UserId(uuid::Uuid::parse_str(&claims.sub)?);
        
        // Преобразование ролей в разрешения
        let permissions = self.resolve_permissions(&claims.roles);
        
        Ok(UserContext {
            user_id,
            roles: claims.roles.into_iter().collect(),
            permissions,
        })
    }
    
    /// Резолвинг разрешений на основе ролей
    fn resolve_permissions(&self, roles: &[Role]) -> HashSet<Permission> {
        let mut permissions = HashSet::new();
        
        for role in roles {
            match role {
                Role::User => {
                    permissions.insert(Permission::ReadOffers);
                    permissions.insert(Permission::CreateOffer);
                    permissions.insert(Permission::ReadReviews);
                    permissions.insert(Permission::CreateReview);
                }
                Role::Moderator => {
                    permissions.insert(Permission::ReadOffers);
                    permissions.insert(Permission::ReadReviews);
                    permissions.insert(Permission::ModerateReview);
                }
                Role::Admin => {
                    permissions.insert(Permission::ReadOffers);
                    permissions.insert(Permission::CreateOffer);
                    permissions.insert(Permission::UpdateOffer);
                    permissions.insert(Permission::DeleteOffer);
                    permissions.insert(Permission::ReadReviews);
                    permissions.insert(Permission::CreateReview);
                    permissions.insert(Permission::ModerateReview);
                    permissions.insert(Permission::ManageUsers);
                }
            }
        }
        
        permissions
    }
}
```

### Error Handling - Централизованная обработка ошибок
```rust
// crates/shared/src/errors.rs
//! Централизованная система обработки ошибок
//! Реализует архитектурный принцип единообразной обработки ошибок

use async_graphql::{ErrorExtensions, Result as GraphQLResult};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Коды ошибок для клиентов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    // Аутентификация и авторизация
    Unauthenticated,
    Unauthorized,
    InvalidToken,
    
    // Валидация данных
    ValidationError,
    InvalidInput,
    
    // Бизнес-логика
    ResourceNotFound,
    ResourceAlreadyExists,
    BusinessRuleViolation,
    
    // Инфраструктура
    DatabaseError,
    CacheError,
    ExternalServiceError,
    
    // Общие ошибки
    InternalError,
    RateLimitExceeded,
}

/// Основной тип ошибки для всей системы
#[derive(Debug)]
pub enum AppError {
    // Ошибки аутентификации
    Authentication(String),
    Authorization(String),
    
    // Ошибки валидации
    Validation(String),
    
    // Ошибки бизнес-логики
    NotFound(String),
    AlreadyExists(String),
    BusinessRule(String),
    
    // Инфраструктурные ошибки
    Database(sqlx::Error),
    Cache(redis::RedisError),
    
    // Внутренние ошибки
    Internal(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            AppError::Authorization(msg) => write!(f, "Authorization error: {}", msg),
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Resource not found: {}", msg),
            AppError::AlreadyExists(msg) => write!(f, "Resource already exists: {}", msg),
            AppError::BusinessRule(msg) => write!(f, "Business rule violation: {}", msg),
            AppError::Database(err) => write!(f, "Database error: {}", err),
            AppError::Cache(err) => write!(f, "Cache error: {}", err),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

/// Расширение ошибок для GraphQL
impl ErrorExtensions for AppError {
    fn extend(&self) -> async_graphql::Error {
        let (code, message) = match self {
            AppError::Authentication(msg) => (ErrorCode::Unauthenticated, msg.clone()),
            AppError::Authorization(msg) => (ErrorCode::Unauthorized, msg.clone()),
            AppError::Validation(msg) => (ErrorCode::ValidationError, msg.clone()),
            AppError::NotFound(msg) => (ErrorCode::ResourceNotFound, msg.clone()),
            AppError::AlreadyExists(msg) => (ErrorCode::ResourceAlreadyExists, msg.clone()),
            AppError::BusinessRule(msg) => (ErrorCode::BusinessRuleViolation, msg.clone()),
            AppError::Database(_) => (ErrorCode::DatabaseError, "Database operation failed".to_string()),
            AppError::Cache(_) => (ErrorCode::CacheError, "Cache operation failed".to_string()),
            AppError::Internal(_) => (ErrorCode::InternalError, "Internal server error".to_string()),
        };
        
        async_graphql::Error::new(message)
            .extend_with(|_, e| e.set("code", code))
    }
}

/// Конвертация из различных типов ошибок
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        AppError::Cache(err)
    }
}

/// Результат для GraphQL операций
pub type AppResult<T> = Result<T, AppError>;
```

## 🐳 Docker Infrastructure Components

### Docker Compose Configuration
```yaml
# docker-compose.yml - Оркестрация всех компонентов
version: '3.8'

services:
  # Apollo Router - Федеративный шлюз
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
      args:
        - RUST_VERSION=1.75
    ports:
      - "4000:4000"
    environment:
      - RUST_LOG=info,apollo_router=debug
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
    depends_on:
      ugc-subgraph:
        condition: service_healthy
      users-subgraph:
        condition: service_healthy
      offers-subgraph:
        condition: service_healthy
    networks:
      - federation-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # UGC Subgraph
  ugc-subgraph:
    build:
      context: .
      dockerfile: crates/ugc-subgraph/Dockerfile
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=info,ugc_subgraph=debug
      - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@postgres:5432/auto_ru_federation
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=${JWT_SECRET}
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - federation-network
      - data-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  postgres_data:
    driver: local
  redis_data:
    driver: local
  prometheus_data:
    driver: local

networks:
  federation-network:
    driver: bridge
  data-network:
    driver: bridge
    internal: true
  monitoring-network:
    driver: bridge
```

### Router Dockerfile
```dockerfile
# crates/apollo-router/Dockerfile
# Multi-stage build для оптимизации размера образа

# Стадия сборки
FROM rust:1.75 as builder

WORKDIR /app

# Копирование манифестов для кеширования зависимостей
COPY Cargo.toml Cargo.lock ./
COPY crates/shared/Cargo.toml ./crates/shared/
COPY crates/apollo-router/Cargo.toml ./crates/apollo-router/

# Создание dummy файлов для сборки зависимостей
RUN mkdir -p crates/shared/src crates/apollo-router/src && \
    echo "fn main() {}" > crates/apollo-router/src/main.rs && \
    echo "" > crates/shared/src/lib.rs

# Сборка зависимостей (будет закеширована)
RUN cargo build --release -p apollo-router

# Копирование исходного кода
COPY crates/ ./crates/

# Пересборка с реальным кодом
RUN touch crates/apollo-router/src/main.rs && \
    cargo build --release -p apollo-router

# Стадия runtime
FROM debian:bookworm-slim

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Создание пользователя для безопасности
RUN useradd -r -s /bin/false apollo

# Копирование бинарника и конфигурации
COPY --from=builder /app/target/release/apollo-router /usr/local/bin/
COPY crates/apollo-router/router.yaml /app/
COPY supergraph.graphql /app/

# Настройка прав доступа
RUN chown -R apollo:apollo /app
USER apollo

EXPOSE 4000

HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:4000/health || exit 1

CMD ["apollo-router", "--config", "/app/router.yaml"]
```

## 🔧 Development Tools Components

### Makefile - Автоматизация команд
```makefile
# Makefile - Централизованная автоматизация всех операций
.PHONY: help dev build test clean docker-build docker-up docker-down lint fmt check

# Цвета для вывода
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

help: ## Показать справку по командам
	@echo "$(GREEN)Auto.ru GraphQL Federation - Available Commands:$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(YELLOW)%-20s$(NC) %s\n", $$1, $$2}'

dev: ## Запустить среду разработки
	@echo "$(GREEN)🚀 Starting development environment...$(NC)"
	./scripts/dev-setup.sh

build: ## Собрать все компоненты
	@echo "$(GREEN)🔨 Building workspace...$(NC)"
	cargo build --workspace --release

test: ## Запустить тесты
	@echo "$(GREEN)🧪 Running tests...$(NC)"
	cargo test --workspace

check: ## Проверить код без сборки
	@echo "$(GREEN)🔍 Checking code...$(NC)"
	cargo check --workspace

lint: ## Запустить линтер
	@echo "$(GREEN)📝 Running clippy...$(NC)"
	cargo clippy --workspace -- -D warnings

fmt: ## Форматировать код
	@echo "$(GREEN)✨ Formatting code...$(NC)"
	cargo fmt --all

docker-build: ## Собрать Docker образы
	@echo "$(GREEN)🐳 Building Docker images...$(NC)"
	docker-compose build

docker-up: ## Запустить контейнеры
	@echo "$(GREEN)📦 Starting containers...$(NC)"
	docker-compose up -d

docker-down: ## Остановить контейнеры
	@echo "$(GREEN)🛑 Stopping containers...$(NC)"
	docker-compose down

docker-logs: ## Показать логи контейнеров
	@echo "$(GREEN)📋 Showing container logs...$(NC)"
	docker-compose logs -f

clean: ## Очистить артефакты сборки
	@echo "$(GREEN)🧹 Cleaning up...$(NC)"
	cargo clean
	docker-compose down -v
	docker system prune -f

reset: clean ## Полная очистка и пересборка
	@echo "$(GREEN)🔄 Resetting environment...$(NC)"
	$(MAKE) docker-build
	$(MAKE) dev

# Команды для разработки
dev-logs: ## Показать логи разработки
	docker-compose logs -f apollo-router ugc-subgraph users-subgraph offers-subgraph

dev-restart: ## Перезапустить сервисы разработки
	docker-compose restart apollo-router ugc-subgraph users-subgraph offers-subgraph

# Команды для тестирования
test-integration: ## Запустить интеграционные тесты
	@echo "$(GREEN)🔗 Running integration tests...$(NC)"
	cargo test --test integration

test-unit: ## Запустить unit тесты
	@echo "$(GREEN)🧪 Running unit tests...$(NC)"
	cargo test --lib

# Команды для мониторинга
monitoring-up: ## Запустить только мониторинг
	docker-compose up -d prometheus jaeger

monitoring-down: ## Остановить мониторинг
	docker-compose stop prometheus jaeger

# Команды для базы данных
db-migrate: ## Применить миграции БД
	@echo "$(GREEN)🗄️ Running database migrations...$(NC)"
	docker-compose exec postgres psql -U postgres -d auto_ru_federation -f /docker-entrypoint-initdb.d/001_create_schema.sql

db-reset: ## Сбросить базу данных
	@echo "$(YELLOW)⚠️ Resetting database...$(NC)"
	docker-compose down postgres
	docker volume rm auto-ru-graphql-federation_postgres_data
	docker-compose up -d postgres
	sleep 10
	$(MAKE) db-migrate
```

### Environment Configuration
```bash
# .env.example - Шаблон конфигурации окружения
# Скопируйте в .env и настройте значения

# Database Configuration
POSTGRES_PASSWORD=secure_password_here
DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@localhost:5432/auto_ru_federation

# Redis Configuration  
REDIS_URL=redis://localhost:6379

# JWT Configuration
JWT_SECRET=your_super_secure_jwt_secret_key_here_at_least_32_characters

# Logging Configuration
RUST_LOG=info,apollo_router=debug,ugc_subgraph=debug,users_subgraph=debug,offers_subgraph=debug

# Apollo Router Configuration
APOLLO_ROUTER_CONFIG_PATH=./crates/apollo-router/router.yaml
APOLLO_ROUTER_SUPERGRAPH_PATH=./supergraph.graphql

# Development Configuration
DEVELOPMENT_MODE=true
HOT_RELOAD=true

# Monitoring Configuration
PROMETHEUS_ENABLED=true
JAEGER_ENABLED=true
METRICS_PORT=9090
TRACING_ENDPOINT=http://localhost:14268/api/traces

# Performance Configuration
DATABASE_MAX_CONNECTIONS=20
REDIS_MAX_CONNECTIONS=10
GRAPHQL_QUERY_COMPLEXITY_LIMIT=1000
GRAPHQL_QUERY_DEPTH_LIMIT=15

# Security Configuration
CORS_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8080
RATE_LIMIT_REQUESTS_PER_MINUTE=1000
```

## 📊 Monitoring Configuration Components

### Prometheus Configuration
```yaml
# monitoring/prometheus.yml - Конфигурация сбора метрик
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'auto-ru-federation'
    environment: 'development'

rule_files:
  - "alert_rules.yml"

scrape_configs:
  # Apollo Router метрики
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    metrics_path: '/metrics'
    scrape_interval: 5s
    scrape_timeout: 5s

  # Subgraph метрики
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:9091']
    metrics_path: '/metrics'
    
  - job_name: 'users-subgraph'
    static_configs:
      - targets: ['users-subgraph:9092']
    metrics_path: '/metrics'
    
  - job_name: 'offers-subgraph'
    static_configs:
      - targets: ['offers-subgraph:9093']
    metrics_path: '/metrics'

  # Infrastructure метрики
  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']
      
  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']

# Alerting configuration
alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

## 🎯 Заключение: Компоненты как архитектурная реализация

Component диаграмма Task 1 демонстрирует **детальную трансформацию архитектурных решений в конкретные компоненты**:

### 🏗️ **Архитектурная модульность → Код**
- **Cargo Workspace** → Структурированная организация крейтов
- **Shared Library** → Переиспользуемые компоненты и типы
- **Type Safety** → Типизированные ID и строгая типизация
- **Error Handling** → Централизованная система обработки ошибок

### 🔧 **Инфраструктурные компоненты → Конфигурация**
- **Docker Compose** → Оркестрация всех сервисов
- **Dockerfiles** → Оптимизированные образы контейнеров
- **Environment Config** → Централизованная конфигурация
- **Makefile** → Автоматизация всех операций

### 📊 **DevOps принципы → Автоматизация**
- **Development Scripts** → One-command setup
- **Monitoring Config** → Prometheus и Jaeger интеграция
- **Health Checks** → Проверка состояния сервисов
- **Network Segmentation** → Безопасная сетевая архитектура

Диаграмма служит **детальным руководством** для разработчиков, показывая как каждое архитектурное решение воплощается в конкретных файлах, модулях и конфигурациях, обеспечивая полное понимание внутренней структуры системы.