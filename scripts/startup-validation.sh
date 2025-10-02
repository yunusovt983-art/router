#!/bin/bash

# Auto.ru GraphQL Federation - Startup Validation Script
# This script runs validation checks before starting the Apollo Router

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SUPERGRAPH_SCHEMA_PATH="$PROJECT_ROOT/supergraph.graphql"
ROUTER_CONFIG_PATH="$PROJECT_ROOT/router.yaml"

# Function to print colored output
print_status() {
    echo -e "${BLUE}[STARTUP]${NC} $1"
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

# Function to check if supergraph schema exists and is valid
check_supergraph_schema() {
    print_status "Checking supergraph schema..."
    
    if [ ! -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_error "Supergraph schema not found at: $SUPERGRAPH_SCHEMA_PATH"
        print_status "Attempting to compose supergraph schema..."
        
        # Try to compose the schema
        if [ -x "$SCRIPT_DIR/compose-supergraph.sh" ]; then
            if "$SCRIPT_DIR/compose-supergraph.sh"; then
                print_success "Supergraph schema composed successfully"
            else
                print_error "Failed to compose supergraph schema"
                return 1
            fi
        else
            print_error "Compose script not found or not executable"
            return 1
        fi
    fi
    
    # Validate the schema
    if [ -x "$SCRIPT_DIR/validate-supergraph.sh" ]; then
        if "$SCRIPT_DIR/validate-supergraph.sh"; then
            print_success "Supergraph schema validation passed"
        else
            print_error "Supergraph schema validation failed"
            return 1
        fi
    else
        print_warning "Validation script not found, skipping detailed validation"
        
        # Basic validation
        if [ -s "$SUPERGRAPH_SCHEMA_PATH" ] && grep -q "type Query" "$SUPERGRAPH_SCHEMA_PATH"; then
            print_success "Basic supergraph schema validation passed"
        else
            print_error "Basic supergraph schema validation failed"
            return 1
        fi
    fi
    
    return 0
}

# Function to check router configuration
check_router_config() {
    print_status "Checking router configuration..."
    
    if [ ! -f "$ROUTER_CONFIG_PATH" ]; then
        print_error "Router configuration not found at: $ROUTER_CONFIG_PATH"
        return 1
    fi
    
    # Check if config references the supergraph schema
    if grep -q "supergraph_path:" "$ROUTER_CONFIG_PATH"; then
        local schema_path=$(grep "supergraph_path:" "$ROUTER_CONFIG_PATH" | sed 's/.*supergraph_path: *//' | tr -d '"')
        
        # Convert relative path to absolute if needed
        if [[ "$schema_path" != /* ]]; then
            schema_path="$PROJECT_ROOT/$schema_path"
        fi
        
        if [ -f "$schema_path" ]; then
            print_success "Router configuration references valid supergraph schema"
        else
            print_error "Router configuration references non-existent schema: $schema_path"
            return 1
        fi
    else
        print_warning "Router configuration does not specify supergraph_path"
    fi
    
    # Check for required sections
    local required_sections=("supergraph" "subgraphs")
    for section in "${required_sections[@]}"; do
        if grep -q "^$section:" "$ROUTER_CONFIG_PATH"; then
            print_success "Found required section: $section"
        else
            print_error "Missing required section in router config: $section"
            return 1
        fi
    done
    
    return 0
}

# Function to check subgraph health (optional)
check_subgraph_health() {
    print_status "Checking subgraph health (optional)..."
    
    # Define subgraph endpoints
    local subgraphs=(
        "ugc:http://ugc-subgraph:4001/health"
        "users:http://users-subgraph:4002/health"
        "offers:http://offers-subgraph:4004/health"
        "catalog:http://catalog-subgraph:4003/health"
        "search:http://search-subgraph:4005/health"
    )
    
    local healthy_count=0
    local total_count=${#subgraphs[@]}
    
    for subgraph_info in "${subgraphs[@]}"; do
        local name="${subgraph_info%%:*}"
        local url="${subgraph_info#*:}"
        
        if curl -s -f "$url" > /dev/null 2>&1; then
            print_success "Subgraph $name is healthy"
            ((healthy_count++))
        else
            print_warning "Subgraph $name is not responding at $url"
        fi
    done
    
    print_status "Subgraph health: $healthy_count/$total_count services responding"
    
    if [ $healthy_count -eq 0 ]; then
        print_warning "No subgraphs are responding - router will start but may not function properly"
    elif [ $healthy_count -lt $total_count ]; then
        print_warning "Some subgraphs are not responding - partial functionality expected"
    else
        print_success "All subgraphs are healthy"
    fi
    
    return 0
}

# Function to check dependencies
check_dependencies() {
    print_status "Checking dependencies..."
    
    # Check if Docker is available (for containerized deployment)
    if command -v docker &> /dev/null; then
        print_success "Docker is available"
    else
        print_warning "Docker is not available"
    fi
    
    # Check if curl is available (for health checks)
    if command -v curl &> /dev/null; then
        print_success "curl is available"
    else
        print_warning "curl is not available - health checks will be skipped"
    fi
    
    return 0
}

# Function to create necessary directories
create_directories() {
    print_status "Creating necessary directories..."
    
    local dirs=("schemas" "logs" "tmp")
    
    for dir in "${dirs[@]}"; do
        local dir_path="$PROJECT_ROOT/$dir"
        if [ ! -d "$dir_path" ]; then
            mkdir -p "$dir_path"
            print_success "Created directory: $dir"
        fi
    done
    
    return 0
}

# Function to set up environment
setup_environment() {
    print_status "Setting up environment..."
    
    # Set default environment variables if not already set
    export APOLLO_ROUTER_CONFIG_PATH="${APOLLO_ROUTER_CONFIG_PATH:-$ROUTER_CONFIG_PATH}"
    export APOLLO_ROUTER_LOG="${APOLLO_ROUTER_LOG:-info}"
    export APOLLO_ROUTER_SUPERGRAPH_PATH="${APOLLO_ROUTER_SUPERGRAPH_PATH:-$SUPERGRAPH_SCHEMA_PATH}"
    
    print_success "Environment variables configured"
    print_status "  APOLLO_ROUTER_CONFIG_PATH=$APOLLO_ROUTER_CONFIG_PATH"
    print_status "  APOLLO_ROUTER_LOG=$APOLLO_ROUTER_LOG"
    print_status "  APOLLO_ROUTER_SUPERGRAPH_PATH=$APOLLO_ROUTER_SUPERGRAPH_PATH"
    
    return 0
}

# Function to perform pre-flight checks
preflight_checks() {
    print_status "Performing pre-flight checks..."
    
    local checks_passed=0
    local total_checks=0
    
    # Critical checks (must pass)
    local critical_checks=(
        "check_supergraph_schema"
        "check_router_config"
    )
    
    for check in "${critical_checks[@]}"; do
        ((total_checks++))
        if $check; then
            ((checks_passed++))
        else
            print_error "Critical check failed: $check"
            return 1
        fi
    done
    
    # Optional checks (warnings only)
    local optional_checks=(
        "check_dependencies"
        "check_subgraph_health"
    )
    
    for check in "${optional_checks[@]}"; do
        $check || true  # Don't fail on optional checks
    done
    
    print_success "Pre-flight checks completed: $checks_passed/$total_checks critical checks passed"
    return 0
}

# Main startup validation function
main() {
    print_status "Starting Apollo Router startup validation..."
    print_status "Project root: $PROJECT_ROOT"
    
    # Create necessary directories
    create_directories
    
    # Set up environment
    setup_environment
    
    # Run pre-flight checks
    if preflight_checks; then
        print_success "🚀 All startup validation checks passed!"
        print_status "Apollo Router is ready to start."
        
        # Optionally start the router if requested
        if [ "$1" = "--start-router" ]; then
            print_status "Starting Apollo Router..."
            
            # Change to project root
            cd "$PROJECT_ROOT"
            
            # Start router with Docker Compose
            if [ -f "docker-compose.yml" ]; then
                docker compose up apollo-router
            else
                print_error "docker-compose.yml not found"
                return 1
            fi
        fi
        
        return 0
    else
        print_error "❌ Startup validation failed!"
        print_error "Please fix the issues before starting Apollo Router."
        return 1
    fi
}

# Show help
show_help() {
    echo "Auto.ru GraphQL Federation - Startup Validation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --start-router    Start Apollo Router after successful validation"
    echo "  --help           Show this help message"
    echo ""
    echo "This script performs startup validation including:"
    echo "  - Supergraph schema validation"
    echo "  - Router configuration validation"
    echo "  - Dependency checks"
    echo "  - Optional subgraph health checks"
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