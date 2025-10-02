# Task 13: Deployment Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_DEPLOYMENT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Диаграмма развертывания Task 13 показывает физическое размещение компонентов системы миграции в различных окружениях. Каждый deployment node имеет конкретную реализацию в виде конфигурационных файлов, скриптов развертывания и инфраструктурного кода.

### Developer Machine Environment

#### Docker Desktop Runtime
**PlantUML элемент:**
```plantuml
Deployment_Node(local_docker, "Docker Desktop", "Container Runtime") {
    Container(local_migration, "Migration Service", "Rust/Docker", "Port 4001")
    Container(local_redis, "Redis Cache", "Docker", "Port 6379")
    Container(local_ugc, "UGC GraphQL", "Rust/Docker", "Port 4002")
}
```

**Docker Compose реализация:**
```yaml
# docker-compose.yml - Local development
version: '3.8'

services:
  # Migration Service
  migration-service:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
      target: development
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=debug
      - RUST_BACKTRACE=1
      - MIGRATION_CONFIG_PATH=/app/feature-flags.yaml
      - REDIS_URL=redis://redis:6379
      - UGC_GRAPHQL_ENDPOINT=http://ugc-service:4002/graphql
    volumes:
      # Hot reload для development
      - ./ugc-subgraph/src:/app/src:ro
      - ./ugc-subgraph/feature-flags.yaml:/app/feature-flags.yaml:ro
      - cargo_cache:/usr/local/cargo/registry
    depends_on:
      - redis
      - ugc-service
    networks:
      - migration-network

  # Redis Cache
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes --maxmemory 256mb
    networks:
      - migration-network

  # UGC GraphQL Service
  ugc-service:
    build:
      context: .
      dockerfile: ugc-subgraph/Dockerfile
      target: runtime
    ports:
      - "4002:4002"
    environment:
      - RUST_LOG=info
      - DATABASE_URL=postgresql://ugc_user:ugc_password@postgres:5432/ugc_db
    depends_on:
      - postgres
    networks:
      - migration-network

volumes:
  redis_data:
  postgres_data:
  cargo_cache:

networks:
  migration-network:
    driver: bridge
```

**Dockerfile для Migration Service:**
```dockerfile
# ugc-subgraph/Dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency files
COPY Cargo.toml Cargo.lock ./
COPY ugc-subgraph/Cargo.toml ./ugc-subgraph/

# Build dependencies (cached layer)
RUN mkdir -p ugc-subgraph/src && \
    echo "fn main() {}" > ugc-subgraph/src/main.rs
RUN cargo build --release --package ugc-subgraph

# Copy source code
COPY . .
RUN touch ugc-subgraph/src/main.rs && \
    cargo build --release --package ugc-subgraph

# Development stage (for hot reload)
FROM rust:1.75-slim as development
WORKDIR /app
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libpq-dev curl
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/
EXPOSE 4001
CMD ["ugc-subgraph"]

# Runtime stage
FROM debian:bookworm-slim as runtime
RUN apt-get update && apt-get install -y \
    ca-certificates libpq5 libssl3 curl \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -r -s /bin/false migration
COPY --from=builder /app/target/release/ugc-subgraph /usr/local/bin/
USER migration
EXPOSE 4001
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1
CMD ["ugc-subgraph"]
```

#### Development Tools
**PlantUML элемент:**
```plantuml
Deployment_Node(dev_tools, "Development Tools", "Native Applications") {
    Container(migration_cli, "Migration CLI", "Rust Binary", "Command-line management")
    Container(config_editor, "Config Editor", "VS Code/Vim", "YAML configuration editing")
}
```

**CLI Tool реализация:**
```bash
#!/bin/bash
# scripts/dev-setup.sh - Development environment setup

echo "🚀 Setting up Migration Development Environment..."

# Check prerequisites
command -v docker >/dev/null 2>&1 || { echo "❌ Docker required"; exit 1; }
command -v docker-compose >/dev/null 2>&1 || { echo "❌ Docker Compose required"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "❌ Rust/Cargo required"; exit 1; }

# Build CLI tool
echo "🔨 Building Migration CLI..."
cargo build --bin migration-cli --release

# Create symlink for easy access
ln -sf $(pwd)/target/release/migration-cli /usr/local/bin/migration-cli

# Start development environment
echo "🐳 Starting Docker services..."
docker-compose up -d

# Wait for services to be ready
echo "⏳ Waiting for services to start..."
sleep 10

# Check service health
echo "🏥 Checking service health..."
curl -f http://localhost:4001/health || echo "⚠️ Migration service not ready"
curl -f http://localhost:6379/ping || echo "⚠️ Redis not ready"

# Initialize feature flags
echo "🏁 Initializing feature flags..."
migration-cli list

echo "✅ Development environment ready!"
echo "📊 Migration Service: http://localhost:4001"
echo "🔧 CLI Tool: migration-cli --help"
```

**VS Code Configuration:**
```json
// .vscode/settings.json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "files.associations": {
    "feature-flags.yaml": "yaml",
    "*.puml": "plantuml"
  },
  "yaml.schemas": {
    "./schemas/feature-flags.schema.json": "feature-flags.yaml"
  },
  "tasks.version": "2.0.0",
  "tasks.tasks": [
    {
      "label": "Start Migration Dev Environment",
      "type": "shell",
      "command": "./scripts/dev-setup.sh",
      "group": "build"
    },
    {
      "label": "Migration CLI - List Flags",
      "type": "shell",
      "command": "migration-cli list",
      "group": "test"
    }
  ]
}
```

### AWS Staging Environment

#### EKS Cluster Configuration
**PlantUML элемент:**
```plantuml
Deployment_Node(staging_nodes, "EKS Worker Nodes", "EC2 t3.medium") {
    Container(staging_migration, "Migration Service", "Kubernetes Pod", "Replicas: 2")
    Container(staging_ugc, "UGC GraphQL Service", "Kubernetes Pod", "Replicas: 2")
}
```

**Kubernetes Deployment реализация:**
```yaml
# k8s/staging/migration-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: migration-service
  namespace: staging
  labels:
    app: migration-service
    environment: staging
spec:
  replicas: 2
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 1
  
  selector:
    matchLabels:
      app: migration-service
  
  template:
    metadata:
      labels:
        app: migration-service
        version: v1
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    
    spec:
      serviceAccountName: migration-service-account
      
      # Security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      
      containers:
      - name: migration-service
        image: auto-ru/migration-service:staging-latest
        imagePullPolicy: Always
        
        ports:
        - name: http
          containerPort: 4001
          protocol: TCP
        - name: metrics
          containerPort: 9090
          protocol: TCP
        
        # Resource allocation для staging
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        
        # Environment configuration
        env:
        - name: RUST_LOG
          value: "info"
        - name: ENVIRONMENT
          value: "staging"
        - name: MIGRATION_CONFIG_PATH
          value: "/app/config/feature-flags.yaml"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: migration-secrets
              key: redis-url
        - name: UGC_GRAPHQL_ENDPOINT
          value: "http://ugc-service:4002/graphql"
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /ready
            port: http
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        
        # Configuration volume
        volumeMounts:
        - name: config-volume
          mountPath: /app/config
          readOnly: true
        - name: tmp
          mountPath: /tmp
      
      volumes:
      - name: config-volume
        configMap:
          name: migration-config
      - name: tmp
        emptyDir: {}

---
apiVersion: v1
kind: Service
metadata:
  name: migration-service
  namespace: staging
  labels:
    app: migration-service
spec:
  type: ClusterIP
  ports:
  - name: http
    port: 4001
    targetPort: http
  - name: metrics
    port: 9090
    targetPort: metrics
  selector:
    app: migration-service

---
apiVersion: v1
kind: ConfigMap
metadata:
  name: migration-config
  namespace: staging
data:
  feature-flags.yaml: |
    feature_flags:
      graphql_reviews_read:
        enabled: true
        rollout_percentage: 10.0
        description: "Enable GraphQL for reading reviews - staging"
        user_whitelist: []
        user_blacklist: []
        conditions: []
    
    canary_deployments:
      graphql_reviews_read:
        initial_percentage: 1.0
        promotion_steps: [1, 5, 10, 25, 50]
        step_duration_minutes: 30
        success_criteria:
          max_error_rate: 0.05
          max_response_time_p95: 500
```

#### AWS Managed Services
**PlantUML элемент:**
```plantuml
Deployment_Node(staging_data, "Staging Data Layer", "AWS Managed Services") {
    ContainerDb(staging_redis, "ElastiCache Redis", "AWS ElastiCache", "cache.t3.micro")
    ContainerDb(staging_config, "Config Storage", "AWS S3", "Configuration files")
}
```

**Terraform Infrastructure реализация:**
```hcl
# terraform/staging/elasticache.tf
resource "aws_elasticache_replication_group" "staging_migration_redis" {
  replication_group_id       = "migration-staging-redis"
  description                = "Redis cluster for migration feature flags - staging"
  
  # Node configuration
  node_type                  = "cache.t3.micro"
  port                       = 6379
  parameter_group_name       = "default.redis7"
  
  # Cluster configuration
  num_cache_clusters         = 2
  automatic_failover_enabled = true
  multi_az_enabled          = false  # staging optimization
  
  # Network configuration
  subnet_group_name = aws_elasticache_subnet_group.staging.name
  security_group_ids = [aws_security_group.elasticache_staging.id]
  
  # Backup configuration
  snapshot_retention_limit = 1  # minimal for staging
  snapshot_window         = "03:00-05:00"
  
  # Maintenance
  maintenance_window = "sun:05:00-sun:07:00"
  
  # Encryption
  at_rest_encryption_enabled = true
  transit_encryption_enabled = false  # staging simplification
  
  tags = {
    Name        = "migration-staging-redis"
    Environment = "staging"
    Service     = "migration"
  }
}

# S3 bucket for configuration
resource "aws_s3_bucket" "staging_migration_config" {
  bucket = "auto-ru-migration-config-staging"
  
  tags = {
    Name        = "migration-config-staging"
    Environment = "staging"
    Service     = "migration"
  }
}

resource "aws_s3_bucket_versioning" "staging_config_versioning" {
  bucket = aws_s3_bucket.staging_migration_config.id
  versioning_configuration {
    status = "Enabled"
  }
}

# Upload feature flags configuration
resource "aws_s3_object" "staging_feature_flags" {
  bucket = aws_s3_bucket.staging_migration_config.id
  key    = "feature-flags.yaml"
  source = "${path.module}/../../ugc-subgraph/feature-flags.yaml"
  etag   = filemd5("${path.module}/../../ugc-subgraph/feature-flags.yaml")
  
  tags = {
    Environment = "staging"
    ConfigType  = "feature-flags"
  }
}
```

### AWS Production Environment

#### Production EKS Configuration
**PlantUML элемент:**
```plantuml
Deployment_Node(prod_nodes, "EKS Worker Nodes", "EC2 c5.large") {
    Container(prod_migration, "Migration Service", "Kubernetes Pod", "Replicas: 3")
    Container(prod_ugc, "UGC GraphQL Service", "Kubernetes Pod", "Replicas: 5")
}
```

**Production Kubernetes реализация:**
```yaml
# k8s/production/migration-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: migration-service
  namespace: production
  labels:
    app: migration-service
    environment: production
spec:
  replicas: 3  # Higher availability for production
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 2  # Faster rollouts
  
  selector:
    matchLabels:
      app: migration-service
  
  template:
    metadata:
      labels:
        app: migration-service
        version: v1
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
    
    spec:
      serviceAccountName: migration-service-account
      
      # Multi-AZ distribution
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchExpressions:
              - key: app
                operator: In
                values:
                - migration-service
            topologyKey: topology.kubernetes.io/zone
      
      containers:
      - name: migration-service
        image: auto-ru/migration-service:v1.0.0  # Pinned version for production
        imagePullPolicy: IfNotPresent
        
        ports:
        - name: http
          containerPort: 4001
        - name: metrics
          containerPort: 9090
        
        # Production resource allocation
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        
        # Production environment
        env:
        - name: RUST_LOG
          value: "warn"  # Reduced logging for production
        - name: ENVIRONMENT
          value: "production"
        - name: MIGRATION_CONFIG_PATH
          value: "/app/config/feature-flags.yaml"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: migration-secrets
              key: redis-url
        
        # Enhanced health checks for production
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 10
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /ready
            port: http
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        startupProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 60

---
# Horizontal Pod Autoscaler for production
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: migration-service-hpa
  namespace: production
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: migration-service
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

#### Production Monitoring Stack
**PlantUML элемент:**
```plantuml
Deployment_Node(prod_monitoring, "Production Monitoring", "EKS Monitoring Stack") {
    Container(prod_prometheus, "Prometheus", "Kubernetes Pod", "HA setup")
    Container(prod_grafana, "Grafana", "Kubernetes Pod", "Production dashboards")
    Container(prod_alertmanager, "AlertManager", "Kubernetes Pod", "Migration alerts")
}
```

**Monitoring Configuration:**
```yaml
# k8s/monitoring/prometheus-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: monitoring
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    
    rule_files:
      - "/etc/prometheus/rules/*.yml"
    
    scrape_configs:
    - job_name: 'migration-service'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - production
          - staging
      
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
      - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2
        target_label: __address__

  migration-alerts.yml: |
    groups:
    - name: migration.rules
      rules:
      - alert: MigrationHighErrorRate
        expr: rate(migration_error_rate_total[5m]) > 0.1
        for: 2m
        labels:
          severity: critical
          service: migration
        annotations:
          summary: "High error rate in migration system"
          description: "Migration error rate is {{ $value }} errors/sec"
      
      - alert: MigrationPerformanceDegradation
        expr: histogram_quantile(0.95, rate(migration_request_duration_seconds_bucket[5m])) > 1.0
        for: 5m
        labels:
          severity: warning
          service: migration
        annotations:
          summary: "Migration performance degradation"
          description: "P95 response time is {{ $value }}s"
      
      - alert: FeatureFlagEvaluationFailure
        expr: rate(feature_flag_evaluation_errors_total[5m]) > 0.05
        for: 1m
        labels:
          severity: critical
          service: migration
        annotations:
          summary: "Feature flag evaluation failures"
          description: "Feature flag evaluation error rate: {{ $value }}"
```

### Deployment Automation

#### CI/CD Pipeline
**PlantUML связь:**
```plantuml
Rel(migration_cli, staging_migration, "Remote management", "HTTPS/API")
```

**GitHub Actions Deployment:**
```yaml
# .github/workflows/deploy-migration.yml
name: Deploy Migration Service

on:
  push:
    branches: [main, develop]
    paths:
      - 'ugc-subgraph/src/migration/**'
      - 'ugc-subgraph/feature-flags.yaml'

env:
  AWS_REGION: us-east-1

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    
    strategy:
      matrix:
        environment: [staging, production]
        exclude:
          - environment: production
            # Only deploy to production from main branch
            if: github.ref != 'refs/heads/main'
    
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
      
      - name: Configure AWS credentials
        uses: aws-actions/configure-aws-credentials@v4
        with:
          aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
          aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          aws-region: ${{ env.AWS_REGION }}
      
      - name: Login to Amazon ECR
        id: login-ecr
        uses: aws-actions/amazon-ecr-login@v2
      
      - name: Build and push Docker image
        env:
          ECR_REGISTRY: ${{ steps.login-ecr.outputs.registry }}
          ECR_REPOSITORY: auto-ru/migration-service
          IMAGE_TAG: ${{ matrix.environment }}-${{ github.sha }}
        run: |
          # Build image
          docker build \
            -f ugc-subgraph/Dockerfile \
            --target runtime \
            -t $ECR_REGISTRY/$ECR_REPOSITORY:$IMAGE_TAG \
            .
          
          # Push image
          docker push $ECR_REGISTRY/$ECR_REPOSITORY:$IMAGE_TAG
          
          # Tag as latest for environment
          docker tag $ECR_REGISTRY/$ECR_REPOSITORY:$IMAGE_TAG \
            $ECR_REGISTRY/$ECR_REPOSITORY:${{ matrix.environment }}-latest
          docker push $ECR_REGISTRY/$ECR_REPOSITORY:${{ matrix.environment }}-latest
      
      - name: Update kubeconfig
        run: |
          aws eks update-kubeconfig \
            --region ${{ env.AWS_REGION }} \
            --name auto-ru-${{ matrix.environment }}
      
      - name: Deploy to Kubernetes
        env:
          IMAGE_TAG: ${{ matrix.environment }}-${{ github.sha }}
        run: |
          # Update deployment image
          kubectl set image deployment/migration-service \
            migration-service=$ECR_REGISTRY/$ECR_REPOSITORY:$IMAGE_TAG \
            -n ${{ matrix.environment }}
          
          # Wait for rollout
          kubectl rollout status deployment/migration-service \
            -n ${{ matrix.environment }} \
            --timeout=300s
      
      - name: Run post-deployment tests
        run: |
          # Get service endpoint
          ENDPOINT=$(kubectl get service migration-service \
            -n ${{ matrix.environment }} \
            -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
          
          # Health check
          curl -f http://$ENDPOINT:4001/health
          
          # Feature flag API test
          curl -f http://$ENDPOINT:4001/api/migration/flags
      
      - name: Notify deployment status
        if: always()
        uses: 8398a7/action-slack@v3
        with:
          status: ${{ job.status }}
          channel: '#deployments'
          webhook_url: ${{ secrets.SLACK_WEBHOOK }}
          fields: repo,message,commit,author,action,eventName,ref,workflow
```

### Configuration Management

#### Environment-Specific Configuration
```bash
# scripts/deploy-config.sh - Configuration deployment script
#!/bin/bash

ENVIRONMENT=${1:-staging}
CONFIG_BUCKET="auto-ru-migration-config-${ENVIRONMENT}"

echo "🔧 Deploying configuration to ${ENVIRONMENT}..."

# Validate configuration
echo "✅ Validating feature-flags.yaml..."
cargo run --bin migration-cli -- validate-config ugc-subgraph/feature-flags.yaml

# Upload to S3
echo "📤 Uploading configuration to S3..."
aws s3 cp ugc-subgraph/feature-flags.yaml \
  s3://${CONFIG_BUCKET}/feature-flags.yaml \
  --metadata "environment=${ENVIRONMENT},version=$(git rev-parse HEAD)"

# Update Kubernetes ConfigMap
echo "🔄 Updating Kubernetes ConfigMap..."
kubectl create configmap migration-config \
  --from-file=feature-flags.yaml=ugc-subgraph/feature-flags.yaml \
  --namespace=${ENVIRONMENT} \
  --dry-run=client -o yaml | kubectl apply -f -

# Restart deployment to pick up new config
echo "🔄 Restarting deployment..."
kubectl rollout restart deployment/migration-service -n ${ENVIRONMENT}

# Wait for rollout
kubectl rollout status deployment/migration-service -n ${ENVIRONMENT}

echo "✅ Configuration deployment complete!"
```

### External Service Integration

#### PagerDuty Integration
**PlantUML связь:**
```plantuml
Rel(prod_alertmanager, pagerduty, "Critical alerts", "Webhook")
```

**AlertManager Configuration:**
```yaml
# k8s/monitoring/alertmanager-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alertmanager-config
  namespace: monitoring
data:
  alertmanager.yml: |
    global:
      pagerduty_url: 'https://events.pagerduty.com/v2/enqueue'
    
    route:
      group_by: ['alertname', 'service']
      group_wait: 10s
      group_interval: 10s
      repeat_interval: 1h
      receiver: 'web.hook'
      routes:
      - match:
          severity: critical
          service: migration
        receiver: 'pagerduty-critical'
      - match:
          severity: warning
          service: migration
        receiver: 'slack-warnings'
    
    receivers:
    - name: 'web.hook'
      webhook_configs:
      - url: 'http://localhost:5001/'
    
    - name: 'pagerduty-critical'
      pagerduty_configs:
      - service_key: '{{ .ExternalSecret "pagerduty-service-key" }}'
        description: 'Migration System Alert: {{ .GroupLabels.alertname }}'
        details:
          firing: '{{ .Alerts.Firing | len }}'
          resolved: '{{ .Alerts.Resolved | len }}'
          environment: '{{ .CommonLabels.environment }}'
    
    - name: 'slack-warnings'
      slack_configs:
      - api_url: '{{ .ExternalSecret "slack-webhook-url" }}'
        channel: '#migration-alerts'
        title: 'Migration Warning: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

### Заключение

Диаграмма развертывания Task 13 обеспечивает полную трассируемость между архитектурными deployment nodes и их физической реализацией:

1. **Local Development** → Docker Compose конфигурации и development tools
2. **AWS Staging** → EKS cluster с Terraform infrastructure
3. **AWS Production** → High-availability deployment с auto-scaling
4. **CI/CD Pipeline** → GitHub Actions workflows и deployment automation
5. **Monitoring Stack** → Prometheus, Grafana, AlertManager конфигурации
6. **External Integration** → PagerDuty, Slack, DataDog integrations

Каждый deployment node имеет конкретную реализацию в виде инфраструктурного кода, что обеспечивает полную согласованность между архитектурным дизайном и физической инфраструктурой.