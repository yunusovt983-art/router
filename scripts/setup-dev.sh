#!/bin/bash

# Auto.ru GraphQL Federation - Development Environment Setup Script

set -e

echo "🚀 Setting up Auto.ru GraphQL Federation development environment..."

# Check if Docker and Docker Compose are installed
if ! command -v docker &> /dev/null; then
    echo "❌ Docker is not installed. Please install Docker first."
    exit 1
fi

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose is not installed. Please install Docker Compose first."
    exit 1
fi

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first."
    echo "Visit: https://rustup.rs/"
    exit 1
fi

# Create necessary directories
echo "📁 Creating project directories..."
mkdir -p ugc-subgraph/src
mkdir -p users-subgraph/src
mkdir -p offers-subgraph/src
mkdir -p catalog-subgraph/src
mkdir -p search-subgraph/src

# Create migration directories
mkdir -p ugc-subgraph/migrations
mkdir -p users-subgraph/migrations
mkdir -p offers-subgraph/migrations
mkdir -p catalog-subgraph/migrations

# Create placeholder main.rs files
echo "📝 Creating placeholder source files..."
for subgraph in ugc users offers catalog search; do
    if [ ! -f "${subgraph}-subgraph/src/main.rs" ]; then
        cat > "${subgraph}-subgraph/src/main.rs" << EOF
fn main() {
    println!("${subgraph}-subgraph placeholder - to be implemented");
}
EOF
    fi
done

# Check workspace compilation
echo "🔧 Checking workspace compilation..."
if cargo check; then
    echo "✅ Workspace compiles successfully"
else
    echo "❌ Workspace compilation failed"
    exit 1
fi

# Create .env file for development
if [ ! -f .env ]; then
    echo "🔐 Creating .env file..."
    cat > .env << EOF
# Database URLs
UGC_DATABASE_URL=postgresql://postgres:password@localhost:5432/ugc_db
USERS_DATABASE_URL=postgresql://postgres:password@localhost:5433/users_db
CATALOG_DATABASE_URL=postgresql://postgres:password@localhost:5434/catalog_db
OFFERS_DATABASE_URL=postgresql://postgres:password@localhost:5435/offers_db

# Elasticsearch
ELASTICSEARCH_URL=http://localhost:9200

# Redis
REDIS_URL=redis://localhost:6379

# JWT Secret
JWT_SECRET=your-super-secret-jwt-key-change-in-production

# Logging
RUST_LOG=info

# Jaeger
JAEGER_ENDPOINT=http://localhost:14268/api/traces
EOF
    echo "✅ Created .env file"
fi

# Create .gitignore if it doesn't exist
if [ ! -f .gitignore ]; then
    echo "📝 Creating .gitignore..."
    cat > .gitignore << EOF
# Rust
/target/
**/*.rs.bk
Cargo.lock

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Environment
.env.local
.env.production

# Logs
*.log

# Database
*.db
*.sqlite

# Docker
.dockerignore

# Temporary files
*.tmp
*.temp
EOF
    echo "✅ Created .gitignore"
fi

echo ""
echo "🎉 Development environment setup complete!"
echo ""
echo "Next steps:"
echo "1. Start the infrastructure: docker-compose up -d postgres redis elasticsearch jaeger prometheus grafana"
echo "2. Run database migrations (when implemented)"
echo "3. Start individual subgraphs for development"
echo "4. Start Apollo Router: docker-compose up apollo-router"
echo ""
echo "Useful URLs:"
echo "- GraphQL Playground: http://localhost:4000"
echo "- Jaeger UI: http://localhost:16686"
echo "- Prometheus: http://localhost:9091"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo ""
echo "Happy coding! 🦀"