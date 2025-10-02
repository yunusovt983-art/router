# Task 7: Deployment Diagram - Подробное объяснение production инфраструктуры отказоустойчивости

## 🎯 Цель диаграммы

Deployment диаграмма Task 7 демонстрирует **production-ready инфраструктуру отказоустойчивости в AWS**, показывая как система обработки ошибок и resilience паттерны развертываются в реальной облачной среде с multi-AZ архитектурой, chaos engineering и disaster recovery.

## 🏗️ AWS Cloud Resilience Platform - Облачная платформа отказоустойчивости

### Production VPC with Resilience - VPC с отказоустойчивостью

#### Multi-AZ архитектура для высокой доступности
```yaml
# infrastructure/terraform/vpc.tf
resource "aws_vpc" "resilience_vpc" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true
  
  tags = {
    Name        = "Auto.ru-Resilience-VPC"
    Environment = "production"
    Purpose     = "resilience-infrastructure"
  }
}

# Публичные подсети для Load Balancer
resource "aws_subnet" "public_resilience" {
  count = 3
  
  vpc_id                  = aws_vpc.resilience_vpc.id
  cidr_block              = "10.0.${count.index + 1}.0/24"
  availability_zone       = data.aws_availability_zones.available.names[count.index]
  map_public_ip_on_launch = true
  
  tags = {
    Name = "Public-Resilience-${count.index + 1}"
    Type = "public"
    Tier = "load-balancer"
  }
}

# Приватные подсети для приложений
resource "aws_subnet" "private_resilience" {
  count = 3
  
  vpc_id            = aws_vpc.resilience_vpc.id
  cidr_block        = "10.0.${count.index + 10}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]
  
  tags = {
    Name = "Private-Resilience-${count.index + 1}"
    Type = "private"
    Tier = "application"
  }
}

# Подсети для баз данных
resource "aws_subnet" "database_resilience" {
  count = 3
  
  vpc_id            = aws_vpc.resilience_vpc.id
  cidr_block        = "10.0.${count.index + 20}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]
  
  tags = {
    Name = "Database-Resilience-${count.index + 1}"
    Type = "private"
    Tier = "database"
  }
}
```

#### Application Load Balancer с health checks
```yaml
# infrastructure/terraform/alb.tf
resource "aws_lb" "resilience_alb" {
  name               = "auto-ru-resilience-alb"
  internal           = false
  load_balancer_type = "application"
  security_groups    = [aws_security_group.alb_sg.id]
  subnets            = aws_subnet.public_resilience[*].id
  
  enable_deletion_protection = true
  
  # Логирование доступа для анализа
  access_logs {
    bucket  = aws_s3_bucket.alb_logs.bucket
    prefix  = "resilience-alb"
    enabled = true
  }
  
  tags = {
    Name        = "Auto.ru-Resilience-ALB"
    Environment = "production"
  }
}

# Target Group с продвинутыми health checks
resource "aws_lb_target_group" "ugc_resilient" {
  name     = "ugc-resilient-tg"
  port     = 8080
  protocol = "HTTP"
  vpc_id   = aws_vpc.resilience_vpc.id
  
  # Продвинутые health checks для resilience
  health_check {
    enabled             = true
    healthy_threshold   = 2
    interval            = 15
    matcher             = "200"
    path                = "/health/ready"
    port                = "traffic-port"
    protocol            = "HTTP"
    timeout             = 5
    unhealthy_threshold = 3
  }
  
  # Sticky sessions для consistency
  stickiness {
    type            = "lb_cookie"
    cookie_duration = 86400
    enabled         = true
  }
  
  tags = {
    Name    = "UGC-Resilient-TG"
    Service = "ugc-subgraph"
  }
}

# Listener с SSL termination
resource "aws_lb_listener" "resilience_https" {
  load_balancer_arn = aws_lb.resilience_alb.arn
  port              = "443"
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-TLS-1-2-2017-01"
  certificate_arn   = aws_acm_certificate.resilience_cert.arn
  
  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.ugc_resilient.arn
  }
}
```

### EKS Resilience Clusters - Кластеры Kubernetes с отказоустойчивостью

#### EKS кластер с resilience операторами
```yaml
# infrastructure/terraform/eks.tf
resource "aws_eks_cluster" "resilience_cluster" {
  name     = "auto-ru-resilience"
  role_arn = aws_iam_role.eks_cluster_role.arn
  version  = "1.28"
  
  vpc_config {
    subnet_ids              = aws_subnet.private_resilience[*].id
    endpoint_private_access = true
    endpoint_public_access  = true
    public_access_cidrs     = ["0.0.0.0/0"]
    
    # Security groups для resilience
    security_group_ids = [
      aws_security_group.eks_cluster_sg.id,
      aws_security_group.resilience_sg.id
    ]
  }
  
  # Логирование для мониторинга
  enabled_cluster_log_types = [
    "api",
    "audit",
    "authenticator",
    "controllerManager",
    "scheduler"
  ]
  
  # Encryption at rest
  encryption_config {
    provider {
      key_arn = aws_kms_key.eks_encryption.arn
    }
    resources = ["secrets"]
  }
  
  tags = {
    Name        = "Auto.ru-Resilience-EKS"
    Environment = "production"
    Purpose     = "resilience-workloads"
  }
}

# Node groups с различными типами инстансов
resource "aws_eks_node_group" "resilience_nodes" {
  cluster_name    = aws_eks_cluster.resilience_cluster.name
  node_group_name = "resilience-nodes"
  node_role_arn   = aws_iam_role.eks_node_role.arn
  subnet_ids      = aws_subnet.private_resilience[*].id
  
  # Разнообразие типов инстансов для resilience
  instance_types = ["m5.large", "m5.xlarge", "c5.large"]
  ami_type       = "AL2_x86_64"
  capacity_type  = "ON_DEMAND"
  
  scaling_config {
    desired_size = 6
    max_size     = 12
    min_size     = 3
  }
  
  update_config {
    max_unavailable_percentage = 25
  }
  
  # Taints для специализированных workloads
  taint {
    key    = "resilience-workload"
    value  = "true"
    effect = "NO_SCHEDULE"
  }
  
  tags = {
    Name = "Resilience-Nodes"
    Type = "application"
  }
}
```

#### UGC Resilient Pods - Поды с отказоустойчивостью
```yaml
# k8s/ugc-resilient-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-resilient
  namespace: resilience
  labels:
    app: ugc-resilient
    version: v1.0.0
    tier: application
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: ugc-resilient
  template:
    metadata:
      labels:
        app: ugc-resilient
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      # Tolerations для resilience nodes
      tolerations:
      - key: "resilience-workload"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
      
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
                  - ugc-resilient
              topologyKey: topology.kubernetes.io/zone
      
      containers:
      - name: ugc-resilient
        image: auto-ru/ugc-resilient:v1.0.0
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 9090
          name: metrics
        
        # Environment variables для resilience
        env:
        - name: RUST_LOG
          value: "info,ugc_subgraph=debug"
        - name: CIRCUIT_BREAKER_FAILURE_THRESHOLD
          value: "5"
        - name: CIRCUIT_BREAKER_TIMEOUT_SECONDS
          value: "60"
        - name: RETRY_MAX_ATTEMPTS
          value: "3"
        - name: RETRY_INITIAL_DELAY_MS
          value: "100"
        - name: FALLBACK_CACHE_TTL_SECONDS
          value: "300"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-credentials
              key: url
        - name: REDIS_URL
          valueFrom:
            configMapKeyRef:
              name: redis-config
              key: url
        
        # Resource limits для stability
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        
        # Startup probe для медленного старта
        startupProbe:
          httpGet:
            path: /health/startup
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 30
        
        # Security context
        securityContext:
          allowPrivilegeEscalation: false
          runAsNonRoot: true
          runAsUser: 1000
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        
        # Volume mounts для temporary files
        volumeMounts:
        - name: tmp
          mountPath: /tmp
        - name: cache
          mountPath: /app/cache
      
      volumes:
      - name: tmp
        emptyDir: {}
      - name: cache
        emptyDir:
          sizeLimit: 1Gi
      
      # Service account для RBAC
      serviceAccountName: ugc-resilient-sa
      
      # DNS policy для service discovery
      dnsPolicy: ClusterFirst
      
      # Restart policy
      restartPolicy: Always
```

### Circuit Breaker Service - Централизованный Circuit Breaker

#### Deployment для Circuit Breaker сервиса
```yaml
# k8s/circuit-breaker-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: circuit-breaker-service
  namespace: resilience
spec:
  replicas: 2
  selector:
    matchLabels:
      app: circuit-breaker-service
  template:
    metadata:
      labels:
        app: circuit-breaker-service
    spec:
      containers:
      - name: circuit-breaker
        image: auto-ru/circuit-breaker:v1.0.0
        ports:
        - containerPort: 8081
          name: grpc
        - containerPort: 9091
          name: metrics
        
        env:
        - name: REDIS_CLUSTER_URLS
          value: "redis-cluster-0:6379,redis-cluster-1:6379,redis-cluster-2:6379"
        - name: CB_STATE_SYNC_INTERVAL_MS
          value: "1000"
        - name: CB_HEALTH_CHECK_INTERVAL_MS
          value: "5000"
        
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "200m"
        
        livenessProbe:
          grpc:
            port: 8081
          initialDelaySeconds: 15
          periodSeconds: 10
        
        readinessProbe:
          grpc:
            port: 8081
          initialDelaySeconds: 5
          periodSeconds: 5

---
apiVersion: v1
kind: Service
metadata:
  name: circuit-breaker-service
  namespace: resilience
spec:
  selector:
    app: circuit-breaker-service
  ports:
  - name: grpc
    port: 8081
    targetPort: 8081
  - name: metrics
    port: 9091
    targetPort: 9091
  type: ClusterIP
```

### Redis Resilience Infrastructure - Инфраструктура Redis

#### Redis Cluster для состояния Circuit Breaker
```yaml
# k8s/redis-cluster.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-cluster
  namespace: resilience
spec:
  serviceName: redis-cluster
  replicas: 6
  selector:
    matchLabels:
      app: redis-cluster
  template:
    metadata:
      labels:
        app: redis-cluster
    spec:
      containers:
      - name: redis
        image: redis:7.0-alpine
        ports:
        - containerPort: 6379
          name: client
        - containerPort: 16379
          name: gossip
        
        command:
        - redis-server
        - /etc/redis/redis.conf
        - --cluster-enabled
        - "yes"
        - --cluster-config-file
        - /data/nodes.conf
        - --cluster-node-timeout
        - "5000"
        - --appendonly
        - "yes"
        - --save
        - "900 1"
        - --save
        - "300 10"
        - --save
        - "60 10000"
        
        env:
        - name: POD_IP
          valueFrom:
            fieldRef:
              fieldPath: status.podIP
        
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "200m"
        
        volumeMounts:
        - name: data
          mountPath: /data
        - name: config
          mountPath: /etc/redis
        
        livenessProbe:
          exec:
            command:
            - redis-cli
            - ping
          initialDelaySeconds: 30
          periodSeconds: 10
        
        readinessProbe:
          exec:
            command:
            - redis-cli
            - ping
          initialDelaySeconds: 5
          periodSeconds: 5
      
      volumes:
      - name: config
        configMap:
          name: redis-config
  
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "gp3"
      resources:
        requests:
          storage: 10Gi
```

### Monitoring Infrastructure - Инфраструктура мониторинга

#### Prometheus для метрик отказоустойчивости
```yaml
# k8s/prometheus-resilience.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: prometheus-resilience
  namespace: monitoring
spec:
  replicas: 1
  selector:
    matchLabels:
      app: prometheus-resilience
  template:
    metadata:
      labels:
        app: prometheus-resilience
    spec:
      containers:
      - name: prometheus
        image: prom/prometheus:v2.45.0
        ports:
        - containerPort: 9090
        
        args:
        - --config.file=/etc/prometheus/prometheus.yml
        - --storage.tsdb.path=/prometheus/
        - --web.console.libraries=/etc/prometheus/console_libraries
        - --web.console.templates=/etc/prometheus/consoles
        - --storage.tsdb.retention.time=15d
        - --web.enable-lifecycle
        - --web.enable-admin-api
        
        volumeMounts:
        - name: config
          mountPath: /etc/prometheus
        - name: storage
          mountPath: /prometheus
        
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
      
      volumes:
      - name: config
        configMap:
          name: prometheus-resilience-config
      - name: storage
        persistentVolumeClaim:
          claimName: prometheus-resilience-storage

---
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-resilience-config
  namespace: monitoring
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
    
    rule_files:
    - "/etc/prometheus/rules/*.yml"
    
    scrape_configs:
    # UGC Resilient service metrics
    - job_name: 'ugc-resilient'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - resilience
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: ugc-resilient
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
    
    # Circuit Breaker service metrics
    - job_name: 'circuit-breaker'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - resilience
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: circuit-breaker-service
    
    # Redis cluster metrics
    - job_name: 'redis-cluster'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - resilience
      relabel_configs:
      - source_labels: [__meta_kubernetes_pod_label_app]
        action: keep
        regex: redis-cluster
      - source_labels: [__address__]
        action: replace
        regex: ([^:]+):.*
        replacement: $1:9121
        target_label: __address__
    
    alerting:
      alertmanagers:
      - kubernetes_sd_configs:
        - role: pod
          namespaces:
            names:
            - monitoring
        relabel_configs:
        - source_labels: [__meta_kubernetes_pod_label_app]
          action: keep
          regex: alertmanager
```

### Chaos Engineering Infrastructure - Инфраструктура Chaos Engineering

#### Chaos Monkey для тестирования отказоустойчивости
```yaml
# k8s/chaos-monkey.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: chaos-monkey
  namespace: chaos-engineering
spec:
  replicas: 1
  selector:
    matchLabels:
      app: chaos-monkey
  template:
    metadata:
      labels:
        app: chaos-monkey
    spec:
      serviceAccountName: chaos-monkey-sa
      containers:
      - name: chaos-monkey
        image: auto-ru/chaos-monkey:v1.0.0
        
        env:
        - name: CHAOS_SCHEDULE
          value: "0 */2 * * *" # Каждые 2 часа
        - name: TARGET_NAMESPACE
          value: "resilience"
        - name: FAILURE_PROBABILITY
          value: "0.1" # 10% вероятность сбоя
        - name: MAX_PODS_TO_KILL
          value: "1"
        - name: NETWORK_CHAOS_ENABLED
          value: "true"
        - name: RESOURCE_CHAOS_ENABLED
          value: "true"
        
        resources:
          requests:
            memory: "64Mi"
            cpu: "50m"
          limits:
            memory: "128Mi"
            cpu: "100m"
        
        securityContext:
          allowPrivilegeEscalation: false
          runAsNonRoot: true
          runAsUser: 1000

---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: chaos-monkey-sa
  namespace: chaos-engineering

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: chaos-monkey-role
rules:
- apiGroups: [""]
  resources: ["pods"]
  verbs: ["get", "list", "delete"]
- apiGroups: ["apps"]
  resources: ["deployments", "replicasets"]
  verbs: ["get", "list", "patch"]

---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: chaos-monkey-binding
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: chaos-monkey-role
subjects:
- kind: ServiceAccount
  name: chaos-monkey-sa
  namespace: chaos-engineering
```

### Disaster Recovery - Аварийное восстановление

#### Cross-Region Replication
```yaml
# infrastructure/terraform/disaster-recovery.tf
# DR регион (us-west-2)
provider "aws" {
  alias  = "dr"
  region = "us-west-2"
}

# DR VPC
resource "aws_vpc" "dr_vpc" {
  provider   = aws.dr
  cidr_block = "10.1.0.0/16"
  
  tags = {
    Name        = "Auto.ru-DR-VPC"
    Environment = "disaster-recovery"
  }
}

# EKS кластер для DR
resource "aws_eks_cluster" "dr_cluster" {
  provider = aws.dr
  name     = "auto-ru-dr"
  role_arn = aws_iam_role.dr_eks_role.arn
  
  vpc_config {
    subnet_ids = aws_subnet.dr_private[*].id
  }
  
  tags = {
    Name = "Auto.ru-DR-EKS"
    Type = "disaster-recovery"
  }
}

# Cross-region replication для Redis
resource "aws_elasticache_replication_group" "dr_redis" {
  provider = aws.dr
  
  replication_group_id         = "auto-ru-dr-redis"
  description                  = "DR Redis for Circuit Breaker state"
  port                         = 6379
  parameter_group_name         = "default.redis7"
  node_type                    = "cache.r6g.large"
  num_cache_clusters           = 3
  
  # Backup configuration
  snapshot_retention_limit = 7
  snapshot_window         = "03:00-05:00"
  
  # Cross-region backup
  global_replication_group_id = aws_elasticache_global_replication_group.redis_global.id
  
  tags = {
    Name = "Auto.ru-DR-Redis"
    Type = "disaster-recovery"
  }
}

# Global replication group
resource "aws_elasticache_global_replication_group" "redis_global" {
  global_replication_group_id_suffix = "auto-ru-global"
  description                        = "Global Redis for Circuit Breaker state"
  
  primary_replication_group_id = aws_elasticache_replication_group.main_redis.id
}
```

## 🚀 Практическое применение

### Полный deployment workflow
```bash
#!/bin/bash
# deploy-resilience.sh

set -e

echo "🚀 Deploying Auto.ru Resilience Infrastructure..."

# 1. Deploy Terraform infrastructure
echo "📦 Deploying AWS infrastructure..."
cd infrastructure/terraform
terraform init
terraform plan -var-file="production.tfvars"
terraform apply -auto-approve

# 2. Configure kubectl
echo "🔧 Configuring kubectl..."
aws eks update-kubeconfig --region us-east-1 --name auto-ru-resilience

# 3. Deploy Kubernetes resources
echo "☸️ Deploying Kubernetes resources..."
kubectl apply -f k8s/namespaces/
kubectl apply -f k8s/rbac/
kubectl apply -f k8s/configmaps/
kubectl apply -f k8s/secrets/
kubectl apply -f k8s/storage/

# 4. Deploy Redis cluster
echo "🔴 Deploying Redis cluster..."
kubectl apply -f k8s/redis-cluster.yaml
kubectl wait --for=condition=ready pod -l app=redis-cluster --timeout=300s

# 5. Initialize Redis cluster
echo "🔗 Initializing Redis cluster..."
kubectl exec -it redis-cluster-0 -- redis-cli --cluster create \
  $(kubectl get pods -l app=redis-cluster -o jsonpath='{range.items[*]}{.status.podIP}:6379 {end}') \
  --cluster-replicas 1 --cluster-yes

# 6. Deploy Circuit Breaker service
echo "⚡ Deploying Circuit Breaker service..."
kubectl apply -f k8s/circuit-breaker-deployment.yaml
kubectl wait --for=condition=available deployment/circuit-breaker-service --timeout=300s

# 7. Deploy UGC Resilient service
echo "🛡️ Deploying UGC Resilient service..."
kubectl apply -f k8s/ugc-resilient-deployment.yaml
kubectl wait --for=condition=available deployment/ugc-resilient --timeout=300s

# 8. Deploy monitoring stack
echo "📊 Deploying monitoring stack..."
kubectl apply -f k8s/prometheus-resilience.yaml
kubectl apply -f k8s/grafana-resilience.yaml
kubectl apply -f k8s/alertmanager-resilience.yaml

# 9. Deploy Chaos Engineering
echo "🐒 Deploying Chaos Engineering..."
kubectl apply -f k8s/chaos-monkey.yaml

# 10. Verify deployment
echo "✅ Verifying deployment..."
kubectl get pods -n resilience
kubectl get pods -n monitoring
kubectl get pods -n chaos-engineering

# 11. Run health checks
echo "🏥 Running health checks..."
kubectl exec deployment/ugc-resilient -- curl -f http://localhost:8080/health/ready
kubectl exec deployment/circuit-breaker-service -- grpc_health_probe -addr=localhost:8081

echo "🎉 Resilience infrastructure deployed successfully!"
echo "📊 Grafana: http://$(kubectl get svc grafana -n monitoring -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')"
echo "📈 Prometheus: http://$(kubectl get svc prometheus -n monitoring -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')"
```

### Мониторинг и алерты
```yaml
# k8s/prometheus-rules.yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: resilience-alerts
  namespace: monitoring
spec:
  groups:
  - name: circuit-breaker-alerts
    rules:
    - alert: CircuitBreakerOpen
      expr: circuit_breaker_state == 1
      for: 1m
      labels:
        severity: warning
        component: circuit-breaker
      annotations:
        summary: "Circuit breaker opened for {{ $labels.service }}"
        description: "Circuit breaker for service {{ $labels.service }} has been open for more than 1 minute"
        runbook_url: "https://runbooks.auto.ru/circuit-breaker-open"
    
    - alert: HighErrorRate
      expr: rate(ugc_errors_total[5m]) > 0.05
      for: 2m
      labels:
        severity: critical
        component: ugc-service
      annotations:
        summary: "High error rate detected"
        description: "Error rate is {{ $value | humanizePercentage }} over the last 5 minutes"
    
    - alert: FallbackUsageHigh
      expr: rate(ugc_fallback_activations_total[5m]) > 0.1
      for: 5m
      labels:
        severity: warning
        component: fallback-system
      annotations:
        summary: "High fallback usage detected"
        description: "Fallback mechanisms are being used frequently, indicating potential issues with external services"
```

Эта Deployment диаграмма демонстрирует полную production-ready инфраструктуру отказоустойчивости в AWS с multi-AZ архитектурой, comprehensive мониторингом, chaos engineering и disaster recovery capabilities для обеспечения максимальной надежности системы Auto.ru.