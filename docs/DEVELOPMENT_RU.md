# Руководство по разработке

Это руководство предоставляет comprehensive информацию для разработчиков, работающих над проектом Auto.ru GraphQL Federation.

## 📋 Содержание

- [Начало работы](#начало-работы)
- [Среда разработки](#среда-разработки)
- [Структура проекта](#структура-проекта)
- [Рабочий процесс разработки](#рабочий-процесс-разработки)
- [Добавление новых функций](#добавление-новых-функций)
- [Стратегия тестирования](#стратегия-тестирования)
- [Качество кода](#качество-кода)
- [Отладка](#отладка)
- [Оптимизация производительности](#оптимизация-производительности)
- [Лучшие практики](#лучшие-практики)

## Начало работы

### Предварительные требования

- **Rust**: 1.75+ (установка через [rustup](https://rustup.rs/))
- **Docker**: 20.10+ с Docker Compose 2.0+
- **Node.js**: 18+ (для инструментов и валидации схем)
- **Git**: Последняя версия
- **IDE**: VS Code с расширением Rust Analyzer (рекомендуется)

### Первоначальная настройка

```bash
# Клонирование репозитория
git clone <repository-url>
cd auto-ru-graphql-federation

# Установка Rust toolchain и компонентов
rustup component add rustfmt clippy

# Установка инструментов разработки
cargo install cargo-watch cargo-audit sqlx-cli

# Настройка среды разработки
make setup

# Проверка установки
make check
```

### Конфигурация IDE

#### Расширения VS Code
- **rust-analyzer**: Языковой сервер Rust
- **GraphQL**: Подсветка синтаксиса GraphQL
- **Docker**: Поддержка Docker
- **YAML**: Поддержка языка YAML
- **GitLens**: Интеграция с Git

#### Настройки VS Code (.vscode/settings.json)
```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.inlayHints.enable": true,
  "files.watcherExclude": {
    "**/target/**": true
  },
  "graphql.useSchemaFileForIntrospection": true
}
```

## Среда разработки

### Режимы окружения

#### 1. Полная разработка в Docker
```bash
# Запуск всего в Docker
make start-dev

# Просмотр логов
make logs-ugc
make logs-router
```

#### 2. Гибридная разработка (Рекомендуется)
```bash
# Запуск инфраструктуры в Docker
make start-infra

# Локальный запуск подграфов для горячей перезагрузки
make dev-ugc    # Терминал 1
make dev-users  # Терминал 2
make dev-offers # Терминал 3

# Запуск роутера
make start-router
```

#### 3. Локальная разработка
```bash
# Ручной запуск баз данных
docker-compose up -d postgres redis elasticsearch

# Установка переменных окружения
export DATABASE_URL="postgresql://postgres:password@localhost:5432/ugc_db"
export REDIS_URL="redis://localhost:6379"

# Запуск сервисов
cd ugc-subgraph && cargo run
```

### Переменные окружения

Создайте файл `.env` в корне проекта:

```bash
# Подключения к базам данных
UGC_DATABASE_URL=postgresql://postgres:password@localhost:5432/ugc_db
USERS_DATABASE_URL=postgresql://postgres:password@localhost:5433/users_db
OFFERS_DATABASE_URL=postgresql://postgres:password@localhost:5434/offers_db
CATALOG_DATABASE_URL=postgresql://postgres:password@localhost:5435/catalog_db

# Внешние сервисы
REDIS_URL=redis://localhost:6379
ELASTICSEARCH_URL=http://localhost:9200

# Безопасность
JWT_SECRET=dev-secret-key-change-in-production

# Логирование
RUST_LOG=debug
RUST_BACKTRACE=1

# Флаги разработки
ENVIRONMENT=development
```

## Структура проекта

```
auto-ru-graphql-federation/
├── Cargo.toml                 # Конфигурация workspace
├── Makefile                   # Команды разработки
├── docker-compose.yml         # Инфраструктура разработки
├── router.yaml               # Конфигурация Apollo Router
├── .github/workflows/        # CI/CD пайплайны
├── docs/                     # Документация
├── scripts/                  # Утилитарные скрипты
├── schemas/                  # GraphQL схемы
│
├── ugc-subgraph/             # Сервис пользовательского контента
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── src/
│   │   ├── main.rs           # Точка входа приложения
│   │   ├── config.rs         # Управление конфигурацией
│   │   ├── database.rs       # Подключение к базе данных
│   │   ├── error.rs          # Типы ошибок
│   │   ├── graphql/          # GraphQL схема и резолверы
│   │   ├── models/           # Модели данных
│   │   ├── repository/       # Слой доступа к данным
│   │   └── service/          # Бизнес-логика
│   ├── migrations/           # Миграции базы данных
│   └── tests/               # Тестовые файлы
│
├── users-subgraph/          # Сервис управления пользователями
├── offers-subgraph/         # Сервис объявлений автомобилей
├── catalog-subgraph/        # Сервис каталога автомобилей
└── search-subgraph/         # Сервис поиска
```

### Архитектура подграфа

Каждый подграф следует слоистой архитектуре:

```
src/
├── main.rs                   # Инициализация приложения
├── config.rs                 # Конфигурация
├── database.rs               # Настройка базы данных
├── error.rs                  # Обработка ошибок
├── health.rs                 # Проверки здоровья
├── telemetry.rs             # Наблюдаемость
│
├── graphql/
│   ├── mod.rs               # GraphQL схема
│   ├── query.rs             # Query резолверы
│   ├── mutation.rs          # Mutation резолверы
│   ├── subscription.rs      # Subscription резолверы
│   └── types.rs             # GraphQL типы
│
├── models/
│   ├── mod.rs               # Модели данных
│   ├── review.rs            # Модель отзыва
│   └── user.rs              # Модель пользователя
│
├── repository/
│   ├── mod.rs               # Трейт репозитория
│   ├── review.rs            # Репозиторий отзывов
│   └── user.rs              # Репозиторий пользователей
│
└── service/
    ├── mod.rs               # Слой сервисов
    ├── review.rs            # Сервис отзывов
    └── auth.rs              # Сервис аутентификации
```

## Рабочий процесс разработки

### Ежедневная разработка

```bash
# 1. Запуск среды разработки
make start-infra

# 2. Запуск подграфа с горячей перезагрузкой
cd ugc-subgraph
cargo watch -x run

# 3. Внесение изменений и тестирование
# Файлы автоматически перекомпилируются при сохранении

# 4. Запуск тестов
cargo test

# 5. Проверка качества кода
make clippy
make fmt
```

### Разработка функций

```bash
# 1. Создание ветки функции
git checkout -b feature/new-review-system

# 2. Реализация изменений
# ... внесите ваши изменения ...

# 3. Запуск comprehensive тестов
make test
make check

# 4. Обновление документации
# ... обновите соответствующую документацию ...

# 5. Коммит и push
git add .
git commit -m "feat: implement new review system"
git push origin feature/new-review-system

# 6. Создание pull request
```

### Миграции базы данных

```bash
# Создание новой миграции
cd ugc-subgraph
sqlx migrate add create_reviews_table

# Редактирование файла миграции
# migrations/001_create_reviews_table.sql

# Запуск миграции
sqlx migrate run --database-url $UGC_DATABASE_URL

# Откат миграции (при необходимости)
sqlx migrate revert --database-url $UGC_DATABASE_URL
```

## Добавление новых функций

### Добавление нового подграфа

1. **Создание директории подграфа**:
```bash
mkdir new-subgraph
cd new-subgraph
```

2. **Создание Cargo.toml**:
```toml
[package]
name = "new-subgraph"
version = "0.1.0"
edition = "2021"

[dependencies]
async-graphql = "6.0"
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono"] }
# ... другие зависимости
```

3. **Добавление в workspace** (корневой Cargo.toml):
```toml
[workspace]
members = [
    "ugc-subgraph",
    "users-subgraph",
    "new-subgraph",  # Добавить здесь
]
```

4. **Создание базовой структуры**:
```bash
mkdir -p src/{graphql,models,repository,service}
touch src/main.rs src/config.rs src/database.rs
```

5. **Реализация GraphQL схемы**:
```rust
// src/graphql/mod.rs
use async_graphql::{Schema, Object, Result};

pub struct Query;

#[Object]
impl Query {
    async fn hello(&self) -> &str {
        "Привет от нового подграфа!"
    }
}

pub type AppSchema = Schema<Query, EmptyMutation, EmptySubscription>;
```

6. **Добавление в Docker Compose**:
```yaml
new-subgraph:
  build:
    context: .
    dockerfile: new-subgraph/Dockerfile
  ports:
    - "4006:4006"
  environment:
    - DATABASE_URL=postgresql://postgres:password@postgres:5432/new_db
```

7. **Обновление конфигурации роутера**:
```yaml
# router.yaml
subgraphs:
  new:
    routing_url: http://new-subgraph:4006/graphql
```

### Добавление нового GraphQL типа

1. **Определение модели**:
```rust
// src/models/product.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub price: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

2. **Создание GraphQL типа**:
```rust
// src/graphql/types.rs
use async_graphql::{Object, ID};
use crate::models::Product as ProductModel;

#[Object]
impl ProductModel {
    async fn id(&self) -> ID {
        ID(self.id.to_string())
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    async fn price(&self) -> i32 {
        self.price
    }
}
```

3. **Добавление методов репозитория**:
```rust
// src/repository/product.rs
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;
use crate::models::Product;

#[async_trait]
pub trait ProductRepository {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Product>>;
    async fn create(&self, product: CreateProductInput) -> Result<Product>;
}

pub struct PostgresProductRepository {
    pool: PgPool,
}

#[async_trait]
impl ProductRepository for PostgresProductRepository {
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Product>> {
        let product = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(product)
    }
}
```

4. **Добавление резолверов**:
```rust
// src/graphql/query.rs
#[Object]
impl Query {
    async fn product(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Product>> {
        let repository = ctx.data::<Arc<dyn ProductRepository>>()?;
        let product_id = Uuid::parse_str(&id)?;
        repository.get_by_id(product_id).await
    }
}
```

### Добавление федеративных директив

1. **Сущность с ключом**:
```rust
use async_graphql::{Object, ID};

#[derive(async_graphql::SimpleObject)]
#[graphql(complex)]
pub struct Product {
    #[graphql(external)]
    pub id: ID,
    pub name: String,
}

#[ComplexObject]
impl Product {
    #[graphql(entity)]
    async fn find_by_id(ctx: &Context<'_>, id: ID) -> Result<Option<Product>> {
        // Реализация reference резолвера
    }
}
```

2. **Расширение внешних типов**:
```rust
#[derive(async_graphql::SimpleObject)]
#[graphql(extends)]
pub struct User {
    #[graphql(external)]
    pub id: ID,
    
    // Новые поля, добавленные этим подграфом
    pub products: Vec<Product>,
}
```

## Стратегия тестирования

### Модульные тесты

```rust
// src/service/review.rs
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_create_review() {
        let mut mock_repo = MockReviewRepository::new();
        mock_repo
            .expect_create()
            .with(eq(CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "Отлично!".to_string(),
            }))
            .times(1)
            .returning(|_| Ok(Review { /* ... */ }));

        let service = ReviewService::new(Arc::new(mock_repo));
        let result = service.create_review(input, user_id).await;
        
        assert!(result.is_ok());
    }
}
```

### Интеграционные тесты

```rust
// tests/integration_test.rs
use testcontainers::*;
use sqlx::PgPool;

#[tokio::test]
async fn test_review_creation_flow() {
    let docker = clients::Cli::default();
    let postgres = docker.run(images::postgres::Postgres::default());
    
    let database_url = format!(
        "postgresql://postgres:postgres@localhost:{}/postgres",
        postgres.get_host_port_ipv4(5432)
    );
    
    let pool = PgPool::connect(&database_url).await.unwrap();
    
    // Запуск миграций
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    
    // Тестирование сервиса
    let service = ReviewService::new(pool);
    let result = service.create_review(input, user_id).await;
    
    assert!(result.is_ok());
}
```

### GraphQL тесты

```rust
// tests/graphql_test.rs
use async_graphql::*;

#[tokio::test]
async fn test_review_query() {
    let schema = create_test_schema().await;
    
    let query = r#"
        query GetReview($id: ID!) {
            review(id: $id) {
                id
                rating
                text
                author {
                    name
                }
            }
        }
    "#;
    
    let result = schema
        .execute(Request::new(query).variables(variables! {
            "id": "test-review-id"
        }))
        .await;
    
    assert!(result.errors.is_empty());
    assert!(result.data.is_some());
}
```

### End-to-End тесты

```bash
# tests/e2e/test_federation.sh
#!/bin/bash

# Запуск сервисов
docker-compose up -d

# Ожидание готовности сервисов
./scripts/wait-for-services.sh

# Выполнение GraphQL запросов
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "{ offers(first: 1) { edges { node { id reviews(first: 1) { edges { node { rating } } } } } } }"
  }' | jq '.errors // empty' | grep -q null

echo "E2E тесты пройдены!"
```## Ка
чество кода

### Форматирование

```bash
# Форматирование всего кода
cargo fmt

# Проверка форматирования
cargo fmt -- --check
```

### Линтинг

```bash
# Запуск clippy
cargo clippy -- -D warnings

# Запуск clippy со всеми функциями
cargo clippy --all-features -- -D warnings
```

### Аудит безопасности

```bash
# Установка cargo-audit
cargo install cargo-audit

# Запуск аудита безопасности
cargo audit

# Проверка на известные уязвимости
cargo audit --deny warnings
```

### Покрытие кода

```bash
# Установка tarpaulin
cargo install cargo-tarpaulin

# Генерация отчета о покрытии
cargo tarpaulin --out Html --output-dir coverage/
```

## Отладка

### Логирование

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self), fields(user_id = %user_id))]
pub async fn create_review(&self, input: CreateReviewInput, user_id: Uuid) -> Result<Review> {
    info!("Создание отзыва для объявления {}", input.offer_id);
    
    // Реализация
    debug!("Валидация входных данных отзыва");
    
    match self.repository.create(input, user_id).await {
        Ok(review) => {
            info!("Отзыв успешно создан: {}", review.id);
            Ok(review)
        }
        Err(e) => {
            error!("Не удалось создать отзыв: {}", e);
            Err(e)
        }
    }
}
```

### Конфигурация отладки

```bash
# Включение debug логирования
export RUST_LOG=debug

# Включение backtraces
export RUST_BACKTRACE=1

# Включение полных backtraces
export RUST_BACKTRACE=full
```

### Отладка базы данных

```rust
// Включение логирования SQL запросов
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

// Логирование всех SQL запросов
sqlx::query!("SELECT * FROM reviews WHERE id = $1", review_id)
    .fetch_one(&pool)
    .await?;
```

### Отладка GraphQL

```rust
use async_graphql::{Schema, extensions::Logger};

let schema = Schema::build(Query, Mutation, Subscription)
    .extension(Logger) // Добавление логирования запросов
    .finish();
```

## Оптимизация производительности

### Оптимизация базы данных

1. **Использование подготовленных запросов**:
```rust
// Хорошо: Использует подготовленный запрос
sqlx::query_as!(Review, "SELECT * FROM reviews WHERE offer_id = $1", offer_id)
    .fetch_all(&pool)
    .await?;

// Избегайте: Динамическое построение запросов
let query = format!("SELECT * FROM reviews WHERE offer_id = '{}'", offer_id);
```

2. **Добавление индексов базы данных**:
```sql
-- migrations/003_add_indexes.sql
CREATE INDEX idx_reviews_offer_id ON reviews(offer_id);
CREATE INDEX idx_reviews_author_id ON reviews(author_id);
CREATE INDEX idx_reviews_created_at ON reviews(created_at DESC);
```

3. **Использование пула соединений**:
```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .acquire_timeout(Duration::from_secs(30))
    .connect(&database_url)
    .await?;
```

### Оптимизация GraphQL

1. **Реализация DataLoader**:
```rust
use async_graphql::dataloader::*;

pub struct UserLoader {
    pool: PgPool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for UserLoader {
    type Value = User;
    type Error = sqlx::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        let users = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ANY($1)",
            keys
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(users.into_iter().map(|user| (user.id, user)).collect())
    }
}
```

2. **Ограничение сложности запросов**:
```rust
use async_graphql::extensions::analyzer::*;

let schema = Schema::build(Query, Mutation, Subscription)
    .extension(Analyzer::new().depth_limit(10).complexity_limit(1000))
    .finish();
```

### Кеширование

1. **Redis кеширование**:
```rust
use redis::AsyncCommands;

pub struct CacheService {
    client: redis::Client,
}

impl CacheService {
    pub async fn get_review(&self, id: Uuid) -> Result<Option<Review>> {
        let mut conn = self.client.get_async_connection().await?;
        let cached: Option<String> = conn.get(format!("review:{}", id)).await?;
        
        match cached {
            Some(data) => Ok(Some(serde_json::from_str(&data)?)),
            None => Ok(None),
        }
    }

    pub async fn set_review(&self, review: &Review) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        let data = serde_json::to_string(review)?;
        conn.set_ex(format!("review:{}", review.id), data, 3600).await?;
        Ok(())
    }
}
```

## Лучшие практики

### Обработка ошибок

1. **Использование типизированных ошибок**:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReviewError {
    #[error("Отзыв не найден: {id}")]
    NotFound { id: Uuid },
    
    #[error("Неавторизованный доступ к отзыву {id}")]
    Unauthorized { id: Uuid },
    
    #[error("Ошибка базы данных: {0}")]
    Database(#[from] sqlx::Error),
}
```

2. **Преобразование в GraphQL ошибки**:
```rust
impl From<ReviewError> for async_graphql::Error {
    fn from(err: ReviewError) -> Self {
        match err {
            ReviewError::NotFound { id } => {
                async_graphql::Error::new("Отзыв не найден")
                    .extend_with(|_, e| e.set("code", "NOT_FOUND"))
                    .extend_with(|_, e| e.set("reviewId", id.to_string()))
            }
            // ... другие варианты
        }
    }
}
```

### Безопасность

1. **Валидация входных данных**:
```rust
use validator::{Validate, ValidationError};

#[derive(Validate)]
pub struct CreateReviewInput {
    pub offer_id: Uuid,
    
    #[validate(range(min = 1, max = 5))]
    pub rating: i32,
    
    #[validate(length(min = 10, max = 1000))]
    pub text: String,
}
```

2. **Гарды авторизации**:
```rust
use async_graphql::{Guard, Context, Result};

pub struct RequireAuth;

#[async_trait::async_trait]
impl Guard for RequireAuth {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        match ctx.data_opt::<UserContext>() {
            Some(_) => Ok(()),
            None => Err("Требуется аутентификация".into()),
        }
    }
}
```

### Конфигурация

1. **Конфигурация на основе окружения**:
```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::from_env()
    }
}
```

### Документация

1. **Документация GraphQL схемы**:
```rust
#[Object]
impl Query {
    /// Получить отзыв по ID
    /// 
    /// Возвращает отзыв, если найден и доступен текущему пользователю.
    /// Требует аутентификации для приватных отзывов.
    async fn review(
        &self,
        ctx: &Context<'_>,
        /// Уникальный идентификатор отзыва
        id: ID,
    ) -> Result<Option<Review>> {
        // Реализация
    }
}
```

2. **Документация кода**:
```rust
/// Сервис для управления пользовательскими отзывами и рейтингами.
/// 
/// Этот сервис обрабатывает CRUD операции для отзывов, включая
/// валидацию, авторизацию и управление кешем.
pub struct ReviewService {
    repository: Arc<dyn ReviewRepository>,
    cache: Arc<CacheService>,
}

impl ReviewService {
    /// Создает новый отзыв для указанного объявления.
    /// 
    /// # Аргументы
    /// 
    /// * `input` - Данные отзыва для создания
    /// * `user_id` - ID пользователя, создающего отзыв
    /// 
    /// # Возвращает
    /// 
    /// Возвращает созданный отзыв или ошибку при неудаче создания.
    /// 
    /// # Ошибки
    /// 
    /// Эта функция вернет ошибку если:
    /// - Пользователь не авторизован для создания отзывов
    /// - Объявление не существует
    /// - Валидация входных данных не прошла
    pub async fn create_review(
        &self,
        input: CreateReviewInput,
        user_id: Uuid,
    ) -> Result<Review, ReviewError> {
        // Реализация
    }
}
```

## 🎯 Дополнительные инструменты разработки

### Полезные команды Makefile

```bash
# Быстрый старт разработки
make dev-start          # Запуск всей инфраструктуры для разработки
make dev-stop           # Остановка всех сервисов
make dev-restart        # Перезапуск сервисов

# Тестирование
make test-unit          # Запуск только unit тестов
make test-integration   # Запуск интеграционных тестов
make test-e2e          # Запуск end-to-end тестов
make test-all          # Запуск всех тестов

# Качество кода
make lint              # Проверка линтером
make format            # Форматирование кода
make audit             # Аудит безопасности
make coverage          # Генерация отчета покрытия

# База данных
make db-migrate        # Применение миграций
make db-rollback       # Откат последней миграции
make db-reset          # Сброс базы данных
make db-seed           # Заполнение тестовыми данными
```

### Горячие клавиши VS Code

```json
// .vscode/keybindings.json
[
  {
    "key": "ctrl+shift+t",
    "command": "workbench.action.terminal.new",
    "when": "!terminalFocus"
  },
  {
    "key": "ctrl+shift+r",
    "command": "rust-analyzer.run",
    "when": "editorTextFocus && editorLangId == rust"
  },
  {
    "key": "ctrl+shift+d",
    "command": "rust-analyzer.debug",
    "when": "editorTextFocus && editorLangId == rust"
  }
]
```

### Сниппеты кода

```json
// .vscode/snippets/rust.json
{
  "GraphQL Object": {
    "prefix": "gql-object",
    "body": [
      "#[Object]",
      "impl ${1:TypeName} {",
      "    async fn ${2:field_name}(&self) -> ${3:ReturnType} {",
      "        ${4:// implementation}",
      "    }",
      "}"
    ],
    "description": "GraphQL Object implementation"
  },
  "Async Test": {
    "prefix": "async-test",
    "body": [
      "#[tokio::test]",
      "async fn ${1:test_name}() {",
      "    ${2:// test implementation}",
      "    assert!(${3:condition});",
      "}"
    ],
    "description": "Async test function"
  }
}
```

---

## 📝 Заключение

Это руководство по разработке предоставляет comprehensive покрытие процесса разработки, от первоначальной настройки до продвинутых техник оптимизации, обеспечивая эффективное участие разработчиков в проекте.

### 🎯 Ключевые принципы разработки:

- **Модульность** - каждый подграф независим и может разрабатываться отдельно
- **Типобезопасность** - использование Rust и GraphQL для end-to-end типобезопасности
- **Тестируемость** - comprehensive стратегия тестирования на всех уровнях
- **Производительность** - оптимизация на уровне базы данных, кеширования и GraphQL
- **Качество кода** - автоматизированные проверки и best practices
- **Документированность** - подробная документация кода и API

### 🚀 Следующие шаги:

1. **Настройте среду разработки** согласно инструкциям
2. **Изучите структуру проекта** и архитектуру подграфов
3. **Запустите тесты** для проверки работоспособности
4. **Начните с простых изменений** для знакомства с кодовой базой
5. **Следуйте best practices** при разработке новых функций

Удачной разработки! 🎉