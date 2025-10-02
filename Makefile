# Auto.ru GraphQL Federation - Development Makefile

.PHONY: help setup build test clean start-infra start-router start-subgraphs stop check fmt clippy

# Default target
help:
	@echo "Auto.ru GraphQL Federation - Available commands:"
	@echo ""
	@echo "  setup           - Setup development environment"
	@echo "  build           - Build all subgraphs"
	@echo "  test            - Run all tests"
	@echo "  check           - Check code compilation"
	@echo "  fmt             - Format code"
	@echo "  clippy          - Run clippy linter"
	@echo "  clean           - Clean build artifacts"
	@echo ""
	@echo "  start-infra     - Start infrastructure services (databases, etc.)"
	@echo "  start-router    - Start Apollo Router"
	@echo "  start-subgraphs - Start all subgraphs in development mode"
	@echo "  stop            - Stop all services"
	@echo ""
	@echo "  compose-schema  - Compose supergraph schema from subgraphs"
	@echo "  validate-schema - Validate supergraph schema"
	@echo "  startup-check   - Run startup validation checks"
	@echo ""
	@echo "  logs-router     - Show Apollo Router logs"
	@echo "  logs-ugc        - Show UGC subgraph logs"
	@echo "  logs-users      - Show Users subgraph logs"
	@echo ""

# Setup development environment
setup:
	@echo "🚀 Setting up development environment..."
	./scripts/setup-dev.sh

# Build all subgraphs
build:
	@echo "🔨 Building all subgraphs..."
	cargo build --workspace

# Build for release
build-release:
	@echo "🔨 Building all subgraphs for release..."
	cargo build --workspace --release

# Run all tests
test:
	@echo "🧪 Running tests..."
	cargo test --workspace

# Check code compilation
check:
	@echo "🔍 Checking code compilation..."
	cargo check --workspace

# Format code
fmt:
	@echo "🎨 Formatting code..."
	cargo fmt --all

# Run clippy linter
clippy:
	@echo "📎 Running clippy..."
	cargo clippy --workspace -- -D warnings

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	docker compose down --volumes --remove-orphans

# Start infrastructure services
start-infra:
	@echo "🏗️ Starting infrastructure services..."
	docker compose up -d ugc-postgres users-postgres offers-postgres catalog-postgres elasticsearch redis jaeger prometheus grafana

# Start Apollo Router
start-router:
	@echo "🚀 Starting Apollo Router..."
	docker compose up apollo-router

# Start all subgraphs in development mode
start-subgraphs:
	@echo "🚀 Starting all subgraphs..."
	docker compose up ugc-subgraph users-subgraph offers-subgraph catalog-subgraph search-subgraph

# Stop all services
stop:
	@echo "🛑 Stopping all services..."
	docker compose down

# Show logs
logs-router:
	docker compose logs -f apollo-router

logs-ugc:
	docker compose logs -f ugc-subgraph

logs-users:
	docker compose logs -f users-subgraph

logs-offers:
	docker compose logs -f offers-subgraph

logs-catalog:
	docker compose logs -f catalog-subgraph

logs-search:
	docker compose logs -f search-subgraph

# Database operations
db-migrate-ugc:
	@echo "🗃️ Running UGC database migrations..."
	cd ugc-subgraph && sqlx migrate run --database-url $(UGC_DATABASE_URL)

db-migrate-users:
	@echo "🗃️ Running Users database migrations..."
	cd users-subgraph && sqlx migrate run --database-url $(USERS_DATABASE_URL)

db-migrate-offers:
	@echo "🗃️ Running Offers database migrations..."
	cd offers-subgraph && sqlx migrate run --database-url $(OFFERS_DATABASE_URL)

db-migrate-catalog:
	@echo "🗃️ Running Catalog database migrations..."
	cd catalog-subgraph && sqlx migrate run --database-url $(CATALOG_DATABASE_URL)

# Development shortcuts
dev-ugc:
	@echo "🚀 Starting UGC subgraph in development mode..."
	cd ugc-subgraph && cargo run

dev-users:
	@echo "🚀 Starting Users subgraph in development mode..."
	cd users-subgraph && cargo run

dev-offers:
	@echo "🚀 Starting Offers subgraph in development mode..."
	cd offers-subgraph && cargo run

dev-catalog:
	@echo "🚀 Starting Catalog subgraph in development mode..."
	cd catalog-subgraph && cargo run

dev-search:
	@echo "🚀 Starting Search subgraph in development mode..."
	cd search-subgraph && cargo run

# Docker operations
docker-build:
	@echo "🐳 Building Docker images..."
	docker compose build

docker-build-prod:
	@echo "🐳 Building Docker images for production..."
	docker compose -f docker-compose.prod.yml build

docker-pull:
	@echo "🐳 Pulling Docker images..."
	docker compose pull

# Production operations
start-prod:
	@echo "🚀 Starting production environment..."
	docker compose -f docker-compose.prod.yml up -d

stop-prod:
	@echo "🛑 Stopping production environment..."
	docker compose -f docker-compose.prod.yml down

logs-prod:
	@echo "📋 Showing production logs..."
	docker compose -f docker-compose.prod.yml logs -f

# Development with override
start-dev:
	@echo "🚀 Starting development environment..."
	docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d

stop-dev:
	@echo "🛑 Stopping development environment..."
	docker compose -f docker-compose.yml -f docker-compose.dev.yml down

# Health checks
health-check:
	@echo "🏥 Running comprehensive health check..."
	./scripts/health-check.sh

# Schema composition and validation
compose-schema:
	@echo "🔧 Composing supergraph schema..."
	./scripts/compose-supergraph.sh

compose-schema-live:
	@echo "🔧 Composing supergraph schema from live services..."
	./scripts/compose-supergraph.sh --live

validate-schema:
	@echo "✅ Validating supergraph schema..."
	./scripts/validate-supergraph.sh

validate-schema-preview:
	@echo "✅ Validating supergraph schema with preview..."
	./scripts/validate-supergraph.sh --preview

startup-check:
	@echo "🚀 Running startup validation checks..."
	./scripts/startup-validation.sh

startup-check-and-start:
	@echo "🚀 Running startup validation and starting router..."
	./scripts/startup-validation.sh --start-router

# Open useful URLs
open-playground:
	@echo "🎮 Opening GraphQL Playground..."
	@open http://localhost:4000 || xdg-open http://localhost:4000

open-jaeger:
	@echo "🔍 Opening Jaeger UI..."
	@open http://localhost:16686 || xdg-open http://localhost:16686

open-grafana:
	@echo "📊 Opening Grafana..."
	@open http://localhost:3000 || xdg-open http://localhost:3000

open-prometheus:
	@echo "📈 Opening Prometheus..."
	@open http://localhost:9091 || xdg-open http://localhost:9091