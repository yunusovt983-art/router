# Task 1: Context Diagram - Мост между архитектурой и реализацией

## Обзор

Context диаграмма Task 1 представляет **архитектурный фундамент** для федеративной GraphQL системы Auto.ru, служа мостом между высокоуровневым дизайном и конкретной реализацией кода. Диаграмма показывает, как архитектурные решения трансформируются в исполняемые компоненты системы.

## 🏗️ Архитектурные решения → Код

### Apollo Router: От концепции к реализации

#### Архитектурное решение
```yaml
# router.yaml - Конфигурация федеративного роутера
supergraph:
  listen: 0.0.0.0:4000
  
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
  users:
    routing_url: http://users-subgraph:4002/graphql
  offers:
    routing_url: http://offers-subgraph:4003/graphql
```

#### Реализация в коде
```rust
// crates/apollo-router/src/main.rs
use apollo_router::{Configuration, RouterSupergraph};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Загрузка конфигурации из архитектурного решения
    let config = Configuration::from_file("router.yaml")?;
    
    // Создание роутера на основе supergraph схемы
    let router = RouterSupergraph::new(config).await?;
    
    // Запуск сервера согласно архитектурным требованиям
    router.serve().await?;
    
    Ok(())
}
```

### Федеративная схема: Архитектура → Типы

#### Архитектурное решение
- **Доменное разделение**: Users, Offers, UGC
- **Федеративные связи**: @key директивы для связи сущностей
- **Единая точка входа**: Композитная схема

#### Реализация в коде
```graphql
# Supergraph Schema - результат композиции
type User @key(fields: "id") {
  id: ID!
  name: String!
  # Федеративное расширение из UGC подграфа
  reviews: [Review!]!
}

type Offer @key(fields: "id") {
  id: ID!
  title: String!
  # Федеративная связь с Users подграфом
  seller: User!
  # Федеративная связь с UGC подграфом
  reviews: [Review!]!
}
```

```rust
// crates/shared/src/types.rs - Общие типы из архитектуры
use async_graphql::*;
use serde::{Deserialize, Serialize};

/// Типизированный ID пользователя - архитектурное решение о type safety
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct UserId(pub uuid::Uuid);

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
```

## 🔧 Инфраструктурные решения → Docker Compose

### Архитектурное решение: Контейнеризация
- **Изоляция сервисов**: Каждый подграф в отдельном контейнере
- **Сетевая сегментация**: Разделение на app и data сети
- **Управление зависимостями**: Порядок запуска сервисов

### Реализация в коде
```yaml
# docker-compose.yml - Воплощение архитектурных решений
version: '3.8'

services:
  # Apollo Router - центральная точка федерации
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
    ports:
      - "4000:4000"
    depends_on:
      - ugc-subgraph
      - users-subgraph
      - offers-subgraph
    networks:
      - app-network
    environment:
      - RUST_LOG=info
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql

  # UGC Subgraph - доменная изоляция
  ugc-subgraph:
    build:
      context: .
      dockerfile: crates/ugc-subgraph/Dockerfile
    ports:
      - "4001:4001"
    depends_on:
      - postgres
      - redis
    networks:
      - app-network
      - data-network
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/autoru
      - REDIS_URL=redis://redis:6379

networks:
  app-network:
    driver: bridge
  data-network:
    driver: bridge
    internal: true  # Архитектурное решение о безопасности
```

## 📊 Мониторинг: От требований к метрикам

### Архитектурное требование
- **Наблюдаемость**: Полная видимость системы
- **Производительность**: Мониторинг SLA
- **Отказоустойчивость**: Раннее обнаружение проблем

### Реализация в коде
```rust
// crates/shared/src/telemetry.rs
use prometheus::{Counter, Histogram, Registry};
use tracing::{info, instrument};

pub struct Metrics {
    pub requests_total: Counter,
    pub request_duration: Histogram,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests_total: Counter::new(
                "graphql_requests_total",
                "Total number of GraphQL requests"
            ).unwrap(),
            request_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "graphql_request_duration_seconds",
                    "Duration of GraphQL requests"
                ).buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0])
            ).unwrap(),
        }
    }
}

#[instrument(skip(metrics))]
pub async fn handle_request(
    request: GraphQLRequest,
    metrics: &Metrics,
) -> GraphQLResponse {
    let start = std::time::Instant::now();
    
    // Обработка запроса
    let response = process_request(request).await;
    
    // Сбор метрик согласно архитектурным требованиям
    metrics.requests_total.inc();
    metrics.request_duration.observe(start.elapsed().as_secs_f64());
    
    response
}
```

## 🗄️ Данные: Архитектура → Схема БД

### Архитектурное решение
- **Доменное разделение**: Таблицы по доменам
- **Референциальная целостность**: FK связи между доменами
- **Производительность**: Индексы для частых запросов

### Реализация в коде
```sql
-- migrations/001_initial_schema.sql
-- Воплощение архитектурных решений в схеме БД

-- Домен Users
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Домен Offers
CREATE TABLE offers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10,2),
    seller_id UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Домен UGC (User Generated Content)
CREATE TABLE reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    offer_id UUID NOT NULL REFERENCES offers(id),
    user_id UUID NOT NULL REFERENCES users(id),
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    text TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Индексы для производительности (архитектурное требование)
CREATE INDEX idx_offers_seller_id ON offers(seller_id);
CREATE INDEX idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX idx_reviews_user_id ON reviews(user_id);
```

```rust
// crates/shared/src/database.rs
use sqlx::{PgPool, Row};
use crate::types::{UserId, OfferId, ReviewId};

/// Реализация архитектурного решения о типизированных ID
pub struct DatabaseConnection {
    pool: PgPool,
}

impl DatabaseConnection {
    pub async fn get_user(&self, user_id: UserId) -> Result<User, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT id, email, name, created_at FROM users WHERE id = $1",
            user_id.0
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(User {
            id: UserId(row.id),
            email: row.email,
            name: row.name,
            created_at: row.created_at,
        })
    }
}
```

## 🔐 Безопасность: Принципы → Middleware

### Архитектурное решение
- **Аутентификация**: JWT токены
- **Авторизация**: Role-based access control
- **Валидация**: Входные данные

### Реализация в коде
```rust
// crates/shared/src/auth.rs
use jsonwebtoken::{decode, DecodingKey, Validation};
use async_graphql::{Context, Guard, Result as GraphQLResult};

/// Архитектурное решение о структуре пользовательского контекста
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

/// Guard для проверки разрешений - реализация RBAC архитектуры
pub struct RequirePermission {
    pub permission: Permission,
}

#[async_trait::async_trait]
impl Guard for RequirePermission {
    async fn check(&self, ctx: &Context<'_>) -> GraphQLResult<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User not authenticated")?;

        if user_context.permissions.contains(&self.permission) {
            Ok(())
        } else {
            Err("Insufficient permissions".into())
        }
    }
}

/// Middleware для извлечения JWT токена
pub async fn extract_user_context(
    headers: &HeaderMap,
    jwt_secret: &str,
) -> Result<UserContext, AuthError> {
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingToken)?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &Validation::default(),
    )?;

    Ok(UserContext {
        user_id: UserId(token_data.claims.user_id),
        roles: token_data.claims.roles,
        permissions: resolve_permissions(&token_data.claims.roles),
    })
}
```

## 🚀 Автоматизация: DevOps → Scripts

### Архитектурное решение
- **Простота разработки**: One-command setup
- **Воспроизводимость**: Идентичные среды
- **Быстрая итерация**: Hot reload

### Реализация в коде
```bash
#!/bin/bash
# scripts/dev-setup.sh - Воплощение архитектурного принципа простоты

set -e

echo "🚀 Setting up Auto.ru GraphQL Federation development environment"

# Проверка архитектурных зависимостей
check_dependencies() {
    echo "📋 Checking dependencies..."
    
    if ! command -v docker &> /dev/null; then
        echo "❌ Docker is required but not installed"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        echo "❌ Rust/Cargo is required but not installed"
        exit 1
    fi
    
    echo "✅ All dependencies satisfied"
}

# Настройка среды согласно архитектурным требованиям
setup_environment() {
    echo "🔧 Setting up environment..."
    
    # Копирование конфигурации из архитектурного шаблона
    if [ ! -f .env ]; then
        cp .env.example .env
        echo "📝 Created .env from template"
    fi
    
    # Создание Docker сетей согласно архитектуре
    docker network create autoru-app-network 2>/dev/null || true
    docker network create autoru-data-network 2>/dev/null || true
    
    echo "✅ Environment configured"
}

# Сборка согласно архитектурным компонентам
build_services() {
    echo "🔨 Building services..."
    
    # Сборка shared библиотеки (архитектурная основа)
    cargo build -p shared
    
    # Сборка всех подграфов
    cargo build -p ugc-subgraph
    cargo build -p users-subgraph
    cargo build -p offers-subgraph
    
    # Сборка Docker образов
    docker-compose build
    
    echo "✅ Services built successfully"
}

# Запуск системы согласно архитектурному дизайну
start_services() {
    echo "🚀 Starting services..."
    
    # Запуск в правильном порядке (архитектурная зависимость)
    docker-compose up -d postgres redis
    sleep 5  # Ожидание готовности БД
    
    # Миграции БД (архитектурная схема)
    cargo run -p migration-tool
    
    # Запуск подграфов
    docker-compose up -d ugc-subgraph users-subgraph offers-subgraph
    sleep 10  # Ожидание готовности подграфов
    
    # Запуск Apollo Router (федеративная композиция)
    docker-compose up -d apollo-router
    
    echo "✅ All services started"
}

# Валидация архитектурной целостности
validate_setup() {
    echo "🔍 Validating setup..."
    
    # Проверка здоровья всех компонентов
    curl -f http://localhost:4000/health || {
        echo "❌ Apollo Router health check failed"
        exit 1
    }
    
    curl -f http://localhost:4001/health || {
        echo "❌ UGC Subgraph health check failed"
        exit 1
    }
    
    # Проверка федеративной схемы
    curl -X POST http://localhost:4000/graphql \
        -H "Content-Type: application/json" \
        -d '{"query": "{ __schema { types { name } } }"}' || {
        echo "❌ GraphQL schema validation failed"
        exit 1
    }
    
    echo "✅ Setup validation passed"
}

# Выполнение всех этапов архитектурной настройки
main() {
    check_dependencies
    setup_environment
    build_services
    start_services
    validate_setup
    
    echo ""
    echo "🎉 Development environment is ready!"
    echo "📊 GraphQL Playground: http://localhost:4000"
    echo "📈 Prometheus: http://localhost:9090"
    echo "🔍 Jaeger: http://localhost:16686"
    echo ""
    echo "Run 'make dev-stop' to stop all services"
}

main "$@"
```

## 🔄 Cargo Workspace: Архитектурная модульность

### Архитектурное решение
```toml
# Cargo.toml - Архитектурная структура проекта
[workspace]
members = [
    "crates/apollo-router",    # Федеративный роутер
    "crates/ugc-subgraph",     # Домен UGC
    "crates/users-subgraph",   # Домен Users  
    "crates/offers-subgraph",  # Домен Offers
    "crates/shared",           # Общие компоненты
]
resolver = "2"

# Общие зависимости для всех крейтов
[workspace.dependencies]
async-graphql = "7.0"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono"] }
```

### Реализация модульности
```rust
// crates/shared/src/lib.rs - Архитектурная основа
//! Shared library для Auto.ru GraphQL Federation
//! 
//! Этот крейт содержит общие компоненты, используемые всеми подграфами:
//! - Типизированные ID для type safety
//! - Утилиты аутентификации и авторизации  
//! - Обработка ошибок
//! - Общие GraphQL типы

pub mod auth;
pub mod errors;
pub mod types;
pub mod utils;
pub mod telemetry;

// Re-exports для удобства использования
pub use auth::*;
pub use errors::*;
pub use types::*;
pub use utils::*;
pub use telemetry::*;

/// Версия API - для совместимости между подграфами
pub const API_VERSION: &str = "1.0.0";
```

## 📈 Метрики: Архитектурная наблюдаемость

### Архитектурное решение
```rust
// crates/ugc-subgraph/src/resolvers.rs
use async_graphql::{Context, Object, Result};
use shared::{RequirePermission, Permission, Metrics};

pub struct ReviewResolver;

#[Object]
impl ReviewResolver {
    /// Создание отзыва с архитектурными гарантиями
    #[graphql(guard = "RequirePermission { permission: Permission::CreateReview }")]
    async fn create_review(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> Result<Review> {
        let metrics = ctx.data::<Metrics>()?;
        let start = std::time::Instant::now();
        
        // Бизнес-логика создания отзыва
        let review = self.create_review_impl(ctx, input).await?;
        
        // Сбор метрик согласно архитектурным требованиям
        metrics.reviews_created.inc();
        metrics.review_creation_duration.observe(start.elapsed().as_secs_f64());
        
        Ok(review)
    }
}
```

## 🎯 Заключение: Архитектура как код

Task 1 Context диаграмма демонстрирует, как **архитектурные решения трансформируются в исполняемый код**:

### 🏗️ **Архитектурные принципы → Код**
- **Федеративная архитектура** → Apollo Router конфигурация и supergraph схема
- **Доменное разделение** → Отдельные Rust крейты для каждого домена
- **Type Safety** → Типизированные ID и строгая типизация GraphQL
- **Безопасность** → JWT middleware и RBAC guards

### 🔧 **Инфраструктурные решения → Автоматизация**
- **Контейнеризация** → Docker Compose оркестрация
- **Мониторинг** → Prometheus метрики и Jaeger трассировка
- **DevOps** → Автоматизированные скрипты настройки

### 📊 **Качество кода → Архитектурная целостность**
- **Модульность** → Cargo workspace структура
- **Переиспользование** → Shared библиотека
- **Наблюдаемость** → Встроенные метрики и трассировка
- **Тестируемость** → Изолированные компоненты

Диаграмма служит **живой документацией**, связывающей архитектурные решения с их конкретной реализацией в коде, обеспечивая понимание системы на всех уровнях абстракции.