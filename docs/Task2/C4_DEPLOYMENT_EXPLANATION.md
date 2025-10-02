# C4 Deployment Diagram - Подробное объяснение Task 2

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_DEPLOYMENT.puml`

Диаграмма развертывания показывает физическую архитектуру UGC системы в среде разработки с использованием Docker контейнеров.

## Архитектура развертывания

### 1. Developer Machine

#### Docker Engine
```plantuml
Deployment_Node(docker_engine, "Docker Engine", "Docker 24.x")
```

**Архитектурная роль**: Контейнеризация и оркестрация сервисов

**Конфигурация Docker Compose**:
```yaml
# docker-compose.yml
version: '3.8'

services:
  # UGC Subgraph Service
  ugc-subgraph:
    build:
      context: .
      dockerfile: crates/ugc-subgraph/Dockerfile
    container_name: ugc-subgraph
    ports:
      - "4001:4001"
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/ugc_db
      - REDIS_URL=redis://redis:6379
      - JWT_SECRET=${JWT_SECRET:-dev-secret-key}
      - RUST_LOG=debug
      - JAEGER_ENDPOINT=http://jaeger:14268/api/traces
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    networks:
      - ugc-network
      - data-network
      - monitoring-network
    volumes:
      - ugc-logs:/app/logs
      - ./crates/ugc-subgraph/config:/app/config:ro
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # PostgreSQL Database
  postgres:
    image: postgres:15-alpine
    container_name: ugc-postgres
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_DB=ugc_db
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=password
      - POSTGRES_INITDB_ARGS=--encoding=UTF-8 --lc-collate=C --lc-ctype=C
    volumes:
      - postgres-data:/var/lib/postgresql/data
      - ./scripts/init-db.sql:/docker-entrypoint-initdb.d/init-db.sql:ro
    networks:
      - data-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres -d ugc_db"]
      interval: 10s
      timeout: 5s
      retries: 5
    command: >
      postgres
      -c shared_preload_libraries=pg_stat_statements
      -c pg_stat_statements.track=all
      -c max_connections=200
      -c shared_buffers=256MB
      -c effective_cache_size=1GB
      -c maintenance_work_mem=64MB
      -c checkpoint_completion_target=0.9
      -c wal_buffers=16MB
      -c default_statistics_target=100

  # Redis Cache
  redis:
    image: redis:7-alpine
    container_name: ugc-redis
    ports:
      - "6379:6379"
    command: >
      redis-server
      --appendonly yes
      --appendfsync everysec
      --maxmemory 512mb
      --maxmemory-policy allkeys-lru
      --save 900 1
      --save 300 10
      --save 60 10000
    volumes:
      - redis-data:/data
      - ./config/redis.conf:/usr/local/etc/redis/redis.conf:ro
    networks:
      - data-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

  # Prometheus Monitoring
  prometheus:
    image: prom/prometheus:latest
    container_name: ugc-prometheus
    ports:
      - "9090:9090"
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      - '--storage.tsdb.retention.time=200h'
      - '--web.enable-lifecycle'
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - ./config/alert_rules.yml:/etc/prometheus/alert_rules.yml:ro
      - prometheus-data:/prometheus
    networks:
      - monitoring-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:9090/-/healthy"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Jaeger Tracing
  jaeger:
    image: jaegertracing/all-in-one:latest
    container_name: ugc-jaeger
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # HTTP collector
    environment:
      - COLLECTOR_OTLP_ENABLED=true
      - SPAN_STORAGE_TYPE=memory
    volumes:
      - jaeger-data:/tmp
    networks:
      - monitoring-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:14269/"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Apollo Router (Federation Gateway)
  apollo-router:
    image: ghcr.io/apollographql/router:latest
    container_name: apollo-router
    ports:
      - "4000:4000"
    environment:
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
    volumes:
      - ./config/supergraph.graphql:/app/supergraph.graphql:ro
      - ./config/router.yaml:/app/router.yaml:ro
    depends_on:
      - ugc-subgraph
    networks:
      - ugc-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

# Networks
networks:
  ugc-network:
    driver: bridge
    name: ugc-network
  data-network:
    driver: bridge
    name: data-network
    internal: true  # Изолированная сеть для данных
  monitoring-network:
    driver: bridge
    name: monitoring-network

# Volumes
volumes:
  postgres-data:
    driver: local
    name: ugc-postgres-data
  redis-data:
    driver: local
    name: ugc-redis-data
  prometheus-data:
    driver: local
    name: ugc-prometheus-data
  jaeger-data:
    driver: local
    name: ugc-jaeger-data
  ugc-logs:
    driver: local
    name: ugc-logs
```

#### UGC Container
```plantuml
Deployment_Node(ugc_container, "UGC Subgraph Container", "Debian Bookworm Slim")
```

**Архитектурная роль**: Основной контейнер с UGC сервисом

**Детальная конфигурация контейнера**:
```dockerfile
# crates/ugc-subgraph/Dockerfile
FROM rust:1.75-slim as builder

# Установка зависимостей для сборки
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Копирование манифестов для кеширования зависимостей
COPY Cargo.toml Cargo.lock ./
COPY crates/ugc-subgraph/Cargo.toml ./crates/ugc-subgraph/
COPY crates/shared/Cargo.toml ./crates/shared/

# Создание пустых файлов для сборки зависимостей
RUN mkdir -p crates/ugc-subgraph/src crates/shared/src && \
    echo "fn main() {}" > crates/ugc-subgraph/src/main.rs && \
    echo "" > crates/shared/src/lib.rs

# Сборка зависимостей (кешируется)
RUN cargo build --release --bin ugc-subgraph
RUN rm -rf crates/ugc-subgraph/src crates/shared/src

# Копирование исходного кода
COPY crates/ ./crates/

# Пересборка с реальным кодом
RUN touch crates/ugc-subgraph/src/main.rs && \
    cargo build --release --bin ugc-subgraph

# Runtime образ
FROM debian:bookworm-slim

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Создание пользователя
RUN groupadd -r ugc && useradd -r -g ugc -s /bin/false ugc

# Создание директорий
RUN mkdir -p /app/logs /app/config /app/migrations && \
    chown -R ugc:ugc /app

WORKDIR /app

# Копирование артефактов
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/ugc-subgraph
COPY --from=builder /app/crates/ugc-subgraph/migrations ./migrations

# Установка прав
RUN chmod +x /usr/local/bin/ugc-subgraph

USER ugc

# Настройка окружения
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV APP_ENV=development

EXPOSE 4001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

CMD ["ugc-subgraph"]
```

#### PostgreSQL Container
```plantuml
Deployment_Node(postgres_container, "PostgreSQL Container", "PostgreSQL 15 Alpine")
```

**Архитектурная роль**: Надежное хранение данных

**Оптимизированная конфигурация PostgreSQL**:
```sql
-- scripts/init-db.sql
-- Инициализация базы данных для UGC

-- Создание расширений
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Создание пользователя для приложения
CREATE USER ugc_app WITH PASSWORD 'ugc_app_password';

-- Создание базы данных
CREATE DATABASE ugc_db OWNER ugc_app;

-- Подключение к базе данных UGC
\c ugc_db;

-- Предоставление прав
GRANT ALL PRIVILEGES ON DATABASE ugc_db TO ugc_app;
GRANT ALL ON SCHEMA public TO ugc_app;

-- Настройка поиска схем
ALTER USER ugc_app SET search_path = public;

-- Создание индексов для производительности
-- (будут созданы через миграции SQLx)

-- Настройка логирования медленных запросов
ALTER SYSTEM SET log_min_duration_statement = 1000;
ALTER SYSTEM SET log_statement = 'mod';
ALTER SYSTEM SET log_line_prefix = '%t [%p]: [%l-1] user=%u,db=%d,app=%a,client=%h ';

-- Применение настроек
SELECT pg_reload_conf();
```

**Конфигурация производительности**:
```conf
# config/postgresql.conf
# Настройки памяти
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
work_mem = 4MB

# Настройки WAL
wal_buffers = 16MB
checkpoint_completion_target = 0.9
checkpoint_timeout = 10min
max_wal_size = 1GB
min_wal_size = 80MB

# Настройки подключений
max_connections = 200
superuser_reserved_connections = 3

# Настройки планировщика
random_page_cost = 1.1
effective_io_concurrency = 200

# Настройки логирования
log_destination = 'stderr'
logging_collector = on
log_directory = 'pg_log'
log_filename = 'postgresql-%Y-%m-%d_%H%M%S.log'
log_rotation_age = 1d
log_rotation_size = 100MB
log_min_duration_statement = 1000
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on

# Настройки статистики
track_activities = on
track_counts = on
track_io_timing = on
track_functions = pl
```

#### Redis Container
```plantuml
Deployment_Node(redis_container, "Redis Container", "Redis 7 Alpine")
```

**Архитектурная роль**: Высокопроизводительное кеширование

**Конфигурация Redis**:
```conf
# config/redis.conf
# Основные настройки
port 6379
bind 0.0.0.0
protected-mode no
timeout 0
tcp-keepalive 300

# Настройки памяти
maxmemory 512mb
maxmemory-policy allkeys-lru
maxmemory-samples 5

# Настройки персистентности
save 900 1
save 300 10
save 60 10000

# RDB настройки
stop-writes-on-bgsave-error yes
rdbcompression yes
rdbchecksum yes
dbfilename dump.rdb

# AOF настройки
appendonly yes
appendfilename "appendonly.aof"
appendfsync everysec
no-appendfsync-on-rewrite no
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb

# Настройки клиентов
maxclients 10000

# Логирование
loglevel notice
syslog-enabled no

# Медленные запросы
slowlog-log-slower-than 10000
slowlog-max-len 128

# Настройки хешей
hash-max-ziplist-entries 512
hash-max-ziplist-value 64

# Настройки списков
list-max-ziplist-size -2
list-compress-depth 0

# Настройки множеств
set-max-intset-entries 512

# Настройки отсортированных множеств
zset-max-ziplist-entries 128
zset-max-ziplist-value 64

# Настройки HyperLogLog
hll-sparse-max-bytes 3000

# Настройки потоков
stream-node-max-bytes 4096
stream-node-max-entries 100
```

### 2. Host File System

#### UGC Workspace
```plantuml
Deployment_Node(ugc_workspace, "UGC Workspace", "Cargo Crate")
```

**Архитектурная роль**: Исходный код и конфигурация проекта

**Структура workspace**:
```
ugc-federation-workspace/
├── Cargo.toml                 # Workspace manifest
├── Cargo.lock                 # Dependency lock file
├── docker-compose.yml         # Development environment
├── docker-compose.prod.yml    # Production environment
├── .env.example               # Environment variables template
├── .gitignore                 # Git ignore rules
├── README.md                  # Project documentation
├── 
├── crates/
│   ├── ugc-subgraph/          # UGC GraphQL subgraph
│   │   ├── Cargo.toml
│   │   ├── Dockerfile
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── lib.rs
│   │   │   ├── models/
│   │   │   ├── resolvers/
│   │   │   ├── services/
│   │   │   ├── repository/
│   │   │   └── database.rs
│   │   ├── migrations/        # SQLx migrations
│   │   │   ├── 001_create_reviews.sql
│   │   │   ├── 002_create_ratings.sql
│   │   │   └── 003_add_indexes.sql
│   │   └── tests/
│   │       ├── unit/
│   │       └── integration/
│   │
│   └── shared/                # Shared utilities
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── auth/
│           ├── cache/
│           ├── errors/
│           ├── metrics/
│           └── tracing/
│
├── config/                    # Configuration files
│   ├── prometheus.yml         # Prometheus configuration
│   ├── alert_rules.yml        # Alerting rules
│   ├── router.yaml           # Apollo Router config
│   ├── supergraph.graphql    # Federation schema
│   └── redis.conf            # Redis configuration
│
├── scripts/                   # Utility scripts
│   ├── dev-setup.sh          # Development setup
│   ├── test-runner.sh        # Test execution
│   ├── migration-runner.sh   # Database migrations
│   ├── health-check.sh       # Health check script
│   └── init-db.sql          # Database initialization
│
├── docs/                     # Documentation
│   ├── api/                  # API documentation
│   ├── architecture/         # Architecture diagrams
│   └── deployment/           # Deployment guides
│
└── k8s/                      # Kubernetes manifests
    ├── namespace.yaml
    ├── configmap.yaml
    ├── secret.yaml
    ├── deployment.yaml
    ├── service.yaml
    └── ingress.yaml
```

#### Development Scripts
```plantuml
Component(dev_scripts, "Development Scripts", "Automation", "dev-setup.sh...")
```

**Скрипты для разработки**:
```bash
#!/bin/bash
# scripts/dev-setup.sh
# Скрипт настройки среды разработки

set -euo pipefail

echo "🚀 Setting up UGC development environment..."

# Проверка зависимостей
check_dependencies() {
    echo "📋 Checking dependencies..."
    
    if ! command -v docker &> /dev/null; then
        echo "❌ Docker is not installed"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        echo "❌ Docker Compose is not installed"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        echo "❌ Rust/Cargo is not installed"
        exit 1
    fi
    
    echo "✅ All dependencies are installed"
}

# Создание .env файла
setup_env() {
    echo "🔧 Setting up environment variables..."
    
    if [ ! -f .env ]; then
        cp .env.example .env
        echo "📝 Created .env file from template"
        echo "⚠️  Please review and update .env file with your settings"
    else
        echo "✅ .env file already exists"
    fi
}

# Сборка Docker образов
build_images() {
    echo "🏗️  Building Docker images..."
    docker-compose build --parallel
    echo "✅ Docker images built successfully"
}

# Запуск инфраструктуры
start_infrastructure() {
    echo "🚀 Starting infrastructure services..."
    docker-compose up -d postgres redis prometheus jaeger
    
    echo "⏳ Waiting for services to be ready..."
    sleep 10
    
    # Проверка готовности PostgreSQL
    until docker-compose exec -T postgres pg_isready -U postgres; do
        echo "⏳ Waiting for PostgreSQL..."
        sleep 2
    done
    
    # Проверка готовности Redis
    until docker-compose exec -T redis redis-cli ping; do
        echo "⏳ Waiting for Redis..."
        sleep 2
    done
    
    echo "✅ Infrastructure services are ready"
}

# Запуск миграций
run_migrations() {
    echo "🗄️  Running database migrations..."
    cargo run --bin ugc-subgraph -- migrate
    echo "✅ Database migrations completed"
}

# Запуск тестов
run_tests() {
    echo "🧪 Running tests..."
    cargo test --workspace
    echo "✅ All tests passed"
}

# Основная функция
main() {
    check_dependencies
    setup_env
    build_images
    start_infrastructure
    run_migrations
    run_tests
    
    echo ""
    echo "🎉 Development environment is ready!"
    echo ""
    echo "📋 Available services:"
    echo "   • UGC Subgraph:    http://localhost:4001/graphql"
    echo "   • Apollo Router:   http://localhost:4000/graphql"
    echo "   • Prometheus:      http://localhost:9090"
    echo "   • Jaeger:          http://localhost:16686"
    echo "   • PostgreSQL:      localhost:5432"
    echo "   • Redis:           localhost:6379"
    echo ""
    echo "🚀 To start the UGC subgraph:"
    echo "   cargo run --bin ugc-subgraph"
    echo ""
    echo "🧪 To run tests:"
    echo "   ./scripts/test-runner.sh"
    echo ""
    echo "🛑 To stop all services:"
    echo "   docker-compose down"
}

main "$@"
```

```bash
#!/bin/bash
# scripts/test-runner.sh
# Скрипт запуска тестов

set -euo pipefail

echo "🧪 Running UGC test suite..."

# Функция очистки
cleanup() {
    echo "🧹 Cleaning up test environment..."
    docker-compose -f docker-compose.test.yml down -v
}

# Установка trap для очистки
trap cleanup EXIT

# Запуск тестовой инфраструктуры
setup_test_env() {
    echo "🏗️  Setting up test environment..."
    docker-compose -f docker-compose.test.yml up -d postgres-test redis-test
    
    # Ожидание готовности сервисов
    echo "⏳ Waiting for test services..."
    sleep 5
    
    until docker-compose -f docker-compose.test.yml exec -T postgres-test pg_isready -U postgres; do
        echo "⏳ Waiting for test PostgreSQL..."
        sleep 2
    done
}

# Запуск unit тестов
run_unit_tests() {
    echo "🔬 Running unit tests..."
    cargo test --lib --bins --workspace
}

# Запуск интеграционных тестов
run_integration_tests() {
    echo "🔗 Running integration tests..."
    export DATABASE_URL="postgres://postgres:password@localhost:5433/test_db"
    export REDIS_URL="redis://localhost:6380"
    
    cargo test --test '*' --workspace
}

# Запуск тестов производительности
run_performance_tests() {
    echo "⚡ Running performance tests..."
    cargo test --release --test performance --workspace -- --ignored
}

# Генерация отчета о покрытии
generate_coverage() {
    echo "📊 Generating coverage report..."
    
    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --out Html --output-dir target/coverage
        echo "📈 Coverage report generated: target/coverage/tarpaulin-report.html"
    else
        echo "⚠️  cargo-tarpaulin not installed, skipping coverage report"
    fi
}

# Основная функция
main() {
    setup_test_env
    run_unit_tests
    run_integration_tests
    
    if [ "${PERFORMANCE_TESTS:-false}" = "true" ]; then
        run_performance_tests
    fi
    
    if [ "${GENERATE_COVERAGE:-false}" = "true" ]; then
        generate_coverage
    fi
    
    echo "✅ All tests completed successfully!"
}

main "$@"
```

### 3. External Services

#### Apollo Router
```plantuml
Container(apollo_router, "Apollo Router", "GraphQL Gateway", "Федеративный роутер...")
```

**Конфигурация федеративного роутера**:
```yaml
# config/router.yaml
supergraph:
  path: /app/supergraph.graphql

# Настройки подграфов
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    
# Настройки производительности
traffic_shaping:
  router:
    timeout: 30s
    global_rate_limit:
      capacity: 1000
      interval: 60s
  
  subgraphs:
    ugc:
      timeout: 10s
      rate_limit:
        capacity: 500
        interval: 60s

# Кеширование
caching:
  redis:
    urls: ["redis://redis:6379"]
    timeout: 2s
    ttl: 300s

# Трассировка
telemetry:
  tracing:
    jaeger:
      endpoint: http://jaeger:14268/api/traces
      batch_size: 512
  
  metrics:
    prometheus:
      enabled: true
      path: /metrics

# CORS
cors:
  allow_origins:
    - "http://localhost:3000"
    - "https://auto.ru"
  allow_headers:
    - "content-type"
    - "authorization"

# Безопасность
security:
  query_depth_limit: 15
  query_complexity_limit: 1000
  introspection: true  # Только для разработки
```

## Сетевая архитектура

### Docker Networks
```yaml
# Изолированные сети для безопасности
networks:
  # Публичная сеть для внешних подключений
  ugc-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
  
  # Внутренняя сеть для данных
  data-network:
    driver: bridge
    internal: true  # Нет доступа в интернет
    ipam:
      config:
        - subnet: 172.21.0.0/16
  
  # Сеть мониторинга
  monitoring-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.22.0.0/16
```

### Port Mapping
```yaml
# Маппинг портов для разработки
ports:
  # Основные сервисы
  - "4000:4000"   # Apollo Router
  - "4001:4001"   # UGC Subgraph
  
  # Базы данных
  - "5432:5432"   # PostgreSQL
  - "6379:6379"   # Redis
  
  # Мониторинг
  - "9090:9090"   # Prometheus
  - "16686:16686" # Jaeger UI
```

## Volumes и Storage

### Persistent Volumes
```yaml
volumes:
  # Данные PostgreSQL
  postgres-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./data/postgres
  
  # Данные Redis
  redis-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./data/redis
  
  # Логи приложения
  ugc-logs:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./logs
  
  # Метрики Prometheus
  prometheus-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./data/prometheus
```

## Health Checks и Monitoring

### Health Check Configuration
```yaml
# Конфигурация health checks для всех сервисов
healthcheck:
  ugc-subgraph:
    test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 40s
  
  postgres:
    test: ["CMD-SHELL", "pg_isready -U postgres -d ugc_db"]
    interval: 10s
    timeout: 5s
    retries: 5
  
  redis:
    test: ["CMD", "redis-cli", "ping"]
    interval: 10s
    timeout: 3s
    retries: 3
  
  prometheus:
    test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:9090/-/healthy"]
    interval: 30s
    timeout: 10s
    retries: 3
```

## Безопасность

### Security Configuration
```yaml
# Настройки безопасности контейнеров
security_opt:
  - no-new-privileges:true

# Ограничения ресурсов
deploy:
  resources:
    limits:
      cpus: '0.5'
      memory: 512M
    reservations:
      cpus: '0.25'
      memory: 256M

# Пользователи без root прав
user: "1001:1001"

# Read-only файловая система
read_only: true

# Временные директории
tmpfs:
  - /tmp
  - /var/tmp
```

## Выводы

Диаграмма развертывания UGC системы демонстрирует:

1. **Контейнеризованную архитектуру** с изоляцией сервисов
2. **Сетевую безопасность** через изолированные Docker сети
3. **Персистентность данных** через Docker volumes
4. **Мониторинг и наблюдаемость** с Prometheus и Jaeger
5. **Автоматизацию развертывания** через Docker Compose
6. **Готовность к продакшену** с health checks и ограничениями ресурсов

Архитектура обеспечивает простое развертывание в среде разработки с возможностью масштабирования для продакшена через Kubernetes или другие оркестраторы.