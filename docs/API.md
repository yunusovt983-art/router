# API Documentation

This document provides comprehensive API documentation for the Auto.ru GraphQL Federation system.

## Table of Contents

- [GraphQL Endpoint](#graphql-endpoint)
- [Authentication](#authentication)
- [Schema Overview](#schema-overview)
- [Query Examples](#query-examples)
- [Mutation Examples](#mutation-examples)
- [Error Handling](#error-handling)
- [Rate Limiting](#rate-limiting)
- [Pagination](#pagination)

## GraphQL Endpoint

**Base URL:** `http://localhost:4000/graphql` (development)
**Production URL:** `https://api.auto-ru-federation.com/graphql`

### Headers

```http
Content-Type: application/json
Authorization: Bearer <jwt-token>  # Optional, for authenticated requests
```

## Authentication

The system uses JWT (JSON Web Tokens) for authentication. Include the token in the Authorization header:

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

### Getting a Token

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

Variables:
```json
{
  "input": {
    "email": "user@example.com",
    "password": "password123"
  }
}
```

## Schema Overview

### Core Types

#### User
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

#### Offer
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
  
  # Extended by UGC subgraph
  reviews: [Review!]!
  averageRating: Float
  reviewsCount: Int!
  
  # Extended by Catalog subgraph
  car: Car
}
```

#### Review
```graphql
type Review {
  id: ID!
  rating: Int!  # 1-5
  text: String!
  createdAt: DateTime!
  updatedAt: DateTime!
  isModerated: Boolean!
  
  # Cross-subgraph references
  offer: Offer!
  author: User!
}
```

#### Car
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

### Connection Types (Pagination)

All list queries use Relay-style cursor pagination:

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

## Query Examples

### Basic Queries

#### Get All Offers
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

Variables:
```json
{
  "first": 10,
  "after": null
}
```

#### Get Offer with Reviews
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

Variables:
```json
{
  "offerId": "550e8400-e29b-41d4-a716-446655440000",
  "reviewsFirst": 5
}
```

### Advanced Queries

#### Search Offers with Filters
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

Variables:
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

#### Get User Profile with Reviews
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

## Mutation Examples

### Create Review
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

Variables:
```json
{
  "input": {
    "offerId": "550e8400-e29b-41d4-a716-446655440000",
    "rating": 5,
    "text": "Отличный автомобиль! Рекомендую к покупке."
  }
}
```

### Update Review
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

Variables:
```json
{
  "id": "123e4567-e89b-12d3-a456-426614174000",
  "input": {
    "rating": 4,
    "text": "Хороший автомобиль, но есть небольшие недостатки."
  }
}
```

### Delete Review
```graphql
mutation DeleteReview($id: ID!) {
  deleteReview(id: $id) {
    success
    message
  }
}
```

### Create Offer
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

Variables:
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

## Error Handling

The API returns errors in the standard GraphQL format:

```json
{
  "errors": [
    {
      "message": "Review not found",
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

### Common Error Codes

- `UNAUTHENTICATED` - No valid authentication token provided
- `UNAUTHORIZED` - User doesn't have permission for this operation
- `VALIDATION_ERROR` - Input validation failed
- `NOT_FOUND` - Requested resource not found
- `RATE_LIMIT_EXCEEDED` - Too many requests
- `INTERNAL_ERROR` - Server error

### Error Response Example

```json
{
  "errors": [
    {
      "message": "Validation error: rating must be between 1 and 5",
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

## Rate Limiting

The API implements rate limiting to prevent abuse:

- **Authenticated users**: 1000 requests per hour
- **Anonymous users**: 100 requests per hour
- **Mutations**: 100 per hour per user

Rate limit headers are included in responses:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1640995200
```

## Pagination

All list queries use cursor-based pagination following the Relay specification:

### Forward Pagination
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

### Backward Pagination
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

### Pagination Best Practices

1. **Use reasonable page sizes** (10-50 items)
2. **Always check `hasNextPage`** before requesting more
3. **Store cursors** for navigation
4. **Handle empty results** gracefully

## Introspection

The schema supports introspection in development:

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

## Subscriptions

Real-time subscriptions are available for certain events:

### New Review Subscription
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

### Offer Updates Subscription
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

## Performance Tips

1. **Use field selection** - Only request fields you need
2. **Implement caching** - Use HTTP caching headers
3. **Batch requests** - Use query batching when possible
4. **Monitor query complexity** - Avoid deeply nested queries
5. **Use pagination** - Don't request large datasets at once

## Testing

### Using curl
```bash
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "query": "{ offers(first: 5) { edges { node { id title price } } } }"
  }'
```

### Using GraphQL Playground
Visit `http://localhost:4000` in your browser for an interactive GraphQL IDE.

### Using Postman
Import the GraphQL schema and use Postman's GraphQL support for testing.