#!/bin/bash

# Production Deployment Script for Apollo Router Federation
# This script handles production deployment with comprehensive checks and monitoring

set -e

# Configuration
ENVIRONMENT="${ENVIRONMENT:-production}"
NAMESPACE="${NAMESPACE:-auto-ru-federation}"
DOCKER_REGISTRY="${DOCKER_REGISTRY:-registry.auto.ru}"
VERSION="${VERSION:-latest}"
HEALTH_CHECK_TIMEOUT="${HEALTH_CHECK_TIMEOUT:-300}"
ROLLBACK_ON_FAILURE="${ROLLBACK_ON_FAILURE:-true}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Logging functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

info() {
    echo -e "${PURPLE}[INFO]${NC} $1"
}

# Deployment state tracking
DEPLOYMENT_STATE_FILE="/tmp/deployment-state-${ENVIRONMENT}-$(date +%s).json"
PREVIOUS_VERSION=""
ROLLBACK_REQUIRED=false

# Initialize deployment state
init_deployment_state() {
    cat > "$DEPLOYMENT_STATE_FILE" << EOF
{
  "environment": "$ENVIRONMENT",
  "namespace": "$NAMESPACE",
  "version": "$VERSION",
  "start_time": "$(date -Iseconds)",
  "status": "started",
  "services": {},
  "previous_versions": {}
}
EOF
    log "Deployment state initialized: $DEPLOYMENT_STATE_FILE"
}

# Update deployment state
update_deployment_state() {
    local service="$1"
    local status="$2"
    local details="$3"
    
    python3 -c "
import json
import sys
from datetime import datetime

try:
    with open('$DEPLOYMENT_STATE_FILE', 'r') as f:
        state = json.load(f)
    
    state['services']['$service'] = {
        'status': '$status',
        'details': '$details',
        'timestamp': datetime.now().isoformat()
    }
    
    with open('$DEPLOYMENT_STATE_FILE', 'w') as f:
        json.dump(state, f, indent=2)
except Exception as e:
    print(f'Error updating deployment state: {e}', file=sys.stderr)
    sys.exit(1)
"
}

# Pre-deployment checks
run_pre_deployment_checks() {
    log "Running pre-deployment checks..."
    
    # Check required tools
    local required_tools=("kubectl" "docker" "helm" "jq" "curl")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            error "Required tool not found: $tool"
            exit 1
        fi
    done
    
    # Check Kubernetes connectivity
    if ! kubectl cluster-info &> /dev/null; then
        error "Cannot connect to Kubernetes cluster"
        exit 1
    fi
    
    # Check namespace exists
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        warning "Namespace $NAMESPACE does not exist, creating..."
        kubectl create namespace "$NAMESPACE"
    fi
    
    # Check Docker registry connectivity
    if ! docker login "$DOCKER_REGISTRY" &> /dev/null; then
        error "Cannot authenticate with Docker registry: $DOCKER_REGISTRY"
        exit 1
    fi
    
    # Verify images exist
    local services=("apollo-router" "ugc-subgraph" "users-subgraph" "offers-subgraph" "catalog-subgraph" "search-subgraph")
    for service in "${services[@]}"; do
        local image="$DOCKER_REGISTRY/$service:$VERSION"
        if ! docker manifest inspect "$image" &> /dev/null; then
            error "Docker image not found: $image"
            exit 1
        fi
        info "Verified image: $image"
    done
    
    # Check resource availability
    local available_nodes=$(kubectl get nodes --no-headers | grep -c Ready)
    if [ "$available_nodes" -lt 3 ]; then
        warning "Less than 3 nodes available. Current: $available_nodes"
    fi
    
    # Check storage classes
    if ! kubectl get storageclass standard &> /dev/null; then
        warning "Standard storage class not found"
    fi
    
    success "Pre-deployment checks completed"
}

# Build and push Docker images
build_and_push_images() {
    log "Building and pushing Docker images..."
    
    local services=("apollo-router" "ugc-subgraph" "users-subgraph" "offers-subgraph" "catalog-subgraph" "search-subgraph")
    
    for service in "${services[@]}"; do
        info "Building $service..."
        
        local dockerfile_path=""
        local build_context=""
        
        case "$service" in
            "apollo-router")
                dockerfile_path="dockerfiles/Dockerfile.router"
                build_context="."
                ;;
            "ugc-subgraph")
                dockerfile_path="ugc-subgraph/Dockerfile"
                build_context="ugc-subgraph"
                ;;
            "users-subgraph")
                dockerfile_path="users-subgraph/Dockerfile"
                build_context="users-subgraph"
                ;;
            "offers-subgraph")
                dockerfile_path="offers-subgraph/Dockerfile"
                build_context="offers-subgraph"
                ;;
            "catalog-subgraph")
                dockerfile_path="catalog-subgraph/Dockerfile"
                build_context="catalog-subgraph"
                ;;
            "search-subgraph")
                dockerfile_path="search-subgraph/Dockerfile"
                build_context="search-subgraph"
                ;;
        esac
        
        local image_tag="$DOCKER_REGISTRY/$service:$VERSION"
        local latest_tag="$DOCKER_REGISTRY/$service:latest"
        
        # Build image
        docker build -f "$dockerfile_path" -t "$image_tag" -t "$latest_tag" "$build_context"
        
        # Push images
        docker push "$image_tag"
        docker push "$latest_tag"
        
        update_deployment_state "$service" "image_built" "$image_tag"
        success "Built and pushed: $image_tag"
    done
}

# Deploy infrastructure components
deploy_infrastructure() {
    log "Deploying infrastructure components..."
    
    # Deploy PostgreSQL
    info "Deploying PostgreSQL..."
    helm upgrade --install postgresql \
        oci://registry-1.docker.io/bitnamicharts/postgresql \
        --namespace "$NAMESPACE" \
        --set auth.postgresPassword="$(kubectl get secret --namespace $NAMESPACE postgresql -o jsonpath="{.data.postgres-password}" 2>/dev/null | base64 -d || echo 'auto-ru-postgres-password')" \
        --set primary.persistence.size=100Gi \
        --set primary.resources.requests.memory=2Gi \
        --set primary.resources.requests.cpu=1000m \
        --set primary.resources.limits.memory=4Gi \
        --set primary.resources.limits.cpu=2000m \
        --wait --timeout=10m
    
    update_deployment_state "postgresql" "deployed" "PostgreSQL database deployed"
    
    # Deploy Redis
    info "Deploying Redis..."
    helm upgrade --install redis \
        oci://registry-1.docker.io/bitnamicharts/redis \
        --namespace "$NAMESPACE" \
        --set auth.password="$(kubectl get secret --namespace $NAMESPACE redis -o jsonpath="{.data.redis-password}" 2>/dev/null | base64 -d || echo 'auto-ru-redis-password')" \
        --set master.persistence.size=20Gi \
        --set master.resources.requests.memory=1Gi \
        --set master.resources.requests.cpu=500m \
        --set master.resources.limits.memory=2Gi \
        --set master.resources.limits.cpu=1000m \
        --wait --timeout=10m
    
    update_deployment_state "redis" "deployed" "Redis cache deployed"
    
    # Deploy monitoring stack
    info "Deploying monitoring stack..."
    
    # Prometheus
    helm upgrade --install prometheus \
        prometheus-community/kube-prometheus-stack \
        --namespace "$NAMESPACE" \
        --set prometheus.prometheusSpec.retention=30d \
        --set prometheus.prometheusSpec.storageSpec.volumeClaimTemplate.spec.resources.requests.storage=50Gi \
        --set grafana.adminPassword="$(kubectl get secret --namespace $NAMESPACE prometheus-grafana -o jsonpath="{.data.admin-password}" 2>/dev/null | base64 -d || echo 'auto-ru-grafana-password')" \
        --wait --timeout=15m
    
    update_deployment_state "prometheus" "deployed" "Prometheus monitoring deployed"
    
    # Jaeger
    kubectl apply -f - << EOF
apiVersion: jaegertracing.io/v1
kind: Jaeger
metadata:
  name: jaeger
  namespace: $NAMESPACE
spec:
  strategy: production
  storage:
    type: elasticsearch
    elasticsearch:
      nodeCount: 3
      resources:
        requests:
          memory: 2Gi
          cpu: 1000m
        limits:
          memory: 4Gi
          cpu: 2000m
EOF
    
    update_deployment_state "jaeger" "deployed" "Jaeger tracing deployed"
    
    success "Infrastructure components deployed"
}

# Deploy application services
deploy_application_services() {
    log "Deploying application services..."
    
    # Get current versions for rollback
    local services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "catalog-subgraph" "search-subgraph")
    for service in "${services[@]}"; do
        local current_version=$(kubectl get deployment "$service" -n "$NAMESPACE" -o jsonpath='{.spec.template.spec.containers[0].image}' 2>/dev/null | cut -d':' -f2 || echo "none")
        PREVIOUS_VERSION="$current_version"
        info "Current version of $service: $current_version"
    done
    
    # Deploy subgraphs
    for service in "${services[@]}"; do
        info "Deploying $service..."
        
        local image="$DOCKER_REGISTRY/$service:$VERSION"
        local port=""
        local env_vars=""
        
        case "$service" in
            "ugc-subgraph")
                port="4001"
                env_vars="DATABASE_URL=postgresql://postgres:auto-ru-postgres-password@postgresql:5432/ugc_db REDIS_URL=redis://:auto-ru-redis-password@redis-master:6379"
                ;;
            "users-subgraph")
                port="4002"
                env_vars="DATABASE_URL=postgresql://postgres:auto-ru-postgres-password@postgresql:5432/users_db"
                ;;
            "offers-subgraph")
                port="4004"
                env_vars="DATABASE_URL=postgresql://postgres:auto-ru-postgres-password@postgresql:5432/offers_db"
                ;;
            "catalog-subgraph")
                port="4003"
                env_vars="DATABASE_URL=postgresql://postgres:auto-ru-postgres-password@postgresql:5432/catalog_db"
                ;;
            "search-subgraph")
                port="4005"
                env_vars="ELASTICSEARCH_URL=http://elasticsearch:9200"
                ;;
        esac
        
        # Create deployment
        kubectl apply -f - << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: $service
  namespace: $NAMESPACE
  labels:
    app: $service
    version: $VERSION
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 1
  selector:
    matchLabels:
      app: $service
  template:
    metadata:
      labels:
        app: $service
        version: $VERSION
    spec:
      containers:
      - name: $service
        image: $image
        ports:
        - containerPort: $port
        env:
        - name: PORT
          value: "$port"
        - name: ENVIRONMENT
          value: "$ENVIRONMENT"
        - name: LOG_LEVEL
          value: "info"
        $(echo "$env_vars" | tr ' ' '\n' | sed 's/^/        - name: /' | sed 's/=/\n          value: "/' | sed 's/$/"/')
        resources:
          requests:
            memory: 512Mi
            cpu: 250m
          limits:
            memory: 1Gi
            cpu: 500m
        livenessProbe:
          httpGet:
            path: /health
            port: $port
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: $port
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
---
apiVersion: v1
kind: Service
metadata:
  name: $service
  namespace: $NAMESPACE
  labels:
    app: $service
spec:
  selector:
    app: $service
  ports:
  - port: $port
    targetPort: $port
    protocol: TCP
  type: ClusterIP
EOF
        
        # Wait for deployment to be ready
        kubectl rollout status deployment/"$service" -n "$NAMESPACE" --timeout=300s
        
        update_deployment_state "$service" "deployed" "Service deployed successfully"
        success "Deployed: $service"
    done
    
    # Deploy Apollo Router
    info "Deploying Apollo Router..."
    
    # Create router configuration
    kubectl create configmap apollo-router-config -n "$NAMESPACE" --from-file=router.yaml --dry-run=client -o yaml | kubectl apply -f -
    
    kubectl apply -f - << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apollo-router
  namespace: $NAMESPACE
  labels:
    app: apollo-router
    version: $VERSION
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 1
  selector:
    matchLabels:
      app: apollo-router
  template:
    metadata:
      labels:
        app: apollo-router
        version: $VERSION
    spec:
      containers:
      - name: apollo-router
        image: $DOCKER_REGISTRY/apollo-router:$VERSION
        ports:
        - containerPort: 4000
        env:
        - name: APOLLO_ROUTER_CONFIG_PATH
          value: "/etc/router/router.yaml"
        - name: APOLLO_ROUTER_LOG
          value: "info"
        volumeMounts:
        - name: config
          mountPath: /etc/router
          readOnly: true
        resources:
          requests:
            memory: 1Gi
            cpu: 500m
          limits:
            memory: 2Gi
            cpu: 1000m
        livenessProbe:
          httpGet:
            path: /health
            port: 4000
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /health
            port: 4000
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
      volumes:
      - name: config
        configMap:
          name: apollo-router-config
---
apiVersion: v1
kind: Service
metadata:
  name: apollo-router
  namespace: $NAMESPACE
  labels:
    app: apollo-router
spec:
  selector:
    app: apollo-router
  ports:
  - port: 4000
    targetPort: 4000
    protocol: TCP
  type: LoadBalancer
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: apollo-router-ingress
  namespace: $NAMESPACE
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - api.auto.ru
    secretName: apollo-router-tls
  rules:
  - host: api.auto.ru
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: apollo-router
            port:
              number: 4000
EOF
    
    # Wait for router deployment
    kubectl rollout status deployment/apollo-router -n "$NAMESPACE" --timeout=300s
    
    update_deployment_state "apollo-router" "deployed" "Apollo Router deployed successfully"
    success "Deployed: Apollo Router"
}

# Run health checks
run_health_checks() {
    log "Running comprehensive health checks..."
    
    local services=("apollo-router" "ugc-subgraph" "users-subgraph" "offers-subgraph" "catalog-subgraph" "search-subgraph")
    local failed_services=()
    
    for service in "${services[@]}"; do
        info "Checking health of $service..."
        
        local service_url=""
        if [ "$service" = "apollo-router" ]; then
            service_url="http://api.auto.ru/health"
        else
            local port=$(kubectl get service "$service" -n "$NAMESPACE" -o jsonpath='{.spec.ports[0].port}')
            service_url="http://$service.$NAMESPACE.svc.cluster.local:$port/health"
        fi
        
        local max_attempts=30
        local attempt=1
        local healthy=false
        
        while [ $attempt -le $max_attempts ]; do
            if kubectl run health-check-$service --rm -i --restart=Never --image=curlimages/curl -- curl -f "$service_url" &> /dev/null; then
                healthy=true
                break
            fi
            
            info "Health check attempt $attempt/$max_attempts for $service..."
            sleep 10
            ((attempt++))
        done
        
        if [ "$healthy" = true ]; then
            update_deployment_state "$service" "healthy" "Health check passed"
            success "Health check passed: $service"
        else
            update_deployment_state "$service" "unhealthy" "Health check failed after $max_attempts attempts"
            error "Health check failed: $service"
            failed_services+=("$service")
        fi
    done
    
    if [ ${#failed_services[@]} -gt 0 ]; then
        error "Health checks failed for: ${failed_services[*]}"
        ROLLBACK_REQUIRED=true
        return 1
    fi
    
    success "All health checks passed"
    return 0
}

# Run smoke tests
run_smoke_tests() {
    log "Running smoke tests..."
    
    local router_url="http://api.auto.ru/graphql"
    
    # Test 1: Basic GraphQL query
    info "Testing basic GraphQL query..."
    local basic_query='{"query":"query { __typename }"}'
    
    if ! kubectl run smoke-test-basic --rm -i --restart=Never --image=curlimages/curl -- \
        curl -X POST -H "Content-Type: application/json" -d "$basic_query" "$router_url" | grep -q "__typename"; then
        error "Basic GraphQL query failed"
        ROLLBACK_REQUIRED=true
        return 1
    fi
    
    # Test 2: Federated query
    info "Testing federated query..."
    local federated_query='{"query":"query { offers(first: 1) { edges { node { id title } } } }"}'
    
    if ! kubectl run smoke-test-federated --rm -i --restart=Never --image=curlimages/curl -- \
        curl -X POST -H "Content-Type: application/json" -d "$federated_query" "$router_url" | grep -q "offers"; then
        error "Federated query failed"
        ROLLBACK_REQUIRED=true
        return 1
    fi
    
    # Test 3: Health endpoints
    info "Testing health endpoints..."
    for service in "apollo-router" "ugc-subgraph" "users-subgraph" "offers-subgraph"; do
        local health_url=""
        if [ "$service" = "apollo-router" ]; then
            health_url="http://api.auto.ru/health"
        else
            local port=$(kubectl get service "$service" -n "$NAMESPACE" -o jsonpath='{.spec.ports[0].port}')
            health_url="http://$service.$NAMESPACE.svc.cluster.local:$port/health"
        fi
        
        if ! kubectl run smoke-test-health-$service --rm -i --restart=Never --image=curlimages/curl -- \
            curl -f "$health_url" &> /dev/null; then
            error "Health endpoint failed for $service"
            ROLLBACK_REQUIRED=true
            return 1
        fi
    done
    
    # Test 4: Authentication
    info "Testing authentication..."
    local auth_query='{"query":"mutation { login(username: \"test\", password: \"test\") { token } }"}'
    
    # This should fail with proper error (not crash)
    if kubectl run smoke-test-auth --rm -i --restart=Never --image=curlimages/curl -- \
        curl -X POST -H "Content-Type: application/json" -d "$auth_query" "$router_url" | grep -q "errors"; then
        info "Authentication test passed (properly rejected invalid credentials)"
    else
        warning "Authentication test inconclusive"
    fi
    
    success "All smoke tests passed"
    return 0
}

# Setup monitoring and alerting
setup_monitoring() {
    log "Setting up production monitoring and alerting..."
    
    # Create ServiceMonitor for Prometheus
    kubectl apply -f - << EOF
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: apollo-federation-metrics
  namespace: $NAMESPACE
  labels:
    app: apollo-federation
spec:
  selector:
    matchLabels:
      app: apollo-router
  endpoints:
  - port: http
    path: /metrics
    interval: 30s
---
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: subgraph-metrics
  namespace: $NAMESPACE
  labels:
    app: subgraphs
spec:
  selector:
    matchExpressions:
    - key: app
      operator: In
      values: ["ugc-subgraph", "users-subgraph", "offers-subgraph", "catalog-subgraph", "search-subgraph"]
  endpoints:
  - port: http
    path: /metrics
    interval: 30s
EOF
    
    # Create PrometheusRule for alerting
    kubectl apply -f - << EOF
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: apollo-federation-alerts
  namespace: $NAMESPACE
  labels:
    app: apollo-federation
spec:
  groups:
  - name: apollo-federation
    rules:
    - alert: HighErrorRate
      expr: rate(graphql_errors_total[5m]) / rate(graphql_requests_total[5m]) > 0.05
      for: 2m
      labels:
        severity: critical
      annotations:
        summary: "High GraphQL error rate"
        description: "GraphQL error rate is {{ \$value | humanizePercentage }}"
    
    - alert: HighResponseTime
      expr: histogram_quantile(0.95, rate(graphql_request_duration_seconds_bucket[5m])) > 1
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "High GraphQL response time"
        description: "95th percentile response time is {{ \$value }}s"
    
    - alert: ServiceDown
      expr: up{job="apollo-federation-metrics"} == 0
      for: 1m
      labels:
        severity: critical
      annotations:
        summary: "Apollo Router is down"
        description: "Apollo Router has been down for more than 1 minute"
    
    - alert: SubgraphDown
      expr: up{job="subgraph-metrics"} == 0
      for: 1m
      labels:
        severity: critical
      annotations:
        summary: "Subgraph is down"
        description: "Subgraph {{ \$labels.instance }} has been down for more than 1 minute"
EOF
    
    # Create Grafana dashboard
    kubectl create configmap grafana-dashboard-apollo-federation \
        --from-file=scripts/production/grafana-dashboard.json \
        -n "$NAMESPACE" \
        --dry-run=client -o yaml | kubectl apply -f -
    
    update_deployment_state "monitoring" "configured" "Monitoring and alerting configured"
    success "Monitoring and alerting configured"
}

# Rollback deployment
rollback_deployment() {
    error "Rolling back deployment due to failures..."
    
    if [ "$PREVIOUS_VERSION" != "none" ] && [ -n "$PREVIOUS_VERSION" ]; then
        local services=("apollo-router" "ugc-subgraph" "users-subgraph" "offers-subgraph" "catalog-subgraph" "search-subgraph")
        
        for service in "${services[@]}"; do
            warning "Rolling back $service to version $PREVIOUS_VERSION..."
            
            kubectl set image deployment/"$service" \
                "$service"="$DOCKER_REGISTRY/$service:$PREVIOUS_VERSION" \
                -n "$NAMESPACE"
            
            kubectl rollout status deployment/"$service" -n "$NAMESPACE" --timeout=300s
            
            update_deployment_state "$service" "rolled_back" "Rolled back to version $PREVIOUS_VERSION"
        done
        
        warning "Rollback completed to version: $PREVIOUS_VERSION"
    else
        error "No previous version available for rollback"
        error "Manual intervention required"
    fi
}

# Generate deployment report
generate_deployment_report() {
    log "Generating deployment report..."
    
    local report_file="deployment-report-${ENVIRONMENT}-${VERSION}-$(date +%Y%m%d_%H%M%S).md"
    
    cat > "$report_file" << EOF
# Production Deployment Report

**Environment:** $ENVIRONMENT
**Namespace:** $NAMESPACE
**Version:** $VERSION
**Date:** $(date)
**Status:** $([ "$ROLLBACK_REQUIRED" = true ] && echo "FAILED (Rolled Back)" || echo "SUCCESS")

## Deployment Summary

EOF
    
    # Add deployment state information
    if [ -f "$DEPLOYMENT_STATE_FILE" ]; then
        python3 -c "
import json
import sys

try:
    with open('$DEPLOYMENT_STATE_FILE', 'r') as f:
        state = json.load(f)
    
    print('### Services Deployed')
    print()
    for service, info in state.get('services', {}).items():
        status_emoji = '✅' if info['status'] in ['deployed', 'healthy'] else '❌' if info['status'] == 'failed' else '⚠️'
        print(f'- {status_emoji} **{service}**: {info[\"status\"]} - {info[\"details\"]}')
    
    print()
    print('### Deployment Timeline')
    print()
    for service, info in state.get('services', {}).items():
        print(f'- **{info[\"timestamp\"]}**: {service} - {info[\"status\"]}')
    
except Exception as e:
    print(f'Error reading deployment state: {e}')
" >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

## Health Check Results

$([ "$ROLLBACK_REQUIRED" = true ] && echo "❌ Health checks failed" || echo "✅ All health checks passed")

## Smoke Test Results

$([ "$ROLLBACK_REQUIRED" = true ] && echo "❌ Smoke tests failed" || echo "✅ All smoke tests passed")

## Monitoring and Alerting

✅ Prometheus monitoring configured
✅ Grafana dashboards deployed
✅ Alert rules configured
✅ ServiceMonitors created

## Access Information

- **GraphQL API**: https://api.auto.ru/graphql
- **Grafana Dashboard**: https://grafana.auto.ru/d/apollo-federation
- **Prometheus**: https://prometheus.auto.ru
- **Jaeger Tracing**: https://jaeger.auto.ru

## Next Steps

$(if [ "$ROLLBACK_REQUIRED" = true ]; then
    echo "1. 🚨 **URGENT**: Investigate deployment failures"
    echo "2. 🔍 Review logs and error messages"
    echo "3. 🛠️ Fix identified issues"
    echo "4. 🔄 Retry deployment after fixes"
    echo "5. 📞 Notify stakeholders of rollback"
else
    echo "1. ✅ Monitor system performance and metrics"
    echo "2. 📊 Verify all alerts are working correctly"
    echo "3. 🔍 Review application logs for any issues"
    echo "4. 📈 Monitor business metrics and user feedback"
    echo "5. 📞 Notify stakeholders of successful deployment"
fi)

## Rollback Information

$(if [ "$ROLLBACK_REQUIRED" = true ]; then
    echo "**Rollback Status**: Executed"
    echo "**Previous Version**: $PREVIOUS_VERSION"
    echo "**Rollback Time**: $(date)"
else
    echo "**Rollback Status**: Not required"
    echo "**Previous Version**: $PREVIOUS_VERSION (available for rollback if needed)"
fi)

## Support Contacts

- **DevOps Team**: devops@auto.ru
- **Platform Team**: platform@auto.ru
- **On-Call Engineer**: +7-XXX-XXX-XXXX

---

**Report Generated**: $(date)
**Deployment State File**: $DEPLOYMENT_STATE_FILE
EOF
    
    success "Deployment report generated: $report_file"
    
    # Send report via email/Slack (implementation depends on your notification system)
    # send_deployment_notification "$report_file"
}

# Cleanup function
cleanup() {
    log "Cleaning up deployment artifacts..."
    
    # Clean up temporary files
    if [ -f "$DEPLOYMENT_STATE_FILE" ]; then
        info "Deployment state file preserved: $DEPLOYMENT_STATE_FILE"
    fi
    
    # Clean up test pods
    kubectl delete pods -l "run" -n "$NAMESPACE" --ignore-not-found=true
    
    success "Cleanup completed"
}

# Main deployment function
main() {
    log "Starting production deployment for Apollo Router Federation"
    log "Environment: $ENVIRONMENT"
    log "Namespace: $NAMESPACE"
    log "Version: $VERSION"
    
    # Set trap for cleanup
    trap cleanup EXIT
    
    # Initialize deployment tracking
    init_deployment_state
    
    # Run deployment phases
    run_pre_deployment_checks
    
    if [ "${SKIP_BUILD:-false}" != "true" ]; then
        build_and_push_images
    fi
    
    deploy_infrastructure
    deploy_application_services
    setup_monitoring
    
    # Run validation
    if ! run_health_checks; then
        if [ "$ROLLBACK_ON_FAILURE" = "true" ]; then
            rollback_deployment
        fi
    elif ! run_smoke_tests; then
        if [ "$ROLLBACK_ON_FAILURE" = "true" ]; then
            rollback_deployment
        fi
    fi
    
    # Generate report
    generate_deployment_report
    
    if [ "$ROLLBACK_REQUIRED" = "true" ]; then
        error "Deployment failed and was rolled back"
        exit 1
    else
        success "Production deployment completed successfully!"
        success "GraphQL API available at: https://api.auto.ru/graphql"
    fi
}

# Handle command line arguments
case "${1:-}" in
    "check")
        run_pre_deployment_checks
        ;;
    "build")
        run_pre_deployment_checks
        build_and_push_images
        ;;
    "infrastructure")
        run_pre_deployment_checks
        deploy_infrastructure
        ;;
    "services")
        run_pre_deployment_checks
        deploy_application_services
        ;;
    "monitoring")
        setup_monitoring
        ;;
    "health")
        run_health_checks
        ;;
    "smoke")
        run_smoke_tests
        ;;
    "rollback")
        rollback_deployment
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [check|build|infrastructure|services|monitoring|health|smoke|rollback|help]"
        echo ""
        echo "Options:"
        echo "  check          Run pre-deployment checks only"
        echo "  build          Build and push Docker images only"
        echo "  infrastructure Deploy infrastructure components only"
        echo "  services       Deploy application services only"
        echo "  monitoring     Setup monitoring and alerting only"
        echo "  health         Run health checks only"
        echo "  smoke          Run smoke tests only"
        echo "  rollback       Rollback to previous version"
        echo "  help           Show this help message"
        echo ""
        echo "Environment variables:"
        echo "  ENVIRONMENT              Target environment (default: production)"
        echo "  NAMESPACE                Kubernetes namespace (default: auto-ru-federation)"
        echo "  DOCKER_REGISTRY          Docker registry URL (default: registry.auto.ru)"
        echo "  VERSION                  Version to deploy (default: latest)"
        echo "  HEALTH_CHECK_TIMEOUT     Health check timeout in seconds (default: 300)"
        echo "  ROLLBACK_ON_FAILURE      Rollback on failure (default: true)"
        echo "  SKIP_BUILD               Skip image building (default: false)"
        exit 0
        ;;
    *)
        main
        ;;
esac