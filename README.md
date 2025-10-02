# Auto.ru GraphQL Federation

A federated GraphQL architecture demonstration inspired by Auto.ru's approach, built with Apollo Router and Rust subgraphs.

## Architecture Overview

This project demonstrates a gradual migration strategy from monolithic REST APIs to a federated GraphQL ecosystem, starting with a UGC (User Generated Content) subgraph for reviews and ratings.

### Components

- **Apollo Router** (Port 4000) - GraphQL gateway and query planner
- **UGC Subgraph** (Port 4001) - Reviews and ratings service
- **Users Subgraph** (Port 4002) - User management service  
- **Offers Subgraph** (Port 4004) - Car listings service
- **Catalog Subgraph** (Port 4003) - Car catalog and specifications
- **Search Subgraph** (Port 4005) - Search and recommendations

### Infrastructure

- **PostgreSQL** - Primary database for most subgraphs
- **Elasticsearch** - Search index and full-text search
- **Redis** - Caching layer
- **Jaeger** - Distributed tracing
- **Prometheus** - Metrics collection
- **Grafana** - Metrics visualization

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable, 1.75+)
- [Docker](https://docs.docker.com/get-docker/) (20.10+)
- [Docker Compose](https://docs.docker.com/compose/install/) (2.0+)
- [Node.js](https://nodejs.org/) (18+) - for schema tooling
- [Rover CLI](https://www.apollographql.com/docs/rover/getting-started) - Apollo's GraphQL CLI

### Setup Options

#### Option 1: Full Docker Development (Recommended)

```bash
# Clone the repository
git clone <repository-url>
cd auto-ru-graphql-federation

# Start development environment
make start-dev

# Check service status
make status

# View logs
make logs-router
```

#### Option 2: Hybrid Development (Docker infrastructure + Local subgraphs)

```bash
# Setup development environment
make setup

# Start infrastructure only
make start-infra

# Start subgraphs locally (in separate terminals)
make dev-ugc
make dev-users
make dev-offers

# Start router
make start-router
```

#### Option 3: Manual Setup

```bash
# 1. Setup environment
./scripts/setup-dev.sh

# 2. Start infrastructure services
docker-compose up -d ugc-postgres users-postgres offers-postgres catalog-postgres elasticsearch redis jaeger prometheus grafana

# 3. Build the workspace
cargo build

# 4. Run database migrations
make db-migrate-ugc
make db-migrate-users
make db-migrate-offers

# 5. Start subgraphs (in separate terminals)
cd ugc-subgraph && cargo run
cd users-subgraph && cargo run
cd offers-subgraph && cargo run

# 6. Start Apollo Router
docker-compose up apollo-router
```

### Verification

After setup, verify everything is working:

```bash
# Run health checks
make health-check

# Test GraphQL endpoint
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ __schema { types { name } } }"}'

# Open GraphQL Playground
make open-playground
```

### Development Workflow

1. **Start individual subgraphs for development:**
   ```bash
   # Terminal 1 - UGC Subgraph
   cd ugc-subgraph && cargo run
   
   # Terminal 2 - Users Subgraph  
   cd users-subgraph && cargo run
   
   # Terminal 3 - Offers Subgraph
   cd offers-subgraph && cargo run
   ```

2. **Access the GraphQL Playground:**
   - Open http://localhost:4000 in your browser
   - Explore the federated schema and run queries

3. **Monitor and observe:**
   - **Jaeger UI**: http://localhost:16686 (distributed tracing)
   - **Prometheus**: http://localhost:9091 (metrics)
   - **Grafana**: http://localhost:3000 (admin/admin)

## Project Structure

```
├── Cargo.toml                 # Workspace configuration
├── docker-compose.yml         # Development infrastructure
├── router.yaml               # Apollo Router configuration
├── prometheus.yml            # Prometheus configuration
├── scripts/
│   └── setup-dev.sh          # Development setup script
├── ugc-subgraph/             # User Generated Content service
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── src/
│   └── migrations/
├── users-subgraph/           # User management service
├── offers-subgraph/          # Car listings service
├── catalog-subgraph/         # Car catalog service
└── search-subgraph/          # Search service
```

## Federation Schema Design

The system uses Apollo Federation 2.0 with the following key patterns:

### Entity Extension
```graphql
# Base entity in Offers subgraph
type Offer @key(fields: "id") {
  id: ID!
  title: String!
  price: Int!
}

# Extended in UGC subgraph
extend type Offer @key(fields: "id") {
  id: ID! @external
  reviews: [Review!]!
  averageRating: Float
  reviewsCount: Int!
}
```

### Cross-Subgraph References
```graphql
type Review @key(fields: "id") {
  id: ID!
  rating: Int!
  text: String!
  # References to other subgraphs
  offer: Offer!    # Resolved by Offers subgraph
  author: User!    # Resolved by Users subgraph
}
```

## Development Guidelines

### Adding a New Subgraph

1. **Create the subgraph directory and Cargo.toml**
2. **Add to workspace members in root Cargo.toml**
3. **Create Dockerfile following the existing pattern**
4. **Add service to docker-compose.yml**
5. **Update router.yaml with subgraph configuration**
6. **Implement GraphQL schema with federation directives**

### Database Migrations

Each subgraph manages its own database schema:

```bash
# Run migrations for a specific subgraph
cd ugc-subgraph
sqlx migrate run --database-url $UGC_DATABASE_URL
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific subgraph
cargo test -p ugc-subgraph

# Run integration tests
cargo test --test integration
```

## Deployment

### Production Build

```bash
# Build all subgraphs
docker-compose build

# Or build specific subgraph
docker build -f ugc-subgraph/Dockerfile -t ugc-subgraph .
```

### Environment Variables

Key environment variables for production:

- `DATABASE_URL` - PostgreSQL connection string
- `ELASTICSEARCH_URL` - Elasticsearch cluster URL
- `REDIS_URL` - Redis connection string
- `JWT_SECRET` - JWT signing secret
- `JAEGER_ENDPOINT` - Jaeger collector endpoint
- `RUST_LOG` - Logging level

## Monitoring and Observability

### Metrics

The system exposes Prometheus metrics at:
- Router: http://localhost:9090/metrics
- Each subgraph: http://localhost:400X/metrics

### Tracing

Distributed traces are sent to Jaeger and can be viewed at http://localhost:16686

### Health Checks

- Router health: http://localhost:4000/health
- Individual subgraph health: http://localhost:400X/health

## Contributing

1. **Follow the task-based development approach outlined in `.kiro/specs/auto-ru-graphql-federation/tasks.md`**
2. **Implement one task at a time**
3. **Ensure all tests pass before submitting**
4. **Update documentation as needed**

## License

MIT License - see LICENSE file for details.