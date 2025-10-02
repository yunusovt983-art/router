# Deployment Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_DEPLOYMENT.puml

### Назначение диаграммы
Диаграмма развертывания показывает физическое размещение заглушек подграфов Task 11 
в различных окружениях и их инфраструктурную интеграцию.

### Local Development Environment

#### Docker Compose Stack
**Платформа:** Docker Compose + Networks
**Назначение:** Локальная разработка с полной федеративной средой

**Конфигурация Docker Compose:**
```yaml
# docker-compose.yml
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
    depends_on:
      - users-subgraph
      - offers-subgraph
      - ugc-subgraph
    environment:
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
      - APOLLO_ROUTER_SUPERGRAPH_PATH=/app/supergraph.graphql
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
      - SERVICE_NAME=users-subgraph
      - SERVICE_PORT=4002
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4002/health"]
      interval: 30s
      timeout: 10s
      retries: 3
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
      - SERVICE_NAME=offers-subgraph
      - SERVICE_PORT=4004
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4004/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    networks:
      - federation-network

  # UGC Subgraph (Full Implementation)
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

  # Development Tools
  graphql-playground:
    image: graphql/playground:latest
    ports:
      - "3000:3000"
    environment:
      - GRAPHQL_ENDPOINT=http://apollo-router:4000/graphql
    depends_on:
      - apollo-router
    networks:
      - federation-network

networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

#### Development Tools Integration
**Компоненты:** GraphQL Playground, Docker Compose CLI
**Назначение:** Удобство разработки и отладки

**Скрипты для разработки:**
```bash
#!/bin/bash
# scripts/dev-setup.sh

echo "🚀 Starting Auto.ru Federation Development Environment"

# Проверка Docker
if ! command -v docker-compose &> /dev/null; then
    echo "❌ Docker Compose not found. Please install Docker."
    exit 1
fi

# Сборка и запуск сервисов
echo "📦 Building services..."
docker-compose build

echo "🔄 Starting services..."
docker-compose up -d

# Ожидание готовности сервисов
echo "⏳ Waiting for services to be ready..."
sleep 10

# Проверка health checks
services=("users-subgraph" "offers-subgraph" "ugc-subgraph")
for service in "${services[@]}"; do
    echo "🔍 Checking $service health..."
    if docker-compose exec $service curl -f http://localhost:$(docker-compose port $service | cut -d: -f2)/health; then
        echo "✅ $service is healthy"
    else
        echo "❌ $service is not responding"
    fi
done

# Композиция supergraph схемы
echo "🔗 Composing supergraph schema..."
docker-compose exec apollo-router rover supergraph compose --config /app/supergraph.yaml > supergraph.graphql

echo "🎉 Development environment is ready!"
echo "📊 GraphQL Playground: http://localhost:3000"
echo "🌐 Apollo Router: http://localhost:4000/graphql"
echo "👥 Users API: http://localhost:4002/graphql"
echo "🚗 Offers API: http://localhost:4004/graphql"
```

### CI/CD Environment

#### GitHub Actions Runner
**Платформа:** Ubuntu + Docker
**Назначение:** Автоматическое тестирование и сборка заглушек

**CI/CD Pipeline конфигурация:**
```yaml
# .github/workflows/subgraph-stubs-ci.yml
name: Subgraph Stubs CI

on:
  push:
    branches: [main, develop]
    paths:
      - 'users-subgraph/**'
      - 'offers-subgraph/**'
      - 'shared/**'
  pull_request:
    branches: [main]
    paths:
      - 'users-subgraph/**'
      - 'offers-subgraph/**'

jobs:
  test-users-subgraph:
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy

      - name: Cache Rust dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-users-${{ hashFiles('users-subgraph/Cargo.lock') }}

      - name: Run Users subgraph tests
        working-directory: users-subgraph
        run: |
          cargo fmt --check
          cargo clippy -- -D warnings
          cargo test --verbose

      - name: Build Users subgraph
        working-directory: users-subgraph
        run: cargo build --release

  test-offers-subgraph:
    runs-on: ubuntu-latest
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Run Offers subgraph tests
        working-directory: offers-subgraph
        run: |
          cargo fmt --check
          cargo clippy -- -D warnings
          cargo test --verbose

      - name: Build Offers subgraph
        working-directory: offers-subgraph
        run: cargo build --release

  federation-integration-tests:
    runs-on: ubuntu-latest
    needs: [test-users-subgraph, test-offers-subgraph]
    
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

    steps:
      - name: Checkout code
        uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Install Apollo CLI
        run: |
          curl -sSL https://rover.apollo.dev/nix/latest | sh
          echo "$HOME/.rover/bin" >> $GITHUB_PATH

      - name: Start subgraph stubs
        run: |
          # Запуск Users subgraph в фоне
          cd users-subgraph
          cargo run &
          USERS_PID=$!
          
          # Запуск Offers subgraph в фоне
          cd ../offers-subgraph
          cargo run &
          OFFERS_PID=$!
          
          # Ожидание готовности сервисов
          sleep 10
          
          # Сохранение PID для cleanup
          echo $USERS_PID > /tmp/users.pid
          echo $OFFERS_PID > /tmp/offers.pid

      - name: Validate federation composition
        run: |
          # Создание supergraph конфигурации
          cat > supergraph.yaml << EOF
          federation_version: 2
          subgraphs:
            users:
              routing_url: http://localhost:4002/graphql
              schema:
                subgraph_url: http://localhost:4002/graphql
            offers:
              routing_url: http://localhost:4004/graphql
              schema:
                subgraph_url: http://localhost:4004/graphql
          EOF
          
          # Композиция и валидация схемы
          rover supergraph compose --config supergraph.yaml

      - name: Run federation tests
        run: |
          cd tests/federation
          cargo test --verbose

      - name: Cleanup
        if: always()
        run: |
          if [ -f /tmp/users.pid ]; then
            kill $(cat /tmp/users.pid) || true
          fi
          if [ -f /tmp/offers.pid ]; then
            kill $(cat /tmp/offers.pid) || true
          fi

  docker-build:
    runs-on: ubuntu-latest
    needs: [federation-integration-tests]
    
    strategy:
      matrix:
        subgraph: [users-subgraph, offers-subgraph]
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2

      - name: Build Docker image
        uses: docker/build-push-action@v3
        with:
          context: .
          file: ./${{ matrix.subgraph }}/Dockerfile
          push: false
          tags: ${{ matrix.subgraph }}:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Test Docker image
        run: |
          docker run --rm -d --name test-${{ matrix.subgraph }} \
            -p 8080:$([ "${{ matrix.subgraph }}" = "users-subgraph" ] && echo "4002" || echo "4004") \
            ${{ matrix.subgraph }}:${{ github.sha }}
          
          sleep 5
          
          # Проверка health endpoint
          curl -f http://localhost:8080/health
          
          docker stop test-${{ matrix.subgraph }}
```

#### Test Infrastructure
**Компоненты:** Test PostgreSQL, Redis, WireMock
**Назначение:** Изолированная среда для тестирования

**Testcontainers интеграция:**
```rust
// tests/support/test_infrastructure.rs
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};

pub struct TestInfrastructure<'a> {
    docker: Cli,
    postgres_container: Option<Container<'a, Postgres>>,
    redis_container: Option<Container<'a, redis::Redis>>,
    wiremock_server: Option<wiremock::MockServer>,
}

impl<'a> TestInfrastructure<'a> {
    pub async fn new() -> Self {
        let docker = Cli::default();
        
        Self {
            docker,
            postgres_container: None,
            redis_container: None,
            wiremock_server: None,
        }
    }

    pub async fn start_postgres(&mut self) -> Result<String, TestError> {
        let postgres_image = Postgres::default()
            .with_db_name("test_db")
            .with_user("test_user")
            .with_password("test_password");

        let container = self.docker.run(postgres_image);
        let port = container.get_host_port_ipv4(5432);
        
        let connection_string = format!(
            "postgresql://test_user:test_password@localhost:{}/test_db",
            port
        );

        self.postgres_container = Some(container);
        Ok(connection_string)
    }

    pub async fn start_redis(&mut self) -> Result<String, TestError> {
        let redis_image = redis::Redis::default();
        let container = self.docker.run(redis_image);
        let port = container.get_host_port_ipv4(6379);
        
        let connection_string = format!("redis://localhost:{}", port);
        
        self.redis_container = Some(container);
        Ok(connection_string)
    }

    pub async fn start_wiremock(&mut self) -> Result<String, TestError> {
        let mock_server = wiremock::MockServer::start().await;
        let base_url = mock_server.uri();
        
        self.wiremock_server = Some(mock_server);
        Ok(base_url)
    }

    pub async fn setup_external_api_mocks(&self) -> Result<(), TestError> {
        if let Some(server) = &self.wiremock_server {
            // Mock Users API responses
            wiremock::Mock::given(wiremock::matchers::method("GET"))
                .and(wiremock::matchers::path_regex(r"/users/.*"))
                .respond_with(wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "id": "user-1",
                        "name": "Test User",
                        "email": "test@example.com"
                    })))
                .mount(server)
                .await;

            // Mock Offers API responses
            wiremock::Mock::given(wiremock::matchers::method("GET"))
                .and(wiremock::matchers::path_regex(r"/offers/.*"))
                .respond_with(wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "id": "offer-1",
                        "title": "Test Offer",
                        "price": 1000000
                    })))
                .mount(server)
                .await;
        }
        
        Ok(())
    }
}
```

### Staging Environment

#### Staging Kubernetes Cluster
**Платформа:** AWS EKS + Worker Nodes
**Назначение:** Production-like тестирование заглушек подграфов

**Kubernetes манифесты:**
```yaml
# k8s/staging/users-subgraph-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: users-subgraph
  namespace: staging
  labels:
    app: users-subgraph
    version: v1
spec:
  replicas: 2
  selector:
    matchLabels:
      app: users-subgraph
  template:
    metadata:
      labels:
        app: users-subgraph
        version: v1
    spec:
      containers:
      - name: users-subgraph
        image: auto-ru/users-subgraph:latest
        ports:
        - containerPort: 4002
        env:
        - name: RUST_LOG
          value: "info"
        - name: SERVICE_NAME
          value: "users-subgraph"
        - name: ENVIRONMENT
          value: "staging"
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "200m"
        livenessProbe:
          httpGet:
            path: /health
            port: 4002
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 4002
          initialDelaySeconds: 5
          periodSeconds: 5
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false

---
apiVersion: v1
kind: Service
metadata:
  name: users-subgraph-service
  namespace: staging
spec:
  selector:
    app: users-subgraph
  ports:
  - protocol: TCP
    port: 4002
    targetPort: 4002
  type: ClusterIP

---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: users-subgraph-ingress
  namespace: staging
  annotations:
    kubernetes.io/ingress.class: "nginx"
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
spec:
  tls:
  - hosts:
    - users-staging.auto.ru
    secretName: users-subgraph-tls
  rules:
  - host: users-staging.auto.ru
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: users-subgraph-service
            port:
              number: 4002
```

#### Staging Data Layer
**Компоненты:** AWS RDS, ElastiCache
**Назначение:** Managed database services для staging

**Terraform конфигурация:**
```hcl
# terraform/staging/data-layer.tf
resource "aws_db_instance" "staging_postgres" {
  identifier = "auto-ru-staging-postgres"
  
  engine         = "postgres"
  engine_version = "14.9"
  instance_class = "db.t3.micro"
  
  allocated_storage     = 20
  max_allocated_storage = 100
  storage_type         = "gp2"
  storage_encrypted    = true
  
  db_name  = "auto_ru_staging"
  username = "postgres"
  password = var.db_password
  
  vpc_security_group_ids = [aws_security_group.rds.id]
  db_subnet_group_name   = aws_db_subnet_group.staging.name
  
  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"
  
  skip_final_snapshot = true
  deletion_protection = false
  
  performance_insights_enabled = true
  monitoring_interval         = 60
  monitoring_role_arn        = aws_iam_role.rds_monitoring.arn
  
  tags = {
    Name        = "auto-ru-staging-postgres"
    Environment = "staging"
    Project     = "auto-ru-federation"
  }
}

resource "aws_elasticache_subnet_group" "staging" {
  name       = "auto-ru-staging-cache-subnet"
  subnet_ids = var.private_subnet_ids
}

resource "aws_elasticache_replication_group" "staging_redis" {
  replication_group_id       = "auto-ru-staging-redis"
  description                = "Redis cluster for Auto.ru staging"
  
  node_type            = "cache.t3.micro"
  port                 = 6379
  parameter_group_name = "default.redis7"
  
  num_cache_clusters = 2
  
  subnet_group_name  = aws_elasticache_subnet_group.staging.name
  security_group_ids = [aws_security_group.redis.id]
  
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  tags = {
    Name        = "auto-ru-staging-redis"
    Environment = "staging"
    Project     = "auto-ru-federation"
  }
}
```

### Monitoring Infrastructure

#### Metrics Collection
**Компоненты:** Prometheus, Grafana
**Назначение:** Мониторинг производительности заглушек

**Prometheus конфигурация:**
```yaml
# monitoring/prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "rules/*.yml"

scrape_configs:
  - job_name: 'users-subgraph'
    static_configs:
      - targets: ['users-subgraph:4002']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  - job_name: 'offers-subgraph'
    static_configs:
      - targets: ['offers-subgraph:4004']
    metrics_path: '/metrics'
    scrape_interval: 10s
    
  - job_name: 'apollo-router'
    static_configs:
      - targets: ['apollo-router:9090']
    metrics_path: '/metrics'
    scrape_interval: 5s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

**Grafana Dashboard для заглушек:**
```json
{
  "dashboard": {
    "title": "Auto.ru Subgraph Stubs Monitoring",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total{job=~\"users-subgraph|offers-subgraph\"}[5m])",
            "legendFormat": "{{job}} - {{method}}"
          }
        ]
      },
      {
        "title": "Response Time",
        "type": "graph", 
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket{job=~\"users-subgraph|offers-subgraph\"}[5m]))",
            "legendFormat": "{{job}} - 95th percentile"
          }
        ]
      },
      {
        "title": "Error Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total{job=~\"users-subgraph|offers-subgraph\",status=~\"5..\"}[5m])",
            "legendFormat": "{{job}} - 5xx errors"
          }
        ]
      }
    ]
  }
}
```

Эта диаграмма развертывания показывает полный жизненный цикл заглушек подграфов Task 11 от локальной разработки до production-ready staging окружения, включая все необходимые инфраструктурные компоненты для поддержки федеративной архитектуры.