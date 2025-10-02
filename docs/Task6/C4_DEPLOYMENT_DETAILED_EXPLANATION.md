# Task 6: Deployment Diagram - Подробное объяснение production инфраструктуры аутентификации

## 🎯 Цель диаграммы

Deployment диаграмма Task 6 демонстрирует **production-ready инфраструктуру системы аутентификации и авторизации** в AWS Cloud, показывая как компоненты безопасности развертываются, масштабируются и обеспечивают высокую доступность в реальной production среде.

## ☁️ AWS Cloud Authentication Platform - Облачная платформа аутентификации

### Production VPC with Security - Production VPC с безопасностью

#### Terraform конфигурация для VPC
```hcl
# infrastructure/terraform/vpc.tf
resource "aws_vpc" "auth_production_vpc" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name        = "auth-production-vpc"
    Environment = "production"
    Project     = "auto-ru-auth"
  }
}

# Public subnets для Load Balancer
resource "aws_subnet" "public_auth_subnets" {
  count             = 2
  vpc_id            = aws_vpc.auth_production_vpc.id
  cidr_block        = "10.0.${count.index + 1}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]
  
  map_public_ip_on_launch = true

  tags = {
    Name = "public-auth-subnet-${count.index + 1}"
    Type = "public"
  }
}

# Private subnets для приложений
resource "aws_subnet" "private_auth_subnets" {
  count             = 3
  vpc_id            = aws_vpc.auth_production_vpc.id
  cidr_block        = "10.0.${count.index + 10}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]

  tags = {
    Name = "private-auth-subnet-${count.index + 1}"
    Type = "private"
  }
}

# WAF для защиты ALB
resource "aws_wafv2_web_acl" "auth_waf" {
  name  = "auth-production-waf"
  scope = "REGIONAL"

  default_action {
    allow {}
  }

  # Защита от SQL injection
  rule {
    name     = "SQLInjectionRule"
    priority = 1

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesSQLiRuleSet"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "SQLInjectionRule"
      sampled_requests_enabled   = true
    }
  }

  # Rate limiting
  rule {
    name     = "RateLimitRule"
    priority = 2

    action {
      block {}
    }

    statement {
      rate_based_statement {
        limit              = 2000
        aggregate_key_type = "IP"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "RateLimitRule"
      sampled_requests_enabled   = true
    }
  }
}
```

### ALB with Auth - Application Load Balancer с аутентификацией

#### ALB конфигурация с JWT валидацией
```hcl
# infrastructure/terraform/alb.tf
resource "aws_lb" "auth_alb" {
  name               = "auth-production-alb"
  internal           = false
  load_balancer_type = "application"
  security_groups    = [aws_security_group.alb_sg.id]
  subnets            = aws_subnet.public_auth_subnets[*].id

  enable_deletion_protection = true
  enable_http2              = true

  tags = {
    Environment = "production"
    Project     = "auto-ru-auth"
  }
}

# Target Group для Apollo Gateway
resource "aws_lb_target_group" "apollo_gateway_tg" {
  name     = "apollo-gateway-tg"
  port     = 4000
  protocol = "HTTP"
  vpc_id   = aws_vpc.auth_production_vpc.id

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

  tags = {
    Name = "apollo-gateway-target-group"
  }
}

# HTTPS Listener с JWT валидацией
resource "aws_lb_listener" "auth_https_listener" {
  load_balancer_arn = aws_lb.auth_alb.arn
  port              = "443"
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-TLS-1-2-2017-01"
  certificate_arn   = aws_acm_certificate.auth_cert.arn

  default_action {
    type = "authenticate-oidc"
    
    authenticate_oidc {
      authorization_endpoint = "https://auth.auto.ru/oauth2/authorize"
      client_id             = var.oidc_client_id
      client_secret         = var.oidc_client_secret
      issuer                = "https://auth.auto.ru"
      token_endpoint        = "https://auth.auto.ru/oauth2/token"
      user_info_endpoint    = "https://auth.auto.ru/oauth2/userinfo"
    }
  }

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.apollo_gateway_tg.arn
  }
}
```

## 🚀 EKS Authentication Clusters - Кластеры аутентификации

### EKS Cluster конфигурация
```yaml
# kubernetes/cluster/eks-cluster.yaml
apiVersion: eksctl.io/v1alpha5
kind: ClusterConfig

metadata:
  name: auth-production-cluster
  region: us-east-1
  version: "1.28"

vpc:
  id: vpc-auth-production
  subnets:
    private:
      us-east-1a: { id: subnet-private-auth-1 }
      us-east-1b: { id: subnet-private-auth-2 }
      us-east-1c: { id: subnet-private-auth-3 }

iam:
  withOIDC: true
  serviceAccounts:
  - metadata:
      name: auth-service-account
      namespace: auth-system
    wellKnownPolicies:
      autoScaler: true
      awsLoadBalancerController: true
      certManager: true
    attachPolicyARNs:
    - arn:aws:iam::aws:policy/SecretsManagerReadWrite
    - arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy

nodeGroups:
- name: auth-workers
  instanceType: m5.xlarge
  desiredCapacity: 3
  minSize: 2
  maxSize: 10
  volumeSize: 100
  volumeType: gp3
  
  iam:
    attachPolicyARNs:
    - arn:aws:iam::aws:policy/AmazonEKSWorkerNodePolicy
    - arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy
    - arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly
    - arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy

  labels:
    workload-type: auth-services
  
  taints:
  - key: auth-dedicated
    value: "true"
    effect: NoSchedule

addons:
- name: vpc-cni
  version: latest
- name: coredns
  version: latest
- name: kube-proxy
  version: latest
- name: aws-ebs-csi-driver
  version: latest
```

### Apollo Gateway Deployment с аутентификацией
```yaml
# kubernetes/deployments/apollo-gateway-auth.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apollo-gateway-auth
  namespace: auth-system
  labels:
    app: apollo-gateway-auth
    version: v1.0.0
spec:
  replicas: 3
  selector:
    matchLabels:
      app: apollo-gateway-auth
  template:
    metadata:
      labels:
        app: apollo-gateway-auth
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: auth-service-account
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 2000
      containers:
      - name: apollo-gateway-auth
        image: auto-ru/apollo-gateway-auth:v1.0.0
        ports:
        - containerPort: 4000
          name: graphql
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        - name: JWT_ISSUER
          value: "https://auth.auto.ru"
        - name: JWT_AUDIENCE
          value: "auto.ru-api"
        - name: JWKS_URI
          value: "https://auth.auto.ru/.well-known/jwks.json"
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-auth-credentials
              key: url
        - name: AUTH_SERVICE_URL
          value: "http://auth-service:8080"
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 4000
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 4000
          initialDelaySeconds: 5
          periodSeconds: 5
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        volumeMounts:
        - name: tmp
          mountPath: /tmp
        - name: cache
          mountPath: /app/cache
      volumes:
      - name: tmp
        emptyDir: {}
      - name: cache
        emptyDir: {}
      tolerations:
      - key: auth-dedicated
        operator: Equal
        value: "true"
        effect: NoSchedule
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
                  - apollo-gateway-auth
              topologyKey: kubernetes.io/hostname
---
apiVersion: v1
kind: Service
metadata:
  name: apollo-gateway-auth-service
  namespace: auth-system
  labels:
    app: apollo-gateway-auth
spec:
  selector:
    app: apollo-gateway-auth
  ports:
  - name: graphql
    port: 4000
    targetPort: 4000
  - name: metrics
    port: 9090
    targetPort: 9090
  type: ClusterIP
```

### Authentication Service Deployment
```yaml
# kubernetes/deployments/auth-service.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: auth-service
  namespace: auth-system
spec:
  replicas: 2
  selector:
    matchLabels:
      app: auth-service
  template:
    metadata:
      labels:
        app: auth-service
    spec:
      serviceAccountName: auth-service-account
      containers:
      - name: auth-service
        image: auto-ru/auth-service:v1.0.0
        ports:
        - containerPort: 8080
          name: grpc
        - containerPort: 9090
          name: metrics
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: postgres-auth-credentials
              key: url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-auth-credentials
              key: url
        - name: JWT_PRIVATE_KEY
          valueFrom:
            secretKeyRef:
              name: jwt-keys
              key: private-key
        - name: JWT_PUBLIC_KEY
          valueFrom:
            secretKeyRef:
              name: jwt-keys
              key: public-key
        - name: OAUTH2_GOOGLE_CLIENT_ID
          valueFrom:
            secretKeyRef:
              name: oauth2-credentials
              key: google-client-id
        - name: OAUTH2_GOOGLE_CLIENT_SECRET
          valueFrom:
            secretKeyRef:
              name: oauth2-credentials
              key: google-client-secret
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "250m"
        livenessProbe:
          grpc:
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          grpc:
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: auth-service
  namespace: auth-system
spec:
  selector:
    app: auth-service
  ports:
  - name: grpc
    port: 8080
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
  type: ClusterIP
```

## 🗄️ Database Infrastructure - Инфраструктура баз данных

### RDS PostgreSQL с шифрованием
```hcl
# infrastructure/terraform/rds.tf
resource "aws_db_subnet_group" "auth_db_subnet_group" {
  name       = "auth-db-subnet-group"
  subnet_ids = aws_subnet.private_auth_subnets[*].id

  tags = {
    Name = "Auth DB subnet group"
  }
}

resource "aws_db_instance" "postgres_auth_primary" {
  identifier = "postgres-auth-primary"
  
  engine         = "postgres"
  engine_version = "15.4"
  instance_class = "db.r6g.xlarge"
  
  allocated_storage     = 100
  max_allocated_storage = 1000
  storage_type         = "gp3"
  storage_encrypted    = true
  kms_key_id          = aws_kms_key.auth_db_key.arn
  
  db_name  = "auth_production"
  username = "auth_admin"
  password = random_password.db_password.result
  
  vpc_security_group_ids = [aws_security_group.rds_sg.id]
  db_subnet_group_name   = aws_db_subnet_group.auth_db_subnet_group.name
  
  backup_retention_period = 30
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"
  
  skip_final_snapshot = false
  final_snapshot_identifier = "postgres-auth-primary-final-snapshot"
  
  # Включение Row Level Security
  parameter_group_name = aws_db_parameter_group.auth_pg.name
  
  # Мониторинг
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_monitoring_role.arn
  
  # Логирование
  enabled_cloudwatch_logs_exports = ["postgresql"]
  
  tags = {
    Name        = "postgres-auth-primary"
    Environment = "production"
  }
}

# Parameter Group для Row Level Security
resource "aws_db_parameter_group" "auth_pg" {
  family = "postgres15"
  name   = "auth-postgres-params"

  parameter {
    name  = "row_security"
    value = "on"
  }
  
  parameter {
    name  = "log_statement"
    value = "all"
  }
  
  parameter {
    name  = "log_min_duration_statement"
    value = "1000"
  }
}

# Read Replica для масштабирования чтения
resource "aws_db_instance" "postgres_auth_replica" {
  identifier = "postgres-auth-replica"
  
  replicate_source_db = aws_db_instance.postgres_auth_primary.id
  instance_class      = "db.r6g.large"
  
  publicly_accessible = false
  
  tags = {
    Name        = "postgres-auth-replica"
    Environment = "production"
  }
}
```

### ElastiCache Redis с TLS
```hcl
# infrastructure/terraform/elasticache.tf
resource "aws_elasticache_subnet_group" "auth_cache_subnet_group" {
  name       = "auth-cache-subnet-group"
  subnet_ids = aws_subnet.private_auth_subnets[*].id
}

resource "aws_elasticache_replication_group" "redis_auth_primary" {
  replication_group_id       = "redis-auth-primary"
  description                = "Redis cluster for authentication caching"
  
  node_type                  = "cache.r6g.large"
  port                       = 6379
  parameter_group_name       = aws_elasticache_parameter_group.auth_redis_params.name
  
  num_cache_clusters         = 3
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  subnet_group_name = aws_elasticache_subnet_group.auth_cache_subnet_group.name
  security_group_ids = [aws_security_group.redis_sg.id]
  
  # Шифрование
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token                = random_password.redis_auth_token.result
  
  # Backup
  snapshot_retention_limit = 7
  snapshot_window         = "03:00-05:00"
  
  # Логирование
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis_slow_log.name
    destination_type = "cloudwatch-logs"
    log_format      = "text"
    log_type        = "slow-log"
  }
  
  tags = {
    Name        = "redis-auth-primary"
    Environment = "production"
  }
}

resource "aws_elasticache_parameter_group" "auth_redis_params" {
  family = "redis7"
  name   = "auth-redis-params"

  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }
  
  parameter {
    name  = "timeout"
    value = "300"
  }
}
```

## 🔐 AWS Security Services Integration

### Secrets Manager для управления секретами
```hcl
# infrastructure/terraform/secrets.tf
resource "aws_secretsmanager_secret" "jwt_keys" {
  name                    = "auth/jwt-keys"
  description            = "JWT signing keys for authentication"
  recovery_window_in_days = 30
  
  kms_key_id = aws_kms_key.auth_secrets_key.arn
  
  tags = {
    Environment = "production"
    Service     = "authentication"
  }
}

resource "aws_secretsmanager_secret_version" "jwt_keys" {
  secret_id = aws_secretsmanager_secret.jwt_keys.id
  secret_string = jsonencode({
    private_key = tls_private_key.jwt_key.private_key_pem
    public_key  = tls_private_key.jwt_key.public_key_pem
  })
}

resource "aws_secretsmanager_secret" "oauth2_credentials" {
  name        = "auth/oauth2-credentials"
  description = "OAuth2 client credentials"
  
  kms_key_id = aws_kms_key.auth_secrets_key.arn
}

resource "aws_secretsmanager_secret_version" "oauth2_credentials" {
  secret_id = aws_secretsmanager_secret.oauth2_credentials.id
  secret_string = jsonencode({
    google_client_id     = var.google_oauth2_client_id
    google_client_secret = var.google_oauth2_client_secret
    github_client_id     = var.github_oauth2_client_id
    github_client_secret = var.github_oauth2_client_secret
  })
}

# KMS ключ для шифрования секретов
resource "aws_kms_key" "auth_secrets_key" {
  description             = "KMS key for auth secrets encryption"
  deletion_window_in_days = 30
  
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "Enable IAM User Permissions"
        Effect = "Allow"
        Principal = {
          AWS = "arn:aws:iam::${data.aws_caller_identity.current.account_id}:root"
        }
        Action   = "kms:*"
        Resource = "*"
      },
      {
        Sid    = "Allow EKS Service Account"
        Effect = "Allow"
        Principal = {
          AWS = aws_iam_role.auth_service_role.arn
        }
        Action = [
          "kms:Decrypt",
          "kms:GenerateDataKey"
        ]
        Resource = "*"
      }
    ]
  })
  
  tags = {
    Name        = "auth-secrets-key"
    Environment = "production"
  }
}
```

### CloudTrail для аудита
```hcl
# infrastructure/terraform/cloudtrail.tf
resource "aws_cloudtrail" "auth_audit_trail" {
  name           = "auth-audit-trail"
  s3_bucket_name = aws_s3_bucket.auth_audit_logs.bucket
  
  include_global_service_events = true
  is_multi_region_trail        = true
  enable_logging               = true
  
  # Шифрование логов
  kms_key_id = aws_kms_key.auth_audit_key.arn
  
  # Фильтрация событий аутентификации
  event_selector {
    read_write_type                 = "All"
    include_management_events       = true
    exclude_management_event_sources = []
    
    data_resource {
      type   = "AWS::S3::Object"
      values = ["${aws_s3_bucket.auth_audit_logs.arn}/*"]
    }
    
    data_resource {
      type   = "AWS::SecretsManager::Secret"
      values = ["${aws_secretsmanager_secret.jwt_keys.arn}"]
    }
  }
  
  tags = {
    Environment = "production"
    Service     = "authentication-audit"
  }
}

resource "aws_s3_bucket" "auth_audit_logs" {
  bucket        = "auto-ru-auth-audit-logs-${random_id.bucket_suffix.hex}"
  force_destroy = false
  
  tags = {
    Name        = "auth-audit-logs"
    Environment = "production"
  }
}

resource "aws_s3_bucket_encryption" "auth_audit_logs_encryption" {
  bucket = aws_s3_bucket.auth_audit_logs.id
  
  server_side_encryption_configuration {
    rule {
      apply_server_side_encryption_by_default {
        kms_master_key_id = aws_kms_key.auth_audit_key.arn
        sse_algorithm     = "aws:kms"
      }
    }
  }
}
```

## 📊 Monitoring and Observability - Мониторинг и наблюдаемость

### Prometheus конфигурация для аутентификации
```yaml
# monitoring/prometheus/auth-config.yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "auth-rules.yml"

scrape_configs:
- job_name: 'apollo-gateway-auth'
  kubernetes_sd_configs:
  - role: pod
    namespaces:
      names:
      - auth-system
  relabel_configs:
  - source_labels: [__meta_kubernetes_pod_label_app]
    action: keep
    regex: apollo-gateway-auth
  - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
    action: keep
    regex: true
  - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
    action: replace
    target_label: __metrics_path__
    regex: (.+)

- job_name: 'auth-service'
  kubernetes_sd_configs:
  - role: pod
    namespaces:
      names:
      - auth-system
  relabel_configs:
  - source_labels: [__meta_kubernetes_pod_label_app]
    action: keep
    regex: auth-service

- job_name: 'rate-limiter-service'
  kubernetes_sd_configs:
  - role: pod
    namespaces:
      names:
      - auth-system
  relabel_configs:
  - source_labels: [__meta_kubernetes_pod_label_app]
    action: keep
    regex: rate-limiter-service

alerting:
  alertmanagers:
  - static_configs:
    - targets:
      - alertmanager:9093
```

### Grafana Dashboard для аутентификации
```json
{
  "dashboard": {
    "title": "Authentication & Authorization Dashboard",
    "panels": [
      {
        "title": "Authentication Success Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(auth_login_successes_total[5m]) / rate(auth_login_attempts_total[5m]) * 100",
            "legendFormat": "Success Rate %"
          }
        ]
      },
      {
        "title": "JWT Validation Latency",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(jwt_validation_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          },
          {
            "expr": "histogram_quantile(0.50, rate(jwt_validation_duration_seconds_bucket[5m]))",
            "legendFormat": "50th percentile"
          }
        ]
      },
      {
        "title": "Rate Limit Violations",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(rate_limit_violations_total[5m])",
            "legendFormat": "Violations per second"
          }
        ]
      },
      {
        "title": "Active Sessions",
        "type": "stat",
        "targets": [
          {
            "expr": "active_sessions",
            "legendFormat": "Active Sessions"
          }
        ]
      }
    ]
  }
}
```

Эта Deployment диаграмма демонстрирует полную production-ready инфраструктуру для системы аутентификации и авторизации в AWS, включая все аспекты безопасности, масштабируемости и мониторинга.