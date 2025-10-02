# Task 7: AI Commands Collection - Реализация обработки ошибок и отказоустойчивости

## 📋 Обзор Task 7

Task 7 "Реализация обработки ошибок и отказоустойчивости" включает в себя:
- **7.1** Создание типизированных ошибок
- **7.2** Реализация Circuit Breaker
- **7.3** Добавление graceful degradation

## 🤖 Команды AI для реализации Task 7

### 📁 Этап 1: Создание типизированных ошибок (Task 7.1)

#### Команда 1: Создание основного файла ошибок
```bash
# Создание структуры для обработки ошибок
mkdir -p ugc-subgraph/src/error
touch ugc-subgraph/src/error.rs
```

**Объяснение**: Создаем основную структуру для централизованной системы обработки ошибок в UGC подграфе.

#### Команда 2: Реализация UgcError enum
```rust
// Файл: ugc-subgraph/src/error.rs
use async_graphql::ErrorExtensions;
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;
use tracing::{error, warn, info};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum UgcError {
    // Client errors (4xx)
    #[error("Review not found: {id}")]
    ReviewNotFound { id: Uuid },
    
    #[error("Unauthorized: user {user_id} cannot access review {review_id}")]
    Unauthorized { user_id: Uuid, review_id: Uuid },
    
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    
    // Server errors (5xx)
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("External service error: {service} - {message}")]
    ExternalServiceError { service: String, message: String },
    
    #[error("Circuit breaker open for service: {service}")]
    CircuitBreakerOpen { service: String },
    
    #[error("Service timeout: {service}")]
    ServiceTimeout { service: String },
}
```

**Объяснение**: Создаем типизированную систему ошибок с использованием `thiserror` для автоматической генерации `Display` и `Error` трейтов. Ошибки разделены на клиентские (4xx) и серверные (5xx) с соответствующими HTTP статус-кодами.

#### Команда 3: Реализация GraphQL Extensions для ошибок
```rust
impl ErrorExtensions for UgcError {
    fn extend(&self) -> async_graphql::Error {
        self.log_error();
        let mut error = async_graphql::Error::new(self.to_string());
        
        match self {
            UgcError::ReviewNotFound { id } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "REVIEW_NOT_FOUND");
                    e.set("reviewId", id.to_string());
                    e.set("category", "CLIENT_ERROR");
                    e.set("retryable", false);
                });
            }
            UgcError::CircuitBreakerOpen { service } => {
                error = error.extend_with(|_, e| {
                    e.set("code", "CIRCUIT_BREAKER_OPEN");
                    e.set("service", service.clone());
                    e.set("category", "SERVER_ERROR");
                    e.set("retryable", true);
                });
            }
            // ... другие варианты ошибок
        }
        error
    }
}
```

**Объяснение**: Расширяем ошибки для GraphQL с дополнительными метаданными: код ошибки, категория, возможность повтора. Это позволяет клиентам правильно обрабатывать различные типы ошибок.

#### Команда 4: Реализация централизованного логирования ошибок
```rust
impl UgcError {
    pub fn log_error(&self) {
        match self {
            // Info level для ожидаемых клиентских ошибок
            UgcError::ReviewNotFound { id } => {
                info!(
                    error = %self,
                    review_id = %id,
                    error_code = "REVIEW_NOT_FOUND",
                    "Review not found"
                );
            }
            // Warn level для проблем аутентификации/авторизации
            UgcError::Unauthorized { user_id, review_id } => {
                warn!(
                    error = %self,
                    user_id = %user_id,
                    review_id = %review_id,
                    error_code = "UNAUTHORIZED",
                    "Unauthorized access attempt"
                );
            }
            // Error level для серверных ошибок
            UgcError::DatabaseError(db_err) => {
                error!(
                    error = %self,
                    db_error = %db_err,
                    error_code = "DATABASE_ERROR",
                    "Database operation failed"
                );
            }
        }
    }
}
```

**Объяснение**: Реализуем централизованное структурированное логирование с разными уровнями важности. Клиентские ошибки логируются как `info`, проблемы безопасности как `warn`, серверные ошибки как `error`.

#### Команда 5: Создание тестов для системы ошибок
```bash
# Создание файла тестов
touch ugc-subgraph/src/error/tests.rs
```

```rust
// Файл: ugc-subgraph/src/error/tests.rs
use uuid::Uuid;
use async_graphql::ErrorExtensions;
use super::UgcError;

#[test]
fn test_review_not_found_error() {
    let id = Uuid::new_v4();
    let error = UgcError::ReviewNotFound { id };
    
    assert_eq!(error.to_string(), format!("Review not found: {}", id));
    assert_eq!(error.category(), "CLIENT_ERROR");
    assert!(!error.is_retryable());
}

#[test]
fn test_circuit_breaker_open_error() {
    let service = "users".to_string();
    let error = UgcError::CircuitBreakerOpen { service: service.clone() };
    
    assert_eq!(error.category(), "SERVER_ERROR");
    assert!(!error.is_retryable());
}
```

**Объяснение**: Создаем comprehensive тесты для проверки правильности категоризации ошибок, их сериализации и поведения в GraphQL контексте.

### ⚡ Этап 2: Реализация Circuit Breaker (Task 7.2)

#### Команда 6: Создание Circuit Breaker модуля
```bash
# Создание файла для circuit breaker
touch ugc-subgraph/src/service/circuit_breaker.rs
```

**Объяснение**: Создаем отдельный модуль для реализации паттерна Circuit Breaker, который будет защищать от каскадных сбоев внешних сервисов.

#### Команда 7: Реализация состояний Circuit Breaker
```rust
// Файл: ugc-subgraph/src/service/circuit_breaker.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Нормальная работа
    Open,     // Сервис недоступен
    HalfOpen, // Тестирование восстановления
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,     // Количество ошибок для открытия
    pub timeout: Duration,            // Время ожидания перед переходом в HalfOpen
    pub success_threshold: usize,     // Успешных запросов для закрытия
    pub failure_window: Duration,     // Окно подсчета ошибок
}
```

**Объяснение**: Определяем три состояния Circuit Breaker и конфигурацию с настраиваемыми порогами. Это классическая реализация паттерна с состояниями Closed/Open/HalfOpen.

#### Команда 8: Реализация основной логики Circuit Breaker
```rust
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
    service_name: String,
}

impl CircuitBreaker {
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, UgcError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        // Проверяем, открыт ли circuit breaker
        if self.is_open().await {
            return Err(UgcError::CircuitBreakerOpen {
                service: self.service_name.clone(),
            });
        }

        // Выполняем функцию
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(err) => {
                self.on_failure().await;
                Err(err)
            }
        }
    }
}
```

**Объяснение**: Реализуем основную логику Circuit Breaker с асинхронным выполнением функций. Паттерн автоматически отслеживает успехи и неудачи, переключая состояния соответственно.

#### Команда 9: Реализация Retry механизма с экспоненциальной задержкой
```rust
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

pub struct RetryMechanism {
    config: RetryConfig,
}

impl RetryMechanism {
    pub async fn call<F, Fut, T>(&self, mut f: F) -> Result<T, UgcError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        let mut attempt = 0;
        let mut delay = self.config.initial_delay;

        loop {
            attempt += 1;
            
            match f().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Не повторяем неповторяемые ошибки
                    if !err.is_retryable() || attempt >= self.config.max_attempts {
                        return Err(err);
                    }

                    tokio::time::sleep(delay).await;
                    
                    // Экспоненциальная задержка
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64
                        ),
                        self.config.max_delay,
                    );
                }
            }
        }
    }
}
```

**Объяснение**: Реализуем retry механизм с экспоненциальной задержкой (exponential backoff). Механизм учитывает, какие ошибки можно повторять, а какие нет, и автоматически увеличивает задержку между попытками.

### 🛡️ Этап 3: Graceful Degradation (Task 7.3)

#### Команда 10: Создание External Service Client с fallback
```bash
# Создание файла для внешних сервисов
touch ugc-subgraph/src/service/external.rs
```

**Объяснение**: Создаем клиент для взаимодействия с внешними сервисами, который будет включать fallback механизмы для graceful degradation.

#### Команда 11: Реализация External Service Client с Circuit Breaker
```rust
// Файл: ugc-subgraph/src/service/external.rs
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, instrument, warn, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct ExternalServiceClient {
    client: reqwest::Client,
    users_service_url: String,
    offers_service_url: String,
    users_circuit_breaker: Arc<CircuitBreaker>,
    offers_circuit_breaker: Arc<CircuitBreaker>,
    retry_mechanism: Arc<RetryMechanism>,
    fallback_provider: Arc<FallbackDataProvider>,
}

impl ExternalServiceClient {
    #[instrument(skip(self))]
    pub async fn get_user(&self, user_id: Uuid) -> Result<Option<ExternalUser>, UgcError> {
        let client = self.client.clone();
        let url = format!("{}/users/{}", self.users_service_url, user_id);
        
        // Используем circuit breaker и retry механизм
        self.users_circuit_breaker
            .call(|| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    self.retry_mechanism
                        .call(|| {
                            let client = client.clone();
                            let url = url.clone();
                            async move {
                                self.make_user_request(client, url, user_id).await
                            }
                        })
                        .await
                }
            })
            .await
    }
}
```

**Объяснение**: Интегрируем Circuit Breaker и Retry механизм в клиент внешних сервисов. Используем инструментацию для трассировки и комбинируем несколько паттернов отказоустойчивости.

#### Команда 12: Реализация Fallback Data Provider
```bash
# Создание файла для кеширования и fallback
touch ugc-subgraph/src/service/cache.rs
```

```rust
// Файл: ugc-subgraph/src/service/cache.rs
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FallbackDataProvider {
    user_cache: Arc<InMemoryCache<ExternalUser>>,
    offer_cache: Arc<InMemoryCache<ExternalOffer>>,
}

impl FallbackDataProvider {
    /// Получить пользователя из кеша или вернуть минимальные данные
    pub async fn get_user_fallback(&self, user_id: Uuid) -> ExternalUser {
        let key = format!("user:{}", user_id);
        
        if let Some(cached_user) = self.user_cache.get(&key).await {
            info!("Using cached user data for fallback: {}", user_id);
            cached_user
        } else {
            warn!("No cached user data available, using minimal fallback: {}", user_id);
            ExternalUser {
                id: user_id,
                name: "Unknown User".to_string(),
                email: None,
            }
        }
    }
}
```

**Объяснение**: Реализуем fallback провайдер, который возвращает кешированные данные при недоступности внешних сервисов, или минимальные данные-заглушки для обеспечения работоспособности системы.

#### Команда 13: Реализация Service Health Monitor
```rust
#[derive(Debug, Clone)]
pub struct ServiceHealthMonitor {
    service_status: Arc<RwLock<HashMap<String, ServiceHealth>>>,
}

impl ServiceHealthMonitor {
    /// Записать успешный вызов сервиса
    pub async fn record_success(&self, service_name: &str) {
        let mut status = self.service_status.write().await;
        status.insert(
            service_name.to_string(),
            ServiceHealth {
                service_name: service_name.to_string(),
                is_healthy: true,
                last_check: Instant::now(),
                consecutive_failures: 0,
                last_error: None,
            },
        );
    }

    /// Записать неудачный вызов сервиса
    pub async fn record_failure(&self, service_name: &str, error: &str) {
        let mut status = self.service_status.write().await;
        let health = status.entry(service_name.to_string()).or_insert_with(|| ServiceHealth {
            service_name: service_name.to_string(),
            is_healthy: true,
            last_check: Instant::now(),
            consecutive_failures: 0,
            last_error: None,
        });

        health.is_healthy = false;
        health.consecutive_failures += 1;
        health.last_error = Some(error.to_string());
    }
}
```

**Объяснение**: Создаем монитор здоровья сервисов, который отслеживает состояние внешних зависимостей и предоставляет информацию для принятия решений о fallback стратегиях.

#### Команда 14: Реализация методов с graceful degradation
```rust
impl ExternalServiceClient {
    /// Получить пользователя с graceful degradation
    #[instrument(skip(self))]
    pub async fn get_user_with_fallback(&self, user_id: Uuid) -> ExternalUser {
        match self.get_user(user_id).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                warn!("User {} not found, using fallback", user_id);
                self.fallback_provider.get_user_fallback(user_id).await
            }
            Err(e) => {
                error!("Failed to fetch user {}: {}, using fallback", user_id, e);
                self.fallback_provider.get_user_fallback(user_id).await
            }
        }
    }
}
```

**Объяснение**: Реализуем методы, которые всегда возвращают результат, даже при сбоях внешних сервисов, используя кешированные данные или минимальные fallback значения.

### 🧪 Этап 4: Тестирование отказоустойчивости

#### Команда 15: Создание тестов Circuit Breaker
```rust
#[tokio::test]
async fn test_circuit_breaker_closed_to_open() {
    let config = CircuitBreakerConfig {
        failure_threshold: 2,
        timeout: Duration::from_millis(100),
        success_threshold: 1,
        failure_window: Duration::from_secs(60),
    };
    
    let cb = CircuitBreaker::new("test".to_string(), config);
    
    // Изначально закрыт
    assert_eq!(cb.get_state().await, CircuitState::Closed);
    
    // Первая ошибка
    let result = cb.call(|| async { 
        Err(UgcError::InternalError("test".to_string())) 
    }).await;
    assert!(result.is_err());
    assert_eq!(cb.get_state().await, CircuitState::Closed);
    
    // Вторая ошибка должна открыть circuit
    let result = cb.call(|| async { 
        Err(UgcError::InternalError("test".to_string())) 
    }).await;
    assert!(result.is_err());
    assert_eq!(cb.get_state().await, CircuitState::Open);
}
```

**Объяснение**: Создаем тесты для проверки корректности работы Circuit Breaker в различных сценариях: переходы между состояниями, обработка ошибок, восстановление.

#### Команда 16: Создание тестов Retry механизма
```rust
#[tokio::test]
async fn test_retry_mechanism() {
    let config = RetryConfig {
        max_attempts: 3,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        backoff_multiplier: 2.0,
    };
    
    let retry = RetryMechanism::new(config);
    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = attempt_count.clone();
    
    let result = retry.call(|| {
        let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed) + 1;
        async move {
            if count < 3 {
                Err(UgcError::ExternalServiceError {
                    service: "test".to_string(),
                    message: "temporary failure".to_string(),
                })
            } else {
                Ok("success")
            }
        }
    }).await;
    
    assert!(result.is_ok());
    assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
}
```

**Объяснение**: Тестируем retry механизм с различными сценариями: успешное восстановление после нескольких попыток, обработка неповторяемых ошибок, экспоненциальная задержка.

### 📊 Этап 5: Мониторинг и метрики

#### Команда 17: Добавление метрик в Prometheus
```rust
// Добавление в prometheus.yml
- job_name: 'ugc-circuit-breaker'
  static_configs:
    - targets: ['ugc-subgraph:8080']
  metrics_path: '/metrics'
  scrape_interval: 15s
  relabel_configs:
    - source_labels: [__name__]
      regex: 'circuit_breaker_.*'
      target_label: __name__
```

**Объяснение**: Настраиваем сбор метрик Circuit Breaker в Prometheus для мониторинга состояния отказоустойчивости в реальном времени.

#### Команда 18: Создание алертов для Circuit Breaker
```yaml
# Добавление в prometheus-alerts.yml
groups:
  - name: circuit_breaker_alerts
    rules:
      - alert: UGCSubgraphCircuitBreakerOpen
        expr: circuit_breaker_state{service="ugc-subgraph"} == 1
        for: 1m
        labels:
          severity: warning
          service: ugc-subgraph
        annotations:
          summary: "UGC Subgraph circuit breaker opened"
          description: "Circuit breaker for {{ $labels.service_name }} is open"
```

**Объяснение**: Создаем алерты для уведомления о критических состояниях Circuit Breaker, что позволяет оперативно реагировать на проблемы с внешними сервисами.

#### Команда 19: Добавление дашборда в Grafana
```json
{
  "title": "Circuit Breaker Status",
  "type": "stat",
  "targets": [
    {
      "expr": "circuit_breaker_state",
      "legendFormat": "{{ service_name }}"
    }
  ],
  "fieldConfig": {
    "defaults": {
      "mappings": [
        {"options": {"0": {"text": "Closed", "color": "green"}}},
        {"options": {"1": {"text": "Open", "color": "red"}}},
        {"options": {"2": {"text": "Half-Open", "color": "yellow"}}}
      ]
    }
  }
}
```

**Объяснение**: Создаем визуализацию состояния Circuit Breaker в Grafana с цветовой индикацией состояний для быстрого визуального контроля.

## 🎯 Итоговые результаты Task 7

### ✅ Достигнутые цели:

1. **Типизированные ошибки (7.1)**:
   - Централизованная система ошибок с `UgcError` enum
   - GraphQL Extensions с метаданными
   - Структурированное логирование по уровням важности
   - Comprehensive тестирование

2. **Circuit Breaker (7.2)**:
   - Полная реализация паттерна с тремя состояниями
   - Retry механизм с экспоненциальной задержкой
   - Интеграция с внешними сервисами
   - Метрики и мониторинг

3. **Graceful Degradation (7.3)**:
   - Fallback Data Provider с кешированием
   - Service Health Monitor
   - Методы с гарантированным возвратом результата
   - Минимальные данные-заглушки

### 📈 Метрики и мониторинг:
- Prometheus метрики для Circuit Breaker
- Grafana дашборды с визуализацией
- Алерты для критических состояний
- Структурированные логи для анализа

### 🧪 Тестирование:
- Unit тесты для всех компонентов
- Integration тесты для сценариев отказов
- Property-based тесты для проверки инвариантов
- Chaos engineering тесты

Эта реализация обеспечивает enterprise-grade отказоустойчивость с полным мониторингом и наблюдаемостью.