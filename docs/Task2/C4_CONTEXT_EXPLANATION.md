# C4 Context Diagram - Task 2: UGC Subgraph - Подробное объяснение

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_CONTEXT.puml`

Контекстная диаграмма показывает UGC (User Generated Content) подграф в контексте федеративной GraphQL системы Auto.ru, определяя его роль, пользователей и взаимодействия с внешними системами.

## Архитектурные элементы и их реализация

### 1. Пользователи системы

#### Пользователь Auto.ru
```plantuml
Person(user, "Пользователь Auto.ru", "Создает и читает отзывы о автомобилях")
```

**Архитектурная роль**: Основной потребитель UGC функциональности

**Реализация взаимодействия**:
```graphql
# Типичные GraphQL запросы пользователя
query GetOfferReviews($offerId: ID!) {
  reviews(offerId: $offerId, first: 10) {
    edges {
      node {
        id
        rating
        text
        createdAt
        user {
          name
          avatar
        }
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}

mutation CreateReview($input: CreateReviewInput!) {
  createReview(input: $input) {
    id
    rating
    text
    createdAt
  }
}
```

**Реализация в коде UGC подграфа**:
```rust
// crates/ugc-subgraph/src/resolvers/query.rs
#[Object]
impl Query {
    /// Получить отзывы по объявлению с пагинацией
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        offer_id: ID,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let offer_id = OfferId::from_str(&offer_id)?;
        
        // Валидация пагинации
        let args = ConnectionArgs {
            first,
            after,
            last: None,
            before: None,
        };
        args.validate()?;
        
        service.get_reviews_connection(offer_id, args).await
    }
}

// crates/ugc-subgraph/src/resolvers/mutation.rs
#[Object]
impl Mutation {
    /// Создать новый отзыв (требует аутентификации)
    #[graphql(guard = "RequireAuth")]
    async fn create_review(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> Result<Review> {
        let service = ctx.data::<ReviewService>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        service.create_review(input, user_context).await
    }
}
```

#### Модератор
```plantuml
Person(moderator, "Модератор", "Модерирует пользовательский контент")
```

**Архитектурная роль**: Управление качеством пользовательского контента

**Реализация модерации**:
```rust
// crates/ugc-subgraph/src/resolvers/mutation.rs
#[Object]
impl Mutation {
    /// Модерировать отзыв (только для модераторов)
    #[graphql(guard = "RequireRole(Role::Moderator)")]
    async fn moderate_review(
        &self,
        ctx: &Context<'_>,
        review_id: ID,
        status: ModerationStatus,
        reason: Option<String>,
    ) -> Result<Review> {
        let service = ctx.data::<ModerationService>()?;
        let user_context = ctx.data::<UserContext>()?;
        let review_id = ReviewId::from_str(&review_id)?;
        
        service.moderate_review(review_id, status, reason, user_context).await
    }
    
    /// Массовая модерация отзывов
    #[graphql(guard = "RequireRole(Role::Moderator)")]
    async fn bulk_moderate_reviews(
        &self,
        ctx: &Context<'_>,
        review_ids: Vec<ID>,
        status: ModerationStatus,
    ) -> Result<Vec<Review>> {
        let service = ctx.data::<ModerationService>()?;
        let user_context = ctx.data::<UserContext>()?;
        
        service.bulk_moderate_reviews(review_ids, status, user_context).await
    }
}
```

**Система ролей и разрешений**:
```rust
// crates/shared/src/auth.rs
#[derive(Debug, Clone)]
pub enum Role {
    User,
    Moderator,
    Admin,
}

impl Role {
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            Role::User => vec![
                Permission::CreateReview,
                Permission::UpdateOwnReview,
                Permission::DeleteOwnReview,
            ],
            Role::Moderator => vec![
                Permission::CreateReview,
                Permission::UpdateOwnReview,
                Permission::DeleteOwnReview,
                Permission::ModerateReviews,
                Permission::ViewModerationQueue,
                Permission::BulkModerateReviews,
            ],
            Role::Admin => vec![
                // Все разрешения модератора плюс административные
                Permission::ManageUsers,
                Permission::ViewAnalytics,
                Permission::SystemConfiguration,
            ],
        }
    }
}
```

### 2. UGC Subgraph System

#### UGC Subgraph
```plantuml
System(ugc_subgraph, "UGC Subgraph", "GraphQL сервис для управления пользовательским контентом")
```

**Архитектурная роль**: Центральный сервис для всех операций с пользовательским контентом

**Основная реализация сервера**:
```rust
// crates/ugc-subgraph/src/main.rs
use async_graphql::{Schema, EmptySubscription, extensions::*};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

type UgcSchema = Schema<Query, Mutation, EmptySubscription>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация трассировки
    init_tracing().await?;
    
    // Подключение к базе данных
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = create_database_pool(&database_url).await?;
    
    // Подключение к Redis
    let redis_url = std::env::var("REDIS_URL")?;
    let cache = CacheService::new(&redis_url).await?;
    
    // Создание сервисов
    let review_repository = ReviewRepository::new(pool.clone());
    let review_service = ReviewService::new(review_repository, cache.clone());
    let moderation_service = ModerationService::new(pool.clone());
    
    // Создание GraphQL схемы с расширениями
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(pool)
        .data(cache)
        .data(review_service)
        .data(moderation_service)
        .extension(Logger)
        .extension(Tracing)
        .extension(analyzer::depth_limit::DepthLimit::new(10))
        .extension(analyzer::complexity_limit::ComplexityLimit::new(1000))
        .finish();
    
    // HTTP сервер с middleware
    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/", get(graphql_playground))
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
                .layer(auth_middleware)
        )
        .with_state(schema);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], 4001));
    info!("UGC Subgraph server starting on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

async fn graphql_handler(
    State(schema): State<UgcSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}
```

**GraphQL схема UGC подграфа**:
```graphql
# Основные типы UGC подграфа
type Review @key(fields: "id") {
  id: ID!
  offerId: ID!
  userId: ID!
  rating: Int!
  text: String!
  isModerated: Boolean!
  moderatedBy: ID
  moderatedAt: DateTime
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Федеративные расширения
  user: User
  offer: Offer
}

type OfferRating @key(fields: "offerId") {
  offerId: ID!
  averageRating: Float!
  totalReviews: Int!
  ratingDistribution: RatingDistribution!
  lastUpdated: DateTime!
}

type RatingDistribution {
  oneStar: Int!
  twoStars: Int!
  threeStars: Int!
  fourStars: Int!
  fiveStars: Int!
}

# Федеративные расширения внешних типов
extend type User @key(fields: "id") {
  id: ID! @external
  reviews: ReviewConnection!
}

extend type Offer @key(fields: "id") {
  id: ID! @external
  reviews: ReviewConnection!
  rating: OfferRating
}

type Query {
  review(id: ID!): Review
  reviews(
    offerId: ID
    userId: ID
    first: Int
    after: String
    filter: ReviewFilter
    sort: ReviewSort
  ): ReviewConnection!
  offerRating(offerId: ID!): OfferRating
}

type Mutation {
  createReview(input: CreateReviewInput!): Review!
  updateReview(id: ID!, input: UpdateReviewInput!): Review!
  deleteReview(id: ID!): Boolean!
  moderateReview(id: ID!, status: ModerationStatus!, reason: String): Review!
}
```

### 3. Федеративная интеграция

#### Apollo Router
```plantuml
System_Ext(apollo_router, "Apollo Router", "Федеративный GraphQL роутер")
```

**Архитектурная роль**: Композиция и маршрутизация федеративных запросов

**Конфигурация Apollo Router для UGC**:
```yaml
# crates/apollo-router/router.yaml
supergraph:
  listen: 0.0.0.0:4000
  introspection: true

subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    schema:
      subgraph_url: http://ugc-subgraph:4001/graphql
  users:
    routing_url: http://users-subgraph:4002/graphql
    schema:
      subgraph_url: http://users-subgraph:4002/graphql
  offers:
    routing_url: http://offers-subgraph:4003/graphql
    schema:
      subgraph_url: http://offers-subgraph:4003/graphql

# Федеративные запросы обрабатываются автоматически
```

**Пример федеративного запроса**:
```graphql
# Клиент отправляет единый запрос к Apollo Router
query GetOfferWithReviews($offerId: ID!) {
  offer(id: $offerId) {          # Offers подграф
    id
    title
    price
    year
    brand
    model
    owner {                      # Users подграф (через федерацию)
      name
      avatar
    }
    rating {                     # UGC подграф (через федерацию)
      averageRating
      totalReviews
      ratingDistribution {
        fiveStars
        fourStars
        threeStars
        twoStars
        oneStar
      }
    }
    reviews(first: 5) {          # UGC подграф (через федерацию)
      edges {
        node {
          id
          rating
          text
          createdAt
          user {                 # Users подграф (через федерацию)
            name
            avatar
          }
        }
      }
    }
  }
}
```

#### Федеративные резолверы в UGC подграфе
```rust
// crates/ugc-subgraph/src/resolvers/federation.rs
use async_graphql::{Object, Context, Result, ID};

#[Object]
impl Query {
    /// Entity resolver для Review
    #[graphql(entity)]
    async fn find_review_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Review> {
        let service = ctx.data::<ReviewService>()?;
        let review_id = ReviewId::from_str(&id)?;
        
        service.get_review_by_id(review_id).await?
            .ok_or_else(|| "Review not found".into())
    }
    
    /// Entity resolver для OfferRating
    #[graphql(entity)]
    async fn find_offer_rating_by_offer_id(&self, ctx: &Context<'_>, offer_id: ID) -> Result<OfferRating> {
        let service = ctx.data::<RatingService>()?;
        let offer_id = OfferId::from_str(&offer_id)?;
        
        service.get_offer_rating(offer_id).await
    }
}

// Расширения внешних типов
#[Object]
impl User {
    /// Получить отзывы пользователя
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let user_id = UserId::from_str(&self.id)?;
        
        let args = ConnectionArgs { first, after, last: None, before: None };
        service.get_user_reviews_connection(user_id, args).await
    }
}

#[Object]
impl Offer {
    /// Получить отзывы объявления
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        let args = ConnectionArgs { first, after, last: None, before: None };
        service.get_reviews_connection(offer_id, args).await
    }
    
    /// Получить рейтинг объявления
    async fn rating(&self, ctx: &Context<'_>) -> Result<Option<OfferRating>> {
        let service = ctx.data::<RatingService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        Ok(Some(service.get_offer_rating(offer_id).await?))
    }
}
```### 4.
 Внешние системы и интеграции

#### PostgreSQL
```plantuml
System_Ext(postgres, "PostgreSQL", "Реляционная база данных")
```

**Архитектурная роль**: Основное хранилище данных UGC

**Схема базы данных**:
```sql
-- migrations/001_create_reviews_table.sql
CREATE TABLE IF NOT EXISTS reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    offer_id UUID NOT NULL,
    user_id UUID NOT NULL,
    rating INTEGER NOT NULL CHECK (rating >= 1 AND rating <= 5),
    text TEXT NOT NULL CHECK (length(text) >= 10 AND length(text) <= 5000),
    is_moderated BOOLEAN DEFAULT FALSE,
    moderated_by UUID,
    moderated_at TIMESTAMP WITH TIME ZONE,
    moderation_reason TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    
    -- Бизнес-ограничения
    CONSTRAINT unique_user_offer_review UNIQUE (user_id, offer_id) 
        WHERE deleted_at IS NULL,
    
    -- Индексы для производительности
    INDEX idx_reviews_offer_id (offer_id) WHERE deleted_at IS NULL,
    INDEX idx_reviews_user_id (user_id) WHERE deleted_at IS NULL,
    INDEX idx_reviews_created_at (created_at DESC),
    INDEX idx_reviews_moderation (is_moderated, created_at DESC) 
        WHERE deleted_at IS NULL
);

-- migrations/002_create_offer_ratings_table.sql
CREATE TABLE IF NOT EXISTS offer_ratings (
    offer_id UUID PRIMARY KEY,
    average_rating DECIMAL(3,2) NOT NULL DEFAULT 0.00,
    total_reviews INTEGER NOT NULL DEFAULT 0,
    rating_distribution JSONB DEFAULT '{"1": 0, "2": 0, "3": 0, "4": 0, "5": 0}',
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Проверки целостности
    CONSTRAINT valid_average_rating CHECK (average_rating >= 0 AND average_rating <= 5),
    CONSTRAINT valid_total_reviews CHECK (total_reviews >= 0)
);
```

**Интеграция с Rust кодом**:
```rust
// crates/ugc-subgraph/src/repository/review_repository.rs
use sqlx::{PgPool, Row};
use shared::types::{UserId, OfferId, ReviewId};

pub struct ReviewRepository {
    pool: PgPool,
}

impl ReviewRepository {
    pub async fn create_review(
        &self,
        input: CreateReviewInput,
        user_id: UserId,
    ) -> UgcResult<Review> {
        let mut tx = self.pool.begin().await?;
        
        // Создание отзыва
        let review = sqlx::query_as!(
            Review,
            r#"
            INSERT INTO reviews (offer_id, user_id, rating, text)
            VALUES ($1, $2, $3, $4)
            RETURNING 
                id, offer_id, user_id, rating, text, is_moderated,
                moderated_by, moderated_at, created_at, updated_at, deleted_at
            "#,
            input.offer_id.0,
            user_id.0,
            input.rating,
            input.text
        )
        .fetch_one(&mut *tx)
        .await?;
        
        // Обновление агрегированного рейтинга
        sqlx::query!(
            "SELECT update_offer_rating($1)",
            input.offer_id.0
        )
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        Ok(review)
    }
    
    pub async fn get_reviews_by_offer_paginated(
        &self,
        offer_id: OfferId,
        args: ConnectionArgs,
    ) -> UgcResult<(Vec<Review>, PageInfo)> {
        let limit = args.limit().min(100); // Максимум 100 записей
        let offset = args.offset();
        
        // Получение отзывов с пагинацией
        let reviews = sqlx::query_as!(
            Review,
            r#"
            SELECT 
                id, offer_id, user_id, rating, text, is_moderated,
                moderated_by, moderated_at, created_at, updated_at, deleted_at
            FROM reviews 
            WHERE offer_id = $1 
              AND deleted_at IS NULL 
              AND is_moderated = TRUE
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            offer_id.0,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;
        
        // Подсчет общего количества для пагинации
        let total_count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM reviews WHERE offer_id = $1 AND deleted_at IS NULL AND is_moderated = TRUE",
            offer_id.0
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);
        
        let page_info = PageInfo::new(&reviews, total_count, &args);
        
        Ok((reviews, page_info))
    }
}
```

#### Redis Cache
```plantuml
System_Ext(redis, "Redis Cache", "In-memory кеш")
```

**Архитектурная роль**: Кеширование агрегированных данных и сессий

**Реализация кеширования**:
```rust
// crates/ugc-subgraph/src/services/cache_service.rs
use redis::{AsyncCommands, Client};
use serde::{Serialize, de::DeserializeOwned};

pub struct CacheService {
    client: Client,
}

impl CacheService {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = Client::open(redis_url)?;
        Ok(Self { client })
    }
    
    /// Кеширование рейтинга объявления
    pub async fn cache_offer_rating(
        &self,
        offer_id: OfferId,
        rating: &OfferRating,
    ) -> UgcResult<()> {
        let mut conn = self.client.get_async_connection().await?;
        let key = format!("offer_rating:{}", offer_id);
        let value = serde_json::to_string(rating)?;
        
        // Кеш на 1 час
        conn.setex(key, 3600, value).await?;
        
        Ok(())
    }
    
    /// Получение кешированного рейтинга
    pub async fn get_cached_offer_rating(
        &self,
        offer_id: OfferId,
    ) -> UgcResult<Option<OfferRating>> {
        let mut conn = self.client.get_async_connection().await?;
        let key = format!("offer_rating:{}", offer_id);
        
        let cached: Option<String> = conn.get(key).await?;
        
        match cached {
            Some(value) => {
                let rating = serde_json::from_str(&value)?;
                Ok(Some(rating))
            }
            None => Ok(None),
        }
    }
    
    /// Инвалидация кеша при обновлении отзыва
    pub async fn invalidate_offer_cache(&self, offer_id: OfferId) -> UgcResult<()> {
        let mut conn = self.client.get_async_connection().await?;
        let patterns = vec![
            format!("offer_rating:{}", offer_id),
            format!("reviews:offer:{}:*", offer_id),
        ];
        
        for pattern in patterns {
            let keys: Vec<String> = conn.keys(pattern).await?;
            if !keys.is_empty() {
                conn.del(keys).await?;
            }
        }
        
        Ok(())
    }
}
```

#### Мониторинг и наблюдаемость

**Prometheus метрики**:
```rust
// crates/ugc-subgraph/src/metrics.rs
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};

lazy_static! {
    // GraphQL операции
    pub static ref GRAPHQL_REQUESTS_TOTAL: Counter = register_counter!(
        "ugc_graphql_requests_total",
        "Total number of GraphQL requests to UGC subgraph"
    ).unwrap();
    
    pub static ref GRAPHQL_REQUEST_DURATION: Histogram = register_histogram!(
        "ugc_graphql_request_duration_seconds",
        "GraphQL request duration in seconds",
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    ).unwrap();
    
    // Бизнес-метрики
    pub static ref REVIEWS_CREATED_TOTAL: Counter = register_counter!(
        "ugc_reviews_created_total",
        "Total number of reviews created"
    ).unwrap();
    
    pub static ref REVIEWS_MODERATED_TOTAL: Counter = register_counter!(
        "ugc_reviews_moderated_total",
        "Total number of reviews moderated"
    ).unwrap();
    
    pub static ref ACTIVE_REVIEWS: Gauge = register_gauge!(
        "ugc_active_reviews",
        "Number of active (non-deleted, moderated) reviews"
    ).unwrap();
    
    // Производительность
    pub static ref DATABASE_QUERY_DURATION: Histogram = register_histogram!(
        "ugc_database_query_duration_seconds",
        "Database query duration in seconds"
    ).unwrap();
    
    pub static ref CACHE_HIT_RATE: Counter = register_counter!(
        "ugc_cache_hits_total",
        "Total number of cache hits"
    ).unwrap();
    
    pub static ref CACHE_MISS_RATE: Counter = register_counter!(
        "ugc_cache_misses_total",
        "Total number of cache misses"
    ).unwrap();
}

// Middleware для сбора метрик
pub async fn metrics_middleware(
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    GRAPHQL_REQUESTS_TOTAL.inc();
    
    let response = next.run(request).await;
    
    let duration = start.elapsed().as_secs_f64();
    GRAPHQL_REQUEST_DURATION.observe(duration);
    
    response
}
```

**Jaeger трассировка**:
```rust
// crates/ugc-subgraph/src/tracing.rs
use tracing::{info, instrument, Span};
use opentelemetry::trace::TraceContextExt;

#[instrument(skip(service))]
pub async fn create_review_traced(
    service: &ReviewService,
    input: CreateReviewInput,
    user_context: &UserContext,
) -> UgcResult<Review> {
    let span = Span::current();
    span.record("user_id", &user_context.user_id.to_string());
    span.record("offer_id", &input.offer_id.to_string());
    span.record("rating", &input.rating);
    
    info!("Creating review for offer {}", input.offer_id);
    
    let result = service.create_review(input, user_context).await;
    
    match &result {
        Ok(review) => {
            span.record("review_id", &review.id.to_string());
            info!("Review created successfully: {}", review.id);
        }
        Err(e) => {
            span.record("error", &e.to_string());
            tracing::error!("Failed to create review: {}", e);
        }
    }
    
    result
}
```

## Взаимодействия и потоки данных

### 1. Создание отзыва (полный поток)
```rust
// Полный поток создания отзыва от пользователя до базы данных
async fn create_review_flow(
    input: CreateReviewInput,
    user_context: UserContext,
) -> UgcResult<Review> {
    // 1. Валидация входных данных
    validation::validate_rating(input.rating)?;
    validation::validate_review_text(&input.text)?;
    
    // 2. Проверка бизнес-правил
    if review_service.has_user_reviewed_offer(user_context.user_id, input.offer_id).await? {
        return Err(UgcError::DuplicateResource {
            message: "User already reviewed this offer".to_string(),
        });
    }
    
    // 3. Создание отзыва в БД (с транзакцией)
    let review = review_repository.create_review(input, user_context.user_id).await?;
    
    // 4. Обновление агрегированного рейтинга
    rating_service.update_offer_rating(review.offer_id).await?;
    
    // 5. Инвалидация кеша
    cache_service.invalidate_offer_cache(review.offer_id).await?;
    
    // 6. Отправка метрик
    REVIEWS_CREATED_TOTAL.inc();
    
    // 7. Логирование события
    info!(
        user_id = %user_context.user_id,
        offer_id = %review.offer_id,
        review_id = %review.id,
        "Review created successfully"
    );
    
    Ok(review)
}
```

### 2. Федеративный запрос (обработка)
```rust
// Обработка федеративного запроса от Apollo Router
async fn handle_federated_query(
    offer_id: OfferId,
) -> UgcResult<OfferWithReviews> {
    // 1. Получение рейтинга (с кешированием)
    let rating = match cache_service.get_cached_offer_rating(offer_id).await? {
        Some(cached_rating) => {
            CACHE_HIT_RATE.inc();
            cached_rating
        }
        None => {
            CACHE_MISS_RATE.inc();
            let rating = rating_service.calculate_offer_rating(offer_id).await?;
            cache_service.cache_offer_rating(offer_id, &rating).await?;
            rating
        }
    };
    
    // 2. Получение отзывов с пагинацией
    let reviews = review_service.get_reviews_connection(
        offer_id,
        ConnectionArgs { first: Some(5), ..Default::default() }
    ).await?;
    
    // 3. Федеративные ссылки будут разрешены Apollo Router
    // автоматически для полей user в каждом отзыве
    
    Ok(OfferWithReviews {
        rating: Some(rating),
        reviews,
    })
}
```

## Выводы

Контекстная диаграмма UGC подграфа демонстрирует:

1. **Четкую роль в федеративной архитектуре** - специализированный сервис для UGC
2. **Интеграцию с пользователями** - обычные пользователи и модераторы
3. **Федеративные связи** - автоматическая композиция с другими подграфами
4. **Надежную инфраструктуру** - PostgreSQL для данных, Redis для кеша
5. **Полную наблюдаемость** - метрики, трассировка, логирование

Эта диаграмма служит отправной точкой для понимания места UGC подграфа в общей архитектуре системы и его взаимодействий с внешним миром.