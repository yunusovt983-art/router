# Context Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CONTEXT.puml

### Назначение диаграммы
Контекстная диаграмма показывает высокоуровневое представление тестовой системы Task 10 
и её взаимодействие с внешними участниками и системами.

### Ключевые участники

#### 1. Разработчик (Developer)
**Роль:** Пишет и запускает тесты для обеспечения качества кода и функциональности
**Взаимодействие с системой:**
- Разработка и запуск тестов через UGC Testing Suite
- Локальная разработка и выполнение тестов

**Связь с кодом:**
```rust
// tests/unit/resolver_tests.rs
#[cfg(test)]
mod resolver_tests {
    use super::*;
    use crate::testing::TestHarness;
    
    #[tokio::test]
    async fn test_create_review_resolver() {
        let harness = TestHarness::new().await;
        // Тест логики резолвера
    }
}
```

#### 2. QA Инженер (QA Engineer)
**Роль:** Создает тестовые сценарии и проводит тестирование качества системы
**Взаимодействие с системой:**
- Тестирование федерации через Federation Testing Framework
- Ручное тестирование и валидация сценариев

**Связь с кодом:**
```rust
// tests/e2e/user_journey_tests.rs
#[tokio::test]
async fn test_complete_review_workflow() {
    let scenario = TestScenario::new("Complete Review Workflow")
        .add_step(CreateUserStep::new())
        .add_step(CreateReviewStep::new())
        .add_step(ValidateReviewStep::new());
    
    scenario.execute().await.expect("Scenario should pass");
}
```#
### 3. DevOps Инженер (DevOps Engineer)
**Роль:** Настраивает CI/CD pipeline и автоматизирует выполнение тестов
**Взаимодействие с системой:**
- Настройка автоматизации через Test Automation Platform
- Конфигурация CI/CD и управление пайплайнами

**Связь с кодом:**
```yaml
# .github/workflows/test.yml
name: Comprehensive Testing
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run Unit Tests
        run: cargo test --lib
      - name: Run Integration Tests  
        run: cargo test --test integration
```

### Основные системы

#### 1. Auto.ru Comprehensive Testing System
**Компоненты:**
- **UGC Testing Suite** - комплексный тестовый набор
- **Federation Testing Framework** - тестирование федерации
- **Test Automation Platform** - автоматизация тестирования

**Реализация в коде:**
```rust
// src/testing/mod.rs
pub struct ComprehensiveTestingSystem {
    pub ugc_suite: UGCTestingSuite,
    pub federation_framework: FederationTestingFramework,
    pub automation_platform: TestAutomationPlatform,
}

impl ComprehensiveTestingSystem {
    pub async fn new() -> Result<Self, TestError> {
        Ok(Self {
            ugc_suite: UGCTestingSuite::new().await?,
            federation_framework: FederationTestingFramework::new().await?,
            automation_platform: TestAutomationPlatform::new().await?,
        })
    }
}
```

#### 2. Testing Infrastructure & Tools
**Внешние системы:**
- Unit Testing Framework (Rust встроенный фреймворк)
- Integration Testing Platform (Testcontainers)
- Contract Testing System (Pact framework)

**Реализация инфраструктуры:**
```rust
// src/testing/infrastructure.rs
pub struct TestingInfrastructure {
    pub unit_framework: UnitTestingFramework,
    pub integration_platform: IntegrationTestingPlatform,
    pub contract_system: ContractTestingSystem,
}
```