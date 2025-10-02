#!/bin/bash

# Simple supergraph composition without Apollo Rover
# This creates a basic combined schema for development purposes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Configuration
SUPERGRAPH_SCHEMA_PATH="./supergraph.graphql"
SCHEMAS_DIR="./schemas"

# Function to create a simple combined schema
create_simple_supergraph() {
    print_status "Creating simple supergraph schema..."
    
    # Create the schemas directory if it doesn't exist
    mkdir -p "$SCHEMAS_DIR"
    
    # Create the combined supergraph schema
    cat > "$SUPERGRAPH_SCHEMA_PATH" << 'EOF'
# Auto.ru GraphQL Federation - Combined Schema
# This is a simplified supergraph schema for development

schema {
  query: Query
  mutation: Mutation
}

# Scalars
scalar DateTime

# Common types
type PageInfo {
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}

# User entity (from Users subgraph)
type User {
  id: ID!
  name: String!
  email: String!
  phone: String
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Extended by UGC subgraph
  reviews(first: Int, after: String): ReviewConnection!
}

# Offer entity (from Offers subgraph)
type Offer {
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
  
  # Relationships
  seller: User!
  
  # Extended by UGC subgraph
  reviews(first: Int, after: String): ReviewConnection!
  averageRating: Float
  reviewsCount: Int!
}

# Review entity (from UGC subgraph)
type Review {
  id: ID!
  offerId: ID!
  authorId: ID!
  rating: Int!
  text: String!
  createdAt: DateTime!
  updatedAt: DateTime!
  isModerated: Boolean!
  
  # Relationships
  offer: Offer!
  author: User!
}

# Review connection for pagination
type ReviewConnection {
  edges: [ReviewEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}

type ReviewEdge {
  node: Review!
  cursor: String!
}

# Brand entity (from Catalog subgraph)
type Brand {
  id: ID!
  name: String!
  country: String!
  logoUrl: String
  foundedYear: Int
}

# Model entity (from Catalog subgraph)
type Model {
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
  
  # Relationships
  brand: Brand
}

# Search entities (from Search subgraph)
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

type SearchConnection {
  edges: [SearchEdge!]!
  pageInfo: PageInfo!
  totalCount: Int!
}

type SearchEdge {
  node: SearchResult!
  cursor: String!
}

type Suggestion {
  text: String!
  category: String!
  count: Int!
}

# Input types
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

# Root Query type
type Query {
  # User queries
  user(id: ID!): User
  users: [User!]!
  
  # Offer queries
  offer(id: ID!): Offer
  offers: [Offer!]!
  offersBySeller(sellerId: ID!): [Offer!]!
  
  # Review queries
  review(id: ID!): Review
  reviews(filter: ReviewsFilter, first: Int, after: String): ReviewConnection!
  
  # Catalog queries
  brand(id: ID!): Brand
  brands: [Brand!]!
  model(id: ID!): Model
  models: [Model!]!
  modelsByBrand(brandId: ID!): [Model!]!
  
  # Search queries
  search(filters: SearchFilters, first: Int, after: String): SearchConnection!
  autocomplete(query: String!): [Suggestion!]!
}

# Root Mutation type
type Mutation {
  # Review mutations
  createReview(input: CreateReviewInput!): Review!
  updateReview(id: ID!, input: UpdateReviewInput!): Review!
  deleteReview(id: ID!): Boolean!
  moderateReview(id: ID!, approved: Boolean!): Review!
  
  # Placeholder mutations for other subgraphs
  placeholder: String
}
EOF

    print_success "Simple supergraph schema created"
}

# Function to validate the created schema
validate_simple_schema() {
    print_status "Validating simple supergraph schema..."
    
    if [ ! -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_error "Supergraph schema file not found"
        return 1
    fi
    
    # Basic validation
    if [ -s "$SUPERGRAPH_SCHEMA_PATH" ] && grep -q "type Query" "$SUPERGRAPH_SCHEMA_PATH"; then
        print_success "Schema validation passed"
        
        # Show schema stats
        local types_count=$(grep -c "^type " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        local enums_count=$(grep -c "^enum " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        local inputs_count=$(grep -c "^input " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        local scalars_count=$(grep -c "^scalar " "$SUPERGRAPH_SCHEMA_PATH" || echo "0")
        
        print_status "Schema statistics:"
        print_status "  - Types: $types_count"
        print_status "  - Enums: $enums_count"
        print_status "  - Inputs: $inputs_count"
        print_status "  - Scalars: $scalars_count"
        print_status "  - Size: $(wc -c < "$SUPERGRAPH_SCHEMA_PATH") bytes"
        
        return 0
    else
        print_error "Schema validation failed"
        return 1
    fi
}

# Main function
main() {
    print_status "Creating simple supergraph schema for development..."
    
    create_simple_supergraph
    
    if validate_simple_schema; then
        print_success "✅ Simple supergraph schema created successfully!"
        print_status "Generated file: $SUPERGRAPH_SCHEMA_PATH"
        print_warning "Note: This is a simplified schema for development."
        print_warning "For production, use Apollo Rover for proper federation composition."
        return 0
    else
        print_error "❌ Failed to create valid supergraph schema"
        return 1
    fi
}

# Run main function
main "$@"