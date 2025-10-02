# Task 8: Deployment Diagram - Подробное объяснение production инфраструктуры телеметрии

## 🎯 Цель диаграммы

Deployment диаграмма Task 8 демонстрирует **production-ready инфраструктуру телеметрии и мониторинга** для GraphQL федерации Auto.ru в AWS облаке, показывая как компоненты наблюдаемости развертываются, масштабируются и интегрируются с управляемыми сервисами AWS для обеспечения enterprise-grade мониторинга.

## 🏗️ Архитектурная эволюция: от development к production

### От локальной разработки к облачной инфраструктуре

#### Было: Локальная разработка с Docker Compose
```yaml
# docker-compose.dev.yml - Development setup
version: '3.8'
services:
  # Простая локальная настройка
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"
    environment:
      - COLLECTOR_OTLP_ENABLED=true
      - SPAN_STORAGE_TYPE=memory
    # Проблемы:
    # - Нет персистентности данных
    # - Нет высокой доступности
    # - Ограниченная масштабируемость
    # - Нет интеграции с облачными сервисами
    # - Отсутствие мониторинга инфраструктуры

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    # Проблемы:
    # - Локальное хранение данных
    # - Нет резервного копирования
    # - Ограниченная retention policy
    # - Отсутствие кластеризации

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    # Проблемы:
    # - Нет персистентности конфигурации
    # - Отсутствие пользовательского управления
    # - Нет интеграции с корпоративной аутентификацией
```

#### Стало: Production AWS инфраструктура с полной наблюдаемостью
```yaml
# terraform/telemetry-infrastructure.tf
# Production AWS infrastructure для телеметрии

# VPC для телеметрии с полной изоляцией
resource "aws_vpc" "telemetry_vpc" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name        = "telemetry-vpc"
    Environment = var.environment
    Project     = "auto-ru-federation"
    Component   = "telemetry"
  }
}

# Подсети в разных AZ для высокой доступности
resource "aws_subnet" "telemetry_public" {
  count             = length(var.availability_zones)
  vpc_id            = aws_vpc.telemetry_vpc.id
  cidr_block        = "10.0.${count.index + 1}.0/24"
  availability_zone = var.availability_zones[count.index]
  
  map_public_ip_on_launch = true

  tags = {
    Name = "telemetry-public-${count.index + 1}"
    Type = "public"
  }
}

resource "aws_subnet" "telemetry_private" {
  count             = length(var.availability_zones)
  vpc_id            = aws_vpc.telemetry_vpc.id
  cidr_block        = "10.0.${count.index + 10}.0/24"
  availability_zone = var.availability_zones[count.index]

  tags = {
    Name = "telemetry-private-${count.index + 1}"
    Type = "private"
  }
}

# EKS кластер для телеметрии с полной конфигурацией
resource "aws_eks_cluster" "telemetry_cluster" {
  name     = "telemetry-cluster"
  role_arn = aws_iam_role.eks_cluster_role.arn
  version  = "1.28"

  vpc_config {
    subnet_ids              = concat(aws_subnet.telemetry_public[*].id, aws_subnet.telemetry_private[*].id)
    endpoint_private_access = true
    endpoint_public_access  = true
    public_access_cidrs     = ["0.0.0.0/0"]
  }

  # Логирование EKS
  enabled_cluster_log_types = [
    "api",
    "audit",
    "authenticator",
    "controllerManager",
    "scheduler"
  ]

  # Шифрование секретов
  encryption_config {
    provider {
      key_arn = aws_kms_key.eks_encryption.arn
    }
    resources = ["secrets"]
  }

  depends_on = [
    aws_iam_role_policy_attachment.eks_cluster_policy,
    aws_iam_role_policy_attachment.eks_vpc_resource_controller,
  ]

  tags = {
    Name        = "telemetry-eks-cluster"
    Environment = var.environment
  }
}

# Node groups для разных типов workloads
resource "aws_eks_node_group" "telemetry_nodes" {
  cluster_name    = aws_eks_cluster.telemetry_cluster.name
  node_group_name = "telemetry-nodes"
  node_role_arn   = aws_iam_role.eks_node_role.arn
  subnet_ids      = aws_subnet.telemetry_private[*].id

  # Конфигурация инстансов
  instance_types = ["m5.large", "m5.xlarge"]
  capacity_type  = "ON_DEMAND"

  # Автоскейлинг
  scaling_config {
    desired_size = 3
    max_size     = 10
    min_size     = 2
  }

  # Обновления
  update_config {
    max_unavailable_percentage = 25
  }

  # Taints для специализированных workloads
  taint {
    key    = "telemetry-workload"
    value  = "true"
    effect = "NO_SCHEDULE"
  }

  tags = {
    Name = "telemetry-node-group"
  }
}

# Amazon Managed Prometheus workspace
resource "aws_prometheus_workspace" "telemetry_prometheus" {
  alias = "auto-ru-telemetry"

  tags = {
    Name        = "auto-ru-telemetry-prometheus"
    Environment = var.environment
  }
}

# Amazon Managed Grafana workspace
resource "aws_grafana_workspace" "telemetry_grafana" {
  account_access_type      = "CURRENT_ACCOUNT"
  authentication_providers = ["AWS_SSO", "SAML"]
  permission_type          = "SERVICE_MANAGED"
  role_arn                = aws_iam_role.grafana_role.arn

  data_sources = [
    "PROMETHEUS",
    "CLOUDWATCH",
    "XRAY"
  ]

  notification_destinations = ["SNS"]

  tags = {
    Name        = "auto-ru-telemetry-grafana"
    Environment = var.environment
  }
}

# CloudWatch Log Groups с retention policies
resource "aws_cloudwatch_log_group" "application_logs" {
  name              = "/aws/eks/telemetry-cluster/application"
  retention_in_days = 30

  tags = {
    Name        = "application-logs"
    Environment = var.environment
  }
}

resource "aws_cloudwatch_log_group" "infrastructure_logs" {
  name              = "/aws/eks/telemetry-cluster/infrastructure"
  retention_in_days = 7

  tags = {
    Name        = "infrastructure-logs"
    Environment = var.environment
  }
}

# X-Ray для distributed tracing
resource "aws_xray_sampling_rule" "telemetry_sampling" {
  rule_name      = "telemetry-sampling"
  priority       = 9000
  version        = 1
  reservoir_size = 1
  fixed_rate     = 0.1
  url_path       = "*"
  host           = "*"
  http_method    = "*"
  service_type   = "*"
  service_name   = "*"
  resource_arn   = "*"
}
```

**Объяснение**: Production инфраструктура использует управляемые сервисы AWS для обеспечения высокой доступности, автоматического масштабирования и интеграции с корпоративными системами, в отличие от простой локальной настройки.

## 🔧 Ключевые компоненты production развертывания

### 1. AWS Cloud Telemetry Platform - Облачная платформа телеметрии

#### Production VPC with Telemetry - VPC с полной изоляцией
```yaml
# kubernetes/networking/vpc-configuration.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vpc-telemetry-config
  namespace: telemetry-system
data:
  vpc-cidr: "10.0.0.0/16"
  public-subnets: |
    - subnet-id: subnet-0123456789abcdef0
      cidr: "10.0.1.0/24"
      az: "us-east-1a"
    - subnet-id: subnet-0123456789abcdef1
      cidr: "10.0.2.0/24"
      az: "us-east-1b"
    - subnet-id: subnet-0123456789abcdef2
      cidr: "10.0.3.0/24"
      az: "us-east-1c"
  private-subnets: |
    - subnet-id: subnet-0123456789abcdef3
      cidr: "10.0.11.0/24"
      az: "us-east-1a"
    - subnet-id: subnet-0123456789abcdef4
      cidr: "10.0.12.0/24"
      az: "us-east-1b"
    - subnet-id: subnet-0123456789abcdef5
      cidr: "10.0.13.0/24"
      az: "us-east-1c"

---
# Network policies для изоляции телеметрии
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: telemetry-isolation
  namespace: telemetry-system
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: application
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 4317  # OTLP gRPC
    - protocol: TCP
      port: 4318  # OTLP HTTP
    - protocol: TCP
      port: 9090  # Prometheus metrics
  egress:
  - to: []  # Allow all egress for external services
```

#### ALB with Telemetry - Load Balancer с телеметрией
```yaml
# kubernetes/ingress/alb-telemetry.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: telemetry-ingress
  namespace: telemetry-system
  annotations:
    # ALB конфигурация
    kubernetes.io/ingress.class: alb
    alb.ingress.kubernetes.io/scheme: internet-facing
    alb.ingress.kubernetes.io/target-type: ip
    alb.ingress.kubernetes.io/load-balancer-name: telemetry-alb
    
    # SSL/TLS конфигурация
    alb.ingress.kubernetes.io/certificate-arn: arn:aws:acm:us-east-1:123456789012:certificate/12345678-1234-1234-1234-123456789012
    alb.ingress.kubernetes.io/ssl-policy: ELBSecurityPolicy-TLS-1-2-2017-01
    alb.ingress.kubernetes.io/ssl-redirect: '443'
    
    # Логирование доступа
    alb.ingress.kubernetes.io/load-balancer-attributes: |
      access_logs.s3.enabled=true,
      access_logs.s3.bucket=auto-ru-telemetry-access-logs,
      access_logs.s3.prefix=alb-logs
    
    # Health checks
    alb.ingress.kubernetes.io/healthcheck-path: /health
    alb.ingress.kubernetes.io/healthcheck-interval-seconds: '30'
    alb.ingress.kubernetes.io/healthcheck-timeout-seconds: '5'
    alb.ingress.kubernetes.io/healthy-threshold-count: '2'
    alb.ingress.kubernetes.io/unhealthy-threshold-count: '3'
    
    # Sticky sessions для Grafana
    alb.ingress.kubernetes.io/target-group-attributes: |
      stickiness.enabled=true,
      stickiness.lb_cookie.duration_seconds=86400
spec:
  rules:
  - host: telemetry.auto.ru
    http:
      paths:
      - path: /grafana
        pathType: Prefix
        backend:
          service:
            name: grafana-service
            port:
              number: 3000
      - path: /jaeger
        pathType: Prefix
        backend:
          service:
            name: jaeger-query-service
            port:
              number: 16686
      - path: /prometheus
        pathType: Prefix
        backend:
          service:
            name: prometheus-service
            port:
              number: 9090
      - path: /kibana
        pathType: Prefix
        backend:
          service:
            name: kibana-service
            port:
              number: 5601
```

### 2. EKS Telemetry Clusters - Kubernetes кластеры для телеметрии

#### UGC Telemetry Pod - Инструментированный UGC сервис
```yaml
# kubernetes/applications/ugc-telemetry-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-service-telemetry
  namespace: application
  labels:
    app: ugc-service
    component: telemetry
    version: v1.0.0
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: ugc-service
  template:
    metadata:
      labels:
        app: ugc-service
        component: telemetry
      annotations:
        # Prometheus scraping
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"
        
        # Jaeger tracing
        sidecar.jaegertracing.io/inject: "true"
        
        # Linkerd service mesh
        linkerd.io/inject: enabled
    spec:
      serviceAccountName: ugc-service-account
      
      # Init container для миграций БД
      initContainers:
      - name: db-migration
        image: auto-ru/ugc-service:v1.0.0
        command: ["./migrate"]
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-credentials
              key: url
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "256Mi"
            cpu: "200m"
      
      containers:
      - name: ugc-service
        image: auto-ru/ugc-service:v1.0.0
        ports:
        - containerPort: 4001
          name: graphql
          protocol: TCP
        - containerPort: 9090
          name: metrics
          protocol: TCP
        - containerPort: 8080
          name: health
          protocol: TCP
        
        # Environment variables для телеметрии
        env:
        - name: SERVICE_NAME
          value: "ugc-subgraph"
        - name: SERVICE_VERSION
          value: "v1.0.0"
        - name: ENVIRONMENT
          value: "production"
        - name: JAEGER_ENDPOINT
          value: "http://jaeger-collector.telemetry-system.svc.cluster.local:14268/api/traces"
        - name: OTEL_EXPORTER_OTLP_ENDPOINT
          value: "http://otel-collector.telemetry-system.svc.cluster.local:4317"
        - name: PROMETHEUS_ENDPOINT
          value: "http://prometheus-server.telemetry-system.svc.cluster.local:9090"
        - name: TRACE_SAMPLE_RATE
          value: "0.1"  # 10% sampling в production
        - name: ENABLE_CONSOLE_LOGS
          value: "false"  # Только structured JSON в production
        - name: LOG_LEVEL
          value: "info"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-credentials
              key: url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: url
        
        # Resource limits
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
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
          failureThreshold: 3
        
        # Security context
        securityContext:
          allowPrivilegeEscalation: false
          runAsNonRoot: true
          runAsUser: 1000
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        
        # Volume mounts для логов
        volumeMounts:
        - name: tmp-volume
          mountPath: /tmp
        - name: logs-volume
          mountPath: /app/logs
      
      # Sidecar для сбора логов
      - name: filebeat-sidecar
        image: elastic/filebeat:8.10.0
        args: [
          "-c", "/etc/filebeat.yml",
          "-e",
        ]
        env:
        - name: ELASTICSEARCH_HOST
          value: "elasticsearch.telemetry-system.svc.cluster.local"
        - name: ELASTICSEARCH_PORT
          value: "9200"
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        resources:
          requests:
            memory: "128Mi"
            cpu: "50m"
          limits:
            memory: "256Mi"
            cpu: "100m"
        volumeMounts:
        - name: filebeat-config
          mountPath: /etc/filebeat.yml
          subPath: filebeat.yml
        - name: logs-volume
          mountPath: /app/logs
          readOnly: true
      
      volumes:
      - name: tmp-volume
        emptyDir: {}
      - name: logs-volume
        emptyDir: {}
      - name: filebeat-config
        configMap:
          name: filebeat-config
      
      # Node affinity для распределения по AZ
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
                  - ugc-service
              topologyKey: kubernetes.io/hostname
        nodeAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
            nodeSelectorTerms:
            - matchExpressions:
              - key: node-type
                operator: In
                values:
                - application
      
      # Tolerations для специализированных нод
      tolerations:
      - key: "application-workload"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"

---
# Service для UGC с телеметрией
apiVersion: v1
kind: Service
metadata:
  name: ugc-service-telemetry
  namespace: application
  labels:
    app: ugc-service
    component: telemetry
  annotations:
    # Prometheus service discovery
    prometheus.io/scrape: "true"
    prometheus.io/port: "9090"
    prometheus.io/path: "/metrics"
    
    # Service mesh annotations
    linkerd.io/inject: enabled
spec:
  type: ClusterIP
  ports:
  - name: graphql
    port: 4001
    targetPort: 4001
    protocol: TCP
  - name: metrics
    port: 9090
    targetPort: 9090
    protocol: TCP
  - name: health
    port: 8080
    targetPort: 8080
    protocol: TCP
  selector:
    app: ugc-service
```

#### OpenTelemetry Collector - Централизованный сбор телеметрии
```yaml
# kubernetes/telemetry/otel-collector-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: otel-collector
  namespace: telemetry-system
  labels:
    app: otel-collector
    component: telemetry-collection
spec:
  replicas: 3
  selector:
    matchLabels:
      app: otel-collector
  template:
    metadata:
      labels:
        app: otel-collector
    spec:
      serviceAccountName: otel-collector-service-account
      containers:
      - name: otel-collector
        image: otel/opentelemetry-collector-contrib:0.88.0
        command:
        - "/otelcol-contrib"
        - "--config=/etc/otel-collector-config.yaml"
        ports:
        - containerPort: 4317  # OTLP gRPC receiver
          name: otlp-grpc
        - containerPort: 4318  # OTLP HTTP receiver
          name: otlp-http
        - containerPort: 8889  # Prometheus metrics
          name: prometheus
        - containerPort: 8888  # Internal metrics
          name: internal
        - containerPort: 13133 # Health check
          name: health
        
        env:
        - name: GOMEMLIMIT
          value: "512MiB"
        - name: AWS_REGION
          value: "us-east-1"
        - name: JAEGER_ENDPOINT
          value: "jaeger-collector.telemetry-system.svc.cluster.local:14250"
        - name: PROMETHEUS_REMOTE_WRITE_ENDPOINT
          value: "https://aps-workspaces.us-east-1.amazonaws.com/workspaces/ws-12345678-1234-1234-1234-123456789012/api/v1/remote_write"
        - name: XRAY_REGION
          value: "us-east-1"
        
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
        
        livenessProbe:
          httpGet:
            path: /
            port: 13133
          initialDelaySeconds: 30
          periodSeconds: 10
        
        readinessProbe:
          httpGet:
            path: /
            port: 13133
          initialDelaySeconds: 5
          periodSeconds: 5
        
        volumeMounts:
        - name: otel-collector-config
          mountPath: /etc/otel-collector-config.yaml
          subPath: otel-collector-config.yaml
        - name: aws-credentials
          mountPath: /etc/aws-credentials
          readOnly: true
      
      volumes:
      - name: otel-collector-config
        configMap:
          name: otel-collector-config
      - name: aws-credentials
        secret:
          secretName: aws-credentials

---
# ConfigMap с конфигурацией OTEL Collector
apiVersion: v1
kind: ConfigMap
metadata:
  name: otel-collector-config
  namespace: telemetry-system
data:
  otel-collector-config.yaml: |
    receivers:
      otlp:
        protocols:
          grpc:
            endpoint: 0.0.0.0:4317
          http:
            endpoint: 0.0.0.0:4318
            cors:
              allowed_origins:
                - "*"
      
      # Prometheus receiver для scraping
      prometheus:
        config:
          scrape_configs:
            - job_name: 'kubernetes-pods'
              kubernetes_sd_configs:
                - role: pod
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
    
    processors:
      # Batch processor для оптимизации
      batch:
        timeout: 1s
        send_batch_size: 1024
        send_batch_max_size: 2048
      
      # Memory limiter для защиты от OOM
      memory_limiter:
        limit_mib: 512
        spike_limit_mib: 128
        check_interval: 5s
      
      # Resource processor для обогащения
      resource:
        attributes:
          - key: service.namespace
            value: "auto-ru-federation"
            action: upsert
          - key: deployment.environment
            value: "production"
            action: upsert
          - key: k8s.cluster.name
            value: "telemetry-cluster"
            action: upsert
      
      # Attributes processor для очистки PII
      attributes:
        actions:
          - key: http.user_agent
            action: delete
          - key: net.peer.ip
            action: hash
          - key: user.id
            action: hash
    
    exporters:
      # Jaeger exporter
      jaeger:
        endpoint: ${JAEGER_ENDPOINT}
        tls:
          insecure: true
      
      # AWS X-Ray exporter
      awsxray:
        region: ${XRAY_REGION}
        no_verify_ssl: false
      
      # Prometheus remote write для AMP
      prometheusremotewrite:
        endpoint: ${PROMETHEUS_REMOTE_WRITE_ENDPOINT}
        auth:
          authenticator: sigv4auth
        resource_to_telemetry_conversion:
          enabled: true
      
      # CloudWatch metrics exporter
      awscloudwatchmetrics:
        region: ${AWS_REGION}
        namespace: "AutoRu/Federation"
        dimension_rollup_option: "NoDimensionRollup"
        metric_declarations:
          - dimensions: [[service.name], [service.name, operation]]
            metric_name_selectors:
              - ".*_duration"
              - ".*_requests_total"
              - ".*_errors_total"
      
      # Logging exporter для debugging
      logging:
        loglevel: info
        sampling_initial: 5
        sampling_thereafter: 200
    
    extensions:
      # Health check extension
      health_check:
        endpoint: 0.0.0.0:13133
      
      # pprof для профилирования
      pprof:
        endpoint: 0.0.0.0:1777
      
      # zpages для debugging
      zpages:
        endpoint: 0.0.0.0:55679
      
      # AWS authenticator
      sigv4auth:
        region: ${AWS_REGION}
        service: "aps"
    
    service:
      extensions: [health_check, pprof, zpages, sigv4auth]
      
      pipelines:
        # Traces pipeline
        traces:
          receivers: [otlp]
          processors: [memory_limiter, resource, attributes, batch]
          exporters: [jaeger, awsxray, logging]
        
        # Metrics pipeline
        metrics:
          receivers: [otlp, prometheus]
          processors: [memory_limiter, resource, batch]
          exporters: [prometheusremotewrite, awscloudwatchmetrics, logging]
      
      telemetry:
        logs:
          level: "info"
        metrics:
          address: 0.0.0.0:8888
```

### 3. Monitoring Region - Управляемые сервисы мониторинга

#### Amazon Managed Prometheus - Управляемый Prometheus
```yaml
# terraform/managed-prometheus.tf
resource "aws_prometheus_workspace" "auto_ru_telemetry" {
  alias = "auto-ru-telemetry"
  
  tags = {
    Name        = "auto-ru-telemetry-prometheus"
    Environment = "production"
    Project     = "auto-ru-federation"
  }
}

# IAM роль для remote write
resource "aws_iam_role" "prometheus_remote_write" {
  name = "prometheus-remote-write-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "eks.amazonaws.com"
        }
      },
      {
        Action = "sts:AssumeRoleWithWebIdentity"
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.eks.arn
        }
        Condition = {
          StringEquals = {
            "${replace(aws_iam_openid_connect_provider.eks.url, "https://", "")}:sub": "system:serviceaccount:telemetry-system:otel-collector-service-account"
          }
        }
      }
    ]
  })
}

resource "aws_iam_role_policy" "prometheus_remote_write" {
  name = "prometheus-remote-write-policy"
  role = aws_iam_role.prometheus_remote_write.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "aps:RemoteWrite",
          "aps:QueryMetrics",
          "aps:GetSeries",
          "aps:GetLabels",
          "aps:GetMetricMetadata"
        ]
        Resource = aws_prometheus_workspace.auto_ru_telemetry.arn
      }
    ]
  })
}

# Alert rules для AMP
resource "aws_prometheus_rule_group_namespace" "telemetry_alerts" {
  workspace_id = aws_prometheus_workspace.auto_ru_telemetry.id
  namespace    = "telemetry-alerts"
  data = yamlencode({
    groups = [
      {
        name = "auto-ru-federation-alerts"
        rules = [
          {
            alert = "HighErrorRate"
            expr  = "rate(http_requests_error_total[5m]) / rate(http_requests_total[5m]) > 0.05"
            for   = "5m"
            labels = {
              severity = "warning"
            }
            annotations = {
              summary     = "High error rate detected"
              description = "Error rate is above 5% for {{ $labels.service }}"
            }
          },
          {
            alert = "HighLatency"
            expr  = "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1"
            for   = "10m"
            labels = {
              severity = "warning"
            }
            annotations = {
              summary     = "High latency detected"
              description = "95th percentile latency is above 1s for {{ $labels.service }}"
            }
          },
          {
            alert = "ServiceDown"
            expr  = "up == 0"
            for   = "1m"
            labels = {
              severity = "critical"
            }
            annotations = {
              summary     = "Service is down"
              description = "Service {{ $labels.service }} is not responding"
            }
          }
        ]
      }
    ]
  })
}
```

#### Amazon Managed Grafana - Управляемый Grafana
```yaml
# terraform/managed-grafana.tf
resource "aws_grafana_workspace" "auto_ru_telemetry" {
  account_access_type      = "CURRENT_ACCOUNT"
  authentication_providers = ["AWS_SSO"]
  permission_type          = "SERVICE_MANAGED"
  role_arn                = aws_iam_role.grafana_service_role.arn
  
  name        = "auto-ru-telemetry"
  description = "Auto.ru Federation Telemetry Dashboards"

  data_sources = [
    "PROMETHEUS",
    "CLOUDWATCH",
    "XRAY"
  ]

  notification_destinations = ["SNS"]
  
  organizational_units = ["ou-root-123456789"]

  vpc_configuration {
    security_group_ids = [aws_security_group.grafana.id]
    subnet_ids         = aws_subnet.telemetry_private[*].id
  }

  tags = {
    Name        = "auto-ru-telemetry-grafana"
    Environment = "production"
  }
}

# Grafana dashboard provisioning
resource "aws_grafana_workspace_configuration" "auto_ru_dashboards" {
  workspace_id = aws_grafana_workspace.auto_ru_telemetry.id
  
  configuration = jsonencode({
    datasources = [
      {
        name = "Amazon Managed Service for Prometheus"
        type = "prometheus"
        url  = "https://aps-workspaces.${var.aws_region}.amazonaws.com/workspaces/${aws_prometheus_workspace.auto_ru_telemetry.id}/"
        access = "proxy"
        jsonData = {
          httpMethod   = "POST"
          sigV4Auth    = true
          sigV4AuthType = "workspace-iam-role"
          sigV4Region  = var.aws_region
        }
        isDefault = true
      },
      {
        name = "CloudWatch"
        type = "cloudwatch"
        jsonData = {
          defaultRegion = var.aws_region
          authType      = "workspace-iam-role"
        }
      },
      {
        name = "X-Ray"
        type = "x-ray-datasource"
        jsonData = {
          authType = "workspace-iam-role"
          defaultRegion = var.aws_region
        }
      }
    ]
    
    dashboards = [
      {
        name = "Auto.ru Federation Overview"
        folder = "Auto.ru"
        definition = file("${path.module}/dashboards/federation-overview.json")
      },
      {
        name = "GraphQL Performance"
        folder = "Auto.ru"
        definition = file("${path.module}/dashboards/graphql-performance.json")
      },
      {
        name = "Business Metrics"
        folder = "Auto.ru"
        definition = file("${path.module}/dashboards/business-metrics.json")
      }
    ]
  })
}
```

### 4. Notification Infrastructure - Инфраструктура уведомлений

#### Multi-channel Notifications - Многоканальные уведомления
```yaml
# terraform/notification-infrastructure.tf

# SNS топики для разных типов алертов
resource "aws_sns_topic" "critical_alerts" {
  name = "auto-ru-critical-alerts"
  
  tags = {
    Name        = "critical-alerts"
    Environment = "production"
  }
}

resource "aws_sns_topic" "warning_alerts" {
  name = "auto-ru-warning-alerts"
  
  tags = {
    Name        = "warning-alerts"
    Environment = "production"
  }
}

# Lambda функция для обработки алертов
resource "aws_lambda_function" "alert_processor" {
  filename         = "alert-processor.zip"
  function_name    = "auto-ru-alert-processor"
  role            = aws_iam_role.lambda_alert_processor.arn
  handler         = "index.handler"
  runtime         = "python3.9"
  timeout         = 30

  environment {
    variables = {
      SLACK_WEBHOOK_URL = var.slack_webhook_url
      PAGERDUTY_API_KEY = var.pagerduty_api_key
      ENVIRONMENT       = "production"
    }
  }

  tags = {
    Name        = "alert-processor"
    Environment = "production"
  }
}

# SES для email уведомлений
resource "aws_ses_configuration_set" "alert_emails" {
  name = "auto-ru-alert-emails"

  delivery_options {
    tls_policy = "Require"
  }

  reputation_metrics_enabled = true
}

resource "aws_ses_template" "critical_alert" {
  name    = "critical-alert-template"
  subject = "[CRITICAL] Auto.ru Federation Alert: {{alertname}}"
  html    = file("${path.module}/templates/critical-alert.html")
  text    = file("${path.module}/templates/critical-alert.txt")
}

# EventBridge rules для маршрутизации алертов
resource "aws_cloudwatch_event_rule" "critical_alerts" {
  name        = "auto-ru-critical-alerts"
  description = "Route critical alerts to appropriate channels"

  event_pattern = jsonencode({
    source      = ["aws.prometheus"]
    detail-type = ["Prometheus Alert"]
    detail = {
      severity = ["critical"]
    }
  })
}

resource "aws_cloudwatch_event_target" "critical_to_pagerduty" {
  rule      = aws_cloudwatch_event_rule.critical_alerts.name
  target_id = "SendToPagerDuty"
  arn       = aws_lambda_function.alert_processor.arn

  input_transformer {
    input_paths = {
      alertname = "$.detail.alertname"
      severity  = "$.detail.severity"
      summary   = "$.detail.summary"
    }
    input_template = jsonencode({
      channel = "pagerduty"
      alert = {
        name     = "<alertname>"
        severity = "<severity>"
        summary  = "<summary>"
      }
    })
  }
}
```

## 🔧 Production операционные паттерны

### 1. High Availability Architecture
- **Multi-AZ развертывание**: Все компоненты развернуты в нескольких зонах доступности
- **Auto Scaling**: Автоматическое масштабирование на основе метрик
- **Circuit Breakers**: Защита от каскадных отказов
- **Graceful Degradation**: Постепенная деградация при проблемах

### 2. Security Best Practices
- **Network Isolation**: VPC с приватными подсетями
- **IAM Roles**: Минимальные привилегии для каждого компонента
- **Encryption**: Шифрование данных в покое и в движении
- **Secret Management**: AWS Secrets Manager для чувствительных данных

### 3. Cost Optimization
- **Spot Instances**: Использование spot инстансов для non-critical workloads
- **Resource Right-sizing**: Оптимизация размеров инстансов
- **Data Retention**: Политики retention для логов и метрик
- **Reserved Instances**: Резервирование инстансов для predictable workloads

### 4. Disaster Recovery
- **Cross-Region Backup**: Резервное копирование в другой регион
- **RTO/RPO Targets**: Четкие цели по времени восстановления
- **Automated Failover**: Автоматическое переключение при отказах
- **Regular DR Drills**: Регулярные учения по восстановлению

## 🎯 Заключение

Deployment диаграмма демонстрирует enterprise-grade инфраструктуру телеметрии, которая обеспечивает:

- **Масштабируемость**: Автоматическое масштабирование под нагрузку
- **Надежность**: Высокая доступность и отказоустойчивость
- **Безопасность**: Полная изоляция и шифрование
- **Наблюдаемость**: Полная видимость всех компонентов системы
- **Операционную эффективность**: Автоматизация и мониторинг

Эта архитектура готова для production использования и может поддерживать крупномасштабную GraphQL федерацию Auto.ru с полной наблюдаемостью.