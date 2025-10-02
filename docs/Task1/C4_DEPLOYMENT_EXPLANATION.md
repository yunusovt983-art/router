# Task 1: Deployment Diagram - Архитектура развертывания

## Обзор

Deployment диаграмма Task 1 показывает **физическую архитектуру развертывания системы** в локальной среде разработки, демонстрируя как архитектурные решения воплощаются в конкретной инфраструктуре Docker. Диаграмма служит мостом между логической архитектурой и физическим развертыванием.

## 🖥️ Developer Machine: Физическая инфраструктура

### Архитектурное решение: Локальная разработка
- **Изоляция среды**: Полная изоляция через Docker
- **Воспроизводимость**: Идентичные среды на всех машинах
- **Простота настройки**: One-command setup

### Реализация в Docker Engine
```yaml
# docker-compose.yml - Физическое развертывание
version: '3.8'

services:
  # Apollo Router - Центральная точка федерации
  apollo-router:
    build:
      context: .
      dockerfile: crates/apollo-router/Dockerfile
    container_name: autoru_apollo_router
    hostname: apollo-router
    ports:
      - "4000:4000"    # Внешний доступ к GraphQL API
    networks:
      - federation-network
    depends_on:
      - ugc-subgraph
      - users-subgraph
      - offers-subgraph
    environment:
      - RUST_LOG=info,apollo_router=debug
      - APOLLO_ROUTER_CONFIG_PATH=/app/router.yaml
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:4000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  # UGC Subgraph - Домен пользовательского контента
  ugc-subgraph:
    build:
      context: .
      dockerfile: crates/ugc-subgraph/Dockerfile
    container_name: autoru_ugc_subgraph
    hostname: ugc-subgraph
    ports:
      - "4001:4001"   # Внутренний порт для федерации
    networks:
      - federation-network
      - data-network
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    environment:
      - RUST_LOG=info,ugc_subgraph=debug
      - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@postgres:5432/auto_ru_federation
      - REDIS_URL=redis://redis:6379
    restart: unless-stopped
```

## 🌐 Network Architecture: Сетевая сегментация

### Application Network - Федеративная сеть
```yaml
# Сеть для GraphQL федерации
networks:
  federation-network:
    driver: bridge
    name: autoru_federation
    ipam:
      driver: default
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1
    driver_opts:
      com.docker.network.bridge.name: br-autoru-fed
      com.docker.network.bridge.enable_icc: "true"
      com.docker.network.bridge.enable_ip_masquerade: "true"
```

### Data Network - Изолированная сеть данных
```yaml
# Изолированная сеть для баз данных
networks:
  data-network:
    driver: bridge
    name: autoru_data
    internal: true  # Нет доступа в интернет
    ipam:
      driver: default
      config:
        - subnet: 172.21.0.0/16
          gateway: 172.21.0.1
    driver_opts:
      com.docker.network.bridge.name: br-autoru-data
```

### Monitoring Network - Сеть мониторинга
```yaml
# Сеть для мониторинга и наблюдаемости
networks:
  monitoring-network:
    driver: bridge
    name: autoru_monitoring
    ipam:
      driver: default
      config:
        - subnet: 172.22.0.0/16
          gateway: 172.22.0.1
```

## 🗄️ Data Layer: Физическое хранение данных

### PostgreSQL Container - Основная БД
```yaml
# PostgreSQL сервис с персистентным хранением
postgres:
  image: postgres:15-alpine
  container_name: autoru_postgres
  hostname: postgres
  ports:
    - "5432:5432"   # Доступ для разработки
  networks:
    - data-network
  environment:
    POSTGRES_DB: auto_ru_federation
    POSTGRES_USER: postgres
    POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    POSTGRES_INITDB_ARGS: "--encoding=UTF8 --locale=C"
  volumes:
    # Персистентное хранение данных
    - postgres_data:/var/lib/postgresql/data
    # Миграции БД при инициализации
    - ./migrations:/docker-entrypoint-initdb.d:ro
    # Конфигурация PostgreSQL
    - ./config/postgresql.conf:/etc/postgresql/postgresql.conf:ro
  command: >
    postgres 
    -c config_file=/etc/postgresql/postgresql.conf
    -c log_statement=all
    -c log_destination=stderr
    -c logging_collector=off
  restart: unless-stopped
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U postgres -d auto_ru_federation"]
    interval: 10s
    timeout: 5s
    retries: 5
    start_period: 30s
```

### PostgreSQL Volume Configuration
```yaml
# Том для персистентного хранения PostgreSQL
volumes:
  postgres_data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ${PWD}/data/postgres
    name: autoru_postgres_data
```

### Redis Container - Кеш и сессии
```yaml
# Redis сервис для кеширования
redis:
  image: redis:7-alpine
  container_name: autoru_redis
  hostname: redis
  ports:
    - "6379:6379"   # Доступ для разработки
  networks:
    - data-network
  command: >
    redis-server 
    --maxmemory 256mb 
    --maxmemory-policy allkeys-lru
    --save 900 1
    --save 300 10
    --save 60 10000
    --appendonly yes
    --appendfsync everysec
  volumes:
    # Персистентное хранение Redis
    - redis_data:/data
    # Конфигурация Redis
    - ./config/redis.conf:/usr/local/etc/redis/redis.conf:ro
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "redis-cli", "--raw", "incr", "ping"]
    interval: 10s
    timeout: 3s
    retries: 5
```

## 📊 Monitoring Infrastructure: Наблюдаемость

### Prometheus Container - Сбор метрик
```yaml
# Prometheus для сбора метрик
prometheus:
  image: prom/prometheus:v2.45.0
  container_name: autoru_prometheus
  hostname: prometheus
  ports:
    - "9090:9090"   # Web UI для разработки
  networks:
    - monitoring-network
    - federation-network  # Доступ к сервисам
  command:
    - '--config.file=/etc/prometheus/prometheus.yml'
    - '--storage.tsdb.path=/prometheus'
    - '--web.console.libraries=/etc/prometheus/console_libraries'
    - '--web.console.templates=/etc/prometheus/consoles'
    - '--storage.tsdb.retention.time=15d'
    - '--web.enable-lifecycle'
  volumes:
    # Конфигурация Prometheus
    - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml:ro
    - ./monitoring/alert_rules.yml:/etc/prometheus/alert_rules.yml:ro
    # Персистентное хранение метрик
    - prometheus_data:/prometheus
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:9090/-/healthy"]
    interval: 30s
    timeout: 10s
    retries: 3
```

### Jaeger Container - Distributed Tracing
```yaml
# Jaeger All-in-One для трассировки
jaeger:
  image: jaegertracing/all-in-one:1.47
  container_name: autoru_jaeger
  hostname: jaeger
  ports:
    - "16686:16686"  # Jaeger UI
    - "14268:14268"  # Jaeger collector HTTP
    - "6831:6831/udp"  # Jaeger agent UDP
  networks:
    - monitoring-network
    - federation-network
  environment:
    - COLLECTOR_OTLP_ENABLED=true
    - COLLECTOR_ZIPKIN_HOST_PORT=:9411
  volumes:
    # Временное хранение трассировок
    - jaeger_data:/tmp
  restart: unless-stopped
  healthcheck:
    test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:14269/"]
    interval: 30s
    timeout: 10s
    retries: 3
```

## 💾 Host File System: Файловая система хоста

### Project Workspace - Исходный код
```bash
# Структура проекта на хосте
auto-ru-graphql-federation/
├── crates/                    # Rust workspace
│   ├── shared/               # Общие компоненты
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs
│   │   │   ├── auth.rs
│   │   │   └── errors.rs
│   │   └── Cargo.toml
│   ├── apollo-router/        # Apollo Router
│   │   ├── src/
│   │   │   └── main.rs
│   │   ├── router.yaml
│   │   ├── Dockerfile
│   │   └── Cargo.toml
│   └── ugc-subgraph/         # UGC подграф
│       ├── src/
│       │   ├── main.rs
│       │   ├── resolvers.rs
│       │   └── models.rs
│       ├── Dockerfile
│       └── Cargo.toml
├── migrations/               # Миграции БД
│   ├── 001_create_schema.sql
│   └── 002_seed_data.sql
├── config/                   # Конфигурации
│   ├── postgresql.conf
│   └── redis.conf
├── monitoring/               # Мониторинг
│   ├── prometheus.yml
│   └── alert_rules.yml
├── scripts/                  # Скрипты автоматизации
│   ├── dev-setup.sh
│   └── dev-stop.sh
├── docker-compose.yml        # Оркестрация
├── Makefile                  # Автоматизация
├── .env.example             # Шаблон конфигурации
└── Cargo.toml               # Workspace конфигурация
```

### Volume Mounts - Монтирование данных
```yaml
# Монтирование исходного кода для hot reload
volumes:
  # Исходный код для разработки
  - type: bind
    source: ./crates
    target: /app/crates
    consistency: cached
  
  # Конфигурации
  - type: bind
    source: ./config
    target: /app/config
    read_only: true
  
  # Миграции БД
  - type: bind
    source: ./migrations
    target: /docker-entrypoint-initdb.d
    read_only: true
```

## 🔧 Build Tools: Инструменты сборки

### Cargo Cache - Кеширование зависимостей
```bash
# Кеширование Rust зависимостей на хосте
~/.cargo/
├── registry/                 # Реестр крейтов
│   ├── index/
│   └── cache/
├── git/                      # Git зависимости
│   ├── db/
│   └── checkouts/
└── bin/                      # Установленные бинарники
    ├── cargo-watch
    └── cargo-expand
```

### Docker Cache - Кеширование образов
```bash
# Docker кеш для быстрой пересборки
/var/lib/docker/
├── image/                    # Слои образов
├── containers/               # Контейнеры
├── volumes/                  # Именованные тома
└── buildkit/                 # BuildKit кеш
```

## 🚀 Development Automation: Автоматизация разработки

### Development Setup Script
```bash
#!/bin/bash
# scripts/dev-setup.sh - Автоматизация настройки среды

set -e

PROJECT_NAME="auto-ru-graphql-federation"
COMPOSE_PROJECT_NAME="autoru"

echo "🚀 Setting up $PROJECT_NAME development environment"

# Проверка системных требований
check_system_requirements() {
    echo "📋 Checking system requirements..."
    
    # Проверка Docker
    if ! command -v docker &> /dev/null; then
        echo "❌ Docker is required but not installed"
        echo "📖 Install from: https://docs.docker.com/get-docker/"
        exit 1
    fi
    
    # Проверка Docker Compose
    if ! docker compose version &> /dev/null; then
        echo "❌ Docker Compose is required but not installed"
        exit 1
    fi
    
    # Проверка Rust
    if ! command -v cargo &> /dev/null; then
        echo "❌ Rust/Cargo is required but not installed"
        echo "📖 Install from: https://rustup.rs/"
        exit 1
    fi
    
    # Проверка доступных портов
    for port in 4000 4001 4002 4003 5432 6379 9090 16686; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            echo "⚠️  Port $port is already in use"
        fi
    done
    
    echo "✅ System requirements satisfied"
}

# Настройка окружения
setup_environment() {
    echo "🔧 Setting up environment..."
    
    # Создание .env из шаблона
    if [ ! -f .env ]; then
        cp .env.example .env
        echo "📝 Created .env from template"
        echo "⚠️  Please review and update .env file with your settings"
    fi
    
    # Создание директорий для данных
    mkdir -p data/{postgres,redis,prometheus,jaeger}
    
    # Создание Docker сетей
    docker network create ${COMPOSE_PROJECT_NAME}_federation 2>/dev/null || true
    docker network create ${COMPOSE_PROJECT_NAME}_data 2>/dev/null || true
    docker network create ${COMPOSE_PROJECT_NAME}_monitoring 2>/dev/null || true
    
    echo "✅ Environment configured"
}

# Сборка сервисов
build_services() {
    echo "🔨 Building services..."
    
    # Сборка Rust workspace
    echo "📦 Building Rust workspace..."
    cargo build --workspace
    
    # Сборка Docker образов
    echo "🐳 Building Docker images..."
    COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose build --parallel
    
    echo "✅ Services built successfully"
}

# Запуск сервисов
start_services() {
    echo "🚀 Starting services..."
    
    # Запуск инфраструктуры (БД, кеш)
    echo "📊 Starting infrastructure services..."
    COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose up -d postgres redis
    
    # Ожидание готовности БД
    echo "⏳ Waiting for database to be ready..."
    timeout 60 bash -c 'until docker compose exec postgres pg_isready -U postgres; do sleep 2; done'
    
    # Запуск мониторинга
    echo "📈 Starting monitoring services..."
    COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose up -d prometheus jaeger
    
    # Запуск подграфов
    echo "🔗 Starting subgraph services..."
    COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose up -d ugc-subgraph users-subgraph offers-subgraph
    
    # Ожидание готовности подграфов
    echo "⏳ Waiting for subgraphs to be ready..."
    sleep 15
    
    # Запуск Apollo Router
    echo "🌐 Starting Apollo Router..."
    COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose up -d apollo-router
    
    echo "✅ All services started"
}

# Валидация развертывания
validate_deployment() {
    echo "🔍 Validating deployment..."
    
    # Проверка здоровья сервисов
    services=(
        "apollo-router:4000"
        "ugc-subgraph:4001"
        "users-subgraph:4002"
        "offers-subgraph:4003"
    )
    
    for service in "${services[@]}"; do
        service_name=${service%:*}
        port=${service#*:}
        
        if curl -f -s "http://localhost:$port/health" >/dev/null; then
            echo "✅ $service_name is healthy"
        else
            echo "❌ $service_name health check failed"
            echo "📋 Checking logs..."
            COMPOSE_PROJECT_NAME=$COMPOSE_PROJECT_NAME docker compose logs --tail=10 $service_name
            exit 1
        fi
    done
    
    # Проверка GraphQL схемы
    echo "🔍 Validating GraphQL schema..."
    if curl -X POST http://localhost:4000/graphql \
        -H "Content-Type: application/json" \
        -d '{"query": "{ __schema { types { name } } }"}' \
        -s | jq -e '.data.__schema.types | length > 0' >/dev/null; then
        echo "✅ GraphQL schema is valid"
    else
        echo "❌ GraphQL schema validation failed"
        exit 1
    fi
    
    echo "✅ Deployment validation passed"
}

# Отображение информации о сервисах
show_service_info() {
    echo ""
    echo "🎉 Development environment is ready!"
    echo ""
    echo "📊 Services:"
    echo "  • GraphQL API:     http://localhost:4000/graphql"
    echo "  • GraphQL Playground: http://localhost:4000"
    echo "  • Prometheus:      http://localhost:9090"
    echo "  • Jaeger UI:       http://localhost:16686"
    echo ""
    echo "🔧 Management Commands:"
    echo "  • View logs:       make dev-logs"
    echo "  • Stop services:   make dev-stop"
    echo "  • Restart:         make dev-restart"
    echo "  • Clean up:        make clean"
    echo ""
    echo "📚 Documentation:"
    echo "  • API Docs:        docs/API.md"
    echo "  • Architecture:    docs/ARCHITECTURE.md"
    echo "  • Development:     docs/DEVELOPMENT.md"
}

# Главная функция
main() {
    check_system_requirements
    setup_environment
    build_services
    start_services
    validate_deployment
    show_service_info
}

# Обработка сигналов для graceful shutdown
trap 'echo "🛑 Interrupted. Cleaning up..."; docker compose down; exit 1' INT TERM

main "$@"
```

## 🎯 Заключение: Физическая архитектура как код

Deployment диаграмма Task 1 демонстрирует **трансформацию логической архитектуры в физическое развертывание**:

### 🏗️ **Архитектурные принципы → Инфраструктура**
- **Изоляция сервисов** → Docker контейнеры с сетевой сегментацией
- **Масштабируемость** → Независимые контейнеры с resource limits
- **Наблюдаемость** → Dedicated мониторинг инфраструктура
- **Персистентность** → Именованные тома для данных

### 🔧 **Технические решения → Docker Compose**
- **Оркестрация** → Декларативная конфигурация сервисов
- **Зависимости** → Правильный порядок запуска с health checks
- **Сети** → Сегментированные Docker сети для безопасности
- **Тома** → Персистентное хранение и bind mounts

### 📊 **DevOps принципы → Автоматизация**
- **Infrastructure as Code** → Версионируемые конфигурации
- **Воспроизводимость** → Идентичные среды через контейнеры
- **Простота развертывания** → One-command setup скрипты
- **Мониторинг** → Встроенная наблюдаемость с первого дня

Диаграмма служит **исполняемой инфраструктурой**, где каждый архитектурный компонент имеет физическое воплощение в виде Docker контейнера с определенной конфигурацией, сетевыми настройками и хранилищем данных.