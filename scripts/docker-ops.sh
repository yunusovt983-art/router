#!/bin/bash

# Docker Operations Script for Auto.ru GraphQL Federation
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if Docker is running
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        log_error "Docker is not running. Please start Docker and try again."
        exit 1
    fi
}

# Function to check if docker-compose is available
check_docker_compose() {
    if ! command -v docker-compose > /dev/null 2>&1 && ! docker compose version > /dev/null 2>&1; then
        log_error "docker-compose is not available. Please install docker-compose."
        exit 1
    fi
}

# Function to build all images
build_images() {
    local env=${1:-"dev"}
    
    log_info "Building Docker images for $env environment..."
    
    if [ "$env" = "prod" ]; then
        docker compose -f docker-compose.prod.yml build --parallel
    else
        docker compose build --parallel
    fi
    
    log_success "Docker images built successfully"
}

# Function to start services
start_services() {
    local env=${1:-"dev"}
    
    log_info "Starting services for $env environment..."
    
    case $env in
        "prod")
            docker compose -f docker-compose.prod.yml up -d
            ;;
        "dev")
            docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d
            ;;
        *)
            docker compose up -d
            ;;
    esac
    
    log_success "Services started successfully"
    
    # Wait for services to be healthy
    log_info "Waiting for services to be healthy..."
    sleep 10
    
    # Check health of key services
    check_service_health "apollo-router" "4000"
    check_service_health "ugc-subgraph" "4001"
    check_service_health "users-subgraph" "4002"
}

# Function to stop services
stop_services() {
    local env=${1:-"dev"}
    
    log_info "Stopping services for $env environment..."
    
    case $env in
        "prod")
            docker compose -f docker-compose.prod.yml down
            ;;
        "dev")
            docker compose -f docker-compose.yml -f docker-compose.dev.yml down
            ;;
        *)
            docker compose down
            ;;
    esac
    
    log_success "Services stopped successfully"
}

# Function to check service health
check_service_health() {
    local service_name=$1
    local port=$2
    local max_attempts=30
    local attempt=1
    
    log_info "Checking health of $service_name on port $port..."
    
    while [ $attempt -le $max_attempts ]; do
        if curl -f -s "http://localhost:$port/health" > /dev/null 2>&1; then
            log_success "$service_name is healthy"
            return 0
        fi
        
        log_info "Attempt $attempt/$max_attempts: $service_name not ready yet..."
        sleep 2
        attempt=$((attempt + 1))
    done
    
    log_warning "$service_name health check failed after $max_attempts attempts"
    return 1
}

# Function to show logs
show_logs() {
    local env=${1:-"dev"}
    local service=${2:-""}
    
    if [ "$env" = "prod" ]; then
        if [ -n "$service" ]; then
            docker compose -f docker-compose.prod.yml logs -f "$service"
        else
            docker compose -f docker-compose.prod.yml logs -f
        fi
    else
        if [ -n "$service" ]; then
            docker compose logs -f "$service"
        else
            docker compose logs -f
        fi
    fi
}

# Function to clean up Docker resources
cleanup() {
    log_info "Cleaning up Docker resources..."
    
    # Stop all services
    docker compose down --remove-orphans
    docker compose -f docker-compose.prod.yml down --remove-orphans 2>/dev/null || true
    
    # Remove unused images
    docker image prune -f
    
    # Remove unused volumes (be careful with this in production)
    if [ "${1:-}" = "--volumes" ]; then
        log_warning "Removing Docker volumes (this will delete all data)..."
        docker volume prune -f
    fi
    
    log_success "Docker cleanup completed"
}

# Function to show service status
show_status() {
    local env=${1:-"dev"}
    
    log_info "Service status for $env environment:"
    
    if [ "$env" = "prod" ]; then
        docker compose -f docker-compose.prod.yml ps
    else
        docker compose ps
    fi
}

# Function to restart a specific service
restart_service() {
    local service=$1
    local env=${2:-"dev"}
    
    if [ -z "$service" ]; then
        log_error "Service name is required"
        exit 1
    fi
    
    log_info "Restarting $service in $env environment..."
    
    if [ "$env" = "prod" ]; then
        docker compose -f docker-compose.prod.yml restart "$service"
    else
        docker compose restart "$service"
    fi
    
    log_success "$service restarted successfully"
}

# Function to scale a service
scale_service() {
    local service=$1
    local replicas=$2
    local env=${3:-"dev"}
    
    if [ -z "$service" ] || [ -z "$replicas" ]; then
        log_error "Service name and replica count are required"
        exit 1
    fi
    
    log_info "Scaling $service to $replicas replicas in $env environment..."
    
    if [ "$env" = "prod" ]; then
        docker compose -f docker-compose.prod.yml up -d --scale "$service=$replicas"
    else
        docker compose up -d --scale "$service=$replicas"
    fi
    
    log_success "$service scaled to $replicas replicas"
}

# Main function
main() {
    check_docker
    check_docker_compose
    
    case "${1:-help}" in
        "build")
            build_images "${2:-dev}"
            ;;
        "start")
            start_services "${2:-dev}"
            ;;
        "stop")
            stop_services "${2:-dev}"
            ;;
        "restart")
            restart_service "$2" "${3:-dev}"
            ;;
        "logs")
            show_logs "${2:-dev}" "$3"
            ;;
        "status")
            show_status "${2:-dev}"
            ;;
        "cleanup")
            cleanup "$2"
            ;;
        "scale")
            scale_service "$2" "$3" "${4:-dev}"
            ;;
        "health")
            check_service_health "$2" "$3"
            ;;
        "help"|*)
            echo "Docker Operations Script for Auto.ru GraphQL Federation"
            echo ""
            echo "Usage: $0 <command> [options]"
            echo ""
            echo "Commands:"
            echo "  build [env]              Build Docker images (env: dev|prod)"
            echo "  start [env]              Start services (env: dev|prod)"
            echo "  stop [env]               Stop services (env: dev|prod)"
            echo "  restart <service> [env]  Restart a specific service"
            echo "  logs [env] [service]     Show logs"
            echo "  status [env]             Show service status"
            echo "  cleanup [--volumes]      Clean up Docker resources"
            echo "  scale <service> <count>  Scale a service"
            echo "  health <service> <port>  Check service health"
            echo "  help                     Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 build prod            Build production images"
            echo "  $0 start dev             Start development environment"
            echo "  $0 logs prod ugc-subgraph Show UGC subgraph logs in production"
            echo "  $0 restart apollo-router Restart Apollo Router"
            echo "  $0 scale ugc-subgraph 3  Scale UGC subgraph to 3 replicas"
            ;;
    esac
}

# Run main function with all arguments
main "$@"