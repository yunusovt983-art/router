#!/bin/bash

# Auto.ru GraphQL Federation - Supergraph Composition Script
# This script composes the supergraph schema from all subgraphs

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ROUTER_CONFIG_PATH="./router.yaml"
SUPERGRAPH_CONFIG_PATH="./supergraph.yaml"
SUPERGRAPH_SCHEMA_PATH="./supergraph.graphql"
SUBGRAPH_SCHEMAS_DIR="./schemas"

# Subgraph endpoints (for local development)
UGC_ENDPOINT="http://localhost:4001/graphql"
USERS_ENDPOINT="http://localhost:4002/graphql"
OFFERS_ENDPOINT="http://localhost:4004/graphql"
CATALOG_ENDPOINT="http://localhost:4003/graphql"
SEARCH_ENDPOINT="http://localhost:4005/graphql"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a service is running
check_service() {
    local url=$1
    local service_name=$2
    
    if curl -s -f "$url" > /dev/null 2>&1; then
        print_success "$service_name is running"
        return 0
    else
        print_warning "$service_name is not running at $url"
        return 1
    fi
}

# Function to fetch schema from a subgraph
fetch_schema() {
    local endpoint=$1
    local service_name=$2
    local output_file=$3
    
    print_status "Fetching schema from $service_name..."
    
    # GraphQL introspection query
    local introspection_query='{
        "query": "query IntrospectionQuery { __schema { queryType { name } mutationType { name } subscriptionType { name } types { ...FullType } directives { name description locations args { ...InputValue } } } } fragment FullType on __Type { kind name description fields(includeDeprecated: true) { name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason } inputFields { ...InputValue } interfaces { ...TypeRef } enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason } possibleTypes { ...TypeRef } } fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue } fragment TypeRef on __Type { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }"
    }'
    
    # Fetch introspection result
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$introspection_query" \
        "$endpoint" 2>/dev/null)
    
    if [ $? -eq 0 ] && [ -n "$response" ]; then
        # Convert introspection result to SDL using rover (if available)
        if command -v rover &> /dev/null; then
            echo "$response" | rover graph introspect --format json - > "$output_file" 2>/dev/null
            if [ $? -eq 0 ]; then
                print_success "Schema fetched from $service_name"
                return 0
            fi
        fi
        
        # Fallback: save raw introspection result
        echo "$response" > "${output_file}.json"
        print_warning "Saved raw introspection result for $service_name (rover not available)"
        return 1
    else
        print_error "Failed to fetch schema from $service_name"
        return 1
    fi
}

# Function to create supergraph configuration
create_supergraph_config() {
    print_status "Creating supergraph configuration..."
    
    cat > "$SUPERGRAPH_CONFIG_PATH" << EOF
federation_version: =2.5.7
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
    routing_url: http://offers-subgraph:4004/graphql
    schema:
      file: ./schemas/offers.graphql
  catalog:
    routing_url: http://catalog-subgraph:4003/graphql
    schema:
      file: ./schemas/catalog.graphql
  search:
    routing_url: http://search-subgraph:4005/graphql
    schema:
      file: ./schemas/search.graphql
EOF
    
    print_success "Supergraph configuration created"
}

# Function to create stub schemas if services are not running
create_stub_schemas() {
    print_status "Creating stub schemas for offline composition..."
    
    mkdir -p "$SUBGRAPH_SCHEMAS_DIR"
    
    # UGC subgraph schema
    cat > "$SUBGRAPH_SCHEMAS_DIR/ugc.graphql" << 'EOF'
extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: ["@key", "@extends", "@external", "@requires", "@provides"])

type Review @key(fields: "id") {
  id: ID!
  offerId: ID!
  authorId: ID!
  rating: Int!
  text: String!
  createdAt: DateTime!
  updatedAt: DateTime!
  isModerated: Boolean!
  
  # Federation relationships
  offer: Offer!
  author: User!
}

extend type Offer @key(fields: "id") {
  id: ID! @external
  reviews(first: Int, after: String): ReviewConnection!
  averageRating: Float
  reviewsCount: Int!
}

extend type User @key(fields: "id") {
  id: ID! @external
  reviews(first: Int, after: String): ReviewConnection!
}

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

input CreateReviewInput {
  offerId: ID!
  rating: Int!
  text: String!
}

input UpdateReviewInput {
  rating: Int
  text: String
}

input ReviewsFilter {
  offerId: ID
  authorId: ID
  minRating: Int
  maxRating: Int
  moderatedOnly: Boolean
}

scalar DateTime

type Query {
  review(id: ID!): Review
  reviews(filter: ReviewsFilter, first: Int, after: String): ReviewConnection!
}

type Mutation {
  createReview(input: CreateReviewInput!): Review!
  updateReview(id: ID!, input: UpdateReviewInput!): Review!
  deleteReview(id: ID!): Boolean!
  moderateReview(id: ID!, approved: Boolean!): Review!
}
EOF

    # Users subgraph schema
    cat > "$SUBGRAPH_SCHEMAS_DIR/users.graphql" << 'EOF'
extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: ["@key"])

type User @key(fields: "id") {
  id: ID!
  name: String!
  email: String!
  phone: String
  createdAt: DateTime!
  updatedAt: DateTime!
}

scalar DateTime

type Query {
  user(id: ID!): User
  users: [User!]!
}

type Mutation {
  placeholder: String
}
EOF

    # Offers subgraph schema
    cat > "$SUBGRAPH_SCHEMAS_DIR/offers.graphql" << 'EOF'
extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: ["@key"])

type Offer @key(fields: "id") {
  id: ID!
  title: String!
  description: String!
  price: Int!
  currency: String!
  year: Int!
  mileage: Int!
  location: String!
  sellerId: ID!
  createdAt: DateTime!
  updatedAt: DateTime!
  isActive: Boolean!
  
  # Federation relationships
  seller: User!
}

scalar DateTime

type Query {
  offer(id: ID!): Offer
  offers: [Offer!]!
  offersBySeller(sellerId: ID!): [Offer!]!
}

type Mutation {
  placeholder: String
}
EOF

    # Catalog subgraph schema
    cat > "$SUBGRAPH_SCHEMAS_DIR/catalog.graphql" << 'EOF'
extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: ["@key"])

type Brand @key(fields: "id") {
  id: ID!
  name: String!
  country: String!
  logoUrl: String
  foundedYear: Int
}

type Model @key(fields: "id") {
  id: ID!
  name: String!
  brandId: ID!
  bodyType: String!
  fuelType: String!
  transmission: String!
  driveType: String!
  engineVolume: Float
  powerHp: Int
  productionStartYear: Int
  productionEndYear: Int
  
  # Federation relationships
  brand: Brand
}

type Query {
  brand(id: ID!): Brand
  brands: [Brand!]!
  model(id: ID!): Model
  models: [Model!]!
  modelsByBrand(brandId: ID!): [Model!]!
}

type Mutation {
  placeholder: String
}
EOF

    # Search subgraph schema
    cat > "$SUBGRAPH_SCHEMAS_DIR/search.graphql" << 'EOF'
extend schema @link(url: "https://specs.apollo.dev/federation/v2.5", import: [])

type SearchResult {
  id: ID!
  title: String!
  description: String!
  price: Int!
  location: String!
  imageUrl: String
  relevanceScore: Float!
  resultType: SearchResultType!
}

enum SearchResultType {
  OFFER
  BRAND
  MODEL
}

input SearchFilters {
  query: String
  minPrice: Int
  maxPrice: Int
  location: String
  brand: String
  model: String
  yearFrom: Int
  yearTo: Int
}

type SearchConnection {
  edges: [SearchEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}

type SearchEdge {
  node: SearchResult!
  cursor: String!
}

type PageInfo {
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}

type Suggestion {
  text: String!
  category: String!
  count: Int!
}

type Query {
  search(filters: SearchFilters, first: Int, after: String): SearchConnection!
  autocomplete(query: String!): [Suggestion!]!
}

type Mutation {
  placeholder: String
}
EOF

    print_success "Stub schemas created"
}

# Function to compose supergraph using rover
compose_supergraph() {
    print_status "Composing supergraph schema..."
    
    if ! command -v rover &> /dev/null; then
        print_warning "Apollo Rover is not installed. Falling back to simple composition..."
        
        # Use simple composition as fallback
        if [ -x "./scripts/simple-compose.sh" ]; then
            if ./scripts/simple-compose.sh; then
                print_success "Simple supergraph schema composed successfully"
                return 0
            else
                print_error "Failed to compose simple supergraph schema"
                return 1
            fi
        else
            print_error "Simple composition script not found"
            return 1
        fi
    fi
    
    # Compose the supergraph
    if rover supergraph compose --config "$SUPERGRAPH_CONFIG_PATH" --output "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Supergraph schema composed successfully"
        return 0
    else
        print_error "Failed to compose supergraph schema"
        return 1
    fi
}

# Function to validate supergraph schema
validate_supergraph() {
    print_status "Validating supergraph schema..."
    
    if [ ! -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_error "Supergraph schema file not found"
        return 1
    fi
    
    # Basic validation - check if file is not empty and contains expected content
    if [ -s "$SUPERGRAPH_SCHEMA_PATH" ] && grep -q "type Query" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Supergraph schema validation passed"
        
        # Show schema stats
        local types_count=$(grep -c "^type " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        local directives_count=$(grep -c "@" "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        
        print_status "Schema statistics:"
        print_status "  - Types: $types_count"
        print_status "  - Directives: $directives_count"
        print_status "  - Size: $(wc -c < "$SUPERGRAPH_SCHEMA_PATH") bytes"
        
        return 0
    else
        print_error "Supergraph schema validation failed"
        return 1
    fi
}

# Function to update router configuration with supergraph schema
update_router_config() {
    print_status "Updating router configuration..."
    
    # Create a backup of the current router config
    cp "$ROUTER_CONFIG_PATH" "${ROUTER_CONFIG_PATH}.backup"
    
    # Add supergraph schema path to router config if not already present
    if ! grep -q "supergraph_path" "$ROUTER_CONFIG_PATH"; then
        # Insert supergraph_path after the supergraph section
        sed -i '/^supergraph:/a\  supergraph_path: ./supergraph.graphql' "$ROUTER_CONFIG_PATH"
        print_success "Router configuration updated with supergraph schema path"
    else
        print_status "Router configuration already contains supergraph schema path"
    fi
}

# Main execution
main() {
    print_status "Starting supergraph composition process..."
    
    # Create directories
    mkdir -p "$SUBGRAPH_SCHEMAS_DIR"
    
    # Check if we should fetch live schemas or use stubs
    local use_live_schemas=false
    
    if [ "$1" = "--live" ]; then
        print_status "Attempting to fetch live schemas from running services..."
        use_live_schemas=true
        
        # Check if services are running
        local services_running=0
        check_service "$UGC_ENDPOINT" "UGC Subgraph" && ((services_running++))
        check_service "$USERS_ENDPOINT" "Users Subgraph" && ((services_running++))
        check_service "$OFFERS_ENDPOINT" "Offers Subgraph" && ((services_running++))
        check_service "$CATALOG_ENDPOINT" "Catalog Subgraph" && ((services_running++))
        check_service "$SEARCH_ENDPOINT" "Search Subgraph" && ((services_running++))
        
        if [ $services_running -lt 5 ]; then
            print_warning "Not all services are running. Falling back to stub schemas."
            use_live_schemas=false
        fi
    fi
    
    if [ "$use_live_schemas" = true ]; then
        # Fetch schemas from running services
        fetch_schema "$UGC_ENDPOINT" "UGC Subgraph" "$SUBGRAPH_SCHEMAS_DIR/ugc.graphql"
        fetch_schema "$USERS_ENDPOINT" "Users Subgraph" "$SUBGRAPH_SCHEMAS_DIR/users.graphql"
        fetch_schema "$OFFERS_ENDPOINT" "Offers Subgraph" "$SUBGRAPH_SCHEMAS_DIR/offers.graphql"
        fetch_schema "$CATALOG_ENDPOINT" "Catalog Subgraph" "$SUBGRAPH_SCHEMAS_DIR/catalog.graphql"
        fetch_schema "$SEARCH_ENDPOINT" "Search Subgraph" "$SUBGRAPH_SCHEMAS_DIR/search.graphql"
    else
        # Use stub schemas
        create_stub_schemas
    fi
    
    # Create supergraph configuration
    create_supergraph_config
    
    # Compose supergraph
    if compose_supergraph; then
        # Validate composed schema
        if validate_supergraph; then
            # Update router configuration
            update_router_config
            
            print_success "Supergraph composition completed successfully!"
            print_status "Generated files:"
            print_status "  - Supergraph config: $SUPERGRAPH_CONFIG_PATH"
            print_status "  - Supergraph schema: $SUPERGRAPH_SCHEMA_PATH"
            print_status "  - Subgraph schemas: $SUBGRAPH_SCHEMAS_DIR/"
            
            return 0
        else
            print_error "Supergraph validation failed"
            return 1
        fi
    else
        print_error "Supergraph composition failed"
        return 1
    fi
}

# Show help
show_help() {
    echo "Auto.ru GraphQL Federation - Supergraph Composition Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --live    Fetch schemas from running services (default: use stub schemas)"
    echo "  --help    Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                # Compose using stub schemas"
    echo "  $0 --live         # Compose using live schemas from running services"
    echo ""
}

# Parse command line arguments
case "${1:-}" in
    --help|-h)
        show_help
        exit 0
        ;;
    *)
        main "$@"
        exit $?
        ;;
esac