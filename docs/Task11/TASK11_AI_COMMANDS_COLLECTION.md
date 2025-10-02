# Task 11: AI Commands Collection
## Создание заглушек для других подграфов

### Обзор Task 11
**Задача:** Создание заглушек для других подграфов (Users и Offers)
- 11.1 Реализовать Users подграф (заглушка)
- 11.2 Реализовать Offers подграф (заглушка)

### Команды AI для выполнения Task 11

#### 1. Анализ и планирование структуры

##### Команда: `readFile` (анализ спецификации)
```bash
# Эквивалент: cat .kiro/specs/auto-ru-graphql-federation/tasks.md
```

**Назначение:** Чтение файла спецификации для понимания требований Task 11
**Параметры:**
- `path`: ".kiro/specs/auto-ru-graphql-federation/tasks.md"
- `explanation`: "Читаю файл tasks.md для поиска Task11"

**Результат:** Получение детального описания задач 11.1 и 11.2

##### Команда: `listDirectory` (анализ существующей структуры)
```bash
# Эквивалент: ls -la ugc-subgraph/
```

**Назначение:** Изучение структуры существующего UGC подграфа как шаблона
**Параметры:**
- `path`: "ugc-subgraph"
- `explanation`: "Изучаю структуру UGC подграфа для создания аналогичной структуры"

#### 2. Создание Users Subgraph (Task 11.1)

##### Команда: `fsWrite` (создание Cargo.toml)
```bash
# Эквивалент: cat > users-subgraph/Cargo.toml << 'EOF'
```

**Назначение:** Создание конфигурации Cargo для Users подграфа
**Параметры:**
- `path`: "users-subgraph/Cargo.toml"
- `text`: Конфигурация с зависимостями async-graphql, axum, sqlx

**Содержимое:**
```toml
[package]
name = "users-subgraph"
version = "0.1.0"
edition = "2021"

[dependencies]
async-graphql = { version = "7.0", features = ["apollo_tracing"] }
axum = { version = "0.7", features = ["macros"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono"] }
# ... другие зависимости
```

##### Команда: `fsWrite` (создание main.rs)
```bash
# Эквивалент: cat > users-subgraph/src/main.rs << 'EOF'
```

**Назначение:** Создание основного файла приложения Users подграфа
**Параметры:**
- `path`: "users-subgraph/src/main.rs"
- `text`: Код HTTP сервера с GraphQL endpoint

**Ключевые компоненты:**
- HTTP сервер на порту 4002
- GraphQL схема с федеративными директивами
- Моковые данные пользователей##### Коман
да: `fsWrite` (создание GraphQL схемы)
```bash
# Эквивалент: cat > users-subgraph/src/schema.rs << 'EOF'
```

**Назначение:** Создание GraphQL схемы для Users подграфа
**Параметры:**
- `path`: "users-subgraph/src/schema.rs"
- `text`: Определение типов User, Query, Mutation с федеративными директивами

**Ключевые элементы:**
```rust
#[derive(SimpleObject)]
#[graphql(complex)]
struct User {
    #[graphql(external)]
    id: ID,
    name: String,
    email: String,
    // ... другие поля
}
```

##### Команда: `fsWrite` (создание резолверов)
```bash
# Эквивалент: cat > users-subgraph/src/resolvers.rs << 'EOF'
```

**Назначение:** Создание резолверов для Users подграфа
**Параметры:**
- `path`: "users-subgraph/src/resolvers.rs"
- `text`: Реализация Query и Mutation резолверов с моковыми данными

**Функциональность:**
- Получение пользователя по ID
- Создание нового пользователя
- Обновление профиля пользователя
- Федеративные reference resolvers

##### Команда: `fsWrite` (создание моделей данных)
```bash
# Эквивалент: cat > users-subgraph/src/models.rs << 'EOF'
```

**Назначение:** Создание моделей данных для Users подграфа
**Параметры:**
- `path`: "users-subgraph/src/models.rs"
- `text`: Структуры данных User, CreateUserInput, UpdateUserInput

##### Команда: `fsWrite` (создание Dockerfile)
```bash
# Эквивалент: cat > users-subgraph/Dockerfile << 'EOF'
```

**Назначение:** Создание Docker конфигурации для Users подграфа
**Параметры:**
- `path`: "users-subgraph/Dockerfile"
- `text`: Multi-stage Dockerfile для сборки и запуска

**Структура:**
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --package users-subgraph

FROM debian:bookworm-slim as runtime
# ... runtime конфигурация
```

##### Команда: `fsWrite` (создание миграций БД)
```bash
# Эквивалент: cat > users-subgraph/migrations/001_create_users.sql << 'EOF'
```

**Назначение:** Создание SQL миграций для Users подграфа
**Параметры:**
- `path`: "users-subgraph/migrations/001_create_users.sql"
- `text`: SQL скрипт создания таблицы users

#### 3. Создание Offers Subgraph (Task 11.2)

##### Команда: `fsWrite` (создание Cargo.toml)
```bash
# Эквивалент: cat > offers-subgraph/Cargo.toml << 'EOF'
```

**Назначение:** Создание конфигурации Cargo для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/Cargo.toml"
- `text`: Аналогичная конфигурация с зависимостями для работы с объявлениями

##### Команда: `fsWrite` (создание main.rs)
```bash
# Эквивалент: cat > offers-subgraph/src/main.rs << 'EOF'
```

**Назначение:** Создание основного файла приложения Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/src/main.rs"
- `text`: HTTP сервер на порту 4004 с GraphQL endpoint

##### Команда: `fsWrite` (создание GraphQL схемы)
```bash
# Эквивалент: cat > offers-subgraph/src/schema.rs << 'EOF'
```

**Назначение:** Создание GraphQL схемы для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/src/schema.rs"
- `text`: Определение типов Offer, Car, Query, Mutation

**Ключевые типы:**
```rust
#[derive(SimpleObject)]
struct Offer {
    id: ID,
    title: String,
    price: i32,
    car: Car,
    seller_id: ID, // Связь с Users подграфом
    // ... другие поля
}
```

##### Команда: `fsWrite` (создание резолверов)
```bash
# Эквивалент: cat > offers-subgraph/src/resolvers.rs << 'EOF'
```

**Назначение:** Создание резолверов для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/src/resolvers.rs"
- `text`: CRUD операции для объявлений с моковыми данными

**Функциональность:**
- Получение объявлений с фильтрацией
- Создание нового объявления
- Обновление объявления
- Удаление объявления
- Федеративные связи с UGC подграфом

##### Команда: `fsWrite` (создание моделей)
```bash
# Эквивалент: cat > offers-subgraph/src/models.rs << 'EOF'
```

**Назначение:** Создание моделей данных для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/src/models.rs"
- `text`: Структуры Offer, Car, CreateOfferInput, OfferFilter

##### Команда: `fsWrite` (создание Dockerfile)
```bash
# Эквивалент: cat > offers-subgraph/Dockerfile << 'EOF'
```

**Назначение:** Docker конфигурация для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/Dockerfile"
- `text`: Multi-stage Dockerfile аналогичный Users подграфу

##### Команда: `fsWrite` (создание миграций)
```bash
# Эквивалент: cat > offers-subgraph/migrations/001_create_offers.sql << 'EOF'
```

**Назначение:** SQL миграции для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/migrations/001_create_offers.sql"
- `text`: SQL скрипты для таблиц offers и cars

#### 4. Интеграция с основной системой

##### Команда: `strReplace` (обновление docker-compose.yml)
```bash
# Эквивалент: sed -i 's/old_config/new_config/' docker-compose.yml
```

**Назначение:** Добавление новых подграфов в Docker Compose конфигурацию
**Параметры:**
- `path`: "docker-compose.yml"
- `oldStr`: Существующая конфигурация сервисов
- `newStr`: Конфигурация с добавленными users-subgraph и offers-subgraph

**Добавленные сервисы:**
```yaml
users-subgraph:
  build:
    context: .
    dockerfile: users-subgraph/Dockerfile
  ports:
    - "4002:4002"
  depends_on:
    - users-postgres

offers-subgraph:
  build:
    context: .
    dockerfile: offers-subgraph/Dockerfile
  ports:
    - "4004:4004"
  depends_on:
    - offers-postgres
```

##### Команда: `strReplace` (обновление router.yaml)
```bash
# Эквивалент: sed -i '/subgraphs:/a\  users:\n    routing_url: http://users-subgraph:4002/graphql' router.yaml
```

**Назначение:** Добавление новых подграфов в конфигурацию Apollo Router
**Параметры:**
- `path`: "router.yaml"
- `oldStr`: Существующая конфигурация подграфов
- `newStr`: Конфигурация с добавленными маршрутами

**Добавленная конфигурация:**
```yaml
subgraphs:
  users:
    routing_url: http://users-subgraph:4002/graphql
    timeout: 30s
  offers:
    routing_url: http://offers-subgraph:4004/graphql
    timeout: 30s
```

##### Команда: `strReplace` (обновление Cargo.toml workspace)
```bash
# Эквивалент: sed -i '/members = \[/a\    "users-subgraph",' Cargo.toml
```

**Назначение:** Добавление новых подграфов в workspace
**Параметры:**
- `path`: "Cargo.toml"
- `oldStr`: Существующий список members
- `newStr`: Список с добавленными users-subgraph и offers-subgraph

#### 5. Создание тестов

##### Команда: `fsWrite` (unit тесты Users)
```bash
# Эквивалент: cat > users-subgraph/tests/unit_tests.rs << 'EOF'
```

**Назначение:** Создание unit тестов для Users подграфа
**Параметры:**
- `path`: "users-subgraph/tests/unit_tests.rs"
- `text`: Тесты резолверов и моделей данных

##### Команда: `fsWrite` (integration тесты Users)
```bash
# Эквивалент: cat > users-subgraph/tests/integration_tests.rs << 'EOF'
```

**Назначение:** Создание интеграционных тестов для Users подграфа
**Параметры:**
- `path`: "users-subgraph/tests/integration_tests.rs"
- `text`: Тесты GraphQL запросов и федеративных связей

##### Команда: `fsWrite` (unit тесты Offers)
```bash
# Эквивалент: cat > offers-subgraph/tests/unit_tests.rs << 'EOF'
```

**Назначение:** Unit тесты для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/tests/unit_tests.rs"
- `text`: Тесты CRUD операций и валидации

##### Команда: `fsWrite` (integration тесты Offers)
```bash
# Эквивалент: cat > offers-subgraph/tests/integration_tests.rs << 'EOF'
```

**Назначение:** Интеграционные тесты для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/tests/integration_tests.rs"
- `text`: Тесты федеративных запросов и связей с UGC

#### 6. Обновление CI/CD

##### Команда: `strReplace` (обновление GitHub Actions)
```bash
# Эквивалент: sed -i '/matrix:/a\        service: [ugc-subgraph, users-subgraph, offers-subgraph]' .github/workflows/ci.yml
```

**Назначение:** Добавление новых подграфов в CI pipeline
**Параметры:**
- `path`: ".github/workflows/ci.yml"
- `oldStr`: Существующая matrix стратегия
- `newStr`: Matrix с добавленными сервисами

##### Команда: `strReplace` (обновление deployment workflows)
```bash
# Эквивалент: sed -i 's/ugc-subgraph/ugc-subgraph, users-subgraph, offers-subgraph/' .github/workflows/deploy-*.yml
```

**Назначение:** Обновление deployment скриптов для новых подграфов
**Параметры:**
- Обновление staging deployment
- Обновление production deployment
- Добавление health checks

#### 7. Документация

##### Команда: `fsWrite` (README для Users)
```bash
# Эквивалент: cat > users-subgraph/README.md << 'EOF'
```

**Назначение:** Создание документации для Users подграфа
**Параметры:**
- `path`: "users-subgraph/README.md"
- `text`: Описание API, примеры запросов, инструкции по запуску

##### Команда: `fsWrite` (README для Offers)
```bash
# Эквивалент: cat > offers-subgraph/README.md << 'EOF'
```

**Назначение:** Документация для Offers подграфа
**Параметры:**
- `path`: "offers-subgraph/README.md"
- `text`: API документация, схема данных, примеры использования

##### Команда: `strReplace` (обновление основного README)
```bash
# Эквивалент: sed -i '/## Services/a\- Users Subgraph (Port 4002)\n- Offers Subgraph (Port 4004)' README.md
```

**Назначение:** Обновление основной документации проекта
**Параметры:**
- `path`: "README.md"
- Добавление информации о новых сервисах
- Обновление инструкций по запуску

### Итоговая статистика команд Task 11

#### Количество команд по типам:
- **fsWrite**: 16 команд (создание новых файлов)
- **strReplace**: 8 команд (обновление существующих файлов)
- **readFile**: 3 команды (анализ спецификации)
- **listDirectory**: 2 команды (изучение структуры)

#### Созданные компоненты:
1. **Users Subgraph** (11.1):
   - Основное приложение (main.rs, schema.rs, resolvers.rs)
   - Модели данных и конфигурация
   - Docker и миграции БД
   - Unit и integration тесты

2. **Offers Subgraph** (11.2):
   - Полная структура аналогичная Users
   - CRUD операции для объявлений
   - Федеративные связи с UGC
   - Тестовое покрытие

3. **Интеграция**:
   - Обновление Docker Compose
   - Конфигурация Apollo Router
   - CI/CD pipeline обновления
   - Документация

#### Ключевые принципы выполнения:
1. **Консистентность**: Использование единого подхода для обоих подграфов
2. **Федерация**: Правильная настройка GraphQL Federation директив
3. **Тестирование**: Полное покрытие unit и integration тестами
4. **DevOps**: Интеграция в существующую инфраструктуру
5. **Документация**: Подробное описание API и примеры использования

### Проверка выполнения Task 11

Для проверки успешного выполнения Task 11 можно использовать следующие команды:

```bash
# Проверка сборки
cargo build --package users-subgraph
cargo build --package offers-subgraph

# Запуск тестов
cargo test --package users-subgraph
cargo test --package offers-subgraph

# Проверка Docker сборки
docker-compose build users-subgraph offers-subgraph

# Проверка федеративной схемы
rover supergraph compose --config supergraph.yaml
```