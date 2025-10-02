# Context Diagram - Подробное объяснение

## Файл: C4_ARCHITECTURE_CONTEXT.puml

### Назначение диаграммы
Контекстная диаграмма показывает высокоуровневое представление Task 11 - создания заглушек подграфов Users и Offers в рамках федеративной экосистемы Auto.ru.

### Ключевые участники системы

#### 1. Frontend Developer
**Роль:** Разрабатывает клиентские приложения используя федеративные GraphQL запросы
**Взаимодействие с системой:**
- Выполнение федеративных запросов через Apollo Router
- Тестирование cross-subgraph queries
- Разработка UI компонентов с данными из разных подграфов

**Связь с кодом:**
```typescript
// Frontend GraphQL query spanning multiple subgraphs
const GET_OFFER_WITH_DETAILS = gql`
  query GetOfferWithDetails($offerId: ID!) {
    offer(id: $offerId) {
      id
      title
      price
      car {
        make
        model
        year
      }
      seller {  # Resolved from Users subgraph
        id
        name
        email
      }
      reviews { # Resolved from UGC subgraph
        id
        content
        rating
      }
    }
  }
`;
```

#### 2. Backend Developer
**Роль:** Разрабатывает и поддерживает подграфы Users и Offers с моковыми данными
**Взаимодействие с системой:**
- Разработка GraphQL схем с federation директивами
- Реализация резолверов с моковыми данными
- Настройка entity resolution для федерации

**Связь с кодом:**
```rust
// users-subgraph/src/lib.rs
#[derive(SimpleObject, Clone)]
pub struct User {
    #[graphql(external)] // Federation directive
    pub id: ID,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

#[Object]
impl Query {
    // Entity resolver for federation
    async fn find_user_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }
}
```

#### 3. QA Engineer
**Роль:** Тестирует федеративную интеграцию между подграфами через заглушки
**Взаимодействие с системой:**
- Выполнение интеграционных тестов федерации
- Валидация cross-subgraph scenarios
- Тестирование entity resolution

**Связь с кодом:**
```rust
// tests/federation_tests.rs
#[tokio::test]
async fn test_cross_subgraph_entity_resolution() {
    let test_client = FederationTestClient::new().await;
    
    // Test that Offer can resolve User entity
    let query = r#"
        query TestOfferUserResolution($offerId: ID!) {
            offer(id: $offerId) {
                id
                seller {
                    id
                    name
                }
            }
        }
    "#;
    
    let response = test_client.execute(query, json!({"offerId": "offer-1"})).await;
    assert!(response.errors.is_empty());
    
    let offer = response.data["offer"].as_object().unwrap();
    let seller = offer["seller"].as_object().unwrap();
    assert_eq!(seller["id"], "user-1");
}
```

### Основные системы

#### 1. Auto.ru Federation Ecosystem

##### Apollo Router
**Назначение:** Федеративный шлюз для композиции и маршрутизации запросов
**Реализация в коде:**
```yaml
# router.yaml
supergraph:
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
    routing_url: http://offers-subgraph:4004/graphql
    schema:
      subgraph_url: http://offers-subgraph:4004/graphql
```

##### Users Subgraph (Stub)
**Назначение:** Заглушка для управления пользователями с моковыми данными
**Реализация в коде:**
```rust
// users-subgraph/src/lib.rs
pub struct UserService {
    users: Vec<User>,
}

impl UserService {
    pub fn new() -> Self {
        let users = vec![
            User {
                id: ID::from("user-1"),
                name: "Иван Иванов".to_string(),
                email: "ivan@example.com".to_string(),
                phone: Some("+7-900-123-45-67".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            // More mock users...
        ];
        Self { users }
    }
    
    pub async fn get_user_by_id(&self, id: &str) -> Option<User> {
        self.users.iter().find(|u| u.id == id).cloned()
    }
}
```

##### Offers Subgraph (Stub)
**Назначение:** Заглушка для объявлений о продаже авто с CRUD операциями
**Реализация в коде:**
```rust
// offers-subgraph/src/lib.rs
#[derive(SimpleObject, Clone)]
pub struct Offer {
    pub id: ID,
    pub title: String,
    pub price: i32,
    pub seller_id: ID, // Reference to User from Users subgraph
    pub car: Car,
    pub location: String,
    pub status: OfferStatus,
}

#[derive(SimpleObject, Clone)]
pub struct Car {
    pub make: String,
    pub model: String,
    pub year: i32,
    pub mileage: Option<i32>,
    pub fuel_type: FuelType,
}

impl OfferService {
    fn generate_mock_offers() -> Vec<Offer> {
        vec![
            Offer {
                id: ID::from("offer-1"),
                title: "Toyota Camry 2020 в отличном состоянии".to_string(),
                price: 2500000,
                seller_id: ID::from("user-1"), // Links to Users subgraph
                car: Car {
                    make: "Toyota".to_string(),
                    model: "Camry".to_string(),
                    year: 2020,
                    mileage: Some(45000),
                    fuel_type: FuelType::Gasoline,
                },
                location: "Москва".to_string(),
                status: OfferStatus::Active,
            },
            // More mock offers...
        ]
    }
}
```

#### 2. Development Infrastructure

##### Mock Data Generator
**Назначение:** Генерация реалистичных тестовых данных для заглушек
**Реализация в коде:**
```rust
// shared/src/mock_data.rs
use fake::{Fake, Faker};
use fake::locales::RU;

pub struct MockDataGenerator;

impl MockDataGenerator {
    pub fn generate_russian_users(count: usize) -> Vec<User> {
        (0..count).map(|i| {
            User {
                id: ID::from(format!("user-{}", i + 1)),
                name: fake::Name!(RU).fake(),
                email: format!("{}@example.com", fake::Internet().username()),
                phone: Some(format!("+7-{}", fake::PhoneNumber!(RU).fake())),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            }
        }).collect()
    }
    
    pub fn generate_car_offers(count: usize) -> Vec<Offer> {
        let car_makes = vec!["Toyota", "BMW", "Mercedes", "Audi", "Volkswagen"];
        let cities = vec!["Москва", "Санкт-Петербург", "Новосибирск", "Екатеринбург"];
        
        (0..count).map(|i| {
            let make = car_makes[i % car_makes.len()];
            Offer {
                id: ID::from(format!("offer-{}", i + 1)),
                title: format!("{} {} в отличном состоянии", make, 2018 + (i % 5)),
                price: (1500000..5000000).fake(),
                seller_id: ID::from(format!("user-{}", (i % 10) + 1)),
                car: Car {
                    make: make.to_string(),
                    model: format!("Model-{}", i % 3 + 1),
                    year: 2018 + (i % 5) as i32,
                    mileage: Some((10000..150000).fake()),
                    fuel_type: FuelType::Gasoline,
                },
                location: cities[i % cities.len()].to_string(),
                status: OfferStatus::Active,
            }
        }).collect()
    }
}
```

##### Federation Validator
**Назначение:** Валидация федеративной композиции и директив
**Реализация в коде:**
```rust
// tools/federation_validator/src/main.rs
use apollo_federation::Supergraph;

pub struct FederationValidator {
    subgraph_schemas: Vec<String>,
}

impl FederationValidator {
    pub async fn validate_composition(&self) -> Result<(), ValidationError> {
        let supergraph = Supergraph::compose(&self.subgraph_schemas)
            .map_err(|e| ValidationError::CompositionFailed(e.to_string()))?;
        
        // Validate federation directives
        self.validate_key_directives(&supergraph)?;
        self.validate_entity_resolution(&supergraph)?;
        self.validate_cross_subgraph_references(&supergraph)?;
        
        Ok(())
    }
    
    fn validate_key_directives(&self, supergraph: &Supergraph) -> Result<(), ValidationError> {
        // Check that all entities have proper @key directives
        for entity in supergraph.entities() {
            if entity.key_fields().is_empty() {
                return Err(ValidationError::MissingKeyDirective(entity.name().to_string()));
            }
        }
        Ok(())
    }
}
```

### Взаимодействия и потоки данных

#### Federation Query Flow
**Процесс:** Выполнение федеративного запроса через Apollo Router
**Реализация:**
```rust
// Example of federation query execution flow

// 1. Client sends query to Apollo Router
let federated_query = r#"
    query GetOfferWithSellerAndReviews($offerId: ID!) {
        offer(id: $offerId) {           # Resolved by Offers subgraph
            id
            title
            price
            seller {                    # Resolved by Users subgraph via federation
                id
                name
                email
            }
            reviews {                   # Resolved by UGC subgraph via federation
                id
                content
                rating
                author {                # Back to Users subgraph
                    name
                }
            }
        }
    }
"#;

// 2. Apollo Router creates query plan
// 3. Router executes queries to individual subgraphs
// 4. Subgraphs resolve their parts:

// Offers subgraph resolves:
impl Query {
    async fn offer(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Offer>> {
        let service = ctx.data::<Arc<OfferService>>()?;
        Ok(service.get_offer_by_id(&id).await)
    }
}

// Users subgraph resolves User entities:
impl Query {
    async fn find_user_by_id(&self, ctx: &Context<'_>, id: ID) -> Result<Option<User>> {
        let service = ctx.data::<Arc<UserService>>()?;
        Ok(service.get_user_by_id(&id).await)
    }
}

// 5. Apollo Router aggregates responses and returns to client
```

#### Cross-Subgraph Entity Resolution
**Процесс:** Разрешение сущностей между подграфами
**Реализация:**
```rust
// Federation entity resolution implementation
use async_graphql::*;

// In Offers subgraph - extending User type
#[derive(SimpleObject)]
#[graphql(extends)]
pub struct User {
    #[graphql(external)]
    pub id: ID,
    
    // Add offers field to User type from Users subgraph
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

// Entity resolver for User references
pub async fn resolve_user_reference(
    ctx: &Context<'_>,
    reference: UserReference,
) -> Result<Option<User>> {
    // This gets called when other subgraphs reference User entities
    let service = ctx.data::<Arc<UserService>>()?;
    Ok(service.get_user_by_id(&reference.id).await)
}
```

### Ключевые архитектурные решения

#### 1. Stub-First Approach
**Решение:** Создание заглушек перед полной реализацией
**Обоснование:** Позволяет быстро прототипировать федеративную архитектуру
**Реализация:** In-memory data stores с моковыми данными

#### 2. Federation Compliance
**Решение:** Полная поддержка Apollo Federation v2
**Обоснование:** Обеспечивает совместимость с production federation gateway
**Реализация:** Правильные @key, @extends, @external директивы

#### 3. Realistic Mock Data
**Решение:** Использование реалистичных данных вместо простых заглушек
**Обоснование:** Более качественное тестирование и разработка UI
**Реализация:** Локализованные данные с правильными форматами

Эта контекстная диаграмма служит отправной точкой для понимания того, как заглушки подграфов Task 11 интегрируются в общую федеративную экосистему Auto.ru и как они поддерживают разработку и тестирование cross-subgraph функциональности.