# Deployment Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_DEPLOYMENT.puml

### Назначение диаграммы
Диаграмма развертывания показывает физическое размещение компонентов тестовой инфраструктуры Task 10 
в различных окружениях и их взаимодействие на уровне инфраструктуры.

### Окружения развертывания

#### 1. GitHub Cloud (CI/CD Pipeline)

##### Unit Test Runner
**Платформа:** GitHub Actions Runner
**Назначение:** Быстрое выполнение изолированных unit тестов

**Реализация в GitHub Actions:**
```yaml
# .github/workflows/unit-tests.yml
name: Unit Tests
on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          override: true
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run unit tests
        run: |
          cargo test --lib --bins --tests
          cargo test --doc
      
      - name: Generate coverage
        run: |
          cargo install tarpaulin
          cargo tarpaulin --out xml --output-dir coverage/
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          file: coverage/cobertura.xml
```

**Связь с кодом:**
```rust
// tests/unit/mod.rs
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_review_resolver_unit() {
        let mock_service = MockReviewService::new();
        mock_service.expect_create_review()
            .returning(|_| Ok(Review::default()));
        
        let resolver = ReviewResolver::new(Arc::new(mock_service));
        let result = resolver.create_review(CreateReviewInput::default()).await;
        
        assert!(result.is_ok());
    }
}
```##### I
ntegration Test Runner
**Платформа:** GitHub Actions Runner + Services
**Назначение:** Тестирование с реальными зависимостями (БД, кеш)

**Реализация в GitHub Actions:**
```yaml
# .github/workflows/integration-tests.yml
name: Integration Tests
on: [push, pull_request]

jobs:
  integration-tests:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: test_password
          POSTGRES_USER: test_user
          POSTGRES_DB: test_ugc_db
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Run database migrations
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_ugc_db
        run: |
          cargo install sqlx-cli
          sqlx migrate run
      
      - name: Run integration tests
        env:
          DATABASE_URL: postgresql://test_user:test_password@localhost:5432/test_ugc_db
          REDIS_URL: redis://localhost:6379
        run: |
          cargo test --test integration -- --test-threads=1
```

**Связь с кодом:**
```rust
// tests/integration/database_tests.rs
#[tokio::test]
async fn test_review_database_integration() {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    
    // Тест создания отзыва с реальной БД
    let review_input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        content: "Integration test review".to_string(),
        rating: 4,
    };
    
    let review_service = ReviewService::new(pool.clone());
    let created_review = review_service.create_review(review_input).await
        .expect("Failed to create review");
    
    assert!(created_review.id.is_some());
    
    // Очистка после теста
    sqlx::query("DELETE FROM reviews WHERE id = $1")
        .bind(created_review.id.unwrap())
        .execute(&pool)
        .await
        .expect("Failed to cleanup test data");
}
```

#### 2. Local Development Environment

##### Local Test Environment
**Платформа:** Docker Compose + Local Services
**Назначение:** Быстрая разработка и отладка тестов

**Реализация Docker Compose:**
```yaml
# docker-compose.test.yml
version: '3.8'

services:
  postgres-test:
    image: postgres:14
    environment:
      POSTGRES_DB: test_ugc_db
      POSTGRES_USER: test_user
      POSTGRES_PASSWORD: test_password
    ports:
      - "5433:5432"
    volumes:
      - postgres_test_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U test_user -d test_ugc_db"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis-test:
    image: redis:7-alpine
    ports:
      - "6380:6379"
    command: redis-server --appendonly yes
    volumes:
      - redis_test_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

  wiremock:
    image: wiremock/wiremock:latest
    ports:
      - "8080:8080"
    volumes:
      - ./tests/fixtures/wiremock:/home/wiremock
    command: --global-response-templating --verbose

volumes:
  postgres_test_data:
  redis_test_data:
```

**Связь с кодом:**
```rust
// src/testing/local_environment.rs
pub struct LocalTestEnvironment {
    postgres_container: Option<Container<'static, Postgres>>,
    redis_container: Option<Container<'static, Redis>>,
    wiremock_server: Option<MockServer>,
}

impl LocalTestEnvironment {
    pub async fn setup() -> Result<Self, TestError> {
        // Проверяем, запущен ли Docker Compose
        if Self::is_docker_compose_running().await? {
            return Ok(Self::from_docker_compose().await?);
        }
        
        // Иначе используем Testcontainers
        let postgres = Postgres::default()
            .with_db_name("test_ugc_db")
            .with_user("test_user")
            .with_password("test_password")
            .start()
            .await?;
            
        let redis = Redis::default().start().await?;
        
        let wiremock = MockServer::start().await;
        
        Ok(Self {
            postgres_container: Some(postgres),
            redis_container: Some(redis),
            wiremock_server: Some(wiremock),
        })
    }
    
    async fn is_docker_compose_running() -> Result<bool, TestError> {
        let output = tokio::process::Command::new("docker-compose")
            .args(&["-f", "docker-compose.test.yml", "ps", "-q"])
            .output()
            .await?;
            
        Ok(!output.stdout.is_empty())
    }
}
```