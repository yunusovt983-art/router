# C4 Architecture Documentation - Task 1: Infrastructure Setup

## Обзор архитектуры

Данная документация описывает архитектуру инфраструктуры, созданной в рамках Task 1 проекта Auto.ru GraphQL Federation, используя модель C4 (Context, Containers, Components, Code).

## Структура документации

### 1. Context Diagram (Контекстная диаграмма)
**Файл**: `C4_ARCHITECTURE_CONTEXT.puml`

Показывает систему на самом высоком уровне:
- **Пользователи**: Разработчики и DevOps инженеры
- **Основная система**: Auto.ru GraphQL Federation System
- **Внешние системы**: PostgreSQL, Redis, Prometheus, Jaeger

**Ключевые взаимодействия**:
- Разработчики отправляют GraphQL запросы через Apollo Router
- DevOps инженеры настраивают и мониторят систему
- Система интегрируется с внешними сервисами для хранения данных и мониторинга

### 2. Container Diagram (Диаграмма контейнеров)
**Файл**: `C4_ARCHITECTURE_CONTAINER.puml`

Детализирует внутреннюю структуру системы:

#### Основные контейнеры:
- **Apollo Router**: Федеративный GraphQL роутер (Rust, Axum)
- **UGC Subgraph**: Сервис пользовательского контента (Rust, async-graphql)
- **Users Subgraph**: Сервис пользователей (Rust, async-graphql)
- **Offers Subgraph**: Сервис объявлений (Rust, async-graphql)
- **Shared Library**: Общие компоненты (Rust crate)

#### Инфраструктурные контейнеры:
- **PostgreSQL**: Основная база данных
- **Redis**: Кеш и сессии
- **Prometheus**: Сбор метрик
- **Jaeger**: Distributed tracing

#### Инструменты разработки:
- **Cargo Workspace**: Система сборки
- **Docker Compose**: Оркестрация контейнеров
- **Development Scripts**: Автоматизация

### 3. Component Diagram (Диаграмма компонентов)
**Файл**: `C4_ARCHITECTURE_COMPONENT.puml`

Показывает внутреннюю структуру ключевых контейнеров:

#### Cargo Workspace компоненты:
- **Workspace Config**: Управление зависимостями и членами workspace
- **Shared Crate**: Общие типы, утилиты аутентификации, обработка ошибок
- **Subgraph Crates**: Dockerfile'ы и конфигурации для каждого подграфа

#### Docker Infrastructure:
- **Docker Compose**: Оркестрация сервисов
- **Service Definitions**: PostgreSQL и Redis сервисы

#### Development Tools:
- **Makefile**: Автоматизация команд
- **Scripts**: Настройка и управление средой разработки
- **Environment Config**: Переменные окружения

### 4. Deployment Diagram (Диаграмма развертывания)
**Файл**: `C4_ARCHITECTURE_DEPLOYMENT.puml`

Описывает физическое развертывание в среде разработки:

#### Машина разработчика:
- **Docker Engine**: Контейнерная платформа
- **Application Network**: Сеть для приложений
- **Data Network**: Изолированная сеть для данных
- **Monitoring Network**: Сеть для мониторинга

#### Контейнеры приложений:
- Apollo Router Container (порт 4000)
- UGC Subgraph Container (порт 4001)
- Users Subgraph Container (порт 4002)
- Offers Subgraph Container (порт 4003)

#### Контейнеры данных:
- PostgreSQL Container (порт 5432) с persistent volume
- Redis Container (порт 6379)

#### Контейнеры мониторинга:
- Prometheus Container (порт 9090)
- Jaeger Container (порт 16686)

## Ключевые архитектурные решения Task 1

### 1. Модульная архитектура
- **Cargo Workspace** для управления множественными crate'ами
- **Shared Library** для переиспользования общего кода
- **Изолированные подграфы** для независимой разработки

### 2. Контейнеризация
- **Multi-stage Docker builds** для оптимизации размера образов
- **Docker Compose** для локальной оркестрации
- **Сетевая изоляция** между различными типами сервисов

### 3. Observability
- **Prometheus** для сбора метрик
- **Jaeger** для distributed tracing
- **Структурированное логирование** во всех сервисах

### 4. Автоматизация разработки
- **Makefile** для стандартизации команд
- **Shell scripts** для настройки среды
- **Environment configuration** для гибкости настроек

### 5. Безопасность
- **Минимальные Docker образы** (Debian slim)
- **Изоляция сетей** в Docker
- **Переменные окружения** для секретов

## Преимущества архитектуры

1. **Масштабируемость**: Легко добавлять новые подграфы
2. **Изоляция**: Каждый сервис независим и контейнеризован
3. **Наблюдаемость**: Полная телеметрия и мониторинг
4. **Простота разработки**: Автоматизированная настройка среды
5. **Переносимость**: Одинаковая среда для всех разработчиков

## Следующие шаги

После завершения Task 1 инфраструктура готова для:
- Реализации бизнес-логики подграфов (Task 2-4)
- Настройки аутентификации и авторизации (Task 6)
- Добавления продвинутых возможностей мониторинга (Task 8)
- Оптимизации производительности (Task 9)

## Использование диаграмм

Для просмотра диаграмм используйте:
- **PlantUML плагины** в IDE (VS Code, IntelliJ)
- **Онлайн PlantUML сервер**: http://www.plantuml.com/plantuml/
- **Локальный PlantUML**: `java -jar plantuml.jar *.puml`

Все диаграммы автоматически обновляются при изменении `.puml` файлов.