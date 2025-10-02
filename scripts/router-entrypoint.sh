#!/bin/bash

# Auto.ru GraphQL Federation - Router Entrypoint Script
# This script runs validation before starting the Apollo Router in Docker

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[ROUTER]${NC} $1"
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
ROUTER_CONFIG_PATH="${APOLLO_ROUTER_CONFIG_PATH:-/dist/config/router.yaml}"
SUPERGRAPH_SCHEMA_PATH="${APOLLO_ROUTER_SUPERGRAPH_PATH:-/dist/config/supergraph.graphql}"
VALIDATION_ENABLED="${APOLLO_ROUTER_VALIDATION_ENABLED:-true}"
STARTUP_DELAY="${APOLLO_ROUTER_STARTUP_DELAY:-5}"

# Function to wait for dependencies
wait_for_dependencies() {
    print_status "Waiting for dependencies to be ready..."
    
    # Wait a bit for services to start
    sleep "$STARTUP_DELAY"
    
    # Check if subgraphs are responding (optional)
    local subgraphs=(
        "ugc-subgraph:4001"
        "users-subgraph:4002"
        "catalog-subgraph:4003"
        "offers-subgraph:4004"
        "search-subgraph:4005"
    )
    
    local ready_count=0
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        ready_count=0
        
        for subgraph in "${subgraphs[@]}"; do
            local host="${subgraph%%:*}"
            local port="${subgraph#*:}"
            
            if nc -z "$host" "$port" 2>/dev/null; then
                ((ready_count++))
            fi
        done
        
        if [ $ready_count -eq ${#subgraphs[@]} ]; then
            print_success "All subgraphs are ready ($ready_count/${#subgraphs[@]})"
            break
        else
            print_status "Waiting for subgraphs... ($ready_count/${#subgraphs[@]} ready) - attempt $attempt/$max_attempts"
            sleep 2
            ((attempt++))
        fi
    done
    
    if [ $ready_count -lt ${#subgraphs[@]} ]; then
        print_warning "Not all subgraphs are ready ($ready_count/${#subgraphs[@]}), but continuing..."
    fi
}

# Function to validate configuration
validate_configuration() {
    print_status "Validating router configuration..."
    
    # Check if config file exists
    if [ ! -f "$ROUTER_CONFIG_PATH" ]; then
        print_error "Router configuration file not found: $ROUTER_CONFIG_PATH"
        return 1
    fi
    
    print_success "Router configuration found: $ROUTER_CONFIG_PATH"
    
    # Check if supergraph schema exists
    if [ ! -f "$SUPERGRAPH_SCHEMA_PATH" ]; then
        print_warning "Supergraph schema file not found: $SUPERGRAPH_SCHEMA_PATH"
        print_status "Router will attempt to fetch schemas from subgraphs at runtime"
    else
        print_success "Supergraph schema found: $SUPERGRAPH_SCHEMA_PATH"
        
        # Basic schema validation
        if [ -s "$SUPERGRAPH_SCHEMA_PATH" ] && grep -q "type Query" "$SUPERGRAPH_SCHEMA_PATH"; then
            print_success "Supergraph schema appears valid"
        else
            print_error "Supergraph schema appears invalid"
            return 1
        fi
    fi
    
    return 0
}

# Function to set up environment
setup_environment() {
    print_status "Setting up environment..."
    
    # Set default values
    export APOLLO_ROUTER_CONFIG_PATH="$ROUTER_CONFIG_PATH"
    export APOLLO_ROUTER_LOG="${APOLLO_ROUTER_LOG:-info}"
    export APOLLO_ROUTER_HOT_RELOAD="${APOLLO_ROUTER_HOT_RELOAD:-false}"
    
    # Development vs Production settings
    if [ "${ENVIRONMENT:-development}" = "production" ]; then
        export APOLLO_ROUTER_LOG="${APOLLO_ROUTER_LOG:-warn}"
        export APOLLO_ROUTER_HOT_RELOAD="false"
        print_status "Production environment detected"
    else
        export APOLLO_ROUTER_LOG="${APOLLO_ROUTER_LOG:-info}"
        export APOLLO_ROUTER_HOT_RELOAD="${APOLLO_ROUTER_HOT_RELOAD:-true}"
        print_status "Development environment detected"
    fi
    
    print_success "Environment configured"
    print_status "  Config: $APOLLO_ROUTER_CONFIG_PATH"
    print_status "  Log Level: $APOLLO_ROUTER_LOG"
    print_status "  Hot Reload: $APOLLO_ROUTER_HOT_RELOAD"
}

# Function to show startup banner
show_banner() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║                Auto.ru GraphQL Federation                    ║"
    echo "║                    Apollo Router                             ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
    print_status "Starting Apollo Router for Auto.ru GraphQL Federation..."
    print_status "Version: $(router --version 2>/dev/null || echo 'Unknown')"
    print_status "Environment: ${ENVIRONMENT:-development}"
    print_status "Timestamp: $(date)"
    echo ""
}

# Function to handle shutdown
handle_shutdown() {
    print_status "Received shutdown signal, stopping Apollo Router..."
    
    # Kill the router process if it's running
    if [ -n "$ROUTER_PID" ]; then
        kill -TERM "$ROUTER_PID" 2>/dev/null || true
        wait "$ROUTER_PID" 2>/dev/null || true
    fi
    
    print_success "Apollo Router stopped gracefully"
    exit 0
}

# Function to start the router
start_router() {
    print_status "Starting Apollo Router..."
    
    # Set up signal handlers for graceful shutdown
    trap handle_shutdown SIGTERM SIGINT
    
    # Start the router in the background
    router --config "$APOLLO_ROUTER_CONFIG_PATH" &
    ROUTER_PID=$!
    
    print_success "Apollo Router started with PID: $ROUTER_PID"
    print_status "GraphQL endpoint: http://0.0.0.0:4000/graphql"
    print_status "Health check: http://0.0.0.0:4000/health"
    print_status "Metrics: http://0.0.0.0:9090/metrics"
    
    # Wait for the router process
    wait "$ROUTER_PID"
    
    # If we get here, the router has exited
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        print_success "Apollo Router exited normally"
    else
        print_error "Apollo Router exited with code: $exit_code"
    fi
    
    exit $exit_code
}

# Main function
main() {
    show_banner
    
    # Set up environment
    setup_environment
    
    # Wait for dependencies if enabled
    if [ "${APOLLO_ROUTER_WAIT_FOR_SUBGRAPHS:-true}" = "true" ]; then
        wait_for_dependencies
    fi
    
    # Validate configuration if enabled
    if [ "$VALIDATION_ENABLED" = "true" ]; then
        if ! validate_configuration; then
            print_error "Configuration validation failed"
            exit 1
        fi
    fi
    
    # Start the router
    start_router
}

# Show help
show_help() {
    echo "Auto.ru GraphQL Federation - Router Entrypoint Script"
    echo ""
    echo "Environment Variables:"
    echo "  APOLLO_ROUTER_CONFIG_PATH          - Path to router configuration (default: /dist/config/router.yaml)"
    echo "  APOLLO_ROUTER_SUPERGRAPH_PATH      - Path to supergraph schema (default: /dist/config/supergraph.graphql)"
    echo "  APOLLO_ROUTER_LOG                  - Log level (default: info)"
    echo "  APOLLO_ROUTER_VALIDATION_ENABLED   - Enable startup validation (default: true)"
    echo "  APOLLO_ROUTER_STARTUP_DELAY        - Delay before starting in seconds (default: 5)"
    echo "  APOLLO_ROUTER_WAIT_FOR_SUBGRAPHS   - Wait for subgraphs to be ready (default: true)"
    echo "  ENVIRONMENT                        - Environment (development/production)"
    echo ""
    echo "Usage:"
    echo "  $0                 - Start Apollo Router with validation"
    echo "  $0 --help          - Show this help message"
    echo "  $0 --no-validation - Start without validation"
    echo ""
}

# Parse command line arguments
case "${1:-}" in
    --help|-h)
        show_help
        exit 0
        ;;
    --no-validation)
        VALIDATION_ENABLED="false"
        main
        ;;
    *)
        main "$@"
        ;;
esac