# GraphQL Migration System

This document describes the comprehensive migration system implemented for gradually transitioning from REST API to GraphQL federation.

## Overview

The migration system provides:

- **Feature Flags**: Control rollout of GraphQL features
- **A/B Testing**: Compare REST vs GraphQL performance
- **Canary Deployments**: Gradual rollout with automatic rollback
- **Traffic Routing**: Intelligent request routing based on flags
- **Monitoring**: Comprehensive metrics and alerting
- **Emergency Controls**: Quick rollback mechanisms

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   REST Client   │    │  GraphQL Client │    │  Management UI  │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼───────────────┐
                    │      Apollo Router          │
                    │   (Traffic Routing)         │
                    └─────────────┬───────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │     UGC Subgraph            │
                    │  ┌─────────────────────────┐ │
                    │  │  Migration System       │ │
                    │  │  ├─ Feature Flags       │ │
                    │  │  ├─ REST Adapter        │ │
                    │  │  ├─ Traffic Router      │ │
                    │  │  ├─ A/B Testing         │ │
                    │  │  ├─ Canary Controller   │ │
                    │  │  └─ Monitoring          │ │
                    │  └─────────────────────────┘ │
                    └─────────────┬───────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │       PostgreSQL            │
                    └─────────────────────────────┘
```

## Feature Flags

### Configuration

Feature flags are configured in `feature-flags.yaml`:

```yaml
feature_flags:
  graphql_reviews_read:
    enabled: false
    rollout_percentage: 0.0
    description: "Enable GraphQL for reading reviews"
    user_whitelist: []
    user_blacklist: []
    conditions: []
```

### Available Flags

- `graphql_reviews_read`: Enable GraphQL for read operations
- `graphql_reviews_write`: Enable GraphQL for write operations
- `rest_api_deprecation_warning`: Show deprecation warnings
- `enable_query_caching`: Enable query result caching
- `enable_dataloader_batching`: Enable DataLoader batching
- `enable_circuit_breaker`: Enable circuit breaker for external services
- `enable_rate_limiting`: Enable rate limiting

### Management

#### CLI Tool

```bash
# List all flags
cargo run --bin migration-cli list

# Enable a flag
cargo run --bin migration-cli enable graphql_reviews_read

# Set rollout percentage
cargo run --bin migration-cli rollout graphql_reviews_read 25.0

# Enable flag for specific user
cargo run --bin migration-cli enable-user graphql_reviews_read user-123
```

#### REST API

```bash
# List flags
curl http://localhost:4001/api/migration/flags

# Enable flag
curl -X POST http://localhost:4001/api/migration/flags/graphql_reviews_read/enable

# Set rollout percentage
curl -X PUT http://localhost:4001/api/migration/flags/graphql_reviews_read/rollout \
  -H "Content-Type: application/json" \
  -d '{"percentage": 25.0}'
```

## A/B Testing

### Creating A/B Tests

```bash
# Create A/B test
cargo run --bin migration-cli create-ab-test graphql_migration_test \
  "Compare REST vs GraphQL performance" 50.0

# Assign user to variant
cargo run --bin migration-cli assign-user graphql_migration_test user-123
```

### Tracking Conversions

```bash
curl -X POST http://localhost:4001/api/migration/ab-tests/graphql_migration_test/track \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-123", "event": "review_created"}'
```

## Canary Deployments

### Starting Canary

```bash
# Start canary deployment (begins at 1%)
cargo run --bin migration-cli start-canary graphql_reviews_read
```

### Promoting Canary

```bash
# Promote to 10%
cargo run --bin migration-cli promote-canary graphql_reviews_read 10.0
```

### Rollback

```bash
# Rollback canary
cargo run --bin migration-cli rollback-canary graphql_reviews_read

# Emergency rollback all flags
cargo run --bin migration-cli emergency-rollback "High error rate detected"
```

## Traffic Routing

The traffic router automatically routes requests based on feature flags:

1. **Request Analysis**: Determines if request is migration candidate
2. **Flag Evaluation**: Checks feature flags for user
3. **Routing Decision**: Routes to GraphQL or REST backend
4. **Metrics Collection**: Records routing decisions and performance

### Routing Logic

```rust
// Example routing decision
if feature_flags.is_enabled("graphql_reviews_read", user_id) {
    route_to_graphql(request)
} else {
    route_to_rest_adapter(request)
}
```

## Monitoring

### Metrics

The system collects comprehensive metrics:

- **Request Metrics**: Volume, latency, errors by backend
- **Feature Flag Metrics**: Evaluations, cache hits/misses
- **Circuit Breaker Metrics**: State changes, trips
- **Business Metrics**: Migration progress, user adoption

### Dashboards

Access monitoring dashboards:

```bash
# Migration status
curl http://localhost:4001/api/migration/status

# Real-time metrics
curl http://localhost:4001/api/migration/metrics

# Health check
curl http://localhost:4001/api/migration/health
```

### Alerts

Automatic alerts are triggered for:

- High error rates (>5%)
- Performance degradation (P95 >500ms)
- Circuit breaker trips
- Data consistency issues

## REST-to-GraphQL Adapter

The REST adapter provides backward compatibility:

### Supported Endpoints

- `GET /api/v1/reviews` → GraphQL `reviews` query
- `POST /api/v1/reviews` → GraphQL `createReview` mutation
- `GET /api/v1/reviews/:id` → GraphQL `review` query
- `PUT /api/v1/reviews/:id` → GraphQL `updateReview` mutation
- `DELETE /api/v1/reviews/:id` → GraphQL `deleteReview` mutation
- `GET /api/v1/offers/:id/reviews` → GraphQL `offer.reviews` query
- `GET /api/v1/users/:id/reviews` → GraphQL `user.reviews` query

### Response Format

REST responses maintain backward compatibility:

```json
{
  "success": true,
  "data": {
    "id": "review-123",
    "rating": 5,
    "text": "Great car!",
    "created_at": "2024-01-15T10:30:00Z"
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Migration Phases

### Phase 1: Read Operations (Target: March 2024)

- Enable `graphql_reviews_read` flag
- Migrate GET endpoints to GraphQL
- Target: 50% of read traffic on GraphQL

### Phase 2: Write Operations (Target: April 2024)

- Enable `graphql_reviews_write` flag
- Migrate POST/PUT/DELETE endpoints
- Target: 25% of write traffic on GraphQL

### Phase 3: Full Migration (Target: June 2024)

- Complete migration to GraphQL
- Deprecate REST endpoints
- Target: 90% of traffic on GraphQL

## Emergency Procedures

### High Error Rate

If GraphQL error rate exceeds 10%:

1. Automatic rollback triggered
2. All GraphQL flags disabled
3. Traffic routed to REST
4. Incident created

### Performance Degradation

If P95 response time exceeds 1000ms:

1. Rollout percentage reduced
2. Circuit breaker enabled
3. Performance alert sent

### Data Inconsistency

If data consistency checks fail:

1. Emergency rollback initiated
2. Migration paused
3. Data integrity investigation started

## Configuration

### Environment Variables

```bash
# Feature flag configuration
MIGRATION_CONFIG_PATH=feature-flags.yaml

# Feature flag overrides
FF_GRAPHQL_REVIEWS_READ_ENABLED=false
FF_GRAPHQL_REVIEWS_READ_ROLLOUT=0.0
FF_GRAPHQL_REVIEWS_WRITE_ENABLED=false
FF_GRAPHQL_REVIEWS_WRITE_ROLLOUT=0.0

# Redis for flag caching
REDIS_URL=redis://localhost:6379
REDIS_ENABLED=true
```

### Docker Compose

```yaml
services:
  ugc-subgraph:
    environment:
      - MIGRATION_CONFIG_PATH=/app/feature-flags.yaml
      - FF_GRAPHQL_REVIEWS_READ_ENABLED=false
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./feature-flags.yaml:/app/feature-flags.yaml
```

## Best Practices

### Feature Flag Management

1. **Start Small**: Begin with 1% rollout
2. **Monitor Closely**: Watch metrics during rollout
3. **Gradual Increase**: Increase by 5-10% steps
4. **Quick Rollback**: Be ready to rollback quickly
5. **Clean Up**: Remove flags after full rollout

### A/B Testing

1. **Clear Hypothesis**: Define what you're testing
2. **Sufficient Sample Size**: Ensure statistical significance
3. **Single Variable**: Test one change at a time
4. **Duration**: Run tests for sufficient time
5. **Document Results**: Record learnings

### Canary Deployments

1. **Health Checks**: Implement comprehensive health checks
2. **Automated Rollback**: Set up automatic rollback triggers
3. **Monitoring**: Monitor all key metrics
4. **Communication**: Keep stakeholders informed
5. **Documentation**: Document deployment process

## Troubleshooting

### Common Issues

#### Feature Flag Not Working

1. Check flag configuration in `feature-flags.yaml`
2. Verify Redis connectivity
3. Check user ID format
4. Review rollout percentage

#### High Error Rate

1. Check GraphQL schema compatibility
2. Verify database connectivity
3. Review external service health
4. Check authentication/authorization

#### Performance Issues

1. Enable query caching
2. Check DataLoader batching
3. Review database query performance
4. Monitor external service latency

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo run
```

Check feature flag evaluation:

```bash
curl http://localhost:4001/api/migration/flags/graphql_reviews_read
```

Monitor metrics:

```bash
curl http://localhost:4001/metrics
```

## Support

For issues or questions:

1. Check this documentation
2. Review logs and metrics
3. Use CLI tools for debugging
4. Contact the platform team

## Contributing

When adding new migration features:

1. Update feature flag configuration
2. Add monitoring metrics
3. Implement emergency rollback
4. Update documentation
5. Add tests