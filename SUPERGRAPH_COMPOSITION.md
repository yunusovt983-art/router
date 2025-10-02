# Auto.ru GraphQL Federation - Supergraph Composition

This document describes the supergraph composition setup for the Auto.ru GraphQL Federation project.

## Overview

The supergraph composition system automatically combines schemas from multiple subgraphs into a single, unified GraphQL schema that can be served by Apollo Router. This enables federated GraphQL architecture where different domains (UGC, Users, Offers, Catalog, Search) can be developed and deployed independently.

## Architecture

```mermaid
graph TB
    subgraph "Subgraphs"
        UGC[UGC Subgraph<br/>:4001]
        Users[Users Subgraph<br/>:4002]
        Catalog[Catalog Subgraph<br/>:4003]
        Offers[Offers Subgraph<br/>:4004]
        Search[Search Subgraph<br/>:4005]
    end
    
    subgraph "Composition"
        Compose[Schema Composition<br/>scripts/compose-supergraph.sh]
        Validate[Schema Validation<br/>scripts/validate-supergraph.sh]
        Startup[Startup Validation<br/>scripts/startup-validation.sh]
    end
    
    subgraph "Output"
        SuperSchema[supergraph.graphql]
        Config[supergraph.yaml]
        Report[validation-report.txt]
    end
    
    subgraph "Router"
        Router[Apollo Router<br/>:4000]
    end
    
    UGC --> Compose
    Users --> Compose
    Catalog --> Compose
    Offers --> Compose
    Search --> Compose
    
    Compose --> SuperSchema
    Compose --> Config
    
    SuperSchema --> Validate
    Config --> Validate
    Validate --> Report
    
    SuperSchema --> Startup
    Config --> Startup
    Startup --> Router
    
    Router --> UGC
    Router --> Users
    Router --> Catalog
    Router --> Offers
    Router --> Search
```

## Components

### 1. Schema Composition Scripts

#### `scripts/compose-supergraph.sh`
Main composition script that:
- Fetches schemas from running subgraphs (with `--live` flag)
- Uses stub schemas for offline development
- Composes supergraph using Apollo Rover (when available)
- Falls back to simple composition for development

**Usage:**
```bash
# Compose using stub schemas (default)
./scripts/compose-supergraph.sh

# Compose using live schemas from running services
./scripts/compose-supergraph.sh --live
```

#### `scripts/simple-compose.sh`
Fallback composition script that creates a basic combined schema for development when Apollo Rover is not available.

#### `scripts/validate-supergraph.sh`
Comprehensive validation script that checks:
- Schema structure and syntax
- Federation directives and patterns
- Router configuration
- Schema compatibility

**Usage:**
```bash
# Basic validation
./scripts/validate-supergraph.sh

# Validation with schema preview
./scripts/validate-supergraph.sh --preview
```

#### `scripts/startup-validation.sh`
Pre-startup validation that ensures everything is ready before starting Apollo Router.

**Usage:**
```bash
# Validation only
./scripts/startup-validation.sh

# Validation and start router
./scripts/startup-validation.sh --start-router
```

### 2. Generated Files

#### `supergraph.graphql`
The composed supergraph schema that combines all subgraph schemas into a single, executable GraphQL schema.

#### `supergraph.yaml`
Configuration file for Apollo Rover that defines:
- Federation version
- Subgraph endpoints
- Schema file locations

#### `schemas/`
Directory containing individual subgraph schemas:
- `ugc.graphql` - UGC subgraph schema
- `users.graphql` - Users subgraph schema
- `offers.graphql` - Offers subgraph schema
- `catalog.graphql` - Catalog subgraph schema
- `search.graphql` - Search subgraph schema

## Federation Features

### Entity Keys and Extensions

The supergraph supports proper federation with:

```graphql
# User entity (owned by Users subgraph)
type User @key(fields: "id") {
  id: ID!
  name: String!
  email: String!
}

# Extended by UGC subgraph
extend type User @key(fields: "id") {
  id: ID! @external
  reviews: [Review!]!
}

# Extended by Offers subgraph
extend type User @key(fields: "id") {
  id: ID! @external
  offers: [Offer!]!
}
```

### Cross-Subgraph Relationships

Entities can reference each other across subgraphs:

```graphql
type Review {
  id: ID!
  author: User!    # Resolved from Users subgraph
  offer: Offer!    # Resolved from Offers subgraph
}
```

## Development Workflow

### 1. Local Development

For local development without Apollo Rover:

```bash
# Start infrastructure
make start-infra

# Start all subgraphs
make start-subgraphs

# Compose schema (uses simple composition)
make compose-schema

# Validate schema
make validate-schema

# Start router
make start-router
```

### 2. Production Deployment

For production with proper federation:

```bash
# Install Apollo Rover
curl -sSL https://rover.apollo.dev/nix/latest | sh

# Compose with live schemas
./scripts/compose-supergraph.sh --live

# Validate
./scripts/validate-supergraph.sh

# Deploy
docker compose up apollo-router
```

### 3. Continuous Integration

The composition can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Compose Supergraph
  run: |
    ./scripts/compose-supergraph.sh --live
    ./scripts/validate-supergraph.sh

- name: Check Schema Changes
  run: |
    # Compare with previous schema
    # Detect breaking changes
    # Generate schema diff
```

## Make Commands

The following Make commands are available for schema operations:

```bash
# Schema composition
make compose-schema          # Compose using stub schemas
make compose-schema-live     # Compose using live services

# Schema validation
make validate-schema         # Basic validation
make validate-schema-preview # Validation with preview

# Startup validation
make startup-check           # Pre-startup checks
make startup-check-and-start # Check and start router
```

## Configuration

### Environment Variables

The composition system supports the following environment variables:

```bash
# Apollo Router configuration
APOLLO_ROUTER_CONFIG_PATH=/path/to/router.yaml
APOLLO_ROUTER_SUPERGRAPH_PATH=/path/to/supergraph.graphql
APOLLO_ROUTER_LOG=info

# Validation settings
APOLLO_ROUTER_VALIDATION_ENABLED=true
APOLLO_ROUTER_STARTUP_DELAY=10
APOLLO_ROUTER_WAIT_FOR_SUBGRAPHS=true

# Environment
ENVIRONMENT=development|production
```

### Router Configuration

The router configuration (`router.yaml`) includes:

```yaml
supergraph:
  listen: 0.0.0.0:4000
  supergraph_path: ./supergraph.graphql
  query_planning:
    cache:
      in_memory:
        limit: 512

subgraphs:
  ugc:
    routing_url: http://ugc-subgraph:4001/graphql
  users:
    routing_url: http://users-subgraph:4002/graphql
  # ... other subgraphs
```

## Monitoring and Observability

### Health Checks

The system includes comprehensive health checks:

- **Schema Validation**: Ensures schema is valid and complete
- **Router Configuration**: Validates router config syntax and completeness
- **Subgraph Health**: Checks if subgraphs are responding
- **Dependency Checks**: Verifies required tools are available

### Validation Reports

Detailed validation reports are generated:

```
Auto.ru GraphQL Federation - Supergraph Validation Report
Generated: 2024-01-15T10:30:00Z
========================================================

Files Checked:
- Supergraph Schema: ./supergraph.graphql
- Router Config: ./router.yaml

Schema Statistics:
- File Size: 3734 bytes
- Lines: 120
- Types: 14
- Enums: 1
- Inputs: 4
- Scalars: 1

Federation Statistics:
- @key directives: 5
- @external fields: 8
- Entity extensions: 3
```

### Metrics and Logging

The composition process includes:

- **Structured Logging**: All operations are logged with context
- **Performance Metrics**: Composition time and schema size tracking
- **Error Reporting**: Detailed error messages with suggestions
- **Status Indicators**: Color-coded output for easy monitoring

## Troubleshooting

### Common Issues

#### 1. Apollo Rover Not Available
**Problem**: `rover` command not found
**Solution**: Install Apollo Rover or use simple composition fallback

```bash
# Install Rover
curl -sSL https://rover.apollo.dev/nix/latest | sh

# Or use simple composition
./scripts/simple-compose.sh
```

#### 2. Subgraphs Not Responding
**Problem**: Cannot fetch live schemas
**Solution**: Ensure subgraphs are running and accessible

```bash
# Check subgraph health
curl http://localhost:4001/health
curl http://localhost:4002/health

# Use stub schemas instead
./scripts/compose-supergraph.sh  # (without --live flag)
```

#### 3. Schema Validation Errors
**Problem**: Composed schema has validation errors
**Solution**: Check individual subgraph schemas

```bash
# Validate individual schemas
./scripts/validate-supergraph.sh --preview

# Check subgraph logs
make logs-ugc
make logs-users
```

#### 4. Federation Directive Issues
**Problem**: Missing or incorrect federation directives
**Solution**: Update subgraph schemas with proper directives

```graphql
# Correct federation directives
type User @key(fields: "id") {
  id: ID!
  name: String!
}

extend type User @key(fields: "id") {
  id: ID! @external
  reviews: [Review!]!
}
```

### Debug Mode

Enable debug mode for detailed logging:

```bash
# Set debug environment
export APOLLO_ROUTER_LOG=debug

# Run with verbose output
./scripts/compose-supergraph.sh --live 2>&1 | tee composition.log
```

## Best Practices

### 1. Schema Design
- Use consistent naming conventions across subgraphs
- Define clear entity boundaries and ownership
- Minimize cross-subgraph dependencies
- Use proper federation directives

### 2. Development Workflow
- Always validate schemas before deployment
- Use stub schemas for offline development
- Test federation queries across subgraphs
- Monitor schema evolution and breaking changes

### 3. Production Deployment
- Use Apollo Rover for proper federation composition
- Implement schema change detection in CI/CD
- Monitor supergraph performance and errors
- Have rollback procedures for schema changes

### 4. Monitoring
- Set up alerts for composition failures
- Monitor schema validation metrics
- Track federation query performance
- Log all schema changes and deployments

## Future Enhancements

### Planned Features
- **Schema Registry Integration**: Connect with Apollo Studio
- **Automated Schema Diffing**: Detect breaking changes automatically
- **Performance Optimization**: Cache composed schemas
- **Advanced Validation**: Custom validation rules
- **Schema Versioning**: Support for schema evolution

### Integration Opportunities
- **CI/CD Pipelines**: Automated composition and validation
- **Monitoring Systems**: Integration with Prometheus/Grafana
- **Development Tools**: IDE plugins for schema validation
- **Testing Frameworks**: Automated federation testing

## References

- [Apollo Federation Documentation](https://www.apollographql.com/docs/federation/)
- [Apollo Router Documentation](https://www.apollographql.com/docs/router/)
- [GraphQL Schema Design Guide](https://www.apollographql.com/docs/apollo-server/schema/schema/)
- [Federation Best Practices](https://www.apollographql.com/docs/federation/federated-types/overview/)