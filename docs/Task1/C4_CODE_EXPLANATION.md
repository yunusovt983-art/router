# Task 1: Code Diagram - Детальная реализация на уровне кода

## Обзор

Code диаграмма Task 1 представляет **самый детальный уровень архитектуры**, показывая конкретную реализацию компонентов в виде Rust модулей, структур и функций. Диаграмма служит прямым мостом между архитектурным дизайном и исполняемым кодом.

## 📚 Shared Crate: Архитектурная основа в коде

### Types Module - Типизированная архитектура
```rust
// crates/shared/src/types.rs
//! Типизированные ID для предотвращения путаницы между доменами

use async_graphql::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Типизированный ID пользователя - архитектурное решение о type safety
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct UserId(pub uuid::Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
    
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(uuid::Uuid::from_str(s)?))
    }
}

#[Scalar]
impl ScalarType for UserId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => {
                Self::from_string(&s)
                    .map_err(|_| InputValueError::custom("Invalid UUID format"))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.to_string())
    }
}

/// Пагинация с валидацией согласно GraphQL Cursor Connections Specification
#[derive(Debug, Clone, InputObject)]
pub struct PaginationInput {
    pub first: Option<i32>,
    pub after: Option<String>,
    pub last: Option<i32>, 
    pub before: Option<String>,
}

impl PaginationInput {
    pub fn validate(&self) -> Result<(), String> {
        match (self.first, self.last) {
            (Some(first), Some(_)) => Err("Cannot specify both first and last".to_string()),
            (Some(first), None) if first < 0 || first > 100 => {
                Err("first must be between 0 and 100".to_string())
            }
            (None, Some(last)) if last < 0 || last > 100 => {
                Err("last must be between 0 and 100".to_string())
            }
            _ => Ok(()),
        }
    }
}
```

### Auth Module - Реализация безопасности
```rust
// crates/shared/src/auth.rs
use async_graphql::{Context, Guard, Result as GraphQLResult};
use jsonwebtoken::{decode, DecodingKey, Validation};

/// Контекст пользователя с типизированными данными
#[derive(Debug, Clone)]
pub struct UserContext {
    pub user_id: UserId,
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
}

/// JWT сервис с валидацией токенов
pub struct JwtService {
    secret: String,
    validation: Validation,
}

impl JwtService {
    pub fn new(secret: String) -> Self {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        validation.validate_nbf = true;
        
        Self { secret, validation }
    }
    
    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &self.validation,
        )?;
        
        Ok(token_data.claims)
    }
}

/// Guard для проверки разрешений в GraphQL
pub struct RequirePermission {
    pub permission: Permission,
}

#[async_trait::async_trait]
impl Guard for RequirePermission {
    async fn check(&self, ctx: &Context<'_>) -> GraphQLResult<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "Authentication required")?;

        if user_context.has_permission(&self.permission) {
            Ok(())
        } else {
            Err("Insufficient permissions".into())
        }
    }
}
```

## 🏗️ Workspace Structure: Файловая организация

### Cargo.toml - Workspace конфигурация
```toml
# Cargo.toml - Корневая конфигурация workspace
[workspace]
members = [
    "crates/apollo-router",
    "crates/ugc-subgraph", 
    "crates/users-subgraph",
    "crates/offers-subgraph",
    "crates/shared"
]
resolver = "2"

[workspace.dependencies]
async-graphql = { version = "7.0", features = ["apollo_tracing"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

### Shared Crate Entry Point
```rust
// crates/shared/src/lib.rs
//! Shared library для Auto.ru GraphQL Federation
//! Содержит общие компоненты для всех подграфов

pub mod auth;
pub mod errors;
pub mod types;
pub mod utils;
pub mod database;
pub mod cache;
pub mod telemetry;

// Re-exports для удобства использования
pub use auth::*;
pub use errors::*;
pub use types::*;
pub use utils::*;
pub use database::*;
pub use cache::*;
pub use telemetry::*;

/// Версия API для совместимости
pub const API_VERSION: &str = "1.0.0";

/// Инициализация shared компонентов
pub async fn init_shared_services() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация телеметрии
    init_telemetry()?;
    
    // Инициализация метрик
    init_metrics()?;
    
    Ok(())
}
```

## 🐳 Docker Infrastructure: Конфигурация как код

### Docker Compose - Оркестрация сервисов
```yaml
# docker-compose.yml
version: '3.8'

services:
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
      target: runtime
    ports:
      - "4000:4000"
    environment:
      - RUST_LOG=info,apollo_router=debug
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
    depends_on:
      - postgres
      - redis
    networks:
      - federation-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  postgres_data:
  redis_data:

networks:
  federation-network:
    driver: bridge
  data-network:
    driver: bridge
    internal: true
```

### Multi-stage Dockerfile
```dockerfile
# crates/apollo-router/Dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Сборка с оптимизациями
RUN cargo build --release -p apollo-router

# Runtime образ
FROM debian:bookworm-slim as runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/apollo-router /usr/local/bin/
COPY crates/apollo-router/router.yaml /app/

EXPOSE 4000
CMD ["apollo-router", "--config", "/app/router.yaml"]
```

## 🔧 Development Tools: Автоматизация разработки

### Makefile - Команды разработки
```makefile
# Makefile
.PHONY: dev build test clean

dev: ## Запуск среды разработки
	@echo "🚀 Starting development environment..."
	./scripts/dev-setup.sh

build: ## Сборка всех компонентов
	@echo "🔨 Building workspace..."
	cargo build --workspace --release

test: ## Запуск тестов
	@echo "🧪 Running tests..."
	cargo test --workspace

clean: ## Очистка
	@echo "🧹 Cleaning up..."
	cargo clean
	docker-compose down -v
```

### Development Setup Script
```bash
#!/bin/bash
# scripts/dev-setup.sh

set -e

echo "🚀 Setting up Auto.ru GraphQL Federation"

# Проверка зависимостей
check_dependencies() {
    command -v docker >/dev/null 2>&1 || {
        echo "❌ Docker required"
        exit 1
    }
    
    command -v cargo >/dev/null 2>&1 || {
        echo "❌ Rust/Cargo required"
        exit 1
    }
}

# Настройка окружения
setup_environment() {
    if [ ! -f .env ]; then
        cp .env.example .env
        echo "📝 Created .env"
    fi
    
    docker network create federation-network 2>/dev/null || true
}

# Сборка и запуск
build_and_start() {
    cargo build --workspace
    docker-compose build
    docker-compose up -d
}

# Валидация
validate_setup() {
    sleep 10
    
    if curl -f http://localhost:4000/health >/dev/null 2>&1; then
        echo "✅ Apollo Router healthy"
    else
        echo "❌ Apollo Router failed"
        exit 1
    fi
}

main() {
    check_dependencies
    setup_environment
    build_and_start
    validate_setup
    
    echo "🎉 Environment ready!"
    echo "📊 GraphQL: http://localhost:4000/graphql"
}

main "$@"
```

### Environment Configuration
```bash
# .env.example
DATABASE_URL=postgresql://postgres:password@localhost:5432/auto_ru_federation
REDIS_URL=redis://localhost:6379
JWT_SECRET=your_secret_key_here
RUST_LOG=info
```

## 📊 Monitoring Configuration: Наблюдаемость

### Prometheus Configuration
```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    metrics_path: '/metrics'
    
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:9091']
```

### Jaeger Configuration
```yaml
# monitoring/jaeger.yml
collector:
  zipkin:
    host-port: ":9411"
  
query:
  base-path: /
  
agent:
  jaeger:
    thrift-compact-port: 6831
```

## 🎯 Заключение: Код как архитектурная реализация

Code диаграмма Task 1 демонстрирует **прямую связь между архитектурными решениями и кодом**:

### 🏗️ **Архитектурные принципы → Rust код**
- **Type Safety** → Типизированные ID структуры
- **Модульность** → Cargo workspace организация  
- **Безопасность** → JWT middleware и Guards
- **Переиспользование** → Shared library

### 🔧 **Инфраструктурные решения → Конфигурация**
- **Контейнеризация** → Docker Compose и Dockerfiles
- **Автоматизация** → Makefile и bash скрипты
- **Мониторинг** → Prometheus и Jaeger конфигурация
- **Среда разработки** → Environment variables

### 📊 **DevOps принципы → Исполняемые скрипты**
- **One-command setup** → dev-setup.sh скрипт
- **Воспроизводимость** → Docker контейнеры
- **Валидация** → Health checks и тесты
- **Простота использования** → Makefile команды

Диаграмма служит **исполняемой документацией**, где каждый архитектурный элемент имеет прямое соответствие в коде, обеспечивая полную трассируемость от дизайна к реализации.