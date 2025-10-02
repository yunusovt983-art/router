# Container Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CONTAINER.puml

### Назначение диаграммы
Диаграмма контейнеров детализирует архитектуру Task 11 на уровне основных контейнеров, показывая как заглушки подграфов Users и Offers интегрируются в федеративную систему.

### Основные слои архитектуры

#### 1. Federation Gateway Layer

##### Apollo Router
**Технологии:** Apollo Router + YAML Config
**Назначение:** Центральный федеративный шлюз для композиции и маршрутизации

**Реализация конфигурации:**
```yaml
# router.yaml
supergraph:
  introspection: true
  defer_support: true

subgraphs:
  users:
    routing_url: http://users-subgraph:4002/graphql
    schema:
      subgraph_url: http://users-subgraph:4002/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s
  
  offers:
    routing_url: http://offers-subgraph:4004/graphql
    schema:
      subgraph_url: http://offers-subgraph:4004/graphql
    timeout: 30s
    retry:
      min_per_sec: 10
      ttl: 10s

cors:
  origins:
    - "http://localhost:3000"
  methods:
    - GET
    - POST
  headers:
    - "content-type"
    - "authorization"

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

##### Supergraph Schema
**Технологии:** GraphQL SDL + Federation
**Назначение:** Композитная схема всех подграфов

**Автоматическая генерация:**
```bash
# Композиция схемы из всех подграфов
rover supergraph compose --config supergraph.yaml > supergraph.graphql
```

**Результирующая схема:**
```graphql
# Композитная схема включает типы из всех подграфов
type Query {
  # From Users subgraph
  user(id: ID!): User
  users: [User!]!
  
  # From Offers subgraph  
  offer(id: ID!): Offer
  offers(filter: OfferFilter, first: Int, after: String): OfferConnection!
  
  # From UGC subgraph
  review(id: ID!): Review
  reviews(filter: ReviewFilter): [Review!]!
}

type User @key(fields: "id") {
  id: ID!
  name: String!
  email: String!
  # Extended by Offers subgraph
  offers: [Offer!]!
  # Extended by UGC subgraph
  reviews: [Review!]!
}

type Offer @key(fields: "id") {
  id: ID!
  title: String!
  seller: User! # Resolved via federation
  reviews: [Review!]! # Resolved via federation
}
```

#### 2. Users Subgraph Container

##### Users GraphQL API
**Технологии:** Rust + async-graphql + Axum
**Порт:** 4002
**Назначение:** GraphQL API для управления пользователями

**Основная реализация:**
```rust
// users-subgraph/src/lib.rs
use async_graphql::{Context, EmptySubscription, Object, Result, Schema, SimpleObject, ID};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};

#[derive(SimpleObject, Clone)]
pub struct User {
    pub id: ID,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Query;

#[Object]
impl Query {
    /// Get user by ID - основной резолвер
    async fn user(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }

    /// Get all users - для тестирования
    async fn users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_users().await)
    }

    /// Federation entity resolver - ключевой для федерации
    async fn find_user_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }
}

// HTTP сервер setup
pub async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let user_service = Arc::new(UserService::new());
    
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(user_service)
        .finish();

    let state = AppState { schema };

    let app = Router::new()
        .route("/graphql", post(graphql_handler))
        .route("/health", get(health_check))
        .route("/.well-known/jwks.json", get(jwks))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:4002").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

##### Users Mock Service
**Технологии:** Rust + In-Memory Store
**Назначение:** Бизнес-логика и хранение моковых данных пользователей

**Реализация сервиса:**
```rust
// users-subgraph/src/service.rs
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct UserService {
    users: Arc<RwLock<Vec<User>>>,
}

impl UserService {
    pub fn new() -> Self {
        let users = Self::generate_mock_users();
        Self {
            users: Arc::new(RwLock::new(users)),
        }
    }

    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        let users = self.users.read().await;
        users.iter().find(|u| u.id == id).cloned()
    }

    pub async fn get_users(&self) -> Vec<User> {
        let users = self.users.read().await;
        users.clone()
    }

    pub async fn create_user(&self, input: CreateUserInput) -> Result<User, ServiceError> {
        let mut users = self.users.write().await;
        
        // Валидация email
        if !self.is_valid_email(&input.email) {
            return Err(ServiceError::InvalidEmail);
        }

        let user = User {
            id: ID::from(uuid::Uuid::new_v4().to_string()),
            name: input.name,
            email: input.email,
            phone: input.phone,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        users.push(user.clone());
        Ok(user)
    }

    fn generate_mock_users() -> Vec<User> {
        let now = chrono::Utc::now().to_rfc3339();
        
        vec![
            User {
                id: ID::from("user-1"),
                name: "Иван Иванов".to_string(),
                email: "ivan@example.com".to_string(),
                phone: Some("+7-900-123-45-67".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            User {
                id: ID::from("user-2"),
                name: "Мария Петрова".to_string(),
                email: "maria@example.com".to_string(),
                phone: Some("+7-900-765-43-21".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            // Больше моковых пользователей...
        ]
    }

    fn is_valid_email(&self, email: &str) -> bool {
        email.contains('@') && email.contains('.')
    }
}
```

##### Users Health Check
**Технологии:** Rust + JSON API
**Назначение:** Мониторинг состояния Users подграфа

**Реализация health check:**
```rust
// users-subgraph/src/health.rs
use axum::response::Json;
use serde_json::{json, Value};

pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "users-subgraph",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "checks": {
            "memory": check_memory_usage(),
            "mock_data": check_mock_data_availability(),
        }
    }))
}

fn check_memory_usage() -> Value {
    // Простая проверка использования памяти
    json!({
        "status": "ok",
        "usage": "low" // В реальной системе - реальные метрики
    })
}

fn check_mock_data_availability() -> Value {
    json!({
        "status": "ok",
        "users_count": 10 // Количество моковых пользователей
    })
}

// JWKS endpoint для JWT валидации (заглушка)
pub async fn jwks() -> Json<Value> {
    Json(json!({
        "keys": [
            {
                "kty": "RSA",
                "use": "sig",
                "kid": "test-key-1",
                "n": "test-modulus",
                "e": "AQAB"
            }
        ]
    }))
}
```

#### 3. Offers Subgraph Container

##### Offers GraphQL API
**Технологии:** Rust + async-graphql + Axum
**Порт:** 4004
**Назначение:** GraphQL API для объявлений о продаже автомобилей

**Схема и резолверы:**
```rust
// offers-subgraph/src/schema.rs
#[derive(SimpleObject, Clone)]
pub struct Offer {
    pub id: ID,
    pub title: String,
    pub description: String,
    pub price: i32,
    pub currency: String,
    pub seller_id: ID, // Ссылка на User из Users subgraph
    pub car: Car,
    pub location: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: OfferStatus,
}

#[derive(SimpleObject, Clone)]
pub struct Car {
    pub make: String,
    pub model: String,
    pub year: i32,
    pub mileage: Option<i32>,
    pub fuel_type: FuelType,
    pub transmission: TransmissionType,
    pub color: String,
    pub vin: Option<String>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum OfferStatus {
    Active,
    Sold,
    Inactive,
}

#[Object]
impl Query {
    /// Get offer by ID
    async fn offer(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&id).await)
    }

    /// Get offers with filtering and pagination
    async fn offers(
        &self,
        ctx: &Context<'_>,
        filter: Option<OfferFilter>,
        first: Option<i32>,
        after: Option<String>
    ) -> Result<OfferConnection> {
        let service = ctx.data::<Arc<OfferService>>()?;
        service.get_offers(filter, first, after).await
    }

    /// Federation entity resolver
    async fn find_offer_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&id).await)
    }
}

// Mutation резолверы для CRUD операций
#[Object]
impl Mutation {
    async fn create_offer(
        &self,
        ctx: &Context<'_>,
        input: CreateOfferInput
    ) -> Result<Offer> {
        let service = ctx.data::<Arc<OfferService>>()?;
        service.create_offer(input).await
    }

    async fn update_offer(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateOfferInput
    ) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        service.update_offer(id, input).await
    }
}
```

##### Offers Mock Service
**Технологии:** Rust + In-Memory Store
**Назначение:** Бизнес-логика объявлений с моковыми автомобильными данными

**Реализация сервиса:**
```rust
// offers-subgraph/src/service.rs
pub struct OfferService {
    offers: Arc<RwLock<Vec<Offer>>>,
}

impl OfferService {
    pub fn new() -> Self {
        let offers = Self::generate_mock_offers();
        Self {
            offers: Arc::new(RwLock::new(offers)),
        }
    }

    pub async fn get_offers(
        &self,
        filter: Option<OfferFilter>,
        first: Option<i32>,
        after: Option<String>
    ) -> Result<OfferConnection, ServiceError> {
        let offers = self.offers.read().await;
        
        // Применение фильтров
        let filtered = self.apply_filter(&offers, filter);
        
        // Пагинация
        let paginated = self.apply_pagination(filtered, first, after);
        
        Ok(OfferConnection::from_offers(paginated))
    }

    fn apply_filter(&self, offers: &[Offer], filter: Option<OfferFilter>) -> Vec<Offer> {
        match filter {
            Some(f) => offers.iter()
                .filter(|offer| {
                    // Фильтр по марке автомобиля
                    if let Some(ref make) = f.make {
                        if offer.car.make != *make {
                            return false;
                        }
                    }
                    
                    // Фильтр по цене
                    if let Some(min_price) = f.price_min {
                        if offer.price < min_price {
                            return false;
                        }
                    }
                    
                    if let Some(max_price) = f.price_max {
                        if offer.price > max_price {
                            return false;
                        }
                    }
                    
                    // Фильтр по статусу
                    if let Some(status) = f.status {
                        if offer.status != status {
                            return false;
                        }
                    }
                    
                    true
                })
                .cloned()
                .collect(),
            None => offers.to_vec(),
        }
    }

    fn generate_mock_offers() -> Vec<Offer> {
        let now = chrono::Utc::now().to_rfc3339();
        
        vec![
            Offer {
                id: ID::from("offer-1"),
                title: "Toyota Camry 2020 в отличном состоянии".to_string(),
                description: "Продаю Toyota Camry 2020 года. Один владелец, полная история обслуживания.".to_string(),
                price: 2500000,
                currency: "RUB".to_string(),
                seller_id: ID::from("user-1"), // Связь с Users subgraph
                car: Car {
                    make: "Toyota".to_string(),
                    model: "Camry".to_string(),
                    year: 2020,
                    mileage: Some(45000),
                    fuel_type: FuelType::Gasoline,
                    transmission: TransmissionType::Automatic,
                    color: "Белый".to_string(),
                    vin: Some("JT2BF28K123456789".to_string()),
                },
                location: "Москва".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
                status: OfferStatus::Active,
            },
            // Больше моковых объявлений...
        ]
    }
}
```

#### 4. Mock Data Management Layer

##### Data Relationship Manager
**Технологии:** Rust + Custom Logic
**Назначение:** Управление связями между данными разных подграфов

**Реализация управления связями:**
```rust
// shared/src/relationship_manager.rs
pub struct DataRelationshipManager {
    user_offer_mapping: HashMap<String, Vec<String>>, // user_id -> offer_ids
    offer_user_mapping: HashMap<String, String>,      // offer_id -> user_id
}

impl DataRelationshipManager {
    pub fn new() -> Self {
        let mut manager = Self {
            user_offer_mapping: HashMap::new(),
            offer_user_mapping: HashMap::new(),
        };
        
        manager.initialize_relationships();
        manager
    }

    fn initialize_relationships(&mut self) {
        // Создание связей между пользователями и объявлениями
        self.add_relationship("user-1", "offer-1");
        self.add_relationship("user-1", "offer-2");
        self.add_relationship("user-2", "offer-3");
        // ...
    }

    pub fn add_relationship(&mut self, user_id: &str, offer_id: &str) {
        // Добавление связи User -> Offers
        self.user_offer_mapping
            .entry(user_id.to_string())
            .or_insert_with(Vec::new)
            .push(offer_id.to_string());
        
        // Добавление связи Offer -> User
        self.offer_user_mapping
            .insert(offer_id.to_string(), user_id.to_string());
    }

    pub fn get_offers_by_user(&self, user_id: &str) -> Vec<String> {
        self.user_offer_mapping
            .get(user_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_user_by_offer(&self, offer_id: &str) -> Option<String> {
        self.offer_user_mapping.get(offer_id).cloned()
    }

    pub fn validate_consistency(&self) -> Result<(), ConsistencyError> {
        // Проверка целостности данных между подграфами
        for (user_id, offer_ids) in &self.user_offer_mapping {
            for offer_id in offer_ids {
                if let Some(mapped_user) = self.offer_user_mapping.get(offer_id) {
                    if mapped_user != user_id {
                        return Err(ConsistencyError::InconsistentMapping {
                            user_id: user_id.clone(),
                            offer_id: offer_id.clone(),
                            mapped_user: mapped_user.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
```

### Federation Integration Patterns

#### Cross-Subgraph Entity Resolution
**Паттерн:** Разрешение сущностей между подграфами через Apollo Router

**Реализация в Offers subgraph:**
```rust
// Расширение User типа в Offers subgraph
#[derive(SimpleObject)]
#[graphql(extends)]
pub struct User {
    #[graphql(external)]
    pub id: ID,
    
    // Добавление поля offers к User типу
    pub offers: Vec<Offer>,
}

#[Object]
impl User {
    #[graphql(requires = "id")]
    async fn offers(&self, ctx: &Context<'_>) -> Result<Vec<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offers_by_seller_id(&self.id).await)
    }
}
```

#### Federation Query Planning
**Процесс:** Apollo Router автоматически планирует выполнение федеративных запросов

**Пример query plan:**
```json
{
  "kind": "QueryPlan",
  "node": {
    "kind": "Sequence",
    "nodes": [
      {
        "kind": "Fetch",
        "serviceName": "offers",
        "query": "query($representations: [_Any!]!) { offer(id: \"offer-1\") { id title price seller_id } }"
      },
      {
        "kind": "Fetch", 
        "serviceName": "users",
        "requires": [{"kind": "InlineFragment", "typeCondition": "Offer", "selections": [{"kind": "Field", "name": "seller_id"}]}],
        "query": "query($representations: [_Any!]!) { _entities(representations: $representations) { ... on User { id name email } } }"
      }
    ]
  }
}
```

Эта диаграмма контейнеров показывает, как заглушки подграфов Task 11 структурированы и взаимодействуют друг с другом через федеративный шлюз, обеспечивая основу для разработки и тестирования cross-subgraph функциональности.