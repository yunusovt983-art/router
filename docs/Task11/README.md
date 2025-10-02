# Task 11: Subgraph Stubs Creation - C4 Architecture Documentation

## Обзор

Данная документация содержит архитектурные диаграммы C4 для Task 11 - создания заглушек для других подграфов (Users и Offers) в рамках Auto.ru GraphQL Federation проекта. Архитектура описывает полную систему заглушек подграфов с федеративной интеграцией.

## Структура документации

### 1. Context Diagram (Контекстная диаграмма)
**Файл:** `C4_ARCHITECTURE_CONTEXT.puml`

Показывает высокоуровневое представление системы заглушек подграфов и их взаимодействие с участниками:
- Frontend разработчики
- Backend разработчики
- QA инженеры
- Федеративная экосистема Auto.ru

### 2. Container Diagram (Диаграмма контейнеров)
**Файл:** `C4_ARCHITECTURE_CONTAINER.puml`

Детализирует основные контейнеры системы заглушек:
- Federation Gateway Layer (Apollo Router)
- Users Subgraph (Stub) - заглушка пользователей
- Offers Subgraph (Stub) - заглушка объявлений
- Mock Data Management - управление моковыми данными
- Federation Testing Layer - тестирование федерации

### 3. Component Diagram (Диаграмма компонентов)
**Файл:** `C4_ARCHITECTURE_COMPONENT.puml`

Показывает детальную структуру компонентов внутри каждого контейнера:
- GraphQL Layer компоненты (схемы, резолверы)
- Service Layer (бизнес-логика, репозитории)
- Data Layer (моковые данные, генераторы)
- Federation Support (директивы, entity resolvers)
- Testing Support (тестовые клиенты, валидаторы)

### 4. Deployment Diagram (Диаграмма развертывания)
**Файл:** `C4_ARCHITECTURE_DEPLOYMENT.puml`

Описывает физическое развертывание заглушек подграфов:
- Local Development Environment (Docker Compose)
- CI/CD Environment (GitHub Actions)
- Staging Environment (AWS EKS)
- Monitoring Infrastructure (Prometheus, Grafana)

### 5. Code Diagram (Диаграмма кода)
**Файл:** `C4_ARCHITECTURE_CODE.puml`

Показывает конкретную реализацию ключевых компонентов на Rust:
- Users Subgraph Implementation
- Offers Subgraph Implementation
- Federation Integration Implementation
- Testing Implementation

## Ключевые особенности архитектуры Task 11

### Технологический стек
- **Язык:** Rust
- **GraphQL:** async-graphql с federation поддержкой
- **HTTP Server:** Axum
- **Federation:** Apollo Federation v2
- **Контейнеризация:** Docker + Docker Compose
- **Тестирование:** Rust test framework + GraphQL testing
- **CI/CD:** GitHub Actions

### Принципы архитектуры

1. **Federation-First Design** - все подграфы спроектированы с учетом федерации
2. **Mock Data Consistency** - согласованные моковые данные между подграфами
3. **Stub Simplicity** - простая реализация для быстрого прототипирования
4. **Testing Integration** - полная интеграция с тестовой инфраструктурой
5. **Development Experience** - удобство локальной разработки

### Созданные подграфы

#### 1. Users Subgraph (Порт 4002)
**Назначение:** Заглушка для управления пользователями
**Функциональность:**
- GraphQL API для пользователей
- Моковые данные российских пользователей
- Federation entity resolver для User
- Health check и JWKS endpoints

**Ключевые типы:**
```graphql
type User @key(fields: "id") {
  id: ID!
  name: String!
  email: String!
  phone: String
  createdAt: String!
  updatedAt: String!
}
```

#### 2. Offers Subgraph (Порт 4004)
**Назначение:** Заглушка для объявлений о продаже автомобилей
**Функциональность:**
- GraphQL API для объявлений и автомобилей
- CRUD операции с фильтрацией
- Связи с Users подграфом через федерацию
- Моковые данные автомобильного рынка

**Ключевые типы:**
```graphql
type Offer @key(fields: "id") {
  id: ID!
  title: String!
  price: Int!
  sellerId: ID! # Reference to User
  car: Car!
  location: String!
  status: OfferStatus!
}

type Car {
  make: String!
  model: String!
  year: Int!
  mileage: Int
  fuelType: FuelType!
  transmission: TransmissionType!
}
```

### Федеративная интеграция

#### Entity Resolution
- **User Entity:** Разрешается по ID через Users подграф
- **Offer Entity:** Разрешается по ID через Offers подграф
- **Cross-Subgraph Queries:** Поддержка запросов между подграфами

#### Federation Directives
- `@key(fields: "id")` - ключевые поля для entity resolution
- `@extends` - расширение типов из других подграфов
- `@external` - внешние поля из других подграфов
- `@requires` - зависимости между полями

### Моковые данные

#### Users Mock Data
- Реалистичные российские имена
- Корректные email адреса
- Российские номера телефонов
- Временные метки в ISO формате

#### Offers Mock Data
- Популярные автомобильные марки и модели
- Реалистичные цены в рублях
- Технические характеристики автомобилей
- Географические данные (города России)

### Тестирование

#### Unit Tests
- Тестирование GraphQL резолверов
- Валидация моковых данных
- Бизнес-логика сервисов
- Error handling

#### Integration Tests
- Федеративные запросы между подграфами
- Entity resolution тестирование
- Cross-subgraph scenarios
- Performance testing

#### Contract Tests
- API совместимость между подграфами
- Schema evolution testing
- Breaking changes detection
- Consumer-driven testing

## Связь с основной системой

### Apollo Router Integration
- Автоматическая композиция схем всех подграфов
- Query planning и execution
- Request routing к соответствующим подграфам
- Response aggregation

### UGC Subgraph Integration
- Федеративные связи Review -> User
- Федеративные связи Review -> Offer
- Cross-subgraph queries для полных данных
- Consistent entity resolution

### Development Workflow
1. **Local Development:** Docker Compose с всеми подграфами
2. **Schema Validation:** Автоматическая проверка федеративной композиции
3. **Testing:** Comprehensive test suite для всех уровней
4. **CI/CD:** Автоматическая сборка и тестирование

## Дальнейшее развитие

### Планируемые улучшения
1. **Database Integration** - замена in-memory storage на PostgreSQL
2. **Authentication** - полная интеграция с JWT аутентификацией
3. **Caching** - добавление Redis кеширования
4. **Monitoring** - расширенная телеметрия и метрики

### Миграция к Production
1. **Data Migration** - перенос с моков на реальные данные
2. **Performance Optimization** - оптимизация запросов и кеширования
3. **Security Hardening** - усиление безопасности API
4. **Scalability** - горизонтальное масштабирование

## Использование диаграмм

Для просмотра диаграмм используйте PlantUML:

1. Установите PlantUML
2. Откройте файлы `.puml` в поддерживаемом редакторе
3. Или используйте онлайн PlantUML сервер

## Связь с Task 11

Эта архитектура напрямую соответствует требованиям Task 11:
- ✅ 11.1 Реализован Users подграф (заглушка)
- ✅ 11.2 Реализован Offers подграф (заглушка)
- ✅ Федеративные директивы для интеграции
- ✅ Моковые данные и простые резолверы
- ✅ Связи с UGC подграфом через федерацию