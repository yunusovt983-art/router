# Task 1: Container Diagram - Подробное объяснение Docker инфраструктуры

## 🎯 Цель диаграммы

Container диаграмма Task 1 демонстрирует **детальную архитектуру Docker-based среды разработки** для федеративной GraphQL системы Auto.ru. Диаграмма служит мостом между высокоуровневым дизайном и конкретной реализацией контейнеризованной инфраструктуры, показывая как архитектурные решения воплощаются в исполняемые Docker контейнеры.

## 🏗️ Архитектурные слои и их реализация

### 1. Docker Development Environment - Контейнеризованная среда

#### Архитектурное решение: Изоляция и воспроизводимость
**Принцип**: Каждый сервис работает в изолированном контейнере с четко определенными зависимостями
**Реализация**:``
`yaml
# docker-compose.yml - Воплощение архитектурной изоляции
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
      - RUST_LOG=info
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
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
```

#### Apollo Router Container - Федеративный шлюз
**Архитектурная функция**: Единая точка входа для всех GraphQL запросов
**Техническая реализация**:
```dockerfile
# crates/apollo-router/Dockerfile
FROM rust:1.75-bookworm as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Оптимизация сборки через кеширование зависимостей
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin apollo-router

FROM debian:bookworm-slim as runtime

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Копирование скомпилированного бинарника
COPY --from=builder /app/target/release/apollo-router /usr/local/bin/
COPY router.yaml supergraph.graphql /app/

EXPOSE 4000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4000/health || exit 1

CMD ["apollo-router", "--config", "/app/router.yaml", "--supergraph", "/app/supergraph.graphql"]
```

**Конфигурация роутера**:
```yaml
# router.yaml - Архитектурная конфигурация федерации
supergraph:
  listen: 0.0.0.0:4000
  introspection: true

cors:
  origins:
    - http://localhost:3000
  allow_credentials: true

telemetry:
  instrumentation:
    spans:
      mode: spec_compliant
  exporters:
    metrics:
      prometheus:
        enabled: true
        listen: 0.0.0.0:9090
    tracing:
      jaeger:
        endpoint: http://jaeger:14268/api/traces

subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
  users:
    routing_url: http://users-subgraph:4002/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
  offers:
    routing_url: http://offers-subgraph:4003/graphql
    retry:
      min_per_sec: 10
      ttl: 10s
```

### 2. Subgraph Containers - Доменные сервисы

#### UGC Subgraph Container - Пользовательский контент
**Архитектурная функция**: Управление отзывами и рейтингами с изоляцией домена
**Техническая реализация**:
```dockerfile
# crates/ugc-subgraph/Dockerfile
FROM rust:1.75-bookworm as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Сборка только UGC подграфа и его зависимостей
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin ugc-subgraph

FROM debian:bookworm-slim as runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/

EXPOSE 4001
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```

**Сервис конфигурация**:
```rust
// crates/ugc-subgraph/src/main.rs
use axum::{routing::post, Router};
use shared::{create_database_pool, init_telemetry, Metrics};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация телеметрии согласно архитектурным требованиям
    init_telemetry("ugc-subgraph", "http://jaeger:14268")?;
    
    // Подключение к базе данных с connection pooling
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let db_pool = create_database_pool(&database_url).await?;
    
    // Инициализация метрик
    let metrics = Arc::new(Metrics::new());
    
    // Создание GraphQL схемы с инструментацией
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(db_pool)
        .data(metrics.clone())
        .extension(Tracing)
        .extension(Logger)
        .finish();
    
    // HTTP сервер с health check endpoint
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(schema);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4001").await?;
    tracing::info!("UGC Subgraph listening on http://0.0.0.0:4001");
    
    axum::serve(listener, app).await?;
    Ok(())
}
```

#### Users Subgraph Container - Управление пользователями
**Архитектурная функция**: Аутентификация и профили с безопасностью
**Техническая реализация**:
```rust
// crates/users-subgraph/src/main.rs
use shared::{AuthService, JwtConfig, UserContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // JWT конфигурация для аутентификации
    let jwt_config = JwtConfig {
        secret: std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
        issuer: "auto.ru".to_string(),
        audience: "auto.ru-api".to_string(),
        expiration: Duration::from_secs(3600), // 1 hour
    };
    
    let auth_service = Arc::new(AuthService::new(jwt_config));
    
    // GraphQL схема с аутентификацией
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(db_pool)
        .data(auth_service)
        .extension(AuthExtension)
        .finish();
    
    // Middleware для извлечения JWT токенов
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(schema);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4002").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn auth_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Извлечение JWT токена из заголовков
    let auth_header = request.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());
    
    if let Some(token) = auth_header.and_then(|h| h.strip_prefix("Bearer ")) {
        let auth_service = request.extensions().get::<Arc<AuthService>>().unwrap();
        
        match auth_service.validate_token(token).await {
            Ok(user_context) => {
                request.extensions_mut().insert(user_context);
            }
            Err(_) => {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
    
    Ok(next.run(request).await)
}
```

### 3. Data Layer Containers - Хранилище данных

#### PostgreSQL Container - Основная база данных
**Архитектурная функция**: Надежное хранение структурированных данных
**Техническая реализация**:
```yaml
# docker-compose.yml - PostgreSQL конфигурация
postgres:
  image: postgres:15-alpine
  environment:
    POSTGRES_DB: autoru
    POSTGRES_USER: postgres
    POSTGRES_PASSWORD: password
    POSTGRES_INITDB_ARGS: "--encoding=UTF8 --locale=C"
  ports:
    - "5432:5432"
  volumes:
    - postgres_data:/var/lib/postgresql/data
    - ./migrations:/docker-entrypoint-initdb.d
  networks:
    - data-network
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U postgres -d autoru"]
    interval: 10s
    timeout: 5s
    retries: 5
    start_period: 10s
  command: >
    postgres
    -c shared_preload_libraries=pg_stat_statements
    -c pg_stat_statements.track=all
    -c max_connections=200
    -c shared_buffers=256MB
    -c effective_cache_size=1GB
```

**Инициализация схемы**:
```sql
-- migrations/001_initial_schema.sql
-- Архитектурная схема данных

-- Включение расширений для UUID и статистики
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Домен Users
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'user',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Домен Offers
CREATE TABLE offers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10,2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'RUB',
    seller_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    car_make VARCHAR(100) NOT NULL,
    car_model VARCHAR(100) NOT NULL,
    car_year INTEGER NOT NULL CHECK (car_year >= 1900 AND car_year <= EXTRACT(YEAR FROM NOW()) + 1),
    car_mileage INTEGER CHECK (car_mileage >= 0),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Домен UGC
CREATE TABLE reviews (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    offer_id UUID NOT NULL REFERENCES offers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    title VARCHAR(255),
    content TEXT,
    is_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(offer_id, user_id) -- Один отзыв на объявление от пользователя
);

-- Индексы для производительности (архитектурное требование)
CREATE INDEX CONCURRENTLY idx_users_email ON users(email);
CREATE INDEX CONCURRENTLY idx_offers_seller_id ON offers(seller_id);
CREATE INDEX CONCURRENTLY idx_offers_car_make_model ON offers(car_make, car_model);
CREATE INDEX CONCURRENTLY idx_offers_price ON offers(price);
CREATE INDEX CONCURRENTLY idx_offers_created_at ON offers(created_at DESC);
CREATE INDEX CONCURRENTLY idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX CONCURRENTLY idx_reviews_user_id ON reviews(user_id);
CREATE INDEX CONCURRENTLY idx_reviews_rating ON reviews(rating);

-- Функции для автоматического обновления updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Триггеры для автоматического обновления timestamps
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_offers_updated_at BEFORE UPDATE ON offers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_reviews_updated_at BEFORE UPDATE ON reviews
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
```

#### Redis Container - Кеширование и сессии
**Архитектурная функция**: Высокопроизводительное кеширование
**Техническая реализация**:
```yaml
# docker-compose.yml - Redis конфигурация
redis:
  image: redis:7-alpine
  ports:
    - "6379:6379"
  volumes:
    - redis_data:/data
    - ./redis.conf:/usr/local/etc/redis/redis.conf
  networks:
    - data-network
  healthcheck:
    test: ["CMD", "redis-cli", "ping"]
    interval: 10s
    timeout: 3s
    retries: 3
    start_period: 10s
  command: redis-server /usr/local/etc/redis/redis.conf
```

**Redis конфигурация**:
```conf
# redis.conf - Оптимизация для production
# Память и производительность
maxmemory 512mb
maxmemory-policy allkeys-lru
tcp-keepalive 300

# Персистентность данных
save 900 1
save 300 10
save 60 10000
rdbcompression yes
rdbchecksum yes

# Безопасность
protected-mode yes
bind 0.0.0.0
requirepass redis_password

# Логирование
loglevel notice
logfile /var/log/redis/redis-server.log

# Клиентские соединения
timeout 300
tcp-backlog 511
maxclients 10000
```

### 4. Monitoring Infrastructure - Наблюдаемость

#### Prometheus Container - Сбор метрик
**Архитектурная функция**: Централизованный сбор и хранение метрик
**Техническая реализация**:
```yaml
# docker-compose.yml - Prometheus конфигурация
prometheus:
  image: prom/prometheus:latest
  ports:
    - "9090:9090"
  volumes:
    - ./prometheus.yml:/etc/prometheus/prometheus.yml
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
```

**Prometheus конфигурация**:
```yaml
# prometheus.yml - Архитектурная конфигурация мониторинга
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "alert_rules.yml"

scrape_configs:
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:4001']
    metrics_path: '/metrics'
    
  - job_name: 'users-subgraph'
    static_configs:
      - targets: ['users-subgraph:4002']
    metrics_path: '/metrics'
    
  - job_name: 'offers-subgraph'
    static_configs:
      - targets: ['offers-subgraph:4003']
    metrics_path: '/metrics'
    
  - job_name: 'postgres-exporter'
    static_configs:
      - targets: ['postgres-exporter:9187']

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

#### Jaeger Container - Distributed Tracing
**Архитектурная функция**: Трассировка запросов в распределенной системе
**Техническая реализация**:
```yaml
# docker-compose.yml - Jaeger конфигурация
jaeger:
  image: jaegertracing/all-in-one:latest
  ports:
    - "16686:16686"  # Jaeger UI
    - "14268:14268"  # HTTP collector
  environment:
    COLLECTOR_OTLP_ENABLED: true
    COLLECTOR_ZIPKIN_HOST_PORT: ":9411"
  networks:
    - monitoring-network
    - federation-network
  volumes:
    - jaeger_data:/badger
```

### 5. Development Tools - Инструменты разработки

#### Cargo Workspace - Система сборки
**Архитектурная функция**: Управление зависимостями и сборка
**Техническая реализация**:
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
async-graphql = { version = "7.0", features = ["tracing", "apollo_tracing"] }
tokio = { version = "1.0", features = ["full"] }
axum = { version = "0.7", features = ["tracing"] }
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono", "runtime-tokio-rustls"] }
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
prometheus = "0.13"
opentelemetry = "0.21"
opentelemetry-jaeger = "0.20"

# Оптимизация для разработки
[profile.dev]
debug = true
opt-level = 0

# Оптимизация для production
[profile.release]
debug = false
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
```

#### Docker Compose - Оркестрация сервисов
**Архитектурная функция**: Управление жизненным циклом контейнеров
**Техническая реализация**:
```yaml
# docker-compose.yml - Полная оркестрация
version: '3.8'

networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
  data-network:
    driver: bridge
    internal: true  # Изоляция данных
  monitoring-network:
    driver: bridge

volumes:
  postgres_data:
    driver: local
  redis_data:
    driver: local
  prometheus_data:
    driver: local
  jaeger_data:
    driver: local

services:
  # [Все сервисы определены выше]
  
  # Дополнительные утилиты для разработки
  postgres-exporter:
    image: prometheuscommunity/postgres-exporter
    environment:
      DATA_SOURCE_NAME: "postgresql://postgres:password@postgres:5432/autoru?sslmode=disable"
    networks:
      - data-network
      - monitoring-network
    depends_on:
      - postgres

  redis-exporter:
    image: oliver006/redis_exporter
    environment:
      REDIS_ADDR: "redis://redis:6379"
      REDIS_PASSWORD: "redis_password"
    networks:
      - data-network
      - monitoring-network
    depends_on:
      - redis
```

## 🔄 Сетевая архитектура и безопасность

### Сегментация сетей
**Архитектурное решение**: Изоляция трафика по назначению
**Реализация**:
```yaml
# Сетевая архитектура с изоляцией
networks:
  # Публичная сеть для API трафика
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
    driver_opts:
      com.docker.network.bridge.name: "autoru-federation"
      
  # Изолированная сеть для данных
  data-network:
    driver: bridge
    internal: true  # Нет доступа к интернету
    ipam:
      config:
        - subnet: 172.21.0.0/16
          gateway: 172.21.0.1
    driver_opts:
      com.docker.network.bridge.name: "autoru-data"
      
  # Сеть мониторинга
  monitoring-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.22.0.0/16
          gateway: 172.22.0.1
```

### Health Checks и Service Discovery
**Архитектурное решение**: Автоматическое обнаружение и проверка здоровья
**Реализация**:
```bash
#!/bin/bash
# scripts/health-check.sh - Комплексная проверка здоровья системы

set -e

echo "🏥 Checking system health..."

# Проверка базовых сервисов
check_service() {
    local service_name=$1
    local health_url=$2
    local max_attempts=30
    local attempt=1
    
    echo "Checking $service_name..."
    
    while [ $attempt -le $max_attempts ]; do
        if curl -f -s "$health_url" > /dev/null 2>&1; then
            echo "✅ $service_name is healthy"
            return 0
        fi
        
        echo "⏳ Waiting for $service_name (attempt $attempt/$max_attempts)..."
        sleep 2
        ((attempt++))
    done
    
    echo "❌ $service_name failed health check"
    return 1
}

# Проверка всех сервисов согласно архитектуре
check_service "PostgreSQL" "http://localhost:5432"
check_service "Redis" "http://localhost:6379"
check_service "UGC Subgraph" "http://localhost:4001/health"
check_service "Users Subgraph" "http://localhost:4002/health"
check_service "Offers Subgraph" "http://localhost:4003/health"
check_service "Apollo Router" "http://localhost:4000/health"
check_service "Prometheus" "http://localhost:9090/-/healthy"
check_service "Jaeger" "http://localhost:16686"

# Проверка федеративной схемы
echo "🔍 Validating GraphQL federation..."
curl -X POST http://localhost:4000/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ __schema { types { name } } }"}' \
    -f -s > /dev/null || {
    echo "❌ GraphQL federation validation failed"
    exit 1
}

echo "✅ All systems are healthy and federation is working!"
```

Эта Container диаграмма демонстрирует как архитектурные принципы Task 1 воплощаются в конкретную Docker инфраструктуру, обеспечивая изоляцию, масштабируемость и наблюдаемость системы.