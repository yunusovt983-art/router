# Task 12: Context Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_CONTEXT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Контекстная диаграмма Task 12 показывает высокоуровневые взаимодействия между участниками процесса разработки и внешними системами. Каждый элемент диаграммы имеет прямое отражение в коде и конфигурационных файлах.

### Архитектурные элементы и их реализация в коде

#### 1. Developer (Разработчик)
**PlantUML элемент:**
```plantuml
Person(developer, "Developer", "Software developer working on Auto.ru GraphQL Federation")
```

**Реализация в коде:**
```bash
# .gitconfig - конфигурация разработчика
[user]
    name = Developer Name
    email = developer@auto.ru

# Локальные команды разработчика
make dev     # Запуск среды разработки
make test    # Запуск тестов
make health  # Проверка состояния сервисов
```

**Связь с кодом:**
- **Makefile** содержит команды для разработчика
- **docker-compose.dev.yml** настроен для локальной разработки
- **.vscode/settings.json** содержит настройки IDE

#### 2. DevOps Engineer (DevOps инженер)
**PlantUML элемент:**
```plantuml
Person(devops, "DevOps Engineer", "Infrastructure and deployment specialist")
```

**Реализация в коде:**
```yaml
# .github/workflows/ci.yml - CI/CD конфигурация DevOps
name: CI
on:
  push:
    branches: [main, develop]
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
```

**Связь с кодом:**
- **GitHub Actions workflows** в `.github/workflows/`
- **Terraform конфигурации** для инфраструктуры AWS
- **Kubernetes manifests** в `k8s/` директории

#### 3. QA Engineer (Инженер по качеству)
**PlantUML элемент:**
```plantuml
Person(qa, "QA Engineer", "Quality assurance and testing specialist")
```

**Реализация в коде:**
```rust
// Тесты, которые анализирует QA
#[cfg(test)]
mod integration_tests {
    #[tokio::test]
    async fn test_federation_query() {
        let response = client
            .post("/graphql")
            .json(&json!({
                "query": "query { reviews { edges { node { id } } } }"
            }))
            .send()
            .await;
        
        assert_eq!(response.status(), 200);
    }
}
```

**Связь с кодом:**
- **Test files** в `tests/` директориях каждого подграфа
- **CI test reports** генерируемые GitHub Actions
- **E2E test scenarios** в `docker-compose.test.yml`

### Системные компоненты и их код

#### 1. Development Environment
**PlantUML элемент:**
```plantuml
System_Boundary(development_env, "Development Environment") {
    System(local_dev, "Local Development", "Docker Compose based local development environment")
    System(ci_cd, "CI/CD Pipeline", "GitHub Actions automated testing and deployment")
    System(documentation, "Documentation System", "Project documentation and API guides")
}
```

**Реализация в коде:**

##### Local Development
```yaml
# docker-compose.yml - основная конфигурация
version: '3.8'
services:
  ugc-subgraph:
    build: ./ugc-subgraph
    ports: ["4001:4001"]
    environment:
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
```

##### CI/CD Pipeline
```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: test_password
```

##### Documentation System
```markdown
# README.md - главная документация
# Auto.ru GraphQL Federation

## Quick Start
1. Clone repository
2. Run `docker-compose up -d`
3. Open http://localhost:4000/graphql
```

#### 2. External Systems Integration

##### GitHub Repository
**PlantUML связь:**
```plantuml
Rel(developer, github, "Pushes code", "Git")
Rel(ci_cd, github, "Triggered by", "Git hooks")
```

**Реализация в коде:**
```yaml
# .github/workflows/ci.yml - Git hooks реализация
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
```

```bash
# Git hooks в .git/hooks/pre-commit
#!/bin/sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

##### Docker Registry
**PlantUML связь:**
```plantuml
Rel(ci_cd, docker_hub, "Pushes images", "Docker Registry API")
```

**Реализация в коде:**
```yaml
# .github/workflows/deploy-staging.yml
- name: Login to Amazon ECR
  uses: aws-actions/amazon-ecr-login@v2

- name: Build and push Docker images
  run: |
    docker build -t $ECR_REGISTRY/ugc-subgraph:$GITHUB_SHA .
    docker push $ECR_REGISTRY/ugc-subgraph:$GITHUB_SHA
```

##### AWS Environments
**PlantUML связь:**
```plantuml
Rel(ci_cd, aws_staging, "Deploys to", "kubectl/Helm")
Rel(ci_cd, aws_prod, "Deploys to", "kubectl/Helm")
```

**Реализация в коде:**
```yaml
# k8s/staging/ugc-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: staging
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: ugc-subgraph
        image: auto-ru/ugc-subgraph:latest
        ports:
        - containerPort: 4001
```

```bash
# Deployment script в CI/CD
aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
kubectl set image deployment/ugc-subgraph ugc-subgraph=$ECR_REGISTRY/ugc-subgraph:$GITHUB_SHA -n staging
kubectl rollout status deployment/ugc-subgraph -n staging --timeout=300s
```

### Потоки данных и их реализация

#### 1. Development Flow
**PlantUML поток:**
```plantuml
Rel(developer, local_dev, "Develops and tests", "Docker Compose")
Rel(developer, documentation, "Reads and updates", "Markdown/API docs")
```

**Код реализации:**
```makefile
# Makefile - команды разработчика
dev: ## Start development environment
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "✅ Development environment started"
	@echo "GraphQL Playground: http://localhost:4000/graphql"

docs: ## Generate documentation
	cargo doc --no-deps --open
```

#### 2. CI/CD Flow
**PlantUML поток:**
```plantuml
Rel(local_dev, github, "Pushes code", "Git")
Rel(github, ci_cd, "Triggered by", "Git hooks")
```

**Код реализации:**
```yaml
# .github/workflows/ci.yml - полный CI/CD поток
name: CI
on:
  push:
    branches: [main, develop]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings

  test:
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --all-features

  build:
    needs: test
    runs-on: ubuntu-latest
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph]
    steps:
      - name: Build Docker image
        run: docker build -f ${{ matrix.service }}/Dockerfile .

  deploy:
    needs: build
    if: github.ref == 'refs/heads/develop'
    runs-on: ubuntu-latest
    steps:
      - name: Deploy to staging
        run: kubectl set image deployment/$SERVICE $SERVICE:$GITHUB_SHA
```

#### 3. Monitoring Flow
**PlantUML поток:**
```plantuml
Rel(aws_staging, monitoring, "Sends metrics", "OpenTelemetry")
Rel(aws_prod, monitoring, "Sends metrics", "OpenTelemetry")
```

**Код реализации:**
```yaml
# router.yaml - мониторинг конфигурация
telemetry:
  metrics:
    prometheus:
      enabled: true
      listen: 0.0.0.0:9090
  tracing:
    jaeger:
      enabled: true
      endpoint: http://jaeger:14268/api/traces
```

```rust
// Rust код для метрик в подграфах
use prometheus::{Counter, Histogram, register_counter, register_histogram};

lazy_static! {
    static ref REQUEST_COUNTER: Counter = register_counter!(
        "graphql_requests_total",
        "Total number of GraphQL requests"
    ).unwrap();
    
    static ref REQUEST_DURATION: Histogram = register_histogram!(
        "graphql_request_duration_seconds",
        "GraphQL request duration in seconds"
    ).unwrap();
}

// В резолвере
async fn resolve_review(&self, ctx: &Context<'_>) -> Result<Review> {
    let _timer = REQUEST_DURATION.start_timer();
    REQUEST_COUNTER.inc();
    
    // Бизнес логика
    let review = self.review_service.get_review(id).await?;
    Ok(review)
}
```

### Конфигурационные файлы как мост

#### 1. Environment Configuration
**Архитектурное решение:** Разделение конфигураций по окружениям
**Код реализации:**
```yaml
# docker-compose.yml - базовая конфигурация
services:
  ugc-subgraph:
    environment:
      - RUST_LOG=info

# docker-compose.dev.yml - development overrides
services:
  ugc-subgraph:
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1

# docker-compose.prod.yml - production overrides
services:
  ugc-subgraph:
    environment:
      - RUST_LOG=warn
    deploy:
      resources:
        limits:
          memory: 512M
```

#### 2. Security Configuration
**Архитектурное решение:** Многоуровневая безопасность
**Код реализации:**
```dockerfile
# Dockerfile - security в контейнере
FROM debian:bookworm-slim
RUN useradd -r -s /bin/false ugc  # Non-root пользователь
USER ugc
```

```yaml
# k8s/security/network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ugc-network-policy
spec:
  podSelector:
    matchLabels:
      app: ugc-subgraph
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: apollo-router
```

### Трассируемость от архитектуры к коду

#### 1. Архитектурное требование → Код
**Требование:** "Developer должен иметь простой способ запуска локальной среды"
**Архитектурное решение:** Local Development System в контексте
**Код реализации:**
```makefile
dev: ## Start development environment with hot reload
	@echo "Starting development environment..."
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
	@echo "✅ Development environment started"
```

#### 2. Внешняя интеграция → Конфигурация
**Требование:** "CI/CD должен автоматически деплоить в AWS"
**Архитектурное решение:** GitHub Actions → AWS EKS связь
**Код реализации:**
```yaml
- name: Deploy to EKS
  run: |
    aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
    kubectl set image deployment/ugc-subgraph ugc-subgraph=$IMAGE_TAG -n staging
```

#### 3. Мониторинг → Observability код
**Требование:** "Система должна предоставлять метрики и трассировку"
**Архитектурное решение:** Monitoring Stack в контексте
**Код реализации:**
```rust
// OpenTelemetry integration
use opentelemetry::trace::TraceContextExt;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[tracing::instrument]
async fn create_review(input: CreateReviewInput) -> Result<Review> {
    let span = tracing::Span::current();
    span.set_attribute("review.rating", input.rating as i64);
    
    // Бизнес логика с автоматической трассировкой
    let review = Review::create(input).await?;
    Ok(review)
}
```

### Заключение

Контекстная диаграмма Task 12 служит мостом между высокоуровневыми архитектурными решениями и конкретной реализацией кода:

1. **Участники системы** отражены в конфигурационных файлах и скриптах
2. **Системные границы** реализованы через Docker Compose и Kubernetes namespaces
3. **Внешние интеграции** имеют прямое отражение в CI/CD workflows и deployment скриптах
4. **Потоки данных** трассируются через код от Git hooks до production deployment

Каждый элемент диаграммы имеет конкретную реализацию в коде, что обеспечивает полную трассируемость от архитектурного дизайна до работающей системы.