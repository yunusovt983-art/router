# Federation Implementation for UGC Subgraph

## Overview

This document describes the federation implementation for the UGC (User Generated Content) subgraph. Due to limitations in async-graphql 7.0 (federation support is not available), we have implemented federation-ready structures that can be easily upgraded when federation support becomes available.

## Federation Directives Implemented

### 1. Entity Keys (@key)

- **Review**: `@key(fields: "id")` - Reviews are owned by the UGC subgraph
- **User**: `@key(fields: "id")` - Users are extended from the Users subgraph  
- **Offer**: `@key(fields: "id")` - Offers are extended from the Offers subgraph

### 2. Entity Extensions (@extends)

- **UserType**: Extends User entity with UGC-specific fields:
  - `reviews()` - Get reviews written by the user
  
- **OfferType**: Extends Offer entity with UGC-specific fields:
  - `reviews()` - Get reviews for the offer
  - `averageRating` - Average rating for the offer
  - `reviewsCount` - Total number of reviews
  - `ratingDistribution` - Distribution of ratings

### 3. External Fields (@external)

- **UserType.id**: External field from Users subgraph
- **OfferType.id**: External field from Offers subgraph

## Reference Resolvers

Reference resolvers are implemented for all federated entities:

```rust
impl UserType {
    pub async fn find_by_id(_ctx: &Context<'_>, id: Uuid) -> Result<UserType> {
        Ok(UserType { id })
    }
}

impl OfferType {
    pub async fn find_by_id(_ctx: &Context<'_>, id: Uuid) -> Result<OfferType> {
        Ok(OfferType { id })
    }
}

impl ReviewType {
    pub async fn find_by_id(ctx: &Context<'_>, id: Uuid) -> Result<Option<ReviewType>> {
        // Resolves review from database
    }
}
```

## External Service Integration

The implementation includes an external service client for communicating with other subgraphs:

- **ExternalServiceClient**: HTTP client for calling Users and Offers services
- **Error Handling**: Graceful fallback when external services are unavailable
- **Configuration**: Service URLs configurable via environment variables

### Environment Variables

```bash
USERS_SERVICE_URL=http://users-service:4002
OFFERS_SERVICE_URL=http://offers-service:4004
```

## Schema Structure

The federation schema is documented in `schema.federation.graphql` showing the complete federated schema with proper directives.

## Migration to Full Federation

When async-graphql adds federation support, the following changes will be needed:

1. **Update Cargo.toml**:
   ```toml
   async-graphql = { version = "X.X", features = ["federation"] }
   ```

2. **Add Federation Directives**:
   ```rust
   #[derive(SimpleObject)]
   #[graphql(extends, complex)]
   pub struct UserType {
       #[graphql(external, key)]
       pub id: Uuid,
   }
   ```

3. **Enable Federation in Schema**:
   ```rust
   let schema = Schema::build(Query, Mutation, EmptySubscription)
       .enable_federation()
       .finish();
   ```

## Testing

Federation structure is tested in `src/graphql/federation_test.rs`:

```bash
cargo test federation_test
```

## Error Handling

The implementation includes robust error handling for external service failures:

- **Service Unavailable**: Returns fallback data when external services are down
- **Network Timeouts**: Configurable timeouts with graceful degradation
- **Data Consistency**: Ensures UGC data remains accessible even when other services fail

## Performance Considerations

- **Caching**: Offer ratings are cached to reduce database load
- **Pagination**: Cursor-based pagination for efficient data retrieval
- **Connection Pooling**: Database connections are pooled for optimal performance
- **Timeouts**: External service calls have configurable timeouts

## Security

- **Input Validation**: All inputs are validated before processing
- **Authorization**: User permissions are checked for review operations
- **Rate Limiting**: Can be implemented at the gateway level
- **Data Sanitization**: Review text is sanitized to prevent XSS attacks