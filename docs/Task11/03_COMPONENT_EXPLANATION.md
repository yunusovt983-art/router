# Component Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_COMPONENT.puml

### Назначение диаграммы
Диаграмма компонентов показывает детальную внутреннюю структуру заглушек подграфов Task 11, 
раскрывая архитектуру на уровне отдельных компонентов и их взаимодействий.

### Users Subgraph Components

#### GraphQL Layer

##### Users Schema
**Технологии:** async-graphql Schema
**Назначение:** Определение GraphQL схемы для пользователей с federation директивами

**Реализация схемы:**
```rust
// users-subgraph/src/schema.rs
use async_graphql::{Schema, EmptySubscription, SimpleObject, Object, ID};

#[derive(SimpleObject, Clone)]
#[graphql(complex)] // Позволяет добавлять сложные резолверы
pub struct User {
    #[graphql(key)] // Federation @key directive
    pub id: ID,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// Комплексные резолверы для federation
#[ComplexObject]
impl User {
    // Этот резолвер будет вызван Apollo Router для entity resolution
    async fn __resolve_reference(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&self.id).await)
    }
}

// Создание схемы с federation поддержкой
pub fn create_schema() -> Schema<Query, Mutation, EmptySubscription> {
    Schema::build(Query, Mutation, EmptySubscription)
        .enable_federation() // Включение Apollo Federation
        .finish()
}
```

##### Users Query Resolver
**Технологии:** async-graphql Object
**Назначение:** Обработка GraphQL запросов для пользователей

**Детальная реализация:**
```rust
// users-subgraph/src/resolvers/query.rs
pub struct Query;

#[Object]
impl Query {
    /// Получение пользователя по ID - основной резолвер
    #[graphql(desc = "Get user by ID")]
    async fn user(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "User ID")] id: ID
    ) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        
        // Логирование для отладки
        tracing::info!("Resolving user with id: {}", id);
        
        match service.get_user_by_id(&id).await {
            Some(user) => {
                tracing::debug!("Found user: {}", user.name);
                Ok(Some(user))
            },
            None => {
                tracing::warn!("User not found with id: {}", id);
                Ok(None)
            }
        }
    }

    /// Получение всех пользователей - для тестирования
    #[graphql(desc = "Get all users")]
    async fn users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        let users = service.get_users().await;
        
        tracing::info!("Returning {} users", users.len());
        Ok(users)
    }

    /// Federation entity resolver - критически важен для федерации
    #[graphql(entity)]
    async fn find_user_by_id(
        &self,
        ctx: &Context<'_>,
        #[graphql(key)] id: ID
    ) -> Result<Option<User>> {
        tracing::debug!("Federation entity resolution for user: {}", id);
        self.user(ctx, id).await
    }
}
```###
## Users Mutation Resolver
**Технологии:** async-graphql Object
**Назначение:** Обработка GraphQL мутаций (заглушки для будущего развития)

**Реализация мутаций:**
```rust
// users-subgraph/src/resolvers/mutation.rs
pub struct Mutation;

#[Object]
impl Mutation {
    /// Создание нового пользователя
    async fn create_user(
        &self,
        ctx: &Context<'_>,
        input: CreateUserInput
    ) -> Result<User> {
        let service = ctx.data::<Arc<UserService>>()?;
        
        // Валидация входных данных
        if input.name.trim().is_empty() {
            return Err("Name cannot be empty".into());
        }
        
        if !input.email.contains('@') {
            return Err("Invalid email format".into());
        }
        
        match service.create_user(input).await {
            Ok(user) => {
                tracing::info!("Created user: {} ({})", user.name, user.id);
                Ok(user)
            },
            Err(e) => {
                tracing::error!("Failed to create user: {:?}", e);
                Err(e.into())
            }
        }
    }

    /// Обновление пользователя
    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateUserInput
    ) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        
        match service.update_user(id.clone(), input).await {
            Ok(Some(user)) => {
                tracing::info!("Updated user: {}", id);
                Ok(Some(user))
            },
            Ok(None) => {
                tracing::warn!("User not found for update: {}", id);
                Ok(None)
            },
            Err(e) => {
                tracing::error!("Failed to update user {}: {:?}", id, e);
                Err(e.into())
            }
        }
    }

    /// Удаление пользователя
    async fn delete_user(
        &self,
        ctx: &Context<'_>,
        id: ID
    ) -> Result<bool> {
        let service = ctx.data::<Arc<UserService>>()?;
        
        match service.delete_user(id.clone()).await {
            Ok(deleted) => {
                if deleted {
                    tracing::info!("Deleted user: {}", id);
                } else {
                    tracing::warn!("User not found for deletion: {}", id);
                }
                Ok(deleted)
            },
            Err(e) => {
                tracing::error!("Failed to delete user {}: {:?}", id, e);
                Err(e.into())
            }
        }
    }
}

// Input типы для мутаций
#[derive(InputObject)]
pub struct CreateUserInput {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateUserInput {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}
```

#### Service Layer

##### Users Service
**Технологии:** Rust Service Struct
**Назначение:** Бизнес-логика управления пользователями

**Полная реализация сервиса:**
```rust
// users-subgraph/src/service.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct UserService {
    users: Arc<RwLock<Vec<User>>>,
    email_index: Arc<RwLock<HashMap<String, String>>>, // email -> user_id
}

impl UserService {
    pub fn new() -> Self {
        let users = Self::generate_mock_users();
        let email_index = Self::build_email_index(&users);
        
        Self {
            users: Arc::new(RwLock::new(users)),
            email_index: Arc::new(RwLock::new(email_index)),
        }
    }

    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        let users = self.users.read().await;
        users.iter().find(|u| u.id == id).cloned()
    }

    pub async fn get_user_by_email(&self, email: &str) -> Option<User> {
        let email_index = self.email_index.read().await;
        if let Some(user_id) = email_index.get(email) {
            self.get_user_by_id(user_id).await
        } else {
            None
        }
    }

    pub async fn get_users(&self) -> Vec<User> {
        let users = self.users.read().await;
        users.clone()
    }

    pub async fn create_user(&self, input: CreateUserInput) -> Result<User, ServiceError> {
        // Проверка уникальности email
        if self.get_user_by_email(&input.email).await.is_some() {
            return Err(ServiceError::EmailAlreadyExists);
        }

        let user_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let user = User {
            id: ID::from(user_id.clone()),
            name: input.name,
            email: input.email.clone(),
            phone: input.phone,
            created_at: now.clone(),
            updated_at: now,
        };

        // Добавление пользователя
        {
            let mut users = self.users.write().await;
            users.push(user.clone());
        }

        // Обновление индекса email
        {
            let mut email_index = self.email_index.write().await;
            email_index.insert(input.email, user_id);
        }

        Ok(user)
    }

    pub async fn update_user(
        &self,
        id: ID,
        input: UpdateUserInput
    ) -> Result<Option<User>, ServiceError> {
        let mut users = self.users.write().await;
        
        if let Some(user) = users.iter_mut().find(|u| u.id == id) {
            let mut email_changed = false;
            let old_email = user.email.clone();

            // Обновление полей
            if let Some(name) = input.name {
                user.name = name;
            }
            
            if let Some(email) = input.email {
                if email != user.email {
                    // Проверка уникальности нового email
                    if users.iter().any(|u| u.email == email && u.id != id) {
                        return Err(ServiceError::EmailAlreadyExists);
                    }
                    user.email = email;
                    email_changed = true;
                }
            }
            
            if let Some(phone) = input.phone {
                user.phone = Some(phone);
            }

            user.updated_at = chrono::Utc::now().to_rfc3339();

            // Обновление email индекса если email изменился
            if email_changed {
                let mut email_index = self.email_index.write().await;
                email_index.remove(&old_email);
                email_index.insert(user.email.clone(), id.to_string());
            }

            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
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
            User {
                id: ID::from("user-3"),
                name: "Алексей Сидоров".to_string(),
                email: "alexey@example.com".to_string(),
                phone: None,
                created_at: now.clone(),
                updated_at: now,
            },
        ]
    }

    fn build_email_index(users: &[User]) -> HashMap<String, String> {
        users.iter()
            .map(|u| (u.email.clone(), u.id.to_string()))
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Email already exists")]
    EmailAlreadyExists,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
```

### Offers Subgraph Components

#### GraphQL Layer

##### Offers Schema
**Технологии:** async-graphql Schema
**Назначение:** GraphQL схема для объявлений и автомобилей

**Реализация схемы:**
```rust
// offers-subgraph/src/schema.rs
#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct Offer {
    #[graphql(key)] // Federation @key directive
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

#[ComplexObject]
impl Offer {
    /// Federation entity resolver
    async fn __resolve_reference(&self, ctx: &Context<'_>) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&self.id).await)
    }

    /// Резолвер для получения продавца (будет разрешен через federation)
    async fn seller(&self, ctx: &Context<'_>) -> Result<User> {
        // В реальной федерации это будет автоматически разрешено Apollo Router
        // Здесь возвращаем заглушку с ID для federation
        Ok(User {
            id: self.seller_id.clone(),
            // Остальные поля будут заполнены Users subgraph
        })
    }
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
    Draft,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum FuelType {
    Gasoline,
    Diesel,
    Electric,
    Hybrid,
    Gas,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum TransmissionType {
    Manual,
    Automatic,
    CVT,
    Robot,
}
```

### Federation Support Components

#### Federation Directives
**Технологии:** GraphQL Directives
**Назначение:** Реализация Apollo Federation директив

**Реализация директив:**
```rust
// shared/src/federation.rs
use async_graphql::*;

/// Макрос для автоматического добавления federation директив
macro_rules! federation_entity {
    ($type:ty, $key:expr) => {
        #[ComplexObject]
        impl $type {
            #[graphql(name = "__resolveReference")]
            async fn resolve_reference(&self, ctx: &Context<'_>) -> Result<Option<Self>> {
                // Автоматическая генерация entity resolver
                let service = ctx.data::<Arc<dyn EntityService<Self>>>()?;
                service.resolve_entity(&self.key_value()).await
            }
        }
    };
}

/// Трейт для entity services
#[async_trait::async_trait]
pub trait EntityService<T>: Send + Sync {
    async fn resolve_entity(&self, key: &str) -> Result<Option<T>, ServiceError>;
}

/// Реализация для User entity
#[async_trait::async_trait]
impl EntityService<User> for UserService {
    async fn resolve_entity(&self, key: &str) -> Result<Option<User>, ServiceError> {
        Ok(self.get_user_by_id(key).await)
    }
}

/// Реализация для Offer entity
#[async_trait::async_trait]
impl EntityService<Offer> for OfferService {
    async fn resolve_entity(&self, key: &str) -> Result<Option<Offer>, ServiceError> {
        Ok(self.get_offer_by_id(key).await)
    }
}
```

#### Entity Resolver
**Технологии:** Federation Entity Logic
**Назначение:** Центральная логика разрешения сущностей

**Реализация entity resolver:**
```rust
// shared/src/entity_resolver.rs
pub struct EntityResolver {
    user_service: Arc<UserService>,
    offer_service: Arc<OfferService>,
}

impl EntityResolver {
    pub fn new(
        user_service: Arc<UserService>,
        offer_service: Arc<OfferService>
    ) -> Self {
        Self {
            user_service,
            offer_service,
        }
    }

    pub async fn resolve_entities(
        &self,
        representations: Vec<EntityRepresentation>
    ) -> Result<Vec<Option<Entity>>, ResolverError> {
        let mut results = Vec::new();

        for repr in representations {
            match repr {
                EntityRepresentation::User { id } => {
                    let user = self.user_service.get_user_by_id(&id).await;
                    results.push(user.map(Entity::User));
                },
                EntityRepresentation::Offer { id } => {
                    let offer = self.offer_service.get_offer_by_id(&id).await;
                    results.push(offer.map(Entity::Offer));
                },
            }
        }

        Ok(results)
    }
}

#[derive(Union)]
pub enum Entity {
    User(User),
    Offer(Offer),
}

#[derive(InputObject)]
#[graphql(untagged)]
pub enum EntityRepresentation {
    User { id: ID },
    Offer { id: ID },
}
```

### Testing Support Components

#### Federation Test Client
**Технологии:** GraphQL Test Client
**Назначение:** Тестирование федеративных запросов

**Реализация тестового клиента:**
```rust
// tests/support/federation_client.rs
pub struct FederationTestClient {
    users_schema: Schema<users::Query, users::Mutation, EmptySubscription>,
    offers_schema: Schema<offers::Query, offers::Mutation, EmptySubscription>,
    router_client: Option<TestRouterClient>,
}

impl FederationTestClient {
    pub async fn new() -> Self {
        let users_schema = users::create_schema();
        let offers_schema = offers::create_schema();
        
        Self {
            users_schema,
            offers_schema,
            router_client: None,
        }
    }

    pub async fn with_router(mut self) -> Self {
        // Настройка тестового Apollo Router
        let router_config = RouterConfig {
            subgraphs: vec![
                SubgraphConfig {
                    name: "users".to_string(),
                    url: "http://localhost:4002/graphql".to_string(),
                    schema: self.users_schema.sdl(),
                },
                SubgraphConfig {
                    name: "offers".to_string(),
                    url: "http://localhost:4004/graphql".to_string(),
                    schema: self.offers_schema.sdl(),
                },
            ],
        };

        self.router_client = Some(TestRouterClient::new(router_config).await);
        self
    }

    pub async fn execute_federated_query(
        &self,
        query: &str,
        variables: serde_json::Value
    ) -> Result<GraphQLResponse, TestError> {
        if let Some(router) = &self.router_client {
            router.execute(query, variables).await
        } else {
            // Fallback: выполнение запроса к отдельным подграфам
            self.execute_subgraph_query(query, variables).await
        }
    }

    async fn execute_subgraph_query(
        &self,
        query: &str,
        variables: serde_json::Value
    ) -> Result<GraphQLResponse, TestError> {
        // Простая логика определения, к какому подграфу направить запрос
        if query.contains("user") || query.contains("User") {
            let request = Request::new(query).variables(Variables::from_json(variables)?);
            let response = self.users_schema.execute(request).await;
            Ok(GraphQLResponse::from(response))
        } else if query.contains("offer") || query.contains("Offer") {
            let request = Request::new(query).variables(Variables::from_json(variables)?);
            let response = self.offers_schema.execute(request).await;
            Ok(GraphQLResponse::from(response))
        } else {
            Err(TestError::UnknownQueryTarget)
        }
    }
}
```

Эта диаграмма компонентов показывает детальную внутреннюю архитектуру заглушек подграфов Task 11, демонстрируя как каждый компонент реализован в коде и как они взаимодействуют для обеспечения федеративной функциональности.