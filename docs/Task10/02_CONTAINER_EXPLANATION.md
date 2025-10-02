# Container Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CONTAINER.puml

### Назначение диаграммы
Диаграмма контейнеров детализирует основные контейнеры тестовой системы Task 10,
показывая их внутреннюю структуру и взаимодействие.

### Слои тестирования

#### 1. Unit Testing Layer (Слой unit тестирования)

##### Unit Test Runner
**Технологии:** Rust, cargo test
**Функции:**
- Выполнение unit тестов
- Тестирование резолверов
- Тестирование бизнес-логики сервисов
- Обработка ошибок

**Реализация в коде:**
```rust
// src/testing/unit/runner.rs
pub struct UnitTestRunner {
    config: TestConfig,
    test_registry: TestRegistry,
}

impl UnitTestRunner {
    pub async fn run_resolver_tests(&self) -> TestResult {
        let tests = self.test_registry.get_resolver_tests();
        for test in tests {
            test.execute().await?;
        }
        Ok(())
    }
    
    pub async fn run_service_tests(&self) -> TestResult {
        let tests = self.test_registry.get_service_tests();
        for test in tests {
            test.execute().await?;
        }
        Ok(())
    }
}
```

##### Mock Framework
**Технологии:** Rust, Mockall
**Функции:**
- Создание моков внешних сервисов
- Мокирование базы данных
- Dependency injection
- Верификация поведения

**Реализация в коде:**
```rust
// src/testing/mocks/framework.rs
use mockall::predicate::*;
use mockall::mock;

mock! {
    ReviewService {}
    
    #[async_trait]
    impl ReviewServiceTrait for ReviewService {
        async fn create_review(&self, input: CreateReviewInput) -> Result<Review, ServiceError>;
        async fn get_review(&self, id: Uuid) -> Result<Option<Review>, ServiceError>;
    }
}

pub struct MockFramework {
    review_service: MockReviewService,
    user_service: MockUserService,
}
```#
### 2. Integration Testing Layer (Слой интеграционного тестирования)

##### Integration Test Runner
**Технологии:** Rust, Testcontainers
**Функции:**
- Интеграция с базой данных
- Коммуникация между сервисами
- Тестирование аутентификации
- Федеративные запросы

**Реализация в коде:**
```rust
// src/testing/integration/runner.rs
pub struct IntegrationTestRunner {
    db_container: DatabaseContainer,
    redis_container: RedisContainer,
    test_client: GraphQLTestClient,
}

impl IntegrationTestRunner {
    pub async fn run_database_tests(&self) -> TestResult {
        let pool = self.db_container.get_pool().await?;
        
        // Тест создания отзыва с реальной БД
        let review_input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            author_id: Uuid::new_v4(),
            content: "Test review".to_string(),
            rating: 5,
        };
        
        let review = create_review(&pool, review_input).await?;
        assert!(review.id.is_some());
        Ok(())
    }
}
```

##### Test Database Manager
**Технологии:** PostgreSQL, Testcontainers
**Функции:**
- Управление жизненным циклом контейнеров
- Миграции схемы
- Настройка тестовых данных
- Изоляция транзакций

**Реализация в коде:**
```rust
// src/testing/database/manager.rs
pub struct TestDatabaseManager {
    container: Container<'static, Postgres>,
    pool: PgPool,
}

impl TestDatabaseManager {
    pub async fn new() -> Result<Self, TestError> {
        let container = Postgres::default()
            .with_db_name("test_ugc_db")
            .with_user("test_user")
            .with_password("test_password")
            .start()
            .await?;
            
        let connection_string = format!(
            "postgresql://test_user:test_password@localhost:{}/test_ugc_db",
            container.get_host_port_ipv4(5432).await?
        );
        
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&connection_string)
            .await?;
            
        // Запуск миграций
        sqlx::migrate!("./migrations").run(&pool).await?;
        
        Ok(Self { container, pool })
    }
}
```