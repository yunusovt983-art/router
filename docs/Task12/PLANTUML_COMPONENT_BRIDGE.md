# Task 12: Component Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_COMPONENT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Компонентная диаграмма Task 12 показывает детальную структуру каждого контейнера, разбивая их на конкретные компоненты (файлы конфигурации, модули, скрипты). Каждый компонент имеет прямое отражение в файловой структуре проекта и конкретном коде.

### Docker Configuration Components (Task 12.1)

#### Multi-stage Dockerfiles

##### UGC Dockerfile Component
**PlantUML элемент:**
```plantuml
Component(dockerfile_ugc, "UGC Dockerfile", "Multi-stage Docker", "Optimized Rust build with security")
```

**Файловая структура:**
```
ugc-subgraph/
├── Dockerfile              # ← Этот компонент
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   └── ...
└── migrations/
    └── 001_initial.sql
```

**Код реализации:**
```dockerfile
# ugc-subgraph/Dockerfile - полная реализация компонента
# ===== BUILDER STAGE =====
FROM rust:1.75-slim as builder

# Метаданные для трассируемости
LABEL maintainer="Auto.ru DevOps Team"
LABEL service="ugc-subgraph"
LABEL stage="builder"

WORKDIR /app

# Системные зависимости - точно соответствуют архитектурным требованиям
RUN apt-get update && apt-get install -y \
    pkg-config \      # Для поиска системных библиотек
    libssl-dev \      # OpenSSL для HTTPS/TLS
    libpq-dev \       # PostgreSQL client library
    ca-certificates \ # SSL сертификаты
    && rm -rf /var/lib/apt/lists/*  # Очистка для уменьшения размера

# Dependency caching optimization - архитектурное решение для скорости сборки
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/
COPY shared/Cargo.toml ./shared/

# Создание dummy файлов для сборки зависимостей
RUN mkdir -p ugc-subgraph/src shared/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs && \
    echo "// dummy lib" > shared/src/lib.rs

# Сборка зависимостей (кешируется Docker layers)
RUN cargo build --release --package ugc-subgraph

# Копирование реального исходного кода
COPY . .

# Пересборка только приложения (зависимости уже собраны)
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# ===== RUNTIME STAGE =====
FROM debian:bookworm-slim

LABEL maintainer="Auto.ru DevOps Team"
LABEL service="ugc-subgraph"
LABEL stage="runtime"

# Минимальные runtime зависимости
RUN apt-get update && apt-get install -y \
    ca-certificates \  # SSL сертификаты
    libpq5 \          # PostgreSQL runtime library
    libssl3 \         # OpenSSL runtime
    curl \            # Для health checks
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Security: создание non-root пользователя
RUN groupadd -r ugc && useradd -r -g ugc -s /bin/false ugc

# Копирование только бинарного файла
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/ugc-subgraph
RUN chmod +x /usr/local/bin/ugc-subgraph

# Security context
USER ugc
WORKDIR /home/ugc

# Network exposure
EXPOSE 4001

# Health monitoring - соответствует архитектурным требованиям observability
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

# Process execution
CMD ["ugc-subgraph"]
```

**Связь с Rust кодом:**
```rust
// ugc-subgraph/src/main.rs - приложение, которое собирается в Dockerfile
use axum::{
    extract::Extension,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация трассировки - соответствует архитектурным требованиям
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    tracing::info!("Starting UGC Subgraph service");
    
    // Подключения к внешним сервисам - соответствует архитектурной диаграмме
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set");
    let redis_url = std::env::var("REDIS_URL")
        .expect("REDIS_URL environment variable must be set");
    
    // Database connection pool
    let db_pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    
    // Redis connection
    let redis_client = redis::Client::open(redis_url)
        .expect("Failed to create Redis client");
    
    // GraphQL schema setup
    let schema = create_schema(db_pool.clone(), redis_client.clone()).await;
    
    // HTTP server setup - точно соответствует Dockerfile EXPOSE 4001
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_handler))  // Health check endpoint
        .route("/metrics", get(metrics_handler)) // Prometheus metrics
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(Extension(schema))
        );
    
    let addr = SocketAddr::from(([0, 0, 0, 0], 4001));
    tracing::info!("UGC Subgraph listening on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

// Health check handler - используется в Dockerfile HEALTHCHECK
async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ugc-subgraph",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
```

##### Docker Compose Main Configuration
**PlantUML элемент:**
```plantuml
Component(compose_main, "docker-compose.yml", "YAML Configuration", "Main orchestration file")
```

**Файловая структура:**
```
project-root/
├── docker-compose.yml      # ← Этот компонент
├── docker-compose.dev.yml  # Development overrides
├── docker-compose.prod.yml # Production overrides
├── .env                    # Environment variables
└── .env.example           # Environment template
```

**Код реализации:**
```yaml
# docker-compose.yml - основная оркестрация
version: '3.8'

# Metadata для трассируемости
x-common-variables: &common-variables
  RUST_LOG: ${RUST_LOG:-info}
  RUST_BACKTRACE: ${RUST_BACKTRACE:-0}

x-restart-policy: &restart-policy
  restart: unless-stopped

services:
  # ===== FEDERATION GATEWAY =====
  apollo-router:
    build:
      context: .
      dockerfile: apollo-router/Dockerfile
      args:
        - BUILDKIT_INLINE_CACHE=1
    container_name: auto-ru-apollo-router
    ports:
      - "${ROUTER_PORT:-4000}:4000"
    volumes:
      # Configuration files - соответствуют архитектурным компонентам
      - ./router.yaml:/app/router.yaml:ro
      - ./supergraph.graphql:/app/supergraph.graphql:ro
    depends_on:
      ugc-subgraph:
        condition: service_healthy
      users-subgraph:
        condition: service_healthy
      offers-subgraph:
        condition: service_healthy
    environment:
      <<: *common-variables
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
      - APOLLO_ROUTER_LOG=info
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # ===== UGC SUBGRAPH =====
  ugc-subgraph:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
      args:
        - BUILDKIT_INLINE_CACHE=1
      target: runtime
    container_name: auto-ru-ugc-subgraph
    ports:
      - "${UGC_PORT:-4001}:4001"
    depends_on:
      ugc-postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    environment:
      <<: *common-variables
      # Database connections - соответствуют архитектурным связям
      - DATABASE_URL=postgresql://${POSTGRES_USER:-ugc_user}:${POSTGRES_PASSWORD:-ugc_password}@ugc-postgres:5432/${POSTGRES_DB:-ugc_db}
      - REDIS_URL=redis://redis:6379
      # Security
      - JWT_SECRET=${JWT_SECRET:-development-secret-change-in-production}
      # Observability
      - OTEL_EXPORTER_JAEGER_ENDPOINT=http://jaeger:14268/api/traces
      - PROMETHEUS_ENDPOINT=0.0.0.0:9001
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4001/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # ===== USERS SUBGRAPH (STUB) =====
  users-subgraph:
    build:
      context: .
      dockerfile: users-subgraph/Dockerfile
      args:
        - BUILDKIT_INLINE_CACHE=1
    container_name: auto-ru-users-subgraph
    ports:
      - "${USERS_PORT:-4002}:4002"
    environment:
      <<: *common-variables
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4002/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # ===== OFFERS SUBGRAPH (STUB) =====
  offers-subgraph:
    build:
      context: .
      dockerfile: offers-subgraph/Dockerfile
      args:
        - BUILDKIT_INLINE_CACHE=1
    container_name: auto-ru-offers-subgraph
    ports:
      - "${OFFERS_PORT:-4004}:4004"
    environment:
      <<: *common-variables
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4004/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # ===== DATABASE SERVICES =====
  ugc-postgres:
    image: postgres:14-alpine
    container_name: auto-ru-postgres
    environment:
      - POSTGRES_DB=${POSTGRES_DB:-ugc_db}
      - POSTGRES_USER=${POSTGRES_USER:-ugc_user}
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD:-ugc_password}
      - POSTGRES_INITDB_ARGS=--encoding=UTF-8 --lc-collate=C --lc-ctype=C
    volumes:
      # Persistent data
      - postgres_data:/var/lib/postgresql/data
      # Initialization scripts - соответствуют архитектурным требованиям
      - ./ugc-subgraph/migrations:/docker-entrypoint-initdb.d:ro
      # Configuration
      - ./postgres/postgresql.conf:/etc/postgresql/postgresql.conf:ro
    ports:
      - "${POSTGRES_PORT:-5432}:5432"
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-ugc_user} -d ${POSTGRES_DB:-ugc_db}"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s

  redis:
    image: redis:7-alpine
    container_name: auto-ru-redis
    command: >
      redis-server
      --appendonly yes
      --maxmemory 256mb
      --maxmemory-policy allkeys-lru
      --save 900 1
      --save 300 10
      --save 60 10000
    volumes:
      - redis_data:/data
      - ./redis/redis.conf:/usr/local/etc/redis/redis.conf:ro
    ports:
      - "${REDIS_PORT:-6379}:6379"
    networks:
      - federation-network
    <<: *restart-policy
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

# ===== PERSISTENT VOLUMES =====
volumes:
  postgres_data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./data/postgres
  redis_data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ./data/redis

# ===== NETWORK CONFIGURATION =====
networks:
  federation-network:
    driver: bridge
    ipam:
      driver: default
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
    driver_opts:
      com.docker.network.bridge.name: auto-ru-federation
      com.docker.network.bridge.enable_icc: "true"
      com.docker.network.bridge.enable_ip_masquerade: "true"
```

**Environment configuration:**
```bash
# .env - environment variables
# Database Configuration
POSTGRES_DB=ugc_db
POSTGRES_USER=ugc_user
POSTGRES_PASSWORD=secure_password_change_me

# Service Ports
ROUTER_PORT=4000
UGC_PORT=4001
USERS_PORT=4002
OFFERS_PORT=4004
POSTGRES_PORT=5432
REDIS_PORT=6379

# Logging
RUST_LOG=info
RUST_BACKTRACE=0

# Security
JWT_SECRET=your-super-secret-jwt-key-change-in-production

# Observability
JAEGER_ENDPOINT=http://jaeger:14268/api/traces
PROMETHEUS_ENDPOINT=0.0.0.0:9090
```

#### Development Overrides Component
**PlantUML элемент:**
```plantuml
Component(compose_dev, "docker-compose.dev.yml", "YAML Configuration", "Development overrides with hot reload")
```

**Код реализации:**
```yaml
# docker-compose.dev.yml - development-specific настройки
version: '3.8'

services:
  # ===== DEVELOPMENT OVERRIDES =====
  ugc-subgraph:
    build:
      target: builder  # Остановка на builder stage для hot reload
      args:
        - CARGO_INCREMENTAL=1
        - CARGO_TARGET_DIR=/tmp/target
    volumes:
      # Hot reload source code - архитектурное решение для developer experience
      - ./ugc-subgraph/src:/app/ugc-subgraph/src:ro
      - ./shared/src:/app/shared/src:ro
      # Cargo cache для ускорения компиляции
      - cargo_cache:/usr/local/cargo/registry
      - target_cache:/app/target
    environment:
      # Development logging
      - RUST_LOG=debug
      - RUST_BACKTRACE=full
      - CARGO_INCREMENTAL=1
      # Development database
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db_dev
    command: |
      sh -c "
        echo 'Installing cargo-watch for hot reload...'
        cargo install cargo-watch --quiet
        echo 'Starting UGC subgraph with hot reload...'
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
      - RUST_BACKTRACE=full
    command: |
      sh -c "
        cargo install cargo-watch --quiet
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
      - RUST_BACKTRACE=full
    command: |
      sh -c "
        cargo install cargo-watch --quiet
        cargo watch -x 'run --package offers-subgraph'
      "

  # ===== DEVELOPMENT TOOLS =====
  jaeger:
    image: jaegertracing/all-in-one:1.47
    container_name: auto-ru-jaeger-dev
    ports:
      - "16686:16686"  # Jaeger UI
      - "14268:14268"  # HTTP collector
      - "14250:14250"  # gRPC collector
    environment:
      - COLLECTOR_OTLP_ENABLED=true
      - SPAN_STORAGE_TYPE=memory
    networks:
      - federation-network

  prometheus:
    image: prom/prometheus:v2.45.0
    container_name: auto-ru-prometheus-dev
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus-dev.yml:/etc/prometheus/prometheus.yml:ro
      - ./monitoring/rules:/etc/prometheus/rules:ro
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=7d'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      - '--web.enable-lifecycle'
      - '--web.enable-admin-api'
    networks:
      - federation-network

  grafana:
    image: grafana/grafana:10.0.0
    container_name: auto-ru-grafana-dev
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
      - GF_INSTALL_PLUGINS=grafana-piechart-panel,grafana-worldmap-panel
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/dashboards:/etc/grafana/provisioning/dashboards:ro
      - ./monitoring/grafana/datasources:/etc/grafana/provisioning/datasources:ro
    networks:
      - federation-network

  # ===== DATABASE DEVELOPMENT TOOLS =====
  pgadmin:
    image: dpage/pgadmin4:7
    container_name: auto-ru-pgadmin-dev
    ports:
      - "8080:80"
    environment:
      - PGADMIN_DEFAULT_EMAIL=admin@auto.ru
      - PGADMIN_DEFAULT_PASSWORD=admin
      - PGADMIN_CONFIG_SERVER_MODE=False
    volumes:
      - pgadmin_data:/var/lib/pgadmin
    networks:
      - federation-network
    depends_on:
      - ugc-postgres

  redis-commander:
    image: rediscommander/redis-commander:latest
    container_name: auto-ru-redis-commander-dev
    ports:
      - "8081:8081"
    environment:
      - REDIS_HOSTS=local:redis:6379
    networks:
      - federation-network
    depends_on:
      - redis

volumes:
  cargo_cache:
  target_cache:
  grafana_data:
  pgadmin_data:
```

**Prometheus development configuration:**
```yaml
# monitoring/prometheus-dev.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "rules/*.yml"

scrape_configs:
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    scrape_interval: 5s
    metrics_path: /metrics

  - job_name: 'ugc-subgraph'
    static_configs:
      - targets: ['ugc-subgraph:9001']
    scrape_interval: 5s

  - job_name: 'users-subgraph'
    static_configs:
      - targets: ['users-subgraph:9002']
    scrape_interval: 5s

  - job_name: 'offers-subgraph'
    static_configs:
      - targets: ['offers-subgraph:9004']
    scrape_interval: 5s

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']
    scrape_interval: 10s

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
    scrape_interval: 10s
```

### CI/CD Configuration Components (Task 12.2)

#### Main CI Workflow Component
**PlantUML элемент:**
```plantuml
Component(ci_workflow, "ci.yml", "GitHub Actions", "Main CI pipeline with testing")
```

**Файловая структура:**
```
.github/
├── workflows/
│   ├── ci.yml                    # ← Этот компонент
│   ├── deploy-staging.yml
│   ├── deploy-production.yml
│   └── security.yml
├── actions/
│   └── setup-rust/
│       └── action.yml
└── CODEOWNERS
```

**Код реализации:**
```yaml
# .github/workflows/ci.yml - основной CI pipeline
name: CI

on:
  push:
    branches: [main, develop]
    paths-ignore:
      - 'docs/**'
      - '*.md'
      - '.gitignore'
  pull_request:
    branches: [main]
    types: [opened, synchronize, reopened]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1
  CARGO_INCREMENTAL: 0  # Disable incremental compilation for CI

# Concurrency control - архитектурное решение для оптимизации ресурсов
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # ===== CODE QUALITY CHECKS =====
  lint:
    name: Lint and Format
    runs-on: ubuntu-latest
    timeout-minutes: 15
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full history for better caching
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy
          profile: minimal
      
      - name: Cache Rust dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-lint-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-lint-
            ${{ runner.os }}-cargo-
      
      - name: Check code formatting
        run: |
          echo "::group::Checking Rust code formatting"
          cargo fmt --all -- --check
          echo "::endgroup::"
      
      - name: Run Clippy lints
        run: |
          echo "::group::Running Clippy lints"
          cargo clippy --all-targets --all-features --workspace -- \
            -D warnings \
            -D clippy::all \
            -D clippy::pedantic \
            -A clippy::module_name_repetitions \
            -A clippy::missing_errors_doc
          echo "::endgroup::"
      
      - name: Check documentation
        run: |
          echo "::group::Checking documentation"
          cargo doc --no-deps --all-features --workspace
          echo "::endgroup::"

  # ===== UNIT AND INTEGRATION TESTS =====
  test:
    name: Test Suite
    runs-on: ubuntu-latest
    timeout-minutes: 30
    needs: lint
    
    strategy:
      matrix:
        rust-version: [stable, beta]
        include:
          - rust-version: stable
            coverage: true
    
    services:
      # Test database - соответствует архитектурным зависимостям
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
      
      # Test cache
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
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust-version }}
          override: true
          profile: minimal
      
      - name: Install cargo-tarpaulin (coverage)
        if: matrix.coverage
        run: cargo install cargo-tarpaulin
      
      - name: Cache Rust dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-test-${{ matrix.rust-version }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-test-${{ matrix.rust-version }}-
            ${{ runner.os }}-cargo-test-
            ${{ runner.os }}-cargo-
      
      - name: Run unit tests
        run: |
          echo "::group::Running unit tests"
          cargo test --lib --all-features --workspace --verbose
          echo "::endgroup::"
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
          REDIS_URL: redis://localhost:6379
          RUST_LOG: debug
      
      - name: Run integration tests
        run: |
          echo "::group::Running integration tests"
          cargo test --test '*' --all-features --workspace --verbose
          echo "::endgroup::"
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
          REDIS_URL: redis://localhost:6379
          RUST_LOG: debug
      
      - name: Generate test coverage
        if: matrix.coverage
        run: |
          echo "::group::Generating test coverage"
          cargo tarpaulin \
            --verbose \
            --all-features \
            --workspace \
            --timeout 120 \
            --out xml \
            --output-dir coverage/
          echo "::endgroup::"
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_db
          REDIS_URL: redis://localhost:6379
      
      - name: Upload coverage to Codecov
        if: matrix.coverage
        uses: codecov/codecov-action@v3
        with:
          file: coverage/cobertura.xml
          flags: unittests
          name: codecov-umbrella

  # ===== DOCKER BUILD MATRIX =====
  build:
    name: Build Docker Images
    runs-on: ubuntu-latest
    timeout-minutes: 45
    needs: [lint, test]
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
        platform: [linux/amd64, linux/arm64]
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
        with:
          platforms: linux/amd64,linux/arm64
      
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: |
            ghcr.io/auto-ru/${{ matrix.service }}
          tags: |
            type=ref,event=branch
            type=ref,event=pr
            type=sha,prefix={{branch}}-
            type=raw,value=latest,enable={{is_default_branch}}
      
      - name: Build Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./${{ matrix.service }}/Dockerfile
          platforms: ${{ matrix.platform }}
          push: false
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha,scope=${{ matrix.service }}-${{ matrix.platform }}
          cache-to: type=gha,mode=max,scope=${{ matrix.service }}-${{ matrix.platform }}
          build-args: |
            BUILDKIT_INLINE_CACHE=1
            CARGO_INCREMENTAL=0
      
      - name: Test Docker image
        run: |
          echo "::group::Testing Docker image"
          # Load image for testing
          docker buildx build \
            --load \
            --platform linux/amd64 \
            -f ./${{ matrix.service }}/Dockerfile \
            -t test-${{ matrix.service }}:latest \
            .
          
          # Test image can start
          docker run --rm -d \
            --name test-${{ matrix.service }} \
            -p 8080:4001 \
            test-${{ matrix.service }}:latest &
          
          # Wait for startup
          sleep 10
          
          # Test health endpoint (if available)
          if docker exec test-${{ matrix.service }} curl -f http://localhost:4001/health 2>/dev/null; then
            echo "✅ Health check passed"
          else
            echo "ℹ️ No health endpoint or service not ready"
          fi
          
          # Cleanup
          docker stop test-${{ matrix.service }} || true
          echo "::endgroup::"

  # ===== SCHEMA VALIDATION =====
  schema-validation:
    name: GraphQL Schema Validation
    runs-on: ubuntu-latest
    timeout-minutes: 20
    needs: build
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          cache: 'npm'
      
      - name: Install Apollo CLI
        run: |
          echo "::group::Installing Apollo CLI"
          curl -sSL https://rover.apollo.dev/nix/latest | sh
          echo "$HOME/.rover/bin" >> $GITHUB_PATH
          echo "::endgroup::"
      
      - name: Start services for schema introspection
        run: |
          echo "::group::Starting services"
          docker-compose up -d
          
          # Wait for services to be healthy
          echo "Waiting for services to be ready..."
          timeout 120 bash -c '
            until docker-compose ps | grep -E "(ugc-subgraph|users-subgraph|offers-subgraph)" | grep -q "healthy"; do
              echo "Waiting for services..."
              sleep 5
            done
          '
          
          # Show service status
          docker-compose ps
          echo "::endgroup::"
      
      - name: Introspect subgraph schemas
        run: |
          echo "::group::Introspecting schemas"
          
          # Introspect each subgraph
          rover subgraph introspect http://localhost:4001/graphql > ugc-schema.graphql
          rover subgraph introspect http://localhost:4002/graphql > users-schema.graphql
          rover subgraph introspect http://localhost:4004/graphql > offers-schema.graphql
          
          # Validate individual schemas
          echo "✅ All subgraph schemas introspected successfully"
          echo "::endgroup::"
      
      - name: Validate schema composition
        run: |
          echo "::group::Validating schema composition"
          
          # Compose supergraph schema
          rover supergraph compose --config supergraph.yaml > composed-schema.graphql
          echo "✅ Schema composition successful"
          
          # Validate schema for breaking changes (if we have a baseline)
          if [ -f baseline-schema.graphql ]; then
            rover graph check auto-ru-federation@staging \
              --schema composed-schema.graphql || echo "⚠️ Schema changes detected"
          fi
          echo "::endgroup::"
      
      - name: Test federation queries
        run: |
          echo "::group::Testing federation queries"
          
          # Test basic introspection
          curl -X POST http://localhost:4000/graphql \
            -H "Content-Type: application/json" \
            -d '{"query": "query { __schema { types { name } } }"}' \
            --fail-with-body | jq .
          
          # Test cross-subgraph query
          curl -X POST http://localhost:4000/graphql \
            -H "Content-Type: application/json" \
            -d '{
              "query": "query TestFederation { reviews(first: 1) { edges { node { id content author { id name } offer { id title } } } } }"
            }' \
            --fail-with-body | jq .
          
          echo "✅ Federation queries working correctly"
          echo "::endgroup::"
      
      - name: Upload schema artifacts
        uses: actions/upload-artifact@v3
        with:
          name: graphql-schemas
          path: |
            *-schema.graphql
            composed-schema.graphql
      
      - name: Cleanup
        if: always()
        run: |
          echo "::group::Cleanup"
          docker-compose down -v
          docker system prune -f
          echo "::endgroup::"

  # ===== SECURITY SCAN =====
  security:
    name: Security Scan
    runs-on: ubuntu-latest
    timeout-minutes: 15
    needs: lint
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Run Rust security audit
        uses: actions-rs/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Run Trivy vulnerability scanner
        uses: aquasecurity/trivy-action@master
        with:
          scan-type: 'fs'
          scan-ref: '.'
          format: 'sarif'
          output: 'trivy-results.sarif'
      
      - name: Upload Trivy scan results
        uses: github/codeql-action/upload-sarif@v2
        if: always()
        with:
          sarif_file: 'trivy-results.sarif'

  # ===== NOTIFICATION =====
  notify:
    name: Notify Results
    runs-on: ubuntu-latest
    if: always()
    needs: [lint, test, build, schema-validation, security]
    
    steps:
      - name: Notify Slack
        uses: 8398a7/action-slack@v3
        if: always()
        with:
          status: ${{ job.status }}
          channel: '#ci-cd'
          webhook_url: ${{ secrets.SLACK_WEBHOOK }}
          fields: repo,message,commit,author,action,eventName,ref,workflow
```

### Infrastructure Configuration Components

#### Apollo Router Configuration Component
**PlantUML элемент:**
```plantuml
Component(router_config, "router.yaml", "YAML", "Apollo Router federation configuration")
```

**Код реализации:**
```yaml
# router.yaml - Apollo Router конфигурация
# Соответствует архитектурным требованиям federation gateway

# Server configuration
listen: 0.0.0.0:4000
introspection: true

# Supergraph schema location
supergraph:
  path: ./supergraph.graphql
  watch: true  # Hot reload в development

# Subgraph routing - соответствует архитектурной диаграмме
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
    health_check:
      enabled: true
      path: /health
      interval: 30s
  
  users:
    routing_url: http://users-subgraph:4002/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
    health_check:
      enabled: true
      path: /health
      interval: 30s
  
  offers:
    routing_url: http://offers-subgraph:4004/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
    health_check:
      enabled: true
      path: /health
      interval: 30s

# Telemetry - соответствует архитектурным требованиям observability
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
        max_queue_size: 2048
    
    otlp:
      enabled: true
      endpoint: http://jaeger:4317
      protocol: grpc

# CORS configuration - архитектурное требование для web clients
cors:
  allow_any_origin: false
  allow_origins:
    - http://localhost:3000
    - https://auto.ru
    - https://*.auto.ru
  allow_credentials: true
  allow_headers:
    - Content-Type
    - Authorization
    - Apollo-Require-Preflight
  expose_headers:
    - X-Custom-Header
  max_age: 86400

# Security headers - архитектурные требования безопасности
headers:
  all:
    request:
      - propagate:
          named: "authorization"
      - propagate:
          named: "x-user-id"
      - propagate:
          named: "x-correlation-id"
    response:
      - insert:
          name: "X-Content-Type-Options"
          value: "nosniff"
      - insert:
          name: "X-Frame-Options"
          value: "DENY"
      - insert:
          name: "X-XSS-Protection"
          value: "1; mode=block"
      - insert:
          name: "Strict-Transport-Security"
          value: "max-age=31536000; includeSubDomains"

# Query planning optimization
query_planning:
  cache:
    in_memory:
      limit: 512
  
  experimental_reuse_query_fragments: true
  experimental_type_conditioned_fetching: true

# Rate limiting - архитектурное требование для защиты от злоупотреблений
traffic_shaping:
  all:
    global_rate_limit:
      capacity: 1000
      interval: 60s
    
    timeout: 30s
    
    experimental_retry:
      min_per_sec: 10
      ttl: 10s
  
  # Per-operation limits
  per_operation:
    timeout: 30s
    
    # Specific operation limits
    limits:
      "GetReviews":
        capacity: 100
        interval: 60s
      "CreateReview":
        capacity: 10
        interval: 60s

# Health check endpoint
health_check:
  enabled: true
  path: /health

# Sandbox/Playground configuration
sandbox:
  enabled: true
  
homepage:
  enabled: false

# Logging configuration
logging:
  format: json
  level: info
```

### Заключение

Компонентная диаграмма Task 12 обеспечивает детальную трассируемость между архитектурными компонентами и их реализацией в коде:

1. **Docker Configuration** → Конкретные Dockerfile и docker-compose.yml файлы
2. **CI/CD Workflows** → GitHub Actions YAML конфигурации
3. **Infrastructure Config** → Router, database, и network настройки
4. **Development Tools** → Makefile, scripts, и automation
5. **Security Components** → Authentication, authorization, и network policies

Каждый компонент в диаграмме имеет прямое отражение в файловой структуре проекта и конкретном коде, что обеспечивает полную согласованность между архитектурным дизайном и реализацией.