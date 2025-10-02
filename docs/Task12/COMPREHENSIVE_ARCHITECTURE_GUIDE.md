# Task 12: Comprehensive Architecture Guide
## Настройка среды разработки и деплоя

### Обзор архитектуры

Task 12 представляет собой комплексную систему настройки среды разработки и деплоя для Auto.ru GraphQL Federation. Архитектура включает три основных компонента:

1. **Docker Configuration (12.1)** - Контейнеризация всех сервисов
2. **CI/CD Pipeline (12.2)** - Автоматизация тестирования и деплоя
3. **Documentation (12.3)** - Документация и руководства

### Архитектурные принципы

#### 1. Контейнеризация (Docker Configuration)

**Multi-stage Docker builds:**
```dockerfile
# Builder stage - компиляция Rust кода
FROM rust:1.75-slim as builder
WORKDIR /app
# Кеширование зависимостей
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --package ugc-subgraph

# Runtime stage - минимальный образ для production
FROM debian:bookworm-slim
RUN useradd -r -s /bin/false ugc
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/
USER ugc
```

**Преимущества:**
- Оптимизированный размер образов (builder stage исключается)
- Безопасность (non-root пользователь)
- Кеширование слоев Docker для быстрой пересборки
- Консистентная среда выполнения

#### 2. Orchestration с Docker Compose

**Основная конфигурация (docker-compose.yml):**
```yaml
services:
  apollo-router:
    build: ./apollo-router
    ports: ["4000:4000"]
    depends_on: [ugc-subgraph, users-subgraph, offers-subgraph]
    
  ugc-subgraph:
    build: ./ugc-subgraph
    ports: ["4001:4001"]
    depends_on: [ugc-postgres, redis]
    environment:
      - DATABASE_URL=postgresql://ugc_user:ugc_password@ugc-postgres:5432/ugc_db
```

**Development overrides (docker-compose.dev.yml):**
```yaml
services:
  ugc-subgraph:
    build:
      target: builder  # Остановка на builder stage
    volumes:
      - ./ugc-subgraph/src:/app/ugc-subgraph/src:ro  # Hot reload
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
    command: cargo run --package ugc-subgraph
```

#### 3. CI/CD Pipeline Architecture

**GitHub Actions Workflow Structure:**

```mermaid
graph TD
    A[Git Push/PR] --> B[CI Workflow]
    B --> C[Code Quality Checks]
    B --> D[Unit Tests]
    B --> E[Integration Tests]
    B --> F[Security Scans]
    
    C --> G[Docker Build Matrix]
    D --> G
    E --> G
    F --> G
    
    G --> H[Schema Validation]
    H --> I[Deploy to Staging]
    I --> J[E2E Tests]
    J --> K[Deploy to Production]
```

**Multi-service Build Matrix:**
```yaml
strategy:
  matrix:
    service: [ugc-subgraph, users-subgraph, offers-subgraph]

steps:
  - name: Build Docker image
    uses: docker/build-push-action@v3
    with:
      context: .
      file: ./${{ matrix.service }}/Dockerfile
      push: false
      tags: ${{ matrix.service }}:${{ github.sha }}
```

### Компонентная архитектура

#### 1. Local Development Environment

```
Developer Machine
├── Docker Desktop
│   ├── UGC Subgraph Container (4001)
│   ├── Users Subgraph Container (4002)
│   ├── Offers Subgraph Container (4004)
│   ├── Apollo Router Container (4000)
│   ├── PostgreSQL Container (5432)
│   └── Redis Container (6379)
├── Development Tools
│   ├── IDE/Editor (VS Code/IntelliJ)
│   ├── Git Client
│   └── Make Tool
└── Configuration Files
    ├── docker-compose.yml
    ├── docker-compose.dev.yml
    ├── Makefile
    └── .dockerignore
```

#### 2. CI/CD Infrastructure

```
GitHub Cloud
├── GitHub Actions
│   ├── CI Runner (Testing)
│   ├── Build Runner (Docker Images)
│   └── Deploy Runner (Kubernetes)
├── GitHub Container Registry
│   ├── UGC Image (auto-ru/ugc:tag)
│   ├── Users Image (auto-ru/users:tag)
│   └── Offers Image (auto-ru/offers:tag)
└── Workflow Files
    ├── ci.yml
    ├── deploy-staging.yml
    ├── deploy-production.yml
    └── security.yml
```

#### 3. Production Infrastructure

```
AWS Cloud
├── Staging Environment (EKS)
│   ├── Worker Nodes (EC2 t3.medium)
│   │   ├── UGC Pod (Replicas: 2)
│   │   ├── Users Pod (Replicas: 1)
│   │   ├── Offers Pod (Replicas: 1)
│   │   └── Router Pod (Replicas: 2)
│   └── Data Layer
│       ├── PostgreSQL RDS (db.t3.micro)
│       └── Redis ElastiCache (cache.t3.micro)
└── Production Environment (EKS)
    ├── Worker Nodes (EC2 c5.large)
    │   ├── UGC Pod (Replicas: 3)
    │   ├── Users Pod (Replicas: 2)
    │   ├── Offers Pod (Replicas: 2)
    │   └── Router Pod (Replicas: 3)
    └── Data Layer
        ├── PostgreSQL RDS (db.r5.large Multi-AZ)
        └── Redis ElastiCache (cache.r5.large Cluster)
```

### Безопасность и оптимизация

#### 1. Docker Security Best Practices

- **Non-root пользователи:** Все контейнеры запускаются под dedicated пользователями
- **Minimal base images:** Использование debian:bookworm-slim для runtime
- **Multi-stage builds:** Исключение build tools из production образов
- **Health checks:** Встроенные проверки состояния сервисов

#### 2. CI/CD Security

- **Dependency scanning:** Автоматическая проверка уязвимостей в зависимостях
- **Container scanning:** Сканирование Docker образов с Trivy
- **Secret management:** Использование GitHub Secrets для чувствительных данных
- **RBAC:** Ограниченные права доступа для deployment

#### 3. Performance Optimizations

- **Layer caching:** Оптимизация порядка COPY команд в Dockerfile
- **Dependency caching:** Отдельная сборка зависимостей для кеширования
- **Parallel builds:** Matrix strategy для параллельной сборки сервисов
- **Resource limits:** Kubernetes resource requests и limits

### Мониторинг и наблюдаемость

#### 1. Health Checks

```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1
```

#### 2. Logging Strategy

- **Structured logging:** JSON формат для централизованного сбора
- **Correlation IDs:** Трассировка запросов между сервисами
- **Log levels:** Настраиваемые уровни логирования через RUST_LOG

#### 3. Metrics Collection

- **Prometheus integration:** Встроенные метрики в Apollo Router
- **Custom metrics:** Бизнес-метрики в каждом подграфе
- **Grafana dashboards:** Визуализация метрик производительности

### Автоматизация разработки

#### 1. Makefile Commands

```makefile
dev: ## Start development environment
	docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

test: ## Run all tests
	cargo test --all-features

build: ## Build all Docker images
	docker-compose build

health: ## Check health of all services
	@curl -s http://localhost:4001/health | jq .
	@curl -s http://localhost:4002/health | jq .
```

#### 2. Development Scripts

- **setup.sh:** Первоначальная настройка окружения
- **test-federation.sh:** Тестирование федеративных запросов
- **cleanup.sh:** Очистка Docker ресурсов

### Стратегия деплоя

#### 1. Environment Promotion

```
Feature Branch → CI Tests → Staging Deploy → E2E Tests → Production Deploy
```

#### 2. Rolling Updates

- **Zero-downtime deployments:** Kubernetes rolling updates
- **Health check integration:** Проверка готовности перед переключением трафика
- **Rollback capability:** Автоматический откат при сбоях

#### 3. Blue-Green Deployment

- **Staging validation:** Полное тестирование в staging окружении
- **Traffic switching:** Постепенное переключение трафика
- **Monitoring:** Непрерывный мониторинг метрик после деплоя

### Документация и знания

#### 1. README Structure

- **Quick Start:** Минимальные шаги для запуска
- **Architecture Overview:** Высокоуровневое описание системы
- **Development Guide:** Детальные инструкции для разработчиков
- **Troubleshooting:** Решение частых проблем

#### 2. API Documentation

- **GraphQL Schema:** Автоматически генерируемая документация
- **Federation Examples:** Примеры федеративных запросов
- **Authentication:** Инструкции по аутентификации

#### 3. Runbooks

- **Deployment Procedures:** Пошаговые инструкции деплоя
- **Incident Response:** Процедуры реагирования на инциденты
- **Monitoring Playbooks:** Интерпретация метрик и алертов

### Заключение

Task 12 создает надежную, масштабируемую и автоматизированную инфраструктуру для разработки и деплоя Auto.ru GraphQL Federation. Архитектура обеспечивает:

- **Developer Experience:** Простой локальный запуск и разработка
- **Quality Assurance:** Автоматизированное тестирование и проверки
- **Operational Excellence:** Надежный деплой и мониторинг
- **Security:** Встроенные практики безопасности
- **Scalability:** Готовность к росту нагрузки и команды