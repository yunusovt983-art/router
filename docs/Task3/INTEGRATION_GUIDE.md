# Руководство по интеграции федеративных подграфов

## Обзор

Данное руководство содержит пошаговые инструкции по интеграции федеративных GraphQL подграфов в системе Auto.ru.

## 1. Настройка Apollo Federation

### Конфигурация супер-схемы

```yaml
# supergraph.yaml - конфигурация федеративной схемы
federation_version: 2
subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
    schema:
      file: ./schemas/ugc.graphql
  
  users:
    routing_url: http://users-subgraph:4002/graphql
    schema:
      file: ./schemas/users.graphql
  
  offers:
    routing_url: http://offers-subgraph:4003/graphql
    schema:
      file: ./schemas/offers.graphql
```

### Федеративные директивы

```graphql
# UGC подграф схема
extend schema
  @link(url: "https://specs.apollo.dev/federation/v2.0",
        import: ["@key", "@requires", "@provides", "@external", "@tag", "@shareable"])

type Review @key(fields: "id") {
  id: ID!
  offerId: ID!
  userId: ID!
  rating: Int!
  text: String!
  createdAt: DateTime!
  
  # Федеративные ссылки с предоставлением данных
  user: User @provides(fields: "name avatar")
  offer: Offer @provides(fields: "title")
}

# Расширение внешних типов
extend type User @key(fields: "id") {
  id: ID! @external
  reviews: [Review!]!
  reviewsCount: Int!
}

extend type Offer @key(fields: "id") {
  id: ID! @external
  reviews: ReviewConnection!
  averageRating: Float
  reviewsCount: Int!
}
```### Refe
rence Resolvers

```rust
// Реализация reference resolvers в UGC подграфе
use async_graphql::{Object, Context, Result, ID};

#[derive(SimpleObject)]
#[graphql(extends)]
pub struct User {
    #[graphql(external)]
    pub id: ID,
}

#[Object]
impl User {
    /// Entity resolver для получения User по ID
    #[graphql(entity)]
    async fn find_by_id(id: ID) -> User {
        User { id }
    }
    
    /// Получение отзывов пользователя
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<Vec<Review>> {
        let service = ctx.data::<ReviewService>()?;
        let user_id = UserId::from_str(&self.id)?;
        
        let args = ConnectionArgs {
            first: first.unwrap_or(10),
            after,
            ..Default::default()
        };
        
        service.get_user_reviews(user_id, args).await
    }
    
    /// Подсчет отзывов пользователя
    async fn reviews_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let service = ctx.data::<ReviewService>()?;
        let user_id = UserId::from_str(&self.id)?;
        
        service.get_user_reviews_count(user_id).await
    }
}

#[derive(SimpleObject)]
#[graphql(extends)]
pub struct Offer {
    #[graphql(external)]
    pub id: ID,
}

#[Object]
impl Offer {
    /// Entity resolver для получения Offer по ID
    #[graphql(entity)]
    async fn find_by_id(id: ID) -> Offer {
        Offer { id }
    }
    
    /// Получение отзывов объявления с пагинацией
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        filter: Option<ReviewFilter>,
    ) -> Result<ReviewConnection> {
        let service = ctx.data::<ReviewService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        let args = ConnectionArgs {
            first: first.unwrap_or(10),
            after,
            ..Default::default()
        };
        
        let filter = filter.unwrap_or_default();
        service.get_reviews_connection(offer_id, args, filter).await
    }
    
    /// Получение среднего рейтинга объявления
    async fn average_rating(&self, ctx: &Context<'_>) -> Result<Option<f64>> {
        let service = ctx.data::<RatingService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        let rating = service.get_offer_rating(offer_id).await?;
        Ok(Some(rating.average_rating))
    }
    
    /// Подсчет отзывов объявления
    async fn reviews_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let service = ctx.data::<RatingService>()?;
        let offer_id = OfferId::from_str(&self.id)?;
        
        let rating = service.get_offer_rating(offer_id).await?;
        Ok(rating.total_reviews)
    }
}
```

## 2. Настройка Apollo Gateway

### Конфигурация роутера

```typescript
// gateway/src/index.ts - настройка Apollo Gateway
import { ApolloGateway, IntrospectAndCompose } from '@apollo/gateway';
import { ApolloServer } from 'apollo-server-express';
import express from 'express';

const gateway = new ApolloGateway({
  serviceList: [
    { name: 'ugc', url: 'http://ugc-subgraph:4001/graphql' },
    { name: 'users', url: 'http://users-subgraph:4002/graphql' },
    { name: 'offers', url: 'http://offers-subgraph:4003/graphql' },
  ],
  
  // Настройка интроспекции и композиции
  supergraphSdl: new IntrospectAndCompose({
    serviceList: [
      { name: 'ugc', url: 'http://ugc-subgraph:4001/graphql' },
      { name: 'users', url: 'http://users-subgraph:4002/graphql' },
      { name: 'offers', url: 'http://offers-subgraph:4003/graphql' },
    ],
    pollIntervalInMs: 30000, // Обновление схемы каждые 30 секунд
  }),
  
  // Настройка сборки сервисов
  buildService: ({ url }) => {
    return new RemoteGraphQLDataSource({
      url,
      willSendRequest({ request, context }) {
        // Передача контекста аутентификации
        if (context.user) {
          request.http.headers.set('x-user-id', context.user.id);
          request.http.headers.set('x-user-roles', JSON.stringify(context.user.roles));
        }
        
        // Передача трассировки
        if (context.traceId) {
          request.http.headers.set('x-trace-id', context.traceId);
        }
      },
    });
  },
});

const server = new ApolloServer({
  gateway,
  subscriptions: false,
  
  // Контекст для федеративных запросов
  context: ({ req }) => {
    return {
      user: req.user, // Из middleware аутентификации
      traceId: req.headers['x-trace-id'] || generateTraceId(),
    };
  },
  
  // Плагины для мониторинга
  plugins: [
    ApolloServerPluginLandingPageGraphQLPlayground(),
    ApolloServerPluginInlineTrace(),
    {
      requestDidStart() {
        return {
          willSendResponse(requestContext) {
            // Логирование производительности
            console.log(`Query executed in ${requestContext.metrics.executionTime}ms`);
          },
        };
      },
    },
  ],
});

const app = express();
server.applyMiddleware({ app, path: '/graphql' });

app.listen(4000, () => {
  console.log('🚀 Gateway ready at http://localhost:4000/graphql');
});
```

### Обработка ошибок в федерации

```typescript
// Централизованная обработка ошибок
import { RemoteGraphQLDataSource } from '@apollo/gateway';

class ErrorHandlingDataSource extends RemoteGraphQLDataSource {
  willSendRequest({ request, context }) {
    // Добавление заголовков
    super.willSendRequest({ request, context });
  }
  
  didReceiveResponse({ response, request, context }) {
    // Обработка ошибок подграфов
    if (response.body && response.body.errors) {
      const errors = response.body.errors.map(error => {
        // Обогащение ошибок контекстом
        return {
          ...error,
          extensions: {
            ...error.extensions,
            subgraph: this.name,
            timestamp: new Date().toISOString(),
            traceId: context.traceId,
          },
        };
      });
      
      response.body.errors = errors;
    }
    
    return response;
  }
  
  didEncounterError(error, request, context) {
    // Логирование ошибок подграфов
    console.error(`Subgraph ${this.name} error:`, {
      error: error.message,
      query: request.query,
      variables: request.variables,
      traceId: context.traceId,
    });
    
    // Метрики ошибок
    errorMetrics.increment({
      subgraph: this.name,
      errorType: error.extensions?.code || 'UNKNOWN',
    });
    
    return error;
  }
}
```

## 3. Интеграционное тестирование

### Contract Testing с Pact

```typescript
// tests/contract/ugc-subgraph.pact.ts
import { Pact } from '@pact-foundation/pact';
import { GraphQLInteraction } from '@pact-foundation/pact';

describe('UGC Subgraph Contract', () => {
  const provider = new Pact({
    consumer: 'apollo-gateway',
    provider: 'ugc-subgraph',
    port: 1234,
  });
  
  beforeAll(() => provider.setup());
  afterAll(() => provider.finalize());
  
  describe('Reviews Query', () => {
    beforeEach(() => {
      const interaction = new GraphQLInteraction()
        .given('reviews exist for offer')
        .uponReceiving('a request for reviews')
        .withQuery(`
          query GetReviews($offerId: ID!) {
            reviews(offerId: $offerId, first: 5) {
              edges {
                node {
                  id
                  rating
                  text
                  user {
                    name
                  }
                }
              }
            }
          }
        `)
        .withVariables({ offerId: 'offer-123' })
        .willRespondWith({
          status: 200,
          headers: { 'Content-Type': 'application/json' },
          body: {
            data: {
              reviews: {
                edges: [
                  {
                    node: {
                      id: 'review-1',
                      rating: 5,
                      text: 'Great car!',
                      user: { name: 'John Doe' },
                    },
                  },
                ],
              },
            },
          },
        });
      
      return provider.addInteraction(interaction);
    });
    
    it('should return reviews for offer', async () => {
      const response = await fetch('http://localhost:1234/graphql', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          query: `
            query GetReviews($offerId: ID!) {
              reviews(offerId: $offerId, first: 5) {
                edges {
                  node {
                    id
                    rating
                    text
                    user {
                      name
                    }
                  }
                }
              }
            }
          `,
          variables: { offerId: 'offer-123' },
        }),
      });
      
      const result = await response.json();
      expect(result.data.reviews.edges).toHaveLength(1);
      expect(result.data.reviews.edges[0].node.rating).toBe(5);
    });
  });
});
```

### End-to-End тестирование

```typescript
// tests/e2e/federation.e2e.ts
import { test, expect } from '@playwright/test';

test.describe('Federation E2E Tests', () => {
  test('should execute federated query across multiple subgraphs', async ({ request }) => {
    const response = await request.post('/graphql', {
      data: {
        query: `
          query GetOfferWithReviews($offerId: ID!) {
            offer(id: $offerId) {
              title
              price
              seller {
                name
                rating
              }
              reviews(first: 3) {
                edges {
                  node {
                    rating
                    text
                    user {
                      name
                      avatar
                    }
                  }
                }
                pageInfo {
                  hasNextPage
                }
              }
              averageRating
            }
          }
        `,
        variables: { offerId: 'test-offer-123' },
      },
    });
    
    expect(response.ok()).toBeTruthy();
    
    const result = await response.json();
    expect(result.data.offer).toBeDefined();
    expect(result.data.offer.title).toBeTruthy();
    expect(result.data.offer.reviews.edges).toBeInstanceOf(Array);
    expect(result.data.offer.averageRating).toBeGreaterThanOrEqual(0);
  });
  
  test('should handle partial failures gracefully', async ({ request }) => {
    // Симуляция недоступности UGC подграфа
    await request.post('/admin/subgraphs/ugc/disable');
    
    const response = await request.post('/graphql', {
      data: {
        query: `
          query GetOfferBasicInfo($offerId: ID!) {
            offer(id: $offerId) {
              title
              price
              reviews {  # Этот блок должен вернуть ошибку
                edges {
                  node {
                    rating
                  }
                }
              }
            }
          }
        `,
        variables: { offerId: 'test-offer-123' },
      },
    });
    
    const result = await response.json();
    
    // Основные данные должны быть доступны
    expect(result.data.offer.title).toBeTruthy();
    expect(result.data.offer.price).toBeTruthy();
    
    // Отзывы должны вернуть null с ошибкой
    expect(result.data.offer.reviews).toBeNull();
    expect(result.errors).toBeDefined();
    expect(result.errors[0].extensions.code).toBe('SUBGRAPH_UNAVAILABLE');
  });
});
```

## 4. Мониторинг федерации

### Метрики Apollo Gateway

```typescript
// monitoring/gateway-metrics.ts
import { ApolloServerPlugin } from 'apollo-server-plugin-base';
import { prometheus } from 'prom-client';

// Метрики для федеративных запросов
const federationMetrics = {
  queryPlanningTime: new prometheus.Histogram({
    name: 'federation_query_planning_duration_seconds',
    help: 'Time spent planning federated queries',
    buckets: [0.001, 0.005, 0.01, 0.05, 0.1, 0.2, 0.5],
  }),
  
  subgraphRequestsTotal: new prometheus.Counter({
    name: 'federation_subgraph_requests_total',
    help: 'Total requests to subgraphs',
    labelNames: ['subgraph', 'operation_type'],
  }),
  
  subgraphResponseTime: new prometheus.Histogram({
    name: 'federation_subgraph_response_duration_seconds',
    help: 'Response time for subgraph requests',
    labelNames: ['subgraph'],
    buckets: [0.01, 0.05, 0.1, 0.2, 0.5, 1, 2],
  }),
  
  federationErrors: new prometheus.Counter({
    name: 'federation_errors_total',
    help: 'Total federation errors',
    labelNames: ['subgraph', 'error_type'],
  }),
};

export const federationMonitoringPlugin: ApolloServerPlugin = {
  requestDidStart() {
    return {
      willSendSubgraphRequest(requestContext) {
        const startTime = Date.now();
        
        return {
          willSendRequest({ request }) {
            federationMetrics.subgraphRequestsTotal
              .labels(requestContext.subgraphName, request.operationType || 'query')
              .inc();
          },
          
          didReceiveResponse({ response }) {
            const duration = (Date.now() - startTime) / 1000;
            
            federationMetrics.subgraphResponseTime
              .labels(requestContext.subgraphName)
              .observe(duration);
            
            if (response.errors && response.errors.length > 0) {
              response.errors.forEach(error => {
                federationMetrics.federationErrors
                  .labels(requestContext.subgraphName, error.extensions?.code || 'UNKNOWN')
                  .inc();
              });
            }
          },
        };
      },
    };
  },
};
```

### Health Checks для подграфов

```typescript
// monitoring/health-checks.ts
import { fetch } from 'node-fetch';

interface SubgraphHealth {
  name: string;
  url: string;
  status: 'healthy' | 'unhealthy' | 'degraded';
  responseTime: number;
  lastCheck: Date;
  error?: string;
}

class SubgraphHealthMonitor {
  private subgraphs: Array<{ name: string; url: string }>;
  private healthStatus = new Map<string, SubgraphHealth>();
  
  constructor(subgraphs: Array<{ name: string; url: string }>) {
    this.subgraphs = subgraphs;
    this.startHealthChecks();
  }
  
  private startHealthChecks(): void {
    setInterval(() => {
      this.checkAllSubgraphs();
    }, 30000); // Проверка каждые 30 секунд
  }
  
  private async checkAllSubgraphs(): Promise<void> {
    const checks = this.subgraphs.map(subgraph => 
      this.checkSubgraphHealth(subgraph)
    );
    
    await Promise.allSettled(checks);
  }
  
  private async checkSubgraphHealth(
    subgraph: { name: string; url: string }
  ): Promise<void> {
    const startTime = Date.now();
    
    try {
      const response = await fetch(`${subgraph.url}/health`, {
        method: 'GET',
        timeout: 5000,
      });
      
      const responseTime = Date.now() - startTime;
      
      const health: SubgraphHealth = {
        name: subgraph.name,
        url: subgraph.url,
        status: response.ok ? 'healthy' : 'unhealthy',
        responseTime,
        lastCheck: new Date(),
      };
      
      if (!response.ok) {
        health.error = `HTTP ${response.status}: ${response.statusText}`;
      }
      
      this.healthStatus.set(subgraph.name, health);
      
      // Обновление метрик
      subgraphHealthMetrics
        .labels(subgraph.name)
        .set(response.ok ? 1 : 0);
        
    } catch (error) {
      const health: SubgraphHealth = {
        name: subgraph.name,
        url: subgraph.url,
        status: 'unhealthy',
        responseTime: Date.now() - startTime,
        lastCheck: new Date(),
        error: error.message,
      };
      
      this.healthStatus.set(subgraph.name, health);
      
      subgraphHealthMetrics
        .labels(subgraph.name)
        .set(0);
    }
  }
  
  getHealthStatus(): SubgraphHealth[] {
    return Array.from(this.healthStatus.values());
  }
  
  isSubgraphHealthy(name: string): boolean {
    const health = this.healthStatus.get(name);
    return health?.status === 'healthy';
  }
}
```

## 5. Deployment и CI/CD

### Docker Compose для разработки

```yaml
# docker-compose.federation.yml
version: '3.8'

services:
  # Apollo Gateway
  apollo-gateway:
    build: ./gateway
    ports:
      - "4000:4000"
    environment:
      - NODE_ENV=development
      - UGC_SUBGRAPH_URL=http://ugc-subgraph:4001/graphql
      - USERS_SUBGRAPH_URL=http://users-subgraph:4002/graphql
      - OFFERS_SUBGRAPH_URL=http://offers-subgraph:4003/graphql
    depends_on:
      - ugc-subgraph
      - users-subgraph
      - offers-subgraph
    networks:
      - federation-network
  
  # UGC Subgraph
  ugc-subgraph:
    build: ./subgraphs/ugc
    ports:
      - "4001:4001"
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/ugc_db
      - REDIS_URL=redis://redis:6379
    depends_on:
      - postgres
      - redis
    networks:
      - federation-network
      - data-network
  
  # Users Subgraph
  users-subgraph:
    build: ./subgraphs/users
    ports:
      - "4002:4002"
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/users_db
    depends_on:
      - postgres
    networks:
      - federation-network
      - data-network
  
  # Offers Subgraph
  offers-subgraph:
    build: ./subgraphs/offers
    ports:
      - "4003:4003"
    environment:
      - DATABASE_URL=postgres://postgres:password@postgres:5432/offers_db
    depends_on:
      - postgres
    networks:
      - federation-network
      - data-network

networks:
  federation-network:
    driver: bridge
  data-network:
    driver: bridge
    internal: true
```

### CI/CD Pipeline

```yaml
# .github/workflows/federation-ci.yml
name: Federation CI/CD

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  # Тестирование подграфов
  test-subgraphs:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        subgraph: [ugc, users, offers]
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          
      - name: Install dependencies
        run: |
          cd subgraphs/${{ matrix.subgraph }}
          npm ci
          
      - name: Run tests
        run: |
          cd subgraphs/${{ matrix.subgraph }}
          npm test
          
      - name: Run linting
        run: |
          cd subgraphs/${{ matrix.subgraph }}
          npm run lint
  
  # Композиция схемы
  compose-schema:
    runs-on: ubuntu-latest
    needs: test-subgraphs
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rover CLI
        run: |
          curl -sSL https://rover.apollo.dev/nix/latest | sh
          echo "$HOME/.rover/bin" >> $GITHUB_PATH
          
      - name: Compose supergraph
        run: |
          rover supergraph compose --config supergraph.yaml > supergraph.graphql
          
      - name: Validate composition
        run: |
          rover graph check auto-ru-federation@main --schema supergraph.graphql
          
      - name: Upload schema artifact
        uses: actions/upload-artifact@v3
        with:
          name: supergraph-schema
          path: supergraph.graphql
  
  # Интеграционные тесты
  integration-tests:
    runs-on: ubuntu-latest
    needs: compose-schema
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Download schema
        uses: actions/download-artifact@v3
        with:
          name: supergraph-schema
          
      - name: Start federation stack
        run: |
          docker-compose -f docker-compose.federation.yml up -d
          
      - name: Wait for services
        run: |
          ./scripts/wait-for-services.sh
          
      - name: Run integration tests
        run: |
          npm run test:integration
          
      - name: Run contract tests
        run: |
          npm run test:contract
          
      - name: Cleanup
        run: |
          docker-compose -f docker-compose.federation.yml down -v
  
  # Деплой
  deploy:
    runs-on: ubuntu-latest
    needs: [test-subgraphs, compose-schema, integration-tests]
    if: github.ref == 'refs/heads/main'
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Deploy to staging
        run: |
          ./scripts/deploy-staging.sh
          
      - name: Run smoke tests
        run: |
          ./scripts/smoke-tests.sh staging
          
      - name: Deploy to production
        if: success()
        run: |
          ./scripts/deploy-production.sh
```

Это руководство обеспечивает полную интеграцию федеративных подграфов с мониторингом, тестированием и автоматизированным деплоем.