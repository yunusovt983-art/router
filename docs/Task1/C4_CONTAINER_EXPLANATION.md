# Task 1: Container Diagram - Архитектурная реализация в контейнерах

## Обзор

Container диаграмма Task 1 показывает **детальную архитектуру системы на уровне контейнеров**, демонстрируя как высокоуровневые архитектурные решения воплощаются в конкретные исполняемые компоненты. Диаграмма служит мостом между системной архитектурой и технической реализацией.

## 🐳 Docker Environment: Архитектура → Контейнеры

### Архитектурное решение: Контейнеризация
- **Изоляция сервисов**: Каждый компонент в отдельном контейнере
- **Воспроизводимость**: Идентичные среды разработки
- **Масштабируемость**: Независимое масштабирование компонентов

### Реализация в Docker Compose
```yaml
# docker-compose.yml - Воплощение архитектурных принципов
version: '3.8'

services:
  # Apollo Router - Федеративный шлюз
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
    ports:
      - "4000:4000"  # Единая точка входа
    depends_on:
      - ugc-subgraph
      - users-subgraph  
      - offers-subgraph
    environment:
      - RUST_LOG=info
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
    networks:
      - federation-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

### Dockerfile для Apollo Router
```dockerfile
# crates/apollo-router/Dockerfile
# Multi-stage build для оптимизации размера образа
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Сборка только Apollo Router
RUN cargo build --release -p apollo-router

# Runtime образ
FROM debian:bookworm-slim

# Установка зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Копирование бинарника
COPY --from=builder /app/target/release/apollo-router /usr/local/bin/
COPY --from=builder /app/crates/apollo-router/router.yaml /app/

EXPOSE 4000
CMD ["apollo-router", "--config", "/app/router.yaml"]
```

## 🔧 Subgraph Containers: Доменная архитектура

### UGC Subgraph Container
```rust
// crates/ugc-subgraph/src/main.rs - Реализация UGC домена
use async_graphql::{Schema, EmptySubscription};
use axum::{routing::post, Router, Extension};
use shared::{create_database_pool, setup_telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация телеметрии согласно архитектурным требованиям
    setup_telemetry("ugc-subgraph")?;
    
    // Подключение к БД (архитектурная зависимость)
    let pool = create_database_pool(&env::var("DATABASE_URL")?).await?;
    
    // Создание GraphQL схемы для UGC домена
    let schema = Schema::build(
        ugc::Query::default(),
        ugc::Mutation::default(), 
        EmptySubscription
    )
    .data(pool)
    .finish();
    
    // HTTP сервер для подграфа
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))
        .layer(Extension(schema));
    
    println!("🚀 UGC Subgraph listening on http://0.0.0.0:4001");
    
    axum::Server::bind(&"0.0.0.0:4001".parse()?)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
```

### Users Subgraph Container
```dockerfile
# crates/users-subgraph/Dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Сборка Users подграфа с оптимизациями
RUN cargo build --release -p users-subgraph

FROM debian:bookworm-slim

# Установка зависимостей для аутентификации
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/users-subgraph /usr/local/bin/

EXPOSE 4002
CMD ["users-subgraph"]
```

## 🗄️ Data Layer: Архитектура данных

### PostgreSQL Container
```yaml
# PostgreSQL сервис с архитектурными требованиями
postgres:
  image: postgres:15
  environment:
    POSTGRES_DB: auto_ru_federation
    POSTGRES_USER: postgres
    POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
  ports:
    - "5432:5432"
  volumes:
    # Персистентность данных
    - postgres_data:/var/lib/postgresql/data
    # Миграции схемы БД
    - ./migrations:/docker-entrypoint-initdb.d
  networks:
    - data-network
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U postgres"]
    interval: 10s
    timeout: 5s
    retries: 5
```

### Database Migrations
```sql
-- migrations/001_create_schema.sql
-- Реализация архитектурного решения о доменном разделении

-- Домен Users
CREATE SCHEMA IF NOT EXISTS users;

CREATE TABLE users.users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Домен Offers  
CREATE SCHEMA IF NOT EXISTS offers;

CREATE TABLE offers.offers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10,2),
    seller_id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Федеративная связь с Users доменом
    FOREIGN KEY (seller_id) REFERENCES users.users(id)
);

-- Домен UGC
CREATE SCHEMA IF NOT EXISTS ugc;

CREATE TABLE ugc.reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    offer_id UUID NOT NULL,
    user_id UUID NOT NULL,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    text TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Федеративные связи
    FOREIGN KEY (offer_id) REFERENCES offers.offers(id),
    FOREIGN KEY (user_id) REFERENCES users.users(id)
);
```

### Redis Container
```yaml
# Redis для кеширования и сессий
redis:
  image: redis:7-alpine
  ports:
    - "6379:6379"
  command: >
    redis-server 
    --maxmemory 256mb 
    --maxmemory-policy allkeys-lru
    --save 900 1
    --save 300 10
    --save 60 10000
  volumes:
    - redis_data:/data
  networks:
    - data-network
  healthcheck:
    test: ["CMD", "redis-cli", "ping"]
    interval: 10s
    timeout: 3s
    retries: 3
```

## 📊 Monitoring & Observability

### Prometheus Container
```yaml
# Prometheus для сбора метрик
prometheus:
  image: prom/prometheus:latest
  ports:
    - "9090:9090"
  volumes:
    - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
    - prometheus_data:/prometheus
  command:
    - '--config.file=/etc/prometheus/prometheus.yml'
    - '--storage.tsdb.path=/prometheus'
    - '--web.console.libraries=/etc/prometheus/console_libraries'
    - '--web.console.templates=/etc/prometheus/consoles'
  networks:
    - monitoring-network
```

### Prometheus Configuration
```yaml
# monitoring/prometheus.yml - Конфигурация сбора метрик
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  # Apollo Router метрики
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    metrics_path: '/metrics'
    scrape_interval: 5s

  # UGC Subgraph метрики  
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:9091']
    metrics_path: '/metrics'
    
  # Users Subgraph метрики
  - job_name: 'users-subgraph'
    static_configs:
      - targets: ['users-subgraph:9092']
    metrics_path: '/metrics'
    
  # Offers Subgraph метрики
  - job_name: 'offers-subgraph'
    static_configs:
      - targets: ['offers-subgraph:9093']
    metrics_path: '/metrics'
```

### Jaeger Container
```yaml
# Jaeger для distributed tracing
jaeger:
  image: jaegertracing/all-in-one:latest
  ports:
    - "16686:16686"  # Jaeger UI
    - "14268:14268"  # Jaeger collector
  environment:
    - COLLECTOR_OTLP_ENABLED=true
  networks:
    - monitoring-network
```

## 🔧 Development Tools: Автоматизация разработки

### Cargo Workspace Configuration
```toml
# Cargo.toml - Архитектурная структура workspace
[workspace]
members = [
    "crates/apollo-router",
    "crates/ugc-subgraph", 
    "crates/users-subgraph",
    "crates/offers-subgraph",
    "crates/shared"
]
resolver = "2"

# Общие зависимости для оптимизации сборки
[workspace.dependencies]
async-graphql = { version = "7.0", features = ["apollo_tracing"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono", "runtime-tokio-rustls"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
prometheus = "0.13"
```

### Makefile для автоматизации
```makefile
# Makefile - Автоматизация архитектурных операций
.PHONY: dev build test clean docker-build docker-up docker-down

# Разработка
dev:
	@echo "🚀 Starting development environment..."
	./scripts/dev-setup.sh

# Сборка всех компонентов
build:
	@echo "🔨 Building all workspace members..."
	cargo build --workspace --release

# Тестирование
test:
	@echo "🧪 Running tests..."
	cargo test --workspace

# Docker операции
docker-build:
	@echo "🐳 Building Docker images..."
	docker-compose build

docker-up:
	@echo "📦 Starting containers..."
	docker-compose up -d

docker-down:
	@echo "🛑 Stopping containers..."
	docker-compose down

# Очистка
clean:
	@echo "🧹 Cleaning up..."
	cargo clean
	docker-compose down -v
	docker system prune -f
```

### Development Setup Script
```bash
#!/bin/bash
# scripts/dev-setup.sh - Реализация архитектурного принципа простоты

set -e

echo "🚀 Setting up Auto.ru GraphQL Federation development environment"

# Проверка зависимостей
check_dependencies() {
    echo "📋 Checking dependencies..."
    
    command -v docker >/dev/null 2>&1 || {
        echo "❌ Docker is required but not installed"
        exit 1
    }
    
    command -v cargo >/dev/null 2>&1 || {
        echo "❌ Rust/Cargo is required but not installed"  
        exit 1
    }
    
    echo "✅ All dependencies satisfied"
}

# Настройка окружения
setup_environment() {
    echo "🔧 Setting up environment..."
    
    # Создание .env из шаблона
    if [ ! -f .env ]; then
        cp .env.example .env
        echo "📝 Created .env from template"
    fi
    
    # Создание Docker сетей
    docker network create federation-network 2>/dev/null || true
    docker network create data-network 2>/dev/null || true
    docker network create monitoring-network 2>/dev/null || true
    
    echo "✅ Environment configured"
}

# Сборка сервисов
build_services() {
    echo "🔨 Building services..."
    
    # Сборка Rust workspace
    cargo build --workspace
    
    # Сборка Docker образов
    docker-compose build
    
    echo "✅ Services built successfully"
}

# Запуск сервисов
start_services() {
    echo "🚀 Starting services..."
    
    # Запуск инфраструктуры
    docker-compose up -d postgres redis prometheus jaeger
    
    # Ожидание готовности БД
    echo "⏳ Waiting for database..."
    sleep 10
    
    # Запуск подграфов
    docker-compose up -d ugc-subgraph users-subgraph offers-subgraph
    
    # Ожидание готовности подграфов
    echo "⏳ Waiting for subgraphs..."
    sleep 15
    
    # Запуск Apollo Router
    docker-compose up -d apollo-router
    
    echo "✅ All services started"
}

# Валидация настройки
validate_setup() {
    echo "🔍 Validating setup..."
    
    # Проверка здоровья сервисов
    services=("apollo-router:4000" "ugc-subgraph:4001" "users-subgraph:4002" "offers-subgraph:4003")
    
    for service in "${services[@]}"; do
        if curl -f "http://localhost:${service#*:}/health" >/dev/null 2>&1; then
            echo "✅ ${service%:*} is healthy"
        else
            echo "❌ ${service%:*} health check failed"
            exit 1
        fi
    done
    
    # Проверка GraphQL схемы
    if curl -X POST http://localhost:4000/graphql \
        -H "Content-Type: application/json" \
        -d '{"query": "{ __schema { types { name } } }"}' >/dev/null 2>&1; then
        echo "✅ GraphQL schema is valid"
    else
        echo "❌ GraphQL schema validation failed"
        exit 1
    fi
    
    echo "✅ Setup validation passed"
}

# Главная функция
main() {
    check_dependencies
    setup_environment  
    build_services
    start_services
    validate_setup
    
    echo ""
    echo "🎉 Development environment is ready!"
    echo ""
    echo "📊 Services:"
    echo "  • GraphQL API: http://localhost:4000/graphql"
    echo "  • Prometheus: http://localhost:9090"
    echo "  • Jaeger UI: http://localhost:16686"
    echo ""
    echo "🛠️  Commands:"
    echo "  • View logs: docker-compose logs -f"
    echo "  • Stop services: make docker-down"
    echo "  • Rebuild: make docker-build"
}

main "$@"
```

## 🌐 Network Architecture

### Network Segmentation
```yaml
# docker-compose.yml - Сетевая архитектура
networks:
  # Сеть для GraphQL федерации
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
  
  # Изолированная сеть для данных
  data-network:
    driver: bridge
    internal: true  # Нет доступа в интернет
    ipam:
      config:
        - subnet: 172.21.0.0/16
        
  # Сеть для мониторинга
  monitoring-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.22.0.0/16
```

## 🎯 Заключение: От архитектуры к контейнерам

Container диаграмма Task 1 демонстрирует **трансформацию архитектурных решений в исполняемые контейнеры**:

### 🏗️ **Архитектурные принципы → Контейнеры**
- **Доменное разделение** → Отдельные контейнеры для каждого подграфа
- **Изоляция сервисов** → Docker контейнеры с сетевой сегментацией
- **Масштабируемость** → Независимое масштабирование контейнеров
- **Наблюдаемость** → Встроенные метрики и трассировка

### 🔧 **Технические решения → Реализация**
- **Федеративная архитектура** → Apollo Router как центральный шлюз
- **Персистентность данных** → PostgreSQL с миграциями схемы
- **Кеширование** → Redis для производительности
- **Мониторинг** → Prometheus + Jaeger стек

### 📊 **DevOps принципы → Автоматизация**
- **Infrastructure as Code** → Docker Compose конфигурация
- **Воспроизводимость** → Идентичные среды через контейнеры
- **Простота разработки** → One-command setup скрипты
- **Быстрая итерация** → Hot reload и health checks

Диаграмма служит **техническим мостом** между архитектурным видением и практической реализацией, показывая как каждое архитектурное решение воплощается в конкретном контейнере с определенной конфигурацией и зависимостями.