# Code Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CODE.puml

### Назначение диаграммы
Диаграмма кода показывает конкретную реализацию ключевых компонентов тестовой инфраструктуры Task 10 
на языке Rust с детальными примерами кода.

### Unit Testing Implementation

#### Test Framework Implementation

##### TestConfig
**Назначение:** Централизованная конфигурация для всех типов тестов
**Технологии:** Rust Struct с Default trait

**Полная реализация:**
```rust
// src/testing/config.rs
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub database_url: String,
    pub redis_url: String,
    pub external_services: HashMap<String, String>,
    pub test_timeout: Duration,
    pub parallel_tests: bool,
    pub max_connections: u32,
    pub cleanup_strategy: CleanupStrategy,
    pub log_level: String,
    pub enable_tracing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupStrategy {
    Truncate,
    Rollback,
    Recreate,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("TEST_DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://test:test@localhost:5433/test_db".to_string()),
            redis_url: std::env::var("TEST_REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6380".to_string()),
            external_services: Self::default_external_services(),
            test_timeout: Duration::from_secs(30),
            parallel_tests: true,
            max_connections: 10,
            cleanup_strategy: CleanupStrategy::Rollback,
            log_level: "info".to_string(),
            enable_tracing: true,
        }
    }
}

impl TestConfig {
    pub fn from_env() -> Result<Self, TestConfigError> {
        let mut config = Self::default();
        
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            config.database_url = url;
        }
        
        if let Ok(url) = std::env::var("TEST_REDIS_URL") {
            config.redis_url = url;
        }
        
        if let Ok(timeout) = std::env::var("TEST_TIMEOUT") {
            config.test_timeout = Duration::from_secs(
                timeout.parse().map_err(|_| TestConfigError::InvalidTimeout)?
            );
        }
        
        Ok(config)
    }
    
    fn default_external_services() -> HashMap<String, String> {
        let mut services = HashMap::new();
        services.insert("users_api".to_string(), "http://localhost:8081".to_string());
        services.insert("offers_api".to_string(), "http://localhost:8082".to_string());
        services.insert("notifications_api".to_string(), "http://localhost:8083".to_string());
        services
    }
    
    pub fn for_integration_tests() -> Self {
        let mut config = Self::default();
        config.parallel_tests = false; // Интеграционные тесты последовательно
        config.cleanup_strategy = CleanupStrategy::Rollback;
        config.max_connections = 5;
        config
    }
    
    pub fn for_load_tests() -> Self {
        let mut config = Self::default();
        config.max_connections = 50;
        config.test_timeout = Duration::from_secs(120);
        config.parallel_tests = true;
        config
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TestConfigError {
    #[error("Invalid timeout value")]
    InvalidTimeout,
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
}
```###
## TestHarness
**Назначение:** Основная структура для управления тестовой средой
**Технологии:** Rust с async/await, SQLx, Redis

**Полная реализация:**
```rust
// src/testing/harness.rs
use sqlx::{PgPool, PgPoolOptions};
use redis::Client as RedisClient;
use wiremock::MockServer;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct TestHarness {
    config: TestConfig,
    db_pool: Option<PgPool>,
    redis_client: Option<RedisClient>,
    mock_server: Option<MockServer>,
    cleanup_handlers: Vec<Box<dyn CleanupHandler>>,
    transaction_stack: Arc<Mutex<Vec<sqlx::Transaction<'static, sqlx::Postgres>>>>,
}

impl TestHarness {
    pub async fn new() -> Result<Self, TestError> {
        let config = TestConfig::from_env()?;
        
        Ok(Self {
            config,
            db_pool: None,
            redis_client: None,
            mock_server: None,
            cleanup_handlers: Vec::new(),
            transaction_stack: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    pub async fn with_config(config: TestConfig) -> Result<Self, TestError> {
        Ok(Self {
            config,
            db_pool: None,
            redis_client: None,
            mock_server: None,
            cleanup_handlers: Vec::new(),
            transaction_stack: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    pub async fn setup_database(&mut self) -> Result<&PgPool, TestError> {
        if self.db_pool.is_none() {
            let pool = PgPoolOptions::new()
                .max_connections(self.config.max_connections)
                .connect(&self.config.database_url)
                .await
                .map_err(TestError::DatabaseConnection)?;
            
            // Запуск миграций
            sqlx::migrate!("./migrations")
                .run(&pool)
                .await
                .map_err(TestError::Migration)?;
            
            self.db_pool = Some(pool);
        }
        
        Ok(self.db_pool.as_ref().unwrap())
    }
    
    pub async fn setup_redis(&mut self) -> Result<&RedisClient, TestError> {
        if self.redis_client.is_none() {
            let client = RedisClient::open(self.config.redis_url.as_str())
                .map_err(TestError::RedisConnection)?;
            
            // Проверка соединения
            let mut conn = client.get_async_connection().await
                .map_err(TestError::RedisConnection)?;
            
            redis::cmd("PING").query_async::<_, String>(&mut conn).await
                .map_err(TestError::RedisConnection)?;
            
            self.redis_client = Some(client);
        }
        
        Ok(self.redis_client.as_ref().unwrap())
    }
    
    pub async fn setup_mock_server(&mut self) -> Result<&MockServer, TestError> {
        if self.mock_server.is_none() {
            let server = MockServer::start().await;
            self.mock_server = Some(server);
        }
        
        Ok(self.mock_server.as_ref().unwrap())
    }
    
    pub async fn begin_transaction(&self) -> Result<TestTransaction, TestError> {
        let pool = self.db_pool.as_ref()
            .ok_or(TestError::DatabaseNotSetup)?;
        
        let tx = pool.begin().await
            .map_err(TestError::TransactionBegin)?;
        
        Ok(TestTransaction::new(tx, self.transaction_stack.clone()))
    }
    
    pub async fn cleanup(&mut self) -> Result<(), TestError> {
        // Выполнение всех зарегистрированных обработчиков очистки
        for handler in &self.cleanup_handlers {
            handler.cleanup().await?;
        }
        
        // Очистка Redis
        if let Some(client) = &self.redis_client {
            let mut conn = client.get_async_connection().await?;
            redis::cmd("FLUSHDB").query_async::<_, ()>(&mut conn).await?;
        }
        
        // Откат всех активных транзакций
        let mut stack = self.transaction_stack.lock().await;
        while let Some(tx) = stack.pop() {
            tx.rollback().await.map_err(TestError::TransactionRollback)?;
        }
        
        Ok(())
    }
    
    pub fn register_cleanup_handler<H: CleanupHandler + 'static>(&mut self, handler: H) {
        self.cleanup_handlers.push(Box::new(handler));
    }
}

pub struct TestTransaction {
    tx: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
    stack: Arc<Mutex<Vec<sqlx::Transaction<'static, sqlx::Postgres>>>>,
}

impl TestTransaction {
    fn new(
        tx: sqlx::Transaction<'static, sqlx::Postgres>,
        stack: Arc<Mutex<Vec<sqlx::Transaction<'static, sqlx::Postgres>>>>
    ) -> Self {
        Self {
            tx: Some(tx),
            stack,
        }
    }
    
    pub async fn commit(mut self) -> Result<(), TestError> {
        if let Some(tx) = self.tx.take() {
            tx.commit().await.map_err(TestError::TransactionCommit)?;
        }
        Ok(())
    }
    
    pub async fn rollback(mut self) -> Result<(), TestError> {
        if let Some(tx) = self.tx.take() {
            tx.rollback().await.map_err(TestError::TransactionRollback)?;
        }
        Ok(())
    }
}

impl Drop for TestTransaction {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            // Добавляем транзакцию в стек для отката при cleanup
            let stack = self.stack.clone();
            tokio::spawn(async move {
                let mut stack = stack.lock().await;
                stack.push(tx);
            });
        }
    }
}

#[async_trait::async_trait]
pub trait CleanupHandler: Send + Sync {
    async fn cleanup(&self) -> Result<(), TestError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Database connection error: {0}")]
    DatabaseConnection(#[from] sqlx::Error),
    #[error("Redis connection error: {0}")]
    RedisConnection(#[from] redis::RedisError),
    #[error("Migration error: {0}")]
    Migration(sqlx::migrate::MigrateError),
    #[error("Database not setup")]
    DatabaseNotSetup,
    #[error("Transaction begin error: {0}")]
    TransactionBegin(sqlx::Error),
    #[error("Transaction commit error: {0}")]
    TransactionCommit(sqlx::Error),
    #[error("Transaction rollback error: {0}")]
    TransactionRollback(sqlx::Error),
    #[error("Config error: {0}")]
    Config(#[from] TestConfigError),
}
```

#### Mock Factory Implementation

##### MockFactory Trait и Implementation
**Назначение:** Фабрика для создания моков с различным поведением
**Технологии:** Rust Traits, Mockall, Generics

**Полная реализация:**
```rust
// src/testing/mocks/factory.rs
use mockall::predicate::*;
use serde_json::Value;
use std::time::Duration;
use std::sync::Arc;

pub trait MockFactory<T> {
    fn create_mock() -> T;
    fn with_behavior(behavior: MockBehavior) -> T;
    fn with_expectations(expectations: Vec<MockExpectation>) -> T;
}

#[derive(Clone)]
pub enum MockBehavior {
    Success(Value),
    Error(String),
    Timeout(Duration),
    Latency(Duration, Box<MockBehavior>),
    Sequence(Vec<MockBehavior>),
    Custom(Arc<dyn Fn() -> Result<Value, String> + Send + Sync>),
}

pub struct MockExpectation {
    pub method: String,
    pub times: Times,
    pub with_args: Vec<Value>,
    pub returning: MockBehavior,
}

#[derive(Clone)]
pub enum Times {
    Once,
    Never,
    AtLeast(usize),
    AtMost(usize),
    Exactly(usize),
    Range(usize, usize),
}

// Реализация для ReviewService
impl MockFactory<MockReviewService> for MockReviewService {
    fn create_mock() -> MockReviewService {
        let mut mock = MockReviewService::new();
        
        // Стандартное поведение
        mock.expect_create_review()
            .returning(|input| {
                Ok(Review {
                    id: Some(Uuid::new_v4()),
                    offer_id: input.offer_id,
                    author_id: input.author_id,
                    content: input.content,
                    rating: input.rating,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    status: ReviewStatus::Published,
                })
            });
        
        mock.expect_get_review()
            .returning(|id| {
                Ok(Some(Review {
                    id: Some(id),
                    offer_id: Uuid::new_v4(),
                    author_id: Uuid::new_v4(),
                    content: "Mock review content".to_string(),
                    rating: 4,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    status: ReviewStatus::Published,
                }))
            });
        
        mock
    }
    
    fn with_behavior(behavior: MockBehavior) -> MockReviewService {
        let mut mock = MockReviewService::new();
        
        match behavior {
            MockBehavior::Success(value) => {
                mock.expect_create_review()
                    .returning(move |_| {
                        let review: Review = serde_json::from_value(value.clone())
                            .unwrap_or_else(|_| Review::default());
                        Ok(review)
                    });
            },
            MockBehavior::Error(error_msg) => {
                mock.expect_create_review()
                    .returning(move |_| Err(ServiceError::ValidationError(error_msg.clone())));
            },
            MockBehavior::Timeout(duration) => {
                mock.expect_create_review()
                    .returning(move |_| {
                        std::thread::sleep(duration);
                        Err(ServiceError::Timeout)
                    });
            },
            MockBehavior::Custom(func) => {
                mock.expect_create_review()
                    .returning(move |_| {
                        match func() {
                            Ok(value) => {
                                let review: Review = serde_json::from_value(value)
                                    .unwrap_or_else(|_| Review::default());
                                Ok(review)
                            },
                            Err(error) => Err(ServiceError::CustomError(error)),
                        }
                    });
            },
            _ => {
                // Обработка других типов поведения
                mock = Self::create_mock();
            }
        }
        
        mock
    }
    
    fn with_expectations(expectations: Vec<MockExpectation>) -> MockReviewService {
        let mut mock = MockReviewService::new();
        
        for expectation in expectations {
            match expectation.method.as_str() {
                "create_review" => {
                    let mut expect = mock.expect_create_review();
                    
                    // Настройка количества вызовов
                    match expectation.times {
                        Times::Once => { expect.times(1); },
                        Times::Never => { expect.times(0); },
                        Times::AtLeast(n) => { expect.times(n..); },
                        Times::AtMost(n) => { expect.times(..=n); },
                        Times::Exactly(n) => { expect.times(n); },
                        Times::Range(min, max) => { expect.times(min..=max); },
                    }
                    
                    // Настройка возвращаемого значения
                    match expectation.returning {
                        MockBehavior::Success(value) => {
                            expect.returning(move |_| {
                                let review: Review = serde_json::from_value(value.clone())
                                    .unwrap_or_else(|_| Review::default());
                                Ok(review)
                            });
                        },
                        MockBehavior::Error(error) => {
                            expect.returning(move |_| {
                                Err(ServiceError::ValidationError(error.clone()))
                            });
                        },
                        _ => {
                            expect.returning(|_| Ok(Review::default()));
                        }
                    }
                },
                _ => {
                    // Обработка других методов
                }
            }
        }
        
        mock
    }
}

// Вспомогательные функции для создания ожиданий
impl MockExpectation {
    pub fn create_review_success() -> Self {
        Self {
            method: "create_review".to_string(),
            times: Times::Once,
            with_args: vec![],
            returning: MockBehavior::Success(serde_json::json!({
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "content": "Test review",
                "rating": 5
            })),
        }
    }
    
    pub fn create_review_error(error: &str) -> Self {
        Self {
            method: "create_review".to_string(),
            times: Times::Once,
            with_args: vec![],
            returning: MockBehavior::Error(error.to_string()),
        }
    }
}
```