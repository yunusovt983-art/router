# C4 Architecture Documentation - Task 2: UGC Subgraph Implementation

## Обзор архитектуры

Данная документация описывает архитектуру UGC (User Generated Content) подграфа, реализованного в рамках Task 2 проекта Auto.ru GraphQL Federation, используя модель C4 (Context, Containers, Components, Code).

## Структура документации

### 1. Context Diagram (Контекстная диаграмма)
**Файл**: `C4_ARCHITECTURE_CONTEXT.puml`

Показывает UGC подграф в контексте федеративной системы:
- **Пользователи**: Обычные пользователи, модераторы, разработчики
- **UGC Subgraph**: Основной сервис для управления отзывами и рейтингами
- **Федеративные сервисы**: Apollo Router, Users Subgraph, Offers Subgraph
- **Инфраструктура**: PostgreSQL, Redis, Prometheus, Jaeger

**Ключевые взаимодействия**:
- Пользователи создают и читают отзывы через Apollo Router
- Модераторы управляют контентом через GraphQL API
- UGC подграф интегрируется с другими подграфами через федерацию
- Данные сохраняются в PostgreSQL и кешируются в Redis

### 2. Container Diagram (Диаграмма контейнеров)
**Файл**: `C4_ARCHITECTURE_CONTAINER.puml`

Детализирует внутреннюю структуру UGC подграфа:

#### Основные контейнеры UGC системы:
- **GraphQL Server**: HTTP сервер с GraphQL API (Rust, Axum, async-graphql)
- **Review Service**: Бизнес-логика для работы с отзывами
- **Review Repository**: Слой доступа к данным (SQLx)
- **Auth Middleware**: Аутентификация и авторизация
- **Cache Service**: Кеширование данных (Redis)

#### Федеративная интеграция:
- **Apollo Router**: Маршрутизация федеративных запросов
- **Users Subgraph**: Информация о пользователях
- **Offers Subgraph**: Информация о объявлениях

#### Инфраструктурные компоненты:
- **PostgreSQL**: Основная база данных с таблицами reviews и offer_ratings
- **Redis**: Кеш для агрегированных данных и сессий
- **Prometheus**: Сбор метрик производительности и бизнес-логики
- **Jaeger**: Distributed tracing для отладки и мониторинга

### 3. Component Diagram (Диаграмма компонентов)
**Файл**: `C4_ARCHITECTURE_COMPONENT.puml`

Показывает детальную внутреннюю структуру UGC подграфа:

#### GraphQL Layer:
- **Query Resolvers**: Резолверы для получения данных (review, reviews, offerRating)
- **Mutation Resolvers**: Резолверы для изменения данных (createReview, updateReview, deleteReview, moderateReview)
- **Federation Types**: Федеративные типы с директивами @key, @extends
- **GraphQL Schema**: Композиция схемы с валидацией

#### Middleware Layer:
- **Auth Guard**: Проверка аутентификации и авторизации
- **Rate Limiter**: Ограничение запросов на основе Redis
- **Error Handler**: Обработка и маппинг ошибок
- **Tracing Middleware**: OpenTelemetry трассировка

#### Service Layer:
- **Review Service**: Основная бизнес-логика отзывов
- **Rating Service**: Управление агрегированными рейтингами
- **Moderation Service**: Модерация пользовательского контента
- **Validation Service**: Валидация входных данных

#### Repository Layer:
- **Review Repository**: CRUD операции для отзывов
- **Rating Repository**: Операции с агрегированными рейтингами
- **Cache Repository**: Операции кеширования

#### Model Layer:
- **Review Model**: Основная модель отзыва
- **OfferRating Model**: Модель агрегированного рейтинга
- **Input Types**: GraphQL входные типы
- **Connection Types**: Типы для пагинации

### 4. Code Diagram (Диаграмма кода)
**Файл**: `C4_ARCHITECTURE_CODE.puml`

Описывает конкретную реализацию на уровне кода:

#### Main Module (src/main.rs):
- **HTTP Server**: Настройка Axum сервера с middleware
- **Schema Builder**: Построение GraphQL схемы с async-graphql
- **Application State**: Инъекция зависимостей (database pool, cache, services)

#### Models Module (src/models/):
- **Review Struct**: Основная структура отзыва с SQLx и GraphQL интеграцией
- **OfferRating Struct**: Структура агрегированного рейтинга
- **Input Structs**: CreateReviewInput, UpdateReviewInput
- **Connection Structs**: ReviewConnection, ReviewEdge для пагинации

#### Resolvers Module (src/resolvers/):
- **Query Implementation**: Реализация GraphQL Query резолверов
- **Mutation Implementation**: Реализация GraphQL Mutation резолверов
- **Federation Resolvers**: Entity resolvers для федерации

#### Services Module (src/services/):
- **ReviewService Implementation**: Бизнес-логика с валидацией
- **RatingService Implementation**: Управление рейтингами и кешированием
- **Validation Logic**: Функции валидации данных

#### Repository Module (src/repository/):
- **ReviewRepository Trait**: Интерфейс для доступа к данным
- **ReviewRepository Implementation**: SQLx реализация с оптимизированными запросами
- **CacheRepository Implementation**: Redis операции

#### Database Module (src/database.rs):
- **Connection Pool Config**: Настройка SQLx пула соединений
- **Database Migrations**: SQL миграции для создания схемы
- **Health Check**: Проверка состояния базы данных

#### Test Module (tests/):
- **Unit Tests**: Тесты с mockall для изоляции
- **Integration Tests**: Тесты с testcontainers для полной интеграции

### 5. Deployment Diagram (Диаграмма развертывания)
**Файл**: `C4_ARCHITECTURE_DEPLOYMENT.puml`

Описывает физическое развертывание UGC подграфа:

#### Developer Machine:
- **Docker Engine**: Контейнерная платформа
- **UGC Network**: Изолированная сеть для UGC сервиса
- **Data Network**: Сеть для баз данных
- **Monitoring Network**: Сеть для мониторинга

#### UGC Container:
- **UGC Subgraph**: Rust бинарный файл (порт 4001, 256MB RAM, 0.5 CPU)
- **Application Logs**: Структурированные JSON логи с ротацией
- **Configuration**: Environment variables и конфигурационные файлы

#### Data Storage:
- **PostgreSQL Container**: База данных с таблицами reviews и offer_ratings
- **Redis Container**: Кеш для рейтингов и сессий

#### Monitoring:
- **Prometheus Container**: Сбор метрик UGC сервиса
- **Jaeger Container**: Трассировка GraphQL запросов

#### Host File System:
- **UGC Workspace**: Исходный код Rust
- **Docker Configuration**: Dockerfile и docker-compose настройки
- **Development Tools**: Cargo инструменты и скрипты автоматизации

## Ключевые архитектурные решения Task 2

### 1. Слоистая архитектура
- **Resolver Layer**: GraphQL интерфейс
- **Service Layer**: Бизнес-логика и валидация
- **Repository Layer**: Доступ к данным
- **Model Layer**: Типизированные структуры данных

### 2. Федеративная интеграция
- **@key директивы** для основных типов
- **@extends директивы** для расширения внешних типов
- **Entity resolvers** для федеративных ссылок
- **Reference resolvers** для получения связанных данных

### 3. Производительность и кеширование
- **Connection pooling** для PostgreSQL
- **Redis кеширование** агрегированных рейтингов
- **Оптимизированные SQL запросы** с индексами
- **Batch операции** для обновления рейтингов

### 4. Observability
- **Structured logging** с контекстом запросов
- **Prometheus metrics** для производительности и бизнес-логики
- **Jaeger tracing** для отладки федеративных запросов
- **Health checks** для мониторинга состояния

### 5. Безопасность и валидация
- **JWT аутентификация** через middleware
- **Role-based авторизация** на уровне резолверов
- **Input валидация** на всех уровнях
- **Rate limiting** для предотвращения злоупотреблений

### 6. Тестируемость
- **Trait-based архитектура** для dependency injection
- **Mock implementations** для unit тестов
- **Integration tests** с testcontainers
- **Contract tests** для федеративной совместимости

## Преимущества архитектуры

1. **Модульность**: Четкое разделение ответственности между слоями
2. **Тестируемость**: Легкое тестирование через dependency injection
3. **Производительность**: Оптимизированные запросы и кеширование
4. **Масштабируемость**: Готовность к горизонтальному масштабированию
5. **Наблюдаемость**: Полная телеметрия и мониторинг
6. **Безопасность**: Многоуровневая защита и валидация

## Следующие шаги

После завершения Task 2 UGC подграф готов для:
- Интеграции с другими подграфами (Users, Offers)
- Расширения функциональности (комментарии, лайки)
- Добавления продвинутых возможностей модерации
- Оптимизации производительности под нагрузкой
- Развертывания в продакшн среде

## Использование диаграмм

Для просмотра диаграмм используйте:
- **PlantUML плагины** в IDE (VS Code, IntelliJ)
- **Онлайн PlantUML сервер**: http://www.plantuml.com/plantuml/
- **Локальный PlantUML**: `java -jar plantuml.jar *.puml`

Все диаграммы автоматически обновляются при изменении `.puml` файлов и отражают текущее состояние архитектуры UGC подграфа.