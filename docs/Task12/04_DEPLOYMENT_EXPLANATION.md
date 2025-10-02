# Task 12: Deployment Level Architecture Explanation
## Настройка среды разработки и деплоя - Диаграмма развертывания

### Обзор архитектуры развертывания

Диаграмма развертывания Task 12 показывает физическое размещение компонентов системы в различных окружениях: от локальной машины разработчика до production инфраструктуры в AWS. Архитектура обеспечивает консистентность между окружениями и надежный процесс доставки кода.

### Developer Machine Environment

#### Локальная машина разработчика

**Операционная система:** Linux/macOS/Windows
**Назначение:** Локальная разработка и тестирование

##### Docker Desktop Runtime

**Технология:** Docker Desktop / Docker Engine
**Ресурсы:** 
- CPU: 4+ cores
- RAM: 8+ GB
- Storage: 50+ GB SSD

**Контейнерная инфраструктура:**
```yaml
# Resource allocation for local development
services:
  ugc-subgraph:
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.25'
          memory: 128M

  apollo-router:
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
        reservations:
          cpus: '0.1'
          memory: 64M

  ugc-postgres:
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 512M
        reservations:
          cpus: '0.1'
          memory: 128M
```

**Сетевая конфигурация:**
```yaml
networks:
  federation-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
          gateway: 172.20.0.1

# Port mapping
ports:
  - "4000:4000"  # Apollo Router
  - "4001:4001"  # UGC Subgraph
  - "4002:4002"  # Users Subgraph
  - "4004:4004"  # Offers Subgraph
  - "5432:5432"  # PostgreSQL
  - "6379:6379"  # Redis
```

##### Development Tools

**IDE/Editor Configuration:**
```json
// .vscode/settings.json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "docker.defaultRegistryPath": "auto-ru",
  "files.watcherExclude": {
    "**/target/**": true
  }
}
```

**Git Configuration:**
```bash
# .gitconfig для проекта
[core]
    autocrlf = input
[push]
    default = current
[pull]
    rebase = true
[alias]
    co = checkout
    br = branch
    ci = commit
    st = status
```

**Make Tool Integration:**
```makefile
# Локальные команды разработки
dev-setup: ## Initial development setup
	@echo "Setting up development environment..."
	docker --version || (echo "Docker not installed" && exit 1)
	docker-compose --version || (echo "Docker Compose not installed" && exit 1)
	cargo --version || (echo "Rust not installed" && exit 1)
	@echo "✅ Development environment ready"

dev-reset: ## Reset development environment
	docker-compose down -v
	docker system prune -f
	docker-compose up -d
```

### GitHub Cloud Infrastructure

#### GitHub Actions Runtime Environment

**Platform:** GitHub-hosted runners
**Specifications:**
- **OS:** Ubuntu 22.04 LTS
- **CPU:** 2-core x86_64
- **RAM:** 7 GB
- **Storage:** 14 GB SSD
- **Network:** High-speed internet

##### CI Runner Configuration

**Workflow Execution Environment:**
```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    
    # Service containers для тестирования
    services:
      postgres:
        image: postgres:14
        env:
          POSTGRES_PASSWORD: test_password
          POSTGRES_USER: test_user
          POSTGRES_DB: test_db
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379

    # Execution steps
    steps:
      - name: Setup Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-
```

##### Build Runner Configuration

**Multi-service Build Matrix:**
```yaml
build:
  runs-on: ubuntu-latest
  strategy:
    matrix:
      service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
      platform: [linux/amd64, linux/arm64]
  
  steps:
    - name: Set up Docker Buildx
      uses: docker/setup-buildx-action@v3
      with:
        platforms: linux/amd64,linux/arm64
    
    - name: Build multi-arch image
      uses: docker/build-push-action@v5
      with:
        context: .
        file: ./${{ matrix.service }}/Dockerfile
        platforms: ${{ matrix.platform }}
        push: false
        tags: ${{ matrix.service }}:${{ github.sha }}
        cache-from: type=gha
        cache-to: type=gha,mode=max
```

##### Deploy Runner Configuration

**AWS Integration:**
```yaml
deploy:
  runs-on: ubuntu-latest
  environment: staging
  
  steps:
    - name: Configure AWS credentials
      uses: aws-actions/configure-aws-credentials@v4
      with:
        aws-access-key-id: ${{ secrets.AWS_ACCESS_KEY_ID }}
        aws-secret-access-key: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
        aws-region: us-east-1
        role-to-assume: arn:aws:iam::123456789012:role/GitHubActionsRole
        role-session-name: GitHubActions-Deploy
    
    - name: Deploy to EKS
      run: |
        aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
        kubectl apply -f k8s/staging/
        kubectl rollout status deployment/ugc-subgraph -n staging --timeout=300s
```

#### GitHub Container Registry

**Registry Configuration:**
```yaml
# Package configuration
name: auto-ru-federation
visibility: private
registry: ghcr.io

# Image naming convention
images:
  - ghcr.io/auto-ru/ugc-subgraph:latest
  - ghcr.io/auto-ru/ugc-subgraph:v1.0.0
  - ghcr.io/auto-ru/ugc-subgraph:sha-abc123
  - ghcr.io/auto-ru/users-subgraph:latest
  - ghcr.io/auto-ru/offers-subgraph:latest
  - ghcr.io/auto-ru/apollo-router:latest
```

**Registry Security:**
```yaml
# .github/workflows/build.yml
- name: Login to GitHub Container Registry
  uses: docker/login-action@v3
  with:
    registry: ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets.GITHUB_TOKEN }}

- name: Push with multiple tags
  run: |
    docker tag $SERVICE:$GITHUB_SHA ghcr.io/auto-ru/$SERVICE:latest
    docker tag $SERVICE:$GITHUB_SHA ghcr.io/auto-ru/$SERVICE:$GITHUB_SHA
    docker tag $SERVICE:$GITHUB_SHA ghcr.io/auto-ru/$SERVICE:v$VERSION
    
    docker push ghcr.io/auto-ru/$SERVICE:latest
    docker push ghcr.io/auto-ru/$SERVICE:$GITHUB_SHA
    docker push ghcr.io/auto-ru/$SERVICE:v$VERSION
```

### AWS Cloud Infrastructure

#### Staging Environment (AWS EKS)

**Cluster Configuration:**
```yaml
# EKS Cluster Specification
apiVersion: eksctl.io/v1alpha5
kind: ClusterConfig

metadata:
  name: auto-ru-staging
  region: us-east-1
  version: "1.28"

nodeGroups:
  - name: staging-workers
    instanceType: t3.medium
    desiredCapacity: 3
    minSize: 2
    maxSize: 5
    
    # Security and networking
    privateNetworking: true
    securityGroups:
      attachIDs: ["sg-staging-workers"]
    
    # Storage
    volumeSize: 50
    volumeType: gp3
    
    # Auto Scaling
    asgSuspendProcesses:
      - AZRebalance
    
    # Labels and taints
    labels:
      environment: staging
      workload-type: web-services
```

##### EKS Worker Nodes

**Node Specifications:**
- **Instance Type:** EC2 t3.medium
- **CPU:** 2 vCPUs
- **RAM:** 4 GB
- **Storage:** 50 GB gp3 EBS
- **Network:** Enhanced networking enabled

**Kubernetes Deployment Configuration:**
```yaml
# k8s/staging/ugc-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: staging
  labels:
    app: ugc-subgraph
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
      app: ugc-subgraph
  
  template:
    metadata:
      labels:
        app: ugc-subgraph
        version: v1
    spec:
      serviceAccountName: ugc-service-account
      
      containers:
      - name: ugc-subgraph
        image: ghcr.io/auto-ru/ugc-subgraph:latest
        ports:
        - containerPort: 4001
          name: http
        
        # Resource limits
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        
        # Environment configuration
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: ugc-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: ugc-secrets
              key: redis-url
        - name: RUST_LOG
          value: "info"
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health
            port: 4001
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /ready
            port: 4001
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
      
      # Pod security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
```

##### Staging Data Layer

**PostgreSQL RDS Configuration:**
```yaml
# Terraform configuration for RDS
resource "aws_db_instance" "staging_postgres" {
  identifier = "auto-ru-staging-postgres"
  
  # Instance configuration
  engine         = "postgres"
  engine_version = "14.9"
  instance_class = "db.t3.micro"
  
  # Storage
  allocated_storage     = 20
  max_allocated_storage = 100
  storage_type         = "gp3"
  storage_encrypted    = true
  
  # Database configuration
  db_name  = "ugc_db"
  username = "ugc_user"
  password = var.db_password
  
  # Network and security
  vpc_security_group_ids = [aws_security_group.rds_staging.id]
  db_subnet_group_name   = aws_db_subnet_group.staging.name
  
  # Backup and maintenance
  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"
  
  # Monitoring
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_monitoring.arn
  
  # Performance Insights
  performance_insights_enabled = true
  
  tags = {
    Environment = "staging"
    Service     = "auto-ru-federation"
  }
}
```

**Redis ElastiCache Configuration:**
```yaml
resource "aws_elasticache_replication_group" "staging_redis" {
  replication_group_id       = "auto-ru-staging-redis"
  description                = "Redis cluster for staging environment"
  
  # Node configuration
  node_type                  = "cache.t3.micro"
  port                       = 6379
  parameter_group_name       = "default.redis7"
  
  # Cluster configuration
  num_cache_clusters         = 2
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  # Network and security
  subnet_group_name = aws_elasticache_subnet_group.staging.name
  security_group_ids = [aws_security_group.elasticache_staging.id]
  
  # Backup
  snapshot_retention_limit = 3
  snapshot_window         = "03:00-05:00"
  
  # Maintenance
  maintenance_window = "sun:05:00-sun:07:00"
  
  # Encryption
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  
  tags = {
    Environment = "staging"
    Service     = "auto-ru-federation"
  }
}
```

#### Production Environment (AWS EKS)

**Cluster Configuration:**
```yaml
# Production EKS cluster
metadata:
  name: auto-ru-production
  region: us-east-1
  version: "1.28"

nodeGroups:
  - name: production-workers
    instanceType: c5.large
    desiredCapacity: 6
    minSize: 3
    maxSize: 12
    
    # Multi-AZ deployment
    availabilityZones: ["us-east-1a", "us-east-1b", "us-east-1c"]
    
    # Enhanced security
    privateNetworking: true
    securityGroups:
      attachIDs: ["sg-production-workers"]
    
    # Storage optimization
    volumeSize: 100
    volumeType: gp3
    volumeIOPS: 3000
    volumeThroughput: 125
    
    # Auto Scaling configuration
    asgSuspendProcesses:
      - AZRebalance
    
    # Spot instances for cost optimization
    spot: true
    instancesDistribution:
      maxPrice: 0.10
      instanceTypes: ["c5.large", "c5.xlarge", "m5.large"]
      onDemandBaseCapacity: 2
      onDemandPercentageAboveBaseCapacity: 50
```

##### Production Worker Nodes

**Node Specifications:**
- **Instance Type:** EC2 c5.large
- **CPU:** 2 vCPUs (Intel Xeon Platinum)
- **RAM:** 4 GB
- **Storage:** 100 GB gp3 EBS (3000 IOPS)
- **Network:** Up to 10 Gbps

**High Availability Deployment:**
```yaml
# k8s/production/ugc-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: production
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 2
  
  template:
    spec:
      # Anti-affinity для распределения по AZ
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchExpressions:
                - key: app
                  operator: In
                  values:
                  - ugc-subgraph
              topologyKey: topology.kubernetes.io/zone
      
      containers:
      - name: ugc-subgraph
        image: ghcr.io/auto-ru/ugc-subgraph:v1.0.0
        
        # Production resource allocation
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        
        # Production environment variables
        env:
        - name: RUST_LOG
          value: "warn"
        - name: RUST_BACKTRACE
          value: "0"
        - name: ENVIRONMENT
          value: "production"
```

##### Production Data Layer

**PostgreSQL RDS (Multi-AZ):**
```yaml
resource "aws_db_instance" "production_postgres" {
  identifier = "auto-ru-production-postgres"
  
  # High-performance instance
  engine         = "postgres"
  engine_version = "14.9"
  instance_class = "db.r5.large"
  
  # High-availability storage
  allocated_storage     = 500
  max_allocated_storage = 2000
  storage_type         = "gp3"
  storage_encrypted    = true
  iops                 = 3000
  
  # Multi-AZ for high availability
  multi_az = true
  
  # Enhanced monitoring
  monitoring_interval = 15
  monitoring_role_arn = aws_iam_role.rds_monitoring.arn
  
  # Performance Insights
  performance_insights_enabled          = true
  performance_insights_retention_period = 7
  
  # Backup configuration
  backup_retention_period = 30
  backup_window          = "03:00-04:00"
  copy_tags_to_snapshot  = true
  
  # Maintenance window
  maintenance_window = "sun:04:00-sun:06:00"
  auto_minor_version_upgrade = false
  
  # Security
  deletion_protection = true
  
  tags = {
    Environment = "production"
    Service     = "auto-ru-federation"
    Backup      = "required"
  }
}
```

**Redis ElastiCache (Cluster Mode):**
```yaml
resource "aws_elasticache_replication_group" "production_redis" {
  replication_group_id       = "auto-ru-production-redis"
  description                = "Redis cluster for production environment"
  
  # High-performance nodes
  node_type                  = "cache.r5.large"
  port                       = 6379
  parameter_group_name       = "default.redis7.cluster.on"
  
  # Cluster mode enabled
  num_node_groups         = 3
  replicas_per_node_group = 2
  
  # High availability
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  # Network and security
  subnet_group_name = aws_elasticache_subnet_group.production.name
  security_group_ids = [aws_security_group.elasticache_production.id]
  
  # Backup configuration
  snapshot_retention_limit = 7
  snapshot_window         = "03:00-05:00"
  final_snapshot_identifier = "auto-ru-production-redis-final-snapshot"
  
  # Maintenance
  maintenance_window = "sun:05:00-sun:07:00"
  
  # Encryption
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token                = var.redis_auth_token
  
  # Notifications
  notification_topic_arn = aws_sns_topic.elasticache_notifications.arn
  
  tags = {
    Environment = "production"
    Service     = "auto-ru-federation"
    Backup      = "required"
  }
}
```

#### Monitoring Infrastructure

**Prometheus Configuration:**
```yaml
# k8s/monitoring/prometheus-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: prometheus
  namespace: monitoring
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: prometheus
        image: prom/prometheus:v2.45.0
        ports:
        - containerPort: 9090
        
        # Resource allocation
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        
        # Configuration
        volumeMounts:
        - name: prometheus-config
          mountPath: /etc/prometheus
        - name: prometheus-storage
          mountPath: /prometheus
        
        # Command line arguments
        args:
        - '--config.file=/etc/prometheus/prometheus.yml'
        - '--storage.tsdb.path=/prometheus'
        - '--storage.tsdb.retention.time=30d'
        - '--web.console.libraries=/etc/prometheus/console_libraries'
        - '--web.console.templates=/etc/prometheus/consoles'
        - '--web.enable-lifecycle'
        - '--web.enable-admin-api'
      
      volumes:
      - name: prometheus-config
        configMap:
          name: prometheus-config
      - name: prometheus-storage
        persistentVolumeClaim:
          claimName: prometheus-storage
```

**Grafana Configuration:**
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: grafana
  namespace: monitoring
spec:
  replicas: 1
  template:
    spec:
      containers:
      - name: grafana
        image: grafana/grafana:10.0.0
        ports:
        - containerPort: 3000
        
        # Environment configuration
        env:
        - name: GF_SECURITY_ADMIN_PASSWORD
          valueFrom:
            secretKeyRef:
              name: grafana-secrets
              key: admin-password
        - name: GF_INSTALL_PLUGINS
          value: "grafana-piechart-panel,grafana-worldmap-panel"
        
        # Resource allocation
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        
        # Persistent storage
        volumeMounts:
        - name: grafana-storage
          mountPath: /var/lib/grafana
```

**Jaeger Configuration:**
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: jaeger
  namespace: monitoring
spec:
  replicas: 1
  template:
    spec:
      containers:
      - name: jaeger-all-in-one
        image: jaegertracing/all-in-one:1.47
        ports:
        - containerPort: 16686  # UI
        - containerPort: 14268  # HTTP collector
        - containerPort: 14250  # gRPC collector
        
        # Environment configuration
        env:
        - name: COLLECTOR_OTLP_ENABLED
          value: "true"
        - name: SPAN_STORAGE_TYPE
          value: "elasticsearch"
        - name: ES_SERVER_URLS
          value: "http://elasticsearch:9200"
        
        # Resource allocation
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
```

### Network Architecture

#### Service Mesh (Istio)

**Istio Configuration:**
```yaml
# istio/gateway.yaml
apiVersion: networking.istio.io/v1beta1
kind: Gateway
metadata:
  name: auto-ru-gateway
  namespace: production
spec:
  selector:
    istio: ingressgateway
  servers:
  - port:
      number: 80
      name: http
      protocol: HTTP
    hosts:
    - api.auto.ru
  - port:
      number: 443
      name: https
      protocol: HTTPS
    tls:
      mode: SIMPLE
      credentialName: auto-ru-tls
    hosts:
    - api.auto.ru

---
apiVersion: networking.istio.io/v1beta1
kind: VirtualService
metadata:
  name: auto-ru-routing
  namespace: production
spec:
  hosts:
  - api.auto.ru
  gateways:
  - auto-ru-gateway
  http:
  - match:
    - uri:
        prefix: /graphql
    route:
    - destination:
        host: apollo-router
        port:
          number: 4000
    timeout: 30s
    retries:
      attempts: 3
      perTryTimeout: 10s
```

#### Load Balancing

**Application Load Balancer:**
```yaml
# AWS ALB configuration
resource "aws_lb" "production_alb" {
  name               = "auto-ru-production-alb"
  internal           = false
  load_balancer_type = "application"
  security_groups    = [aws_security_group.alb_production.id]
  subnets           = aws_subnet.public[*].id

  # Access logs
  access_logs {
    bucket  = aws_s3_bucket.alb_logs.bucket
    prefix  = "production-alb"
    enabled = true
  }

  # Security
  drop_invalid_header_fields = true
  
  tags = {
    Environment = "production"
    Service     = "auto-ru-federation"
  }
}

resource "aws_lb_target_group" "apollo_router" {
  name     = "apollo-router-production"
  port     = 4000
  protocol = "HTTP"
  vpc_id   = aws_vpc.main.id
  
  # Health check configuration
  health_check {
    enabled             = true
    healthy_threshold   = 2
    interval            = 30
    matcher             = "200"
    path                = "/health"
    port                = "traffic-port"
    protocol            = "HTTP"
    timeout             = 5
    unhealthy_threshold = 2
  }
  
  # Stickiness
  stickiness {
    type            = "lb_cookie"
    cookie_duration = 86400
    enabled         = false
  }
}
```

### Security Architecture

#### Network Security

**Security Groups:**
```yaml
# Security group for EKS worker nodes
resource "aws_security_group" "eks_workers" {
  name_prefix = "auto-ru-eks-workers"
  vpc_id      = aws_vpc.main.id

  # Ingress rules
  ingress {
    description = "HTTPS from ALB"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    security_groups = [aws_security_group.alb_production.id]
  }

  ingress {
    description = "HTTP from ALB"
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    security_groups = [aws_security_group.alb_production.id]
  }

  ingress {
    description = "Node to node communication"
    from_port   = 0
    to_port     = 65535
    protocol    = "tcp"
    self        = true
  }

  # Egress rules
  egress {
    description = "All outbound traffic"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "auto-ru-eks-workers"
    Environment = "production"
  }
}
```

#### IAM Roles and Policies

**EKS Service Account:**
```yaml
# k8s/rbac/service-account.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: ugc-service-account
  namespace: production
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/UGCServiceRole

---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  namespace: production
  name: ugc-role
rules:
- apiGroups: [""]
  resources: ["secrets", "configmaps"]
  verbs: ["get", "list"]
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list", "watch"]

---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: ugc-role-binding
  namespace: production
subjects:
- kind: ServiceAccount
  name: ugc-service-account
  namespace: production
roleRef:
  kind: Role
  name: ugc-role
  apiGroup: rbac.authorization.k8s.io
```

### Disaster Recovery

#### Backup Strategy

**Database Backups:**
```yaml
# RDS automated backups
resource "aws_db_instance" "production_postgres" {
  # ... other configuration ...
  
  # Backup configuration
  backup_retention_period = 30
  backup_window          = "03:00-04:00"
  copy_tags_to_snapshot  = true
  delete_automated_backups = false
  
  # Point-in-time recovery
  enabled_cloudwatch_logs_exports = ["postgresql"]
}

# Manual snapshot schedule
resource "aws_cloudwatch_event_rule" "db_snapshot" {
  name                = "auto-ru-db-snapshot"
  description         = "Trigger manual DB snapshot"
  schedule_expression = "cron(0 6 * * ? *)"  # Daily at 6 AM UTC
}
```

**Application State Backup:**
```yaml
# Velero backup configuration
apiVersion: velero.io/v1
kind: Schedule
metadata:
  name: auto-ru-daily-backup
  namespace: velero
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  template:
    includedNamespaces:
    - production
    - staging
    excludedResources:
    - events
    - events.events.k8s.io
    storageLocation: aws-s3-backup
    ttl: 720h0m0s  # 30 days retention
```

### Заключение

Архитектура развертывания Task 12 обеспечивает:

- **Scalability:** Горизонтальное масштабирование на всех уровнях
- **High Availability:** Multi-AZ deployment и автоматический failover
- **Security:** Многоуровневая защита с network isolation и RBAC
- **Monitoring:** Comprehensive observability с метриками, логами и трассировкой
- **Disaster Recovery:** Автоматические бэкапы и процедуры восстановления
- **Cost Optimization:** Spot instances и right-sizing ресурсов
- **Developer Experience:** Консистентность между локальной разработкой и production