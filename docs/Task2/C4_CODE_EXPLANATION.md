# C4 Code Diagram - Подробное объяснение Task 2

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_CODE.puml`

Диаграмма кода показывает детальную структуру UGC подграфа на уровне модулей, файлов и их взаимосвязей в Rust проекте.

## Структура проекта и реализация

### 1. Main Module (src/main.rs)

#### HTTP Server
```plantuml
Component(http_server, "HTTP Server", "Axum", "HTTP сервер настройка...")
```

**Архитектурная роль**: Точка входа приложения и настройка HTTP сервера

**Реализация**:
```rust
// crates/ugc-subgraph/src/main.rs
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация логирования
    shared::tracing::init_tracing("ugc-subgraph")?;
    
    // Загрузка конфигурации
    let config = UgcConfig::from_env()?;
    
    // Инициализация зависимостей
    let app_state = initialize_app_state(&config).await?;
    
    // Создание HTTP роутера
    let app = create_router(app_state);
    
    // Запуск сервера
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Starting UGC subgraph on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    
    Ok(())
}
```

Продолжение следует...##
## Schema Builder
```plantuml
Component(schema_builder, "Schema Builder", "async-graphql", "Построение GraphQL схемы...")
```

**Реализация построения схемы**:
```rust
// crates/ugc-subgraph/src/schema.rs
use async_graphql::{Schema, EmptySubscription};
use crate::resolvers::{Query, Mutation};

pub type UgcSchema = Schema<Query, Mutation, EmptySubscription>;

pub fn create_schema(app_state: &AppState) -> UgcSchema {
    Schema::build(Query, Mutation, EmptySubscription)
        // Внедрение зависимостей
        .data(app_state.database_pool.clone())
        .data(app_state.review_service.clone())
        .data(app_state.rating_service.clone())
        .data(app_state.moderation_service.clone())
        .data(app_state.cache_service.clone())
        .data(app_state.jwt_service.clone())
        
        // Настройка федерации
        .enable_federation()
        
        // Настройка валидации
        .limit_depth(10)
        .limit_complexity(1000)
        
        // Расширения
        .extension(async_graphql::extensions::Tracing)
        .extension(async_graphql::extensions::Logger)
        
        .finish()
}
```

### 2. Models Module (src/models/)

#### Review Struct
```plantuml
Component(review_struct, "Review Struct", "Rust + SQLx + GraphQL", "struct Review {...}")
```

**Реализация модели отзыва**:
```rust
// crates/ugc-subgraph/src/models/review.rs
use async_graphql::{SimpleObject, ID};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Debug, Clone, SimpleObject, FromRow, Serialize, Deserialize)]
#[graphql(extends)]
pub struct Review {
    #[graphql(key)]
    pub id: ID,
    pub offer_id: ID,
    pub user_id: ID,
    pub rating: i32,
    pub title: Option<String>,
    pub text: String,
    pub is_moderated: bool,
    pub moderated_by: Option<ID>,
    pub moderated_at: Option<DateTime<Utc>>,
    pub helpful_count: i32,
    pub not_helpful_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Review {
    /// Проверка, является ли отзыв активным
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none() && self.is_moderated
    }
    
    /// Вычисление полезности отзыва
    pub fn helpfulness_ratio(&self) -> f64 {
        let total = self.helpful_count + self.not_helpful_count;
        if total == 0 {
            0.0
        } else {
            self.helpful_count as f64 / total as f64
        }
    }
}
```

#### Input Types
```plantuml
Component(input_structs, "Input Structs", "GraphQL InputObject", "CreateReviewInput {...}")
```

**Реализация входных типов**:
```rust
// crates/ugc-subgraph/src/models/input.rs
use async_graphql::InputObject;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, InputObject, Serialize, Deserialize)]
pub struct CreateReviewInput {
    pub offer_id: ID,
    pub rating: i32,
    pub title: Option<String>,
    pub text: String,
}

impl CreateReviewInput {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Валидация рейтинга
        if !(1..=5).contains(&self.rating) {
            return Err(ValidationError::new("Rating must be between 1 and 5"));
        }
        
        // Валидация текста
        if self.text.len() < 10 {
            return Err(ValidationError::new("Review text too short"));
        }
        
        if self.text.len() > 5000 {
            return Err(ValidationError::new("Review text too long"));
        }
        
        // Валидация заголовка
        if let Some(title) = &self.title {
            if title.len() > 200 {
                return Err(ValidationError::new("Title too long"));
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone, InputObject, Serialize, Deserialize)]
pub struct UpdateReviewInput {
    pub rating: Option<i32>,
    pub title: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, InputObject, Serialize, Deserialize)]
pub struct ReviewFilter {
    pub min_rating: Option<i32>,
    pub max_rating: Option<i32>,
    pub user_id: Option<ID>,
    pub only_moderated: Option<bool>,
    pub sort_by: Option<String>,
}

impl ReviewFilter {
    pub fn cache_key(&self) -> String {
        format!(
            "filter:{}:{}:{}:{}:{}",
            self.min_rating.unwrap_or(0),
            self.max_rating.unwrap_or(5),
            self.user_id.as_deref().unwrap_or(""),
            self.only_moderated.unwrap_or(true),
            self.sort_by.as_deref().unwrap_or("created_at")
        )
    }
}
```

### 3. Resolvers Module (src/resolvers/)

#### Query Implementation
```plantuml
Component(query_impl, "Query Implementation", "#[Object] impl Query", "async fn review(...)")
```

**Реализация Query резолверов**:
```rust
// crates/ugc-subgraph/src/resolvers/query.rs
use async_graphql::{Object, Context, Result, ID};

#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
    /// Получение отзыва по ID
    async fn review(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<Review>> {
        let service = ctx.data::<ReviewService>()?;
        let review_id = ReviewId::from_str(&id)?;
        
        let span = tracing::info_span!("query_review", review_id = %review_id);
        let _enter = span.enter();
        
        match service.get_review_by_id(review_id).await {
            Ok(review) => Ok(Some(review)),
            Err(UgcError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    /// Получение отзывов по объявлению с пагинацией
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        offer_id: ID,
        first: Option<i32>,
        after: Option<String>,
        filter: Option<ReviewFilter>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let offer_id = OfferId::from_str(&offer_id)?;
        
        let args = ConnectionArgs {
            first: first.unwrap_or(10).min(100),
            after,
            ..Default::default()
        };
        
        let filter = filter.unwrap_or_default();
        service.get_reviews_connection(offer_id, args, filter).await
    }
    
    /// Получение агрегированного рейтинга объявления
    async fn offer_rating(
        &self,
        ctx: &Context<'_>,
        offer_id: ID,
    ) -> Result<OfferRating> {
        let service = ctx.data::<RatingService>()?;
        let offer_id = OfferId::from_str(&offer_id)?;
        
        service.get_offer_rating(offer_id).await
    }
}
```

#### Mutation Implementation
```plantuml
Component(mutation_impl, "Mutation Implementation", "#[Object] impl Mutation", "async fn create_review(...)")
```

**Реализация Mutation резолверов**:
```rust
// crates/ugc-subgraph/src/resolvers/mutation.rs
use async_graphql::{Object, Context, Result, ID};
use shared::auth::{RequireAuth, RequireRole, Role};

#[derive(Default)]
pub struct Mutation;

#[Object]
impl Mutation {
    /// Создание нового отзыва
    #[graphql(guard = "RequireAuth")]
    async fn create_review(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> Result<Review> {
        let service = ctx.data::<ReviewService>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        // Валидация входных данных
        input.validate()?;
        
        let review = service.create_review(input, user_context).await?;
        
        // Метрики
        shared::metrics::REVIEWS_CREATED_TOTAL.inc();
        
        Ok(review)
    }
    
    /// Модерация отзыва
    #[graphql(guard = "RequireRole(Role::Moderator)")]
    async fn moderate_review(
        &self,
        ctx: &Context<'_>,
        review_id: ID,
        action: ModerationAction,
        reason: Option<String>,
    ) -> Result<Review> {
        let service = ctx.data::<ModerationService>()?;
        let user_context = ctx.data::<UserContext>()?;
        let review_id = ReviewId::from_str(&review_id)?;
        
        service.moderate_review(review_id, action, reason, user_context).await
    }
}
```

### 4. Services Module (src/services/)

#### Review Service Implementation
```plantuml
Component(review_service_impl, "ReviewService Implementation", "Business Logic", "impl ReviewService {...}")
```

**Реализация бизнес-логики**:
```rust
// crates/ugc-subgraph/src/services/review_service.rs
use std::sync::Arc;
use shared::types::{UserId, OfferId, ReviewId};

pub struct ReviewService {
    repository: Arc<dyn ReviewRepositoryTrait>,
    cache_service: Arc<CacheService>,
    validation_service: Arc<ValidationService>,
}

impl ReviewService {
    pub fn new(
        repository: Arc<dyn ReviewRepositoryTrait>,
        cache_service: Arc<CacheService>,
        validation_service: Arc<ValidationService>,
    ) -> Self {
        Self {
            repository,
            cache_service,
            validation_service,
        }
    }
    
    /// Создание нового отзыва
    pub async fn create_review(
        &self,
        input: CreateReviewInput,
        user_context: &UserContext,
    ) -> UgcResult<Review> {
        // Валидация
        self.validation_service.validate_create_review_input(&input)?;
        
        // Проверка дублирования
        if self.repository.has_user_reviewed_offer(
            user_context.user_id,
            input.offer_id,
        ).await? {
            return Err(UgcError::DuplicateResource {
                message: "User has already reviewed this offer".to_string(),
                conflicting_field: Some("user_id, offer_id".to_string()),
            });
        }
        
        // Санитизация текста
        let sanitized_input = CreateReviewInput {
            text: self.validation_service.sanitize_text(&input.text),
            ..input
        };
        
        // Создание отзыва
        let review = self.repository.create_review(
            sanitized_input,
            user_context.user_id,
        ).await?;
        
        // Инвалидация кешей
        self.invalidate_related_caches(review.offer_id, user_context.user_id).await?;
        
        Ok(review)
    }
    
    /// Получение отзывов с пагинацией
    pub async fn get_reviews_connection(
        &self,
        offer_id: OfferId,
        args: ConnectionArgs,
        filter: ReviewFilter,
    ) -> UgcResult<ReviewConnection> {
        // Проверка кеша
        let cache_key = format!(
            "reviews:{}:{}:{}:{}",
            offer_id,
            args.first,
            args.after.as_deref().unwrap_or(""),
            filter.cache_key()
        );
        
        if let Some(cached) = self.cache_service.get(&cache_key).await? {
            return Ok(cached);
        }
        
        // Получение из репозитория
        let (reviews, has_next_page) = self.repository
            .get_reviews_with_pagination(offer_id, &args, &filter)
            .await?;
        
        // Построение connection
        let connection = self.build_review_connection(reviews, args, has_next_page)?;
        
        // Кеширование
        self.cache_service.set_with_ttl(&cache_key, &connection, 300).await?;
        
        Ok(connection)
    }
}
```

### 5. Repository Module (src/repository/)

#### Review Repository Implementation
```plantuml
Component(review_repo_impl, "ReviewRepository Implementation", "SQLx", "impl ReviewRepositoryTrait {...}")
```

**Реализация доступа к данным**:
```rust
// crates/ugc-subgraph/src/repository/review_repository.rs
use sqlx::{PgPool, QueryBuilder, Postgres};

pub struct ReviewRepository {
    pool: PgPool,
}

impl ReviewRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ReviewRepositoryTrait for ReviewRepository {
    async fn create_review(
        &self,
        input: CreateReviewInput,
        user_id: UserId,
    ) -> UgcResult<Review> {
        let review = sqlx::query_as!(
            Review,
            r#"
            INSERT INTO reviews (
                offer_id, user_id, rating, title, text,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            RETURNING *
            "#,
            input.offer_id.0,
            user_id.0,
            input.rating,
            input.title,
            input.text
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            match &e {
                sqlx::Error::Database(db_err) => {
                    if let Some(constraint) = db_err.constraint() {
                        match constraint {
                            "unique_user_offer_review" => {
                                UgcError::DuplicateResource {
                                    message: "User already reviewed this offer".to_string(),
                                    conflicting_field: Some("user_id, offer_id".to_string()),
                                }
                            }
                            _ => UgcError::from(e),
                        }
                    } else {
                        UgcError::from(e)
                    }
                }
                _ => UgcError::from(e),
            }
        })?;
        
        Ok(review)
    }
    
    async fn get_reviews_with_pagination(
        &self,
        offer_id: OfferId,
        args: &ConnectionArgs,
        filter: &ReviewFilter,
    ) -> UgcResult<(Vec<Review>, bool)> {
        let limit = args.first as i64 + 1;
        let offset = self.calculate_offset(args).await?;
        
        // Динамическое построение запроса
        let mut query_builder = QueryBuilder::<Postgres>::new(
            "SELECT * FROM reviews WHERE offer_id = "
        );
        query_builder.push_bind(offer_id.0);
        query_builder.push(" AND deleted_at IS NULL");
        
        // Применение фильтров
        if filter.only_moderated.unwrap_or(true) {
            query_builder.push(" AND is_moderated = TRUE");
        }
        
        if let Some(min_rating) = filter.min_rating {
            query_builder.push(" AND rating >= ");
            query_builder.push_bind(min_rating);
        }
        
        // Сортировка
        match filter.sort_by.as_deref().unwrap_or("created_at") {
            "rating" => query_builder.push(" ORDER BY rating DESC, created_at DESC"),
            "created_at" => query_builder.push(" ORDER BY created_at DESC"),
            _ => query_builder.push(" ORDER BY created_at DESC"),
        };
        
        query_builder.push(" LIMIT ");
        query_builder.push_bind(limit);
        query_builder.push(" OFFSET ");
        query_builder.push_bind(offset);
        
        let mut reviews = query_builder
            .build_query_as::<Review>()
            .fetch_all(&self.pool)
            .await
            .map_err(UgcError::from)?;
        
        let has_next_page = reviews.len() > args.first;
        if has_next_page {
            reviews.pop();
        }
        
        Ok((reviews, has_next_page))
    }
}
```

### 6. Database Module (src/database.rs)

#### Connection Pool Configuration
```plantuml
Component(pool_config, "Connection Pool Config", "SQLx PgPoolOptions", "PgPoolOptions::new()...")
```

**Реализация пула подключений**:
```rust
// crates/ugc-subgraph/src/database.rs
use sqlx::{PgPool, PgPoolOptions, migrate::MigrateDatabase, Postgres};
use std::time::Duration;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    // Создание базы данных если не существует
    if !Postgres::database_exists(database_url).await.unwrap_or(false) {
        Postgres::create_database(database_url).await?;
        tracing::info!("Database created");
    }
    
    // Настройка пула подключений
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .test_before_acquire(true)
        .connect(database_url)
        .await?;
    
    tracing::info!("Database pool created successfully");
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    tracing::info!("Running database migrations");
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("Database migrations completed");
    Ok(())
}

pub async fn health_check(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}
```

### 7. Test Module (tests/)

#### Unit Tests
```plantuml
Component(unit_tests, "Unit Tests", "Mockall + Tokio", "#[tokio::test]...")
```

**Реализация модульных тестов**:
```rust
// crates/ugc-subgraph/tests/unit/review_service_test.rs
use mockall::predicate::*;
use tokio_test;

#[tokio::test]
async fn test_create_review_success() {
    // Arrange
    let mut mock_repo = MockReviewRepository::new();
    let mut mock_cache = MockCacheService::new();
    let validation_service = Arc::new(ValidationService::new());
    
    let input = CreateReviewInput {
        offer_id: OfferId::new(),
        rating: 4,
        title: Some("Great car".to_string()),
        text: "Really enjoyed driving this car".to_string(),
    };
    
    let expected_review = Review {
        id: ReviewId::new(),
        offer_id: input.offer_id,
        user_id: UserId::new(),
        rating: input.rating,
        text: input.text.clone(),
        is_moderated: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        deleted_at: None,
    };
    
    // Mock expectations
    mock_repo
        .expect_has_user_reviewed_offer()
        .with(eq(expected_review.user_id), eq(input.offer_id))
        .times(1)
        .returning(|_, _| Ok(false));
    
    mock_repo
        .expect_create_review()
        .with(eq(input.clone()), eq(expected_review.user_id))
        .times(1)
        .returning(move |_, _| Ok(expected_review.clone()));
    
    mock_cache
        .expect_delete_pattern()
        .times(2)
        .returning(|_| Ok(()));
    
    let service = ReviewService::new(
        Arc::new(mock_repo),
        Arc::new(mock_cache),
        validation_service,
    );
    
    let user_context = UserContext {
        user_id: expected_review.user_id,
        roles: vec![],
        permissions: vec![],
        session_id: SessionId::new(),
    };
    
    // Act
    let result = service.create_review(input, &user_context).await;
    
    // Assert
    assert!(result.is_ok());
    let review = result.unwrap();
    assert_eq!(review.rating, 4);
    assert_eq!(review.text, "Really enjoyed driving this car");
}

#[tokio::test]
async fn test_create_review_duplicate_error() {
    // Arrange
    let mut mock_repo = MockReviewRepository::new();
    let mock_cache = MockCacheService::new();
    let validation_service = Arc::new(ValidationService::new());
    
    mock_repo
        .expect_has_user_reviewed_offer()
        .times(1)
        .returning(|_, _| Ok(true)); // Пользователь уже оставил отзыв
    
    let service = ReviewService::new(
        Arc::new(mock_repo),
        Arc::new(mock_cache),
        validation_service,
    );
    
    let input = CreateReviewInput {
        offer_id: OfferId::new(),
        rating: 4,
        title: None,
        text: "Test review".to_string(),
    };
    
    let user_context = UserContext {
        user_id: UserId::new(),
        roles: vec![],
        permissions: vec![],
        session_id: SessionId::new(),
    };
    
    // Act
    let result = service.create_review(input, &user_context).await;
    
    // Assert
    assert!(result.is_err());
    match result.unwrap_err() {
        UgcError::DuplicateResource { message, .. } => {
            assert!(message.contains("already reviewed"));
        }
        _ => panic!("Expected DuplicateResource error"),
    }
}
```

#### Integration Tests
```plantuml
Component(integration_tests, "Integration Tests", "Testcontainers", "#[tokio::test]...")
```

**Реализация интеграционных тестов**:
```rust
// crates/ugc-subgraph/tests/integration/database_test.rs
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};
use sqlx::PgPool;

struct TestContext {
    _container: Container<'static, Postgres>,
    pool: PgPool,
}

impl TestContext {
    async fn new() -> Self {
        let docker = Cli::default();
        let container = docker.run(Postgres::default());
        let connection_string = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            container.get_host_port_ipv4(5432)
        );
        
        let pool = create_pool(&connection_string).await.unwrap();
        run_migrations(&pool).await.unwrap();
        
        Self {
            _container: container,
            pool,
        }
    }
}

#[tokio::test]
async fn test_create_and_retrieve_review() {
    // Arrange
    let ctx = TestContext::new().await;
    let repository = ReviewRepository::new(ctx.pool.clone());
    
    let input = CreateReviewInput {
        offer_id: OfferId::new(),
        rating: 5,
        title: Some("Excellent".to_string()),
        text: "This is a great car with excellent performance".to_string(),
    };
    
    let user_id = UserId::new();
    
    // Act - Create review
    let created_review = repository.create_review(input.clone(), user_id).await.unwrap();
    
    // Assert - Check created review
    assert_eq!(created_review.rating, 5);
    assert_eq!(created_review.text, input.text);
    assert_eq!(created_review.user_id, user_id.to_string());
    
    // Act - Retrieve review
    let retrieved_review = repository
        .get_review_by_id(ReviewId::from_str(&created_review.id).unwrap())
        .await
        .unwrap()
        .unwrap();
    
    // Assert - Check retrieved review
    assert_eq!(retrieved_review.id, created_review.id);
    assert_eq!(retrieved_review.rating, created_review.rating);
    assert_eq!(retrieved_review.text, created_review.text);
}

#[tokio::test]
async fn test_pagination_works_correctly() {
    // Arrange
    let ctx = TestContext::new().await;
    let repository = ReviewRepository::new(ctx.pool.clone());
    
    let offer_id = OfferId::new();
    let user_ids: Vec<UserId> = (0..15).map(|_| UserId::new()).collect();
    
    // Create 15 reviews
    for (i, user_id) in user_ids.iter().enumerate() {
        let input = CreateReviewInput {
            offer_id,
            rating: (i % 5) + 1,
            title: Some(format!("Review {}", i)),
            text: format!("This is review number {}", i),
        };
        
        repository.create_review(input, *user_id).await.unwrap();
    }
    
    // Act - Get first page
    let args = ConnectionArgs {
        first: 10,
        after: None,
    };
    let filter = ReviewFilter::default();
    
    let (reviews, has_next_page) = repository
        .get_reviews_with_pagination(offer_id, &args, &filter)
        .await
        .unwrap();
    
    // Assert
    assert_eq!(reviews.len(), 10);
    assert!(has_next_page);
    
    // Act - Get second page
    let last_review = reviews.last().unwrap();
    let cursor = base64::encode(format!("review:{}", last_review.id));
    
    let args = ConnectionArgs {
        first: 10,
        after: Some(cursor),
    };
    
    let (second_page_reviews, has_next_page) = repository
        .get_reviews_with_pagination(offer_id, &args, &filter)
        .await
        .unwrap();
    
    // Assert
    assert_eq!(second_page_reviews.len(), 5); // Remaining reviews
    assert!(!has_next_page);
}
```

### 8. Cargo Configuration

#### Cargo.toml
```plantuml
Component(cargo_toml, "Cargo.toml", "Package Config", "[package]...")
```

**Конфигурация проекта**:
```toml
# crates/ugc-subgraph/Cargo.toml
[package]
name = "ugc-subgraph"
version = "0.1.0"
edition = "2021"
authors = ["Auto.ru Team <team@auto.ru>"]
description = "UGC (User Generated Content) GraphQL subgraph for Auto.ru"

[dependencies]
# GraphQL
async-graphql = { workspace = true, features = ["chrono", "uuid"] }
async-graphql-axum = { workspace = true }

# HTTP Server
axum = { workspace = true, features = ["macros"] }
tower = { workspace = true, features = ["full"] }
tower-http = { workspace = true, features = ["cors", "trace", "compression"] }

# Database
sqlx = { workspace = true, features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "migrate"] }

# Async Runtime
tokio = { workspace = true, features = ["full"] }

# Serialization
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }

# Logging and Tracing
tracing = { workspace = true }
tracing-subscriber = { workspace = true, features = ["env-filter"] }

# Error Handling
thiserror = { workspace = true }
anyhow = { workspace = true }

# Utilities
uuid = { workspace = true, features = ["v4", "serde"] }
chrono = { workspace = true, features = ["serde"] }
base64 = { workspace = true }

# Shared crate
shared = { path = "../shared" }

[dev-dependencies]
# Testing
tokio-test = "0.4"
mockall = "0.11"
testcontainers = "0.14"
fake = { version = "2.5", features = ["derive", "chrono"] }

# Test utilities
serial_test = "2.0"
rstest = "0.18"

[features]
default = []
test-utils = ["mockall"]
```

#### Dockerfile
```plantuml
Component(dockerfile, "Dockerfile", "Multi-stage Build", "FROM rust:1.75-slim...")
```

**Многоэтапная сборка Docker образа**:
```dockerfile
# crates/ugc-subgraph/Dockerfile
# Этап 1: Сборка зависимостей
FROM rust:1.75-slim as chef
RUN cargo install cargo-chef
WORKDIR /app

# Этап 2: Планирование сборки
FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Этап 3: Сборка зависимостей
FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json

# Установка системных зависимостей
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Сборка зависимостей (кешируется)
RUN cargo chef cook --release --recipe-path recipe.json

# Копирование исходного кода и сборка приложения
COPY . .
RUN cargo build --release --bin ugc-subgraph

# Этап 4: Runtime образ
FROM debian:bookworm-slim as runtime

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Создание пользователя для безопасности
RUN groupadd -r ugc && useradd -r -g ugc ugc

# Создание директорий
RUN mkdir -p /app/migrations
WORKDIR /app

# Копирование бинарника и миграций
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/ugc-subgraph
COPY --from=builder /app/crates/ugc-subgraph/migrations ./migrations

# Установка прав
RUN chown -R ugc:ugc /app
USER ugc

# Настройка портов и переменных
EXPOSE 4001
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1

# Запуск приложения
CMD ["ugc-subgraph"]
```

## Взаимосвязи между модулями

### Поток выполнения запроса
```rust
// Полный поток от HTTP запроса до ответа

// 1. HTTP Request -> Axum Router
POST /graphql
Content-Type: application/json
{
  "query": "query GetReviews($offerId: ID!) { reviews(offerId: $offerId) { ... } }"
}

// 2. Axum Router -> GraphQL Handler
async fn graphql_handler(State(schema): State<UgcSchema>, req: GraphQLRequest)

// 3. GraphQL Handler -> Schema Execution
schema.execute(req.into_inner()).await

// 4. Schema -> Query Resolver
Query::reviews(ctx, offer_id, first, after, filter)

// 5. Query Resolver -> Review Service
service.get_reviews_connection(offer_id, args, filter).await

// 6. Review Service -> Cache Check
cache_service.get::<ReviewConnection>(&cache_key).await

// 7a. Cache Hit -> Return cached data
// 7b. Cache Miss -> Repository Query
repository.get_reviews_with_pagination(offer_id, &args, &filter).await

// 8. Repository -> Database Query
sqlx::query_as!(Review, "SELECT * FROM reviews WHERE ...").fetch_all(&pool).await

// 9. Database -> Repository -> Service -> Resolver -> Schema -> Handler -> Response
```

### Структура зависимостей
```rust
// Граф зависимостей модулей

main.rs
├── schema.rs (создание GraphQL схемы)
├── resolvers/
│   ├── query.rs (Query резолверы)
│   ├── mutation.rs (Mutation резолверы)
│   └── federation.rs (федеративные типы)
├── services/
│   ├── review_service.rs (бизнес-логика отзывов)
│   ├── rating_service.rs (бизнес-логика рейтингов)
│   ├── moderation_service.rs (модерация)
│   └── validation_service.rs (валидация)
├── repository/
│   ├── review_repository.rs (доступ к данным отзывов)
│   └── rating_repository.rs (доступ к данным рейтингов)
├── models/
│   ├── review.rs (модель отзыва)
│   ├── rating.rs (модель рейтинга)
│   └── input.rs (входные типы)
└── database.rs (настройка БД)

shared/ (общая библиотека)
├── auth/ (аутентификация)
├── cache/ (кеширование)
├── errors/ (обработка ошибок)
├── metrics/ (метрики)
└── tracing/ (трассировка)
```

## Выводы

Диаграмма кода UGC подграфа демонстрирует:

1. **Четкую модульную структуру** с разделением по слоям ответственности
2. **Типобезопасность** через использование системы типов Rust
3. **Тестируемость** через dependency injection и mock объекты
4. **Производительность** через асинхронное выполнение и кеширование
5. **Надежность** через обработку ошибок и валидацию данных
6. **Масштабируемость** через модульную архитектуру и оптимизированные запросы

Каждый модуль имеет конкретную реализацию с полным покрытием тестами, что обеспечивает высокое качество кода и простоту сопровождения.