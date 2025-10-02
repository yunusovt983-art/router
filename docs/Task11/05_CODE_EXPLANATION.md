# Code Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CODE.puml

### Назначение диаграммы
Диаграмма кода показывает конкретную реализацию заглушек подграфов Task 11 на языке Rust,
демонстрируя как архитектурные решения воплощаются в рабочем коде.

### Users Subgraph Implementation

#### GraphQL Schema Implementation

##### User Entity
**Назначение:** Основная сущность пользователя с federation поддержкой
**Технологии:** Rust + async-graphql

**Полная реализация:**
```rust
// users-subgraph/src/entities/user.rs
use async_graphql::{SimpleObject, ComplexObject, Context, Result, ID};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
#[graphql(complex)] // Позволяет добавлять сложные резолверы
pub struct User {
    /// Уникальный идентификатор пользователя (federation key)
    #[graphql(key)] // Apollo Federation @key directive
    pub id: ID,
    
    /// Полное имя пользователя
    #[graphql(desc = "User's full name")]
    pub name: String,
    
    /// Email адрес пользователя
    #[graphql(desc = "User's email address")]
    pub email: String,
    
    /// Номер телефона (опционально)
    #[graphql(desc = "User's phone number")]
    pub phone: Option<String>,
    
    /// Дата создания аккаунта
    #[graphql(desc = "Account creation timestamp")]
    pub created_at: String,
    
    /// Дата последнего обновления
    #[graphql(desc = "Last update timestamp")]
    pub updated_at: String,
}

#[ComplexObject]
impl User {
    /// Federation entity resolver - ключевой для Apollo Federation
    /// Этот метод вызывается Apollo Router для разрешения User entities
    async fn __resolve_reference(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        tracing::debug!("Resolving User entity reference for id: {}", self.id);
        
        let service = ctx.data::<Arc<UserService>>()
            .map_err(|_| "UserService not found in context")?;
        
        match service.get_user_by_id(&self.id).await {
            Some(user) => {
                tracing::info!("Successfully resolved User entity: {}", user.name);
                Ok(Some(user))
            },
            None => {
                tracing::warn!("User entity not found for id: {}", self.id);
                Ok(None)
            }
        }
    }

    /// Computed field - количество объявлений пользователя
    /// Это поле будет разрешено Offers subgraph через federation
    async fn offers_count(&self, ctx: &Context<'_>) -> Result<i32> {
        // В заглушке возвращаем моковое значение
        // В реальной федерации это будет разрешено Offers subgraph
        Ok(rand::random::<i32>() % 10)
    }

    /// Computed field - рейтинг пользователя как продавца
    async fn seller_rating(&self, ctx: &Context<'_>) -> Result<Option<f64>> {
        // Моковый рейтинг для демонстрации
        Ok(Some(4.5 + (rand::random::<f64>() - 0.5)))
    }
}

impl User {
    /// Конструктор для создания нового пользователя
    pub fn new(name: String, email: String, phone: Option<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        
        Self {
            id: ID::from(uuid::Uuid::new_v4().to_string()),
            name,
            email,
            phone,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Обновление пользователя
    pub fn update(&mut self, name: Option<String>, email: Option<String>, phone: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        if let Some(email) = email {
            self.email = email;
        }
        if phone.is_some() {
            self.phone = phone;
        }
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Валидация данных пользователя
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        
        if !self.email.contains('@') || !self.email.contains('.') {
            return Err(ValidationError::InvalidEmail);
        }
        
        if let Some(ref phone) = self.phone {
            if !phone.starts_with('+') {
                return Err(ValidationError::InvalidPhone);
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Name cannot be empty")]
    EmptyName,
    #[error("Invalid email format")]
    InvalidEmail,
    #[error("Invalid phone format")]
    InvalidPhone,
}
```

##### Users Query Implementation
**Назначение:** GraphQL Query резолверы для пользователей
**Технологии:** async-graphql Object

**Детальная реализация:**
```rust
// users-subgraph/src/resolvers/query.rs
use async_graphql::{Object, Context, Result, ID};
use std::sync::Arc;
use tracing::{info, debug, warn, instrument};

pub struct Query;

#[Object]
impl Query {
    /// Получение пользователя по ID
    #[instrument(skip(self, ctx), fields(user_id = %id))]
    async fn user(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "User ID to fetch")] id: ID
    ) -> Result<Option<User>> {
        debug!("Starting user query resolution");
        
        let service = ctx.data::<Arc<UserService>>()
            .map_err(|_| "UserService not available")?;
        
        let start_time = std::time::Instant::now();
        
        match service.get_user_by_id(&id).await {
            Some(user) => {
                let duration = start_time.elapsed();
                info!(
                    user_name = %user.name,
                    duration_ms = duration.as_millis(),
                    "Successfully resolved user"
                );
                Ok(Some(user))
            },
            None => {
                warn!("User not found");
                Ok(None)
            }
        }
    }

    /// Получение всех пользователей с пагинацией
    #[instrument(skip(self, ctx))]
    async fn users(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Number of users to fetch", default = 10)] first: i32,
        #[graphql(desc = "Cursor for pagination")] after: Option<String>
    ) -> Result<UserConnection> {
        debug!("Starting users query with first: {}, after: {:?}", first, after);
        
        let service = ctx.data::<Arc<UserService>>()?;
        
        // Ограничение на количество пользователей
        let limit = std::cmp::min(first, 100) as usize;
        
        let users = service.get_users_paginated(limit, after).await
            .map_err(|e| format!("Failed to fetch users: {}", e))?;
        
        info!("Returning {} users", users.len());
        
        Ok(UserConnection::from_users(users, limit))
    }

    /// Поиск пользователей по имени или email
    #[instrument(skip(self, ctx), fields(query = %search_query))]
    async fn search_users(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Search query")] search_query: String,
        #[graphql(desc = "Maximum results", default = 10)] limit: i32
    ) -> Result<Vec<User>> {
        debug!("Starting user search");
        
        if search_query.len() < 2 {
            return Err("Search query must be at least 2 characters".into());
        }
        
        let service = ctx.data::<Arc<UserService>>()?;
        let limit = std::cmp::min(limit, 50) as usize;
        
        let results = service.search_users(&search_query, limit).await
            .map_err(|e| format!("Search failed: {}", e))?;
        
        info!("Search returned {} results", results.len());
        Ok(results)
    }

    /// Federation entity resolver - КРИТИЧЕСКИ ВАЖЕН для Apollo Federation
    /// Этот резолвер вызывается Apollo Router для разрешения User entities
    /// из других подграфов
    #[graphql(entity)]
    #[instrument(skip(self, ctx), fields(entity_id = %id))]
    async fn find_user_by_id(
        &self,
        ctx: &Context<'_>,
        #[graphql(key)] id: ID
    ) -> Result<Option<User>> {
        debug!("Federation entity resolution requested");
        
        // Добавляем специальную метку для federation запросов
        tracing::Span::current().record("federation_request", &true);
        
        // Используем тот же резолвер, что и для обычных запросов
        self.user(ctx, id).await
    }
}

// Типы для пагинации (Relay-style connections)
#[derive(SimpleObject)]
pub struct UserConnection {
    pub edges: Vec<UserEdge>,
    pub page_info: PageInfo,
}

#[derive(SimpleObject)]
pub struct UserEdge {
    pub node: User,
    pub cursor: String,
}

#[derive(SimpleObject)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

impl UserConnection {
    pub fn from_users(users: Vec<User>, limit: usize) -> Self {
        let edges: Vec<UserEdge> = users.into_iter()
            .enumerate()
            .map(|(i, user)| UserEdge {
                cursor: base64::encode(format!("user:{}", i)),
                node: user,
            })
            .collect();

        let page_info = PageInfo {
            has_next_page: edges.len() >= limit,
            has_previous_page: false, // Упрощенная реализация
            start_cursor: edges.first().map(|e| e.cursor.clone()),
            end_cursor: edges.last().map(|e| e.cursor.clone()),
        };

        Self { edges, page_info }
    }
}
```

#### Service Layer Implementation

##### UserService
**Назначение:** Бизнес-логика управления пользователями
**Технологии:** Rust + async/await + in-memory storage

**Полная реализация сервиса:**
```rust
// users-subgraph/src/services/user_service.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error, instrument};

pub struct UserService {
    // Основное хранилище пользователей
    users: Arc<RwLock<HashMap<String, User>>>,
    
    // Индекс для поиска по email
    email_index: Arc<RwLock<HashMap<String, String>>>, // email -> user_id
    
    // Индекс для поиска по имени (простой)
    name_index: Arc<RwLock<HashMap<String, Vec<String>>>>, // lowercase_name -> user_ids
    
    // Метрики сервиса
    metrics: Arc<ServiceMetrics>,
}

impl UserService {
    pub fn new() -> Self {
        info!("Initializing UserService with mock data");
        
        let users = Self::generate_mock_users();
        let email_index = Self::build_email_index(&users);
        let name_index = Self::build_name_index(&users);
        
        let users_map: HashMap<String, User> = users.into_iter()
            .map(|user| (user.id.to_string(), user))
            .collect();

        info!("UserService initialized with {} users", users_map.len());

        Self {
            users: Arc::new(RwLock::new(users_map)),
            email_index: Arc::new(RwLock::new(email_index)),
            name_index: Arc::new(RwLock::new(name_index)),
            metrics: Arc::new(ServiceMetrics::new()),
        }
    }

    #[instrument(skip(self), fields(user_id = %id))]
    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        debug!("Fetching user by ID");
        
        let start_time = std::time::Instant::now();
        
        let users = self.users.read().await;
        let result = users.get(id).cloned();
        
        let duration = start_time.elapsed();
        
        match &result {
            Some(user) => {
                debug!(user_name = %user.name, "User found");
                self.metrics.record_get_success(duration).await;
            },
            None => {
                debug!("User not found");
                self.metrics.record_get_miss(duration).await;
            }
        }
        
        result
    }

    #[instrument(skip(self), fields(email = %email))]
    pub async fn get_user_by_email(&self, email: &str) -> Option<User> {
        debug!("Fetching user by email");
        
        let email_index = self.email_index.read().await;
        
        if let Some(user_id) = email_index.get(email) {
            drop(email_index); // Освобождаем lock
            self.get_user_by_id(user_id).await
        } else {
            debug!("No user found with email");
            None
        }
    }

    #[instrument(skip(self))]
    pub async fn get_users_paginated(
        &self,
        limit: usize,
        after_cursor: Option<String>
    ) -> Result<Vec<User>, ServiceError> {
        debug!("Fetching paginated users with limit: {}", limit);
        
        let users = self.users.read().await;
        
        let mut user_list: Vec<User> = users.values().cloned().collect();
        
        // Сортировка по дате создания (новые первыми)
        user_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        // Применение cursor-based пагинации
        let start_index = if let Some(cursor) = after_cursor {
            self.decode_cursor(&cursor)? + 1
        } else {
            0
        };
        
        let end_index = std::cmp::min(start_index + limit, user_list.len());
        
        let result = user_list[start_index..end_index].to_vec();
        
        debug!("Returning {} users (from {} to {})", result.len(), start_index, end_index);
        
        Ok(result)
    }

    #[instrument(skip(self), fields(query = %search_query, limit = %limit))]
    pub async fn search_users(
        &self,
        search_query: &str,
        limit: usize
    ) -> Result<Vec<User>, ServiceError> {
        debug!("Searching users");
        
        let query_lower = search_query.to_lowercase();
        let users = self.users.read().await;
        
        let mut results = Vec::new();
        
        // Простой поиск по имени и email
        for user in users.values() {
            if results.len() >= limit {
                break;
            }
            
            let name_match = user.name.to_lowercase().contains(&query_lower);
            let email_match = user.email.to_lowercase().contains(&query_lower);
            
            if name_match || email_match {
                results.push(user.clone());
            }
        }
        
        // Сортировка результатов по релевантности
        results.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower || a.email.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower || b.email.to_lowercase() == query_lower;
            
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        debug!("Search completed with {} results", results.len());
        
        Ok(results)
    }

    #[instrument(skip(self, input))]
    pub async fn create_user(&self, input: CreateUserInput) -> Result<User, ServiceError> {
        info!("Creating new user");
        
        // Валидация входных данных
        self.validate_create_input(&input).await?;
        
        // Проверка уникальности email
        if self.get_user_by_email(&input.email).await.is_some() {
            warn!("Attempt to create user with existing email");
            return Err(ServiceError::EmailAlreadyExists);
        }
        
        let user = User::new(input.name, input.email, input.phone);
        
        // Добавление в основное хранилище
        {
            let mut users = self.users.write().await;
            users.insert(user.id.to_string(), user.clone());
        }
        
        // Обновление индексов
        {
            let mut email_index = self.email_index.write().await;
            email_index.insert(user.email.clone(), user.id.to_string());
        }
        
        {
            let mut name_index = self.name_index.write().await;
            let name_key = user.name.to_lowercase();
            name_index.entry(name_key)
                .or_insert_with(Vec::new)
                .push(user.id.to_string());
        }
        
        info!(user_id = %user.id, user_name = %user.name, "User created successfully");
        
        self.metrics.record_create_success().await;
        
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
            User {
                id: ID::from("user-3"),
                name: "Алексей Сидоров".to_string(),
                email: "alexey@example.com".to_string(),
                phone: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            User {
                id: ID::from("user-4"),
                name: "Елена Козлова".to_string(),
                email: "elena@example.com".to_string(),
                phone: Some("+7-900-555-12-34".to_string()),
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            User {
                id: ID::from("user-5"),
                name: "Дмитрий Волков".to_string(),
                email: "dmitry@example.com".to_string(),
                phone: Some("+7-900-777-88-99".to_string()),
                created_at: now.clone(),
                updated_at: now,
            },
        ]
    }

    fn build_email_index(users: &[User]) -> HashMap<String, String> {
        users.iter()
            .map(|user| (user.email.clone(), user.id.to_string()))
            .collect()
    }

    fn build_name_index(users: &[User]) -> HashMap<String, Vec<String>> {
        let mut index = HashMap::new();
        
        for user in users {
            let name_key = user.name.to_lowercase();
            index.entry(name_key)
                .or_insert_with(Vec::new)
                .push(user.id.to_string());
        }
        
        index
    }

    async fn validate_create_input(&self, input: &CreateUserInput) -> Result<(), ServiceError> {
        if input.name.trim().is_empty() {
            return Err(ServiceError::InvalidInput("Name cannot be empty".to_string()));
        }
        
        if !input.email.contains('@') || !input.email.contains('.') {
            return Err(ServiceError::InvalidInput("Invalid email format".to_string()));
        }
        
        if let Some(ref phone) = input.phone {
            if !phone.starts_with('+') {
                return Err(ServiceError::InvalidInput("Phone must start with +".to_string()));
            }
        }
        
        Ok(())
    }

    fn decode_cursor(&self, cursor: &str) -> Result<usize, ServiceError> {
        let decoded = base64::decode(cursor)
            .map_err(|_| ServiceError::InvalidCursor)?;
        
        let cursor_str = String::from_utf8(decoded)
            .map_err(|_| ServiceError::InvalidCursor)?;
        
        if let Some(index_str) = cursor_str.strip_prefix("user:") {
            index_str.parse::<usize>()
                .map_err(|_| ServiceError::InvalidCursor)
        } else {
            Err(ServiceError::InvalidCursor)
        }
    }
}

// Метрики сервиса
pub struct ServiceMetrics {
    get_requests: Arc<RwLock<u64>>,
    get_hits: Arc<RwLock<u64>>,
    create_requests: Arc<RwLock<u64>>,
    total_response_time: Arc<RwLock<std::time::Duration>>,
}

impl ServiceMetrics {
    pub fn new() -> Self {
        Self {
            get_requests: Arc::new(RwLock::new(0)),
            get_hits: Arc::new(RwLock::new(0)),
            create_requests: Arc::new(RwLock::new(0)),
            total_response_time: Arc::new(RwLock::new(std::time::Duration::ZERO)),
        }
    }

    pub async fn record_get_success(&self, duration: std::time::Duration) {
        let mut requests = self.get_requests.write().await;
        *requests += 1;
        
        let mut hits = self.get_hits.write().await;
        *hits += 1;
        
        let mut total_time = self.total_response_time.write().await;
        *total_time += duration;
    }

    pub async fn record_get_miss(&self, duration: std::time::Duration) {
        let mut requests = self.get_requests.write().await;
        *requests += 1;
        
        let mut total_time = self.total_response_time.write().await;
        *total_time += duration;
    }

    pub async fn record_create_success(&self) {
        let mut requests = self.create_requests.write().await;
        *requests += 1;
    }

    pub async fn get_stats(&self) -> ServiceStats {
        let requests = *self.get_requests.read().await;
        let hits = *self.get_hits.read().await;
        let creates = *self.create_requests.read().await;
        let total_time = *self.total_response_time.read().await;

        ServiceStats {
            total_get_requests: requests,
            cache_hit_rate: if requests > 0 { hits as f64 / requests as f64 } else { 0.0 },
            total_create_requests: creates,
            average_response_time_ms: if requests > 0 { 
                total_time.as_millis() as f64 / requests as f64 
            } else { 0.0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceStats {
    pub total_get_requests: u64,
    pub cache_hit_rate: f64,
    pub total_create_requests: u64,
    pub average_response_time_ms: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Email already exists")]
    EmailAlreadyExists,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid cursor format")]
    InvalidCursor,
    #[error("Internal error: {0}")]
    Internal(String),
}

// Input типы
#[derive(InputObject, Debug)]
pub struct CreateUserInput {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}
```

Эта диаграмма кода демонстрирует как архитектурные решения Task 11 воплощаются в конкретном Rust коде, показывая полную реализацию заглушек подграфов с federation поддержкой, валидацией, метриками и comprehensive error handling.