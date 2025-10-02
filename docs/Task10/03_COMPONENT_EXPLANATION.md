# Component Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_COMPONENT.puml

### Назначение диаграммы
Диаграмма компонентов показывает детальную структуру компонентов внутри каждого контейнера
тестовой инфраструктуры Task 10.

### Unit Testing Components

#### Test Execution Engine (Движок выполнения тестов)

##### Test Runner
**Технологии:** Rust, cargo test
**Функции:**
- Обнаружение тестов
- Параллельное выполнение
- Сбор результатов
- Отчеты об ошибках

**Реализация в коде:**
```rust
// src/testing/execution/runner.rs
pub struct TestRunner {
    config: TestConfig,
    executor: TestExecutor,
    reporter: TestReporter,
}

impl TestRunner {
    pub async fn discover_tests(&self, path: &Path) -> Vec<TestCase> {
        let mut tests = Vec::new();
        
        // Сканирование директорий для поиска тестов
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.path().extension() == Some(OsStr::new("rs")) {
                let test_cases = self.parse_test_file(&entry.path()).await?;
                tests.extend(test_cases);
            }
        }
        
        tests
    }
    
    pub async fn execute_parallel(&self, tests: Vec<TestCase>) -> TestResults {
        let semaphore = Arc::new(Semaphore::new(self.config.max_parallel_tests));
        let results = Arc::new(Mutex::new(Vec::new()));
        
        let handles: Vec<_> = tests.into_iter().map(|test| {
            let semaphore = semaphore.clone();
            let results = results.clone();
            
            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                let result = test.execute().await;
                results.lock().await.push(result);
            })
        }).collect();
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        TestResults::new(Arc::try_unwrap(results).unwrap().into_inner())
    }
}
```##
### Test Harness
**Технологии:** Rust, tokio-test
**Функции:**
- Поддержка асинхронных тестов
- Изоляция тестов
- Управление ресурсами
- Обработка очистки

**Реализация в коде:**
```rust
// src/testing/harness/mod.rs
pub struct TestHarness {
    runtime: Runtime,
    isolation_manager: IsolationManager,
    resource_manager: ResourceManager,
    cleanup_handlers: Vec<CleanupHandler>,
}

impl TestHarness {
    pub async fn setup_test_environment(&mut self) -> Result<TestEnvironment, TestError> {
        // Создание изолированной среды для теста
        let isolation_id = self.isolation_manager.create_isolation().await?;
        
        // Настройка ресурсов
        let resources = self.resource_manager.allocate_resources(isolation_id).await?;
        
        // Регистрация обработчиков очистки
        let cleanup = CleanupHandler::new(isolation_id, resources.clone());
        self.cleanup_handlers.push(cleanup);
        
        Ok(TestEnvironment {
            isolation_id,
            resources,
            database_pool: self.setup_test_database().await?,
            redis_client: self.setup_test_redis().await?,
        })
    }
    
    pub async fn cleanup_after_test(&mut self, env: TestEnvironment) -> Result<(), TestError> {
        // Откат транзакций
        if let Some(tx) = env.database_transaction {
            tx.rollback().await?;
        }
        
        // Очистка Redis
        env.redis_client.flushdb().await?;
        
        // Освобождение ресурсов
        self.resource_manager.deallocate_resources(env.isolation_id).await?;
        
        Ok(())
    }
}
```

#### Mocking System (Система мокирования)

##### Mock Generator
**Технологии:** Rust, Mockall
**Функции:**
- Мокирование трейтов
- Заглушки методов
- Верификация вызовов
- Конфигурация поведения

**Реализация в коде:**
```rust
// src/testing/mocks/generator.rs
pub struct MockGenerator {
    mock_registry: MockRegistry,
    behavior_config: BehaviorConfig,
}

impl MockGenerator {
    pub fn generate_service_mock<T>(&self) -> T 
    where 
        T: MockableService 
    {
        let mut mock = T::new_mock();
        
        // Применение стандартного поведения
        self.apply_default_behavior(&mut mock);
        
        // Применение кастомной конфигурации
        if let Some(config) = self.behavior_config.get_config::<T>() {
            self.apply_custom_behavior(&mut mock, config);
        }
        
        mock
    }
    
    fn apply_default_behavior<T>(&self, mock: &mut T) 
    where 
        T: MockableService 
    {
        // Настройка стандартных ожиданий
        mock.expect_health_check()
            .returning(|| Ok(HealthStatus::Healthy));
            
        mock.expect_get_metrics()
            .returning(|| Ok(ServiceMetrics::default()));
    }
}
```