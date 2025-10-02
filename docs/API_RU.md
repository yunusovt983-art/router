# Документация API

Этот документ предоставляет comprehensive документацию по API для системы Apollo Router Federation Auto.ru.

## 📋 Содержание

- [GraphQL Endpoint](#graphql-endpoint)
- [Аутентификация](#аутентификация)
- [Обзор схемы](#обзор-схемы)
- [Примеры запросов](#примеры-запросов)
- [Примеры мутаций](#примеры-мутаций)
- [Обработка ошибок](#обработка-ошибок)
- [Ограничение скорости](#ограничение-скорости)
- [Пагинация](#пагинация)

## GraphQL Endpoint

**Базовый URL:** `http://localhost:4000/graphql` (разработка)
**Production URL:** `https://api.auto-ru-federation.com/graphql`

### Заголовки

```http
Content-Type: application/json
Authorization: Bearer <jwt-token>  # Опционально, для аутентифицированных запросов
```

## Аутентификация

Система использует JWT (JSON Web Tokens) для аутентификации. Включите токен в заголовок Authorization:

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

### Получение токена

```graphql
mutation Login($input: LoginInput!) {
  login(input: $input) {
    token
    user {
      id
      name
      email
    }
  }
}
```

Переменные:
```json
{
  "input": {
    "email": "user@example.com",
    "password": "password123"
  }
}
```

## Обзор схемы

### Основные типы

#### User (Пользователь)
```graphql
type User {
  id: ID!
  name: String!
  email: String
  phone: String
  createdAt: DateTime!
  reviews: [Review!]!
}
```

#### Offer (Объявление)
```graphql
type Offer {
  id: ID!
  title: String!
  description: String
  price: Int!
  currency: String!
  year: Int
  mileage: Int
  location: String
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Расширено UGC подграфом
  reviews: [Review!]!
  averageRating: Float
  reviewsCount: Int!
  
  # Расширено Catalog подграфом
  car: Car
}
```

#### Review (Отзыв)
```graphql
type Review {
  id: ID!
  rating: Int!  # 1-5
  text: String!
  createdAt: DateTime!
  updatedAt: DateTime!
  isModerated: Boolean!
  
  # Межподграфовые ссылки
  offer: Offer!
  author: User!
}
```

#### Car (Автомобиль)
```graphql
type Car {
  id: ID!
  make: String!
  model: String!
  generation: String
  bodyType: String
  engineType: String
  transmission: String
  driveType: String
  specifications: [Specification!]!
}
```

### Типы соединений (Пагинация)

Все списочные запросы используют пагинацию в стиле Relay:

```graphql
type ReviewConnection {
  edges: [ReviewEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}

type ReviewEdge {
  node: Review!
  cursor: String!
}

type PageInfo {
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}
```

## Примеры запросов

### Базовые запросы

#### Получить все объявления
```graphql
query GetOffers($first: Int, $after: String) {
  offers(first: $first, after: $after) {
    edges {
      node {
        id
        title
        price
        currency
        year
        mileage
        location
        createdAt
      }
      cursor
    }
    pageInfo {
      hasNextPage
      endCursor
    }
    totalCount
  }
}
```

Переменные:
```json
{
  "first": 10,
  "after": null
}
```

#### Получить объявление с отзывами
```graphql
query GetOfferWithReviews($offerId: ID!, $reviewsFirst: Int) {
  offer(id: $offerId) {
    id
    title
    price
    currency
    description
    averageRating
    reviewsCount
    
    reviews(first: $reviewsFirst) {
      edges {
        node {
          id
          rating
          text
          createdAt
          author {
            id
            name
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
    
    car {
      make
      model
      generation
      bodyType
    }
  }
}
```

Переменные:
```json
{
  "offerId": "550e8400-e29b-41d4-a716-446655440000",
  "reviewsFirst": 5
}
```

### Продвинутые запросы

#### Поиск объявлений с фильтрами
```graphql
query SearchOffers(
  $query: String
  $filters: OfferFilters
  $first: Int
  $after: String
) {
  searchOffers(
    query: $query
    filters: $filters
    first: $first
    after: $after
  ) {
    edges {
      node {
        id
        title
        price
        currency
        year
        mileage
        averageRating
        reviewsCount
        car {
          make
          model
          bodyType
        }
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
    totalCount
    facets {
      make {
        value
        count
      }
      priceRange {
        min
        max
        count
      }
    }
  }
}
```

Переменные:
```json
{
  "query": "BMW X5",
  "filters": {
    "priceMin": 1000000,
    "priceMax": 5000000,
    "yearMin": 2015,
    "make": ["BMW", "Mercedes"],
    "bodyType": ["SUV"]
  },
  "first": 20
}
```

#### Получить профиль пользователя с отзывами
```graphql
query GetUserProfile($userId: ID!) {
  user(id: $userId) {
    id
    name
    email
    createdAt
    
    reviews(first: 10) {
      edges {
        node {
          id
          rating
          text
          createdAt
          offer {
            id
            title
            car {
              make
              model
            }
          }
        }
      }
      totalCount
    }
  }
}
```

## Примеры мутаций

### Создать отзыв
```graphql
mutation CreateReview($input: CreateReviewInput!) {
  createReview(input: $input) {
    id
    rating
    text
    createdAt
    offer {
      id
      title
      averageRating
      reviewsCount
    }
    author {
      id
      name
    }
  }
}
```

Переменные:
```json
{
  "input": {
    "offerId": "550e8400-e29b-41d4-a716-446655440000",
    "rating": 5,
    "text": "Отличный автомобиль! Рекомендую к покупке."
  }
}
```

### Обновить отзыв
```graphql
mutation UpdateReview($id: ID!, $input: UpdateReviewInput!) {
  updateReview(id: $id, input: $input) {
    id
    rating
    text
    updatedAt
  }
}
```

Переменные:
```json
{
  "id": "123e4567-e89b-12d3-a456-426614174000",
  "input": {
    "rating": 4,
    "text": "Хороший автомобиль, но есть небольшие недостатки."
  }
}
```

### Удалить отзыв
```graphql
mutation DeleteReview($id: ID!) {
  deleteReview(id: $id) {
    success
    message
  }
}
```

### Создать объявление
```graphql
mutation CreateOffer($input: CreateOfferInput!) {
  createOffer(input: $input) {
    id
    title
    price
    currency
    description
    year
    mileage
    location
    createdAt
    car {
      make
      model
    }
  }
}
```

Переменные:
```json
{
  "input": {
    "title": "BMW X5 2020 года",
    "description": "Отличное состояние, один владелец",
    "price": 3500000,
    "currency": "RUB",
    "year": 2020,
    "mileage": 45000,
    "location": "Москва",
    "carId": "bmw-x5-g05"
  }
}
```

## Обработка ошибок

API возвращает ошибки в стандартном формате GraphQL:

```json
{
  "errors": [
    {
      "message": "Отзыв не найден",
      "extensions": {
        "code": "REVIEW_NOT_FOUND",
        "reviewId": "123e4567-e89b-12d3-a456-426614174000"
      },
      "path": ["review"]
    }
  ],
  "data": null
}
```

### Распространенные коды ошибок

- `UNAUTHENTICATED` - Не предоставлен действительный токен аутентификации
- `UNAUTHORIZED` - У пользователя нет разрешения на эту операцию
- `VALIDATION_ERROR` - Ошибка валидации входных данных
- `NOT_FOUND` - Запрашиваемый ресурс не найден
- `RATE_LIMIT_EXCEEDED` - Слишком много запросов
- `INTERNAL_ERROR` - Ошибка сервера

### Пример ответа с ошибкой

```json
{
  "errors": [
    {
      "message": "Ошибка валидации: рейтинг должен быть от 1 до 5",
      "extensions": {
        "code": "VALIDATION_ERROR",
        "field": "rating",
        "value": 6
      },
      "path": ["createReview", "input", "rating"]
    }
  ]
}
```

## Ограничение скорости

API реализует ограничение скорости для предотвращения злоупотреблений:

- **Аутентифицированные пользователи**: 1000 запросов в час
- **Анонимные пользователи**: 100 запросов в час
- **Мутации**: 100 в час на пользователя

Заголовки ограничения скорости включены в ответы:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1640995200
```

## Пагинация

Все списочные запросы используют курсорную пагинацию по спецификации Relay:

### Прямая пагинация
```graphql
query GetReviews($first: Int!, $after: String) {
  reviews(first: $first, after: $after) {
    edges {
      node { id rating text }
      cursor
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
```

### Обратная пагинация
```graphql
query GetReviews($last: Int!, $before: String) {
  reviews(last: $last, before: $before) {
    edges {
      node { id rating text }
      cursor
    }
    pageInfo {
      hasPreviousPage
      startCursor
    }
  }
}
```

### Лучшие практики пагинации

1. **Используйте разумные размеры страниц** (10-50 элементов)
2. **Всегда проверяйте `hasNextPage`** перед запросом следующих данных
3. **Сохраняйте курсоры** для навигации
4. **Обрабатывайте пустые результаты** корректно

## Интроспекция

Схема поддерживает интроспекцию в режиме разработки:

```graphql
query IntrospectionQuery {
  __schema {
    types {
      name
      description
      fields {
        name
        type {
          name
        }
      }
    }
  }
}
```

## Подписки

Доступны подписки в реальном времени для определенных событий:

### Подписка на новые отзывы
```graphql
subscription OnNewReview($offerId: ID!) {
  reviewAdded(offerId: $offerId) {
    id
    rating
    text
    createdAt
    author {
      name
    }
  }
}
```

### Подписка на обновления объявлений
```graphql
subscription OnOfferUpdate($offerId: ID!) {
  offerUpdated(offerId: $offerId) {
    id
    title
    price
    averageRating
    reviewsCount
  }
}
```

## Советы по производительности

1. **Используйте выбор полей** - запрашивайте только нужные поля
2. **Реализуйте кеширование** - используйте HTTP заголовки кеширования
3. **Группируйте запросы** - используйте батчинг запросов когда возможно
4. **Мониторьте сложность запросов** - избегайте глубоко вложенных запросов
5. **Используйте пагинацию** - не запрашивайте большие наборы данных сразу

## Тестирование

### Использование curl
```bash
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "query": "{ offers(first: 5) { edges { node { id title price } } } }"
  }'
```

### Использование GraphQL Playground
Посетите `http://localhost:4000` в браузере для интерактивной GraphQL IDE.

### Использование Postman
Импортируйте GraphQL схему и используйте поддержку GraphQL в Postman для тестирования.