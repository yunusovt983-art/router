# C4 Deployment Diagram - Подробное объяснение Task 3

## Обзор диаграммы

**Файл**: `C4_ARCHITECTURE_DEPLOYMENT.puml`

Диаграмма развертывания Task 3 показывает production-ready архитектуру системы интеграции и мониторинга федеративной GraphQL системы в облачной среде.

## Архитектура развертывания

### 1. Cloud Provider Infrastructure

#### Kubernetes Cluster
```plantuml
Deployment_Node(k8s_cluster, "Kubernetes Cluster", "EKS/GKE/AKS")
```

**Архитектурная роль**: Оркестрация контейнеров и автомасштабирование

**Конфигурация кластера**:
```yaml
# k8s/cluster/cluster-config.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: cluster-config
  namespace: kube-system
data:
  cluster.yaml: |
    apiVersion: eksctl.io/v1alpha5
    kind: ClusterConfig
    
    metadata:
      name: auto-ru-federation
      region: us-west-2
      version: "1.28"
    
    # Managed node groups
    managedNodeGroups:
      - name: integration-nodes
        instanceType: m5.large
        minSize: 3
        maxSize: 10
        desiredCapacity: 5
        volumeSize: 100
        ssh:
          allow: false
        labels:
          role: integration
        taints:
          - key: workload
            value: integration
            effect: NoSchedule
        tags:
          Environment: production
          Team: platform
      
      - name: monitoring-nodes
        instanceType: m5.xlarge
        minSize: 2
        maxSize: 5
        desiredCapacity: 3
        volumeSize: 200
        labels:
          role: monitoring
        taints:
          - key: workload
            value: monitoring
            effect: NoSchedule
    
    # Add-ons
    addons:
      - name: vpc-cni
        version: latest
      - name: coredns
        version: latest
      - name: kube-proxy
        version: latest
      - name: aws-ebs-csi-driver
        version: latest
    
    # CloudWatch logging
    cloudWatch:
      clusterLogging:
        enable: true
        logTypes: ["api", "audit", "authenticator", "controllerManager", "scheduler"]
```

#### Integration Namespace
```plantuml
Deployment_Node(integration_namespace, "Integration Namespace", "Kubernetes Namespace")
```

**Конфигурация namespace и RBAC**:
```yaml
# k8s/namespaces/integration-namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: integration
  labels:
    name: integration
    environment: production
    team: platform
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: integration-hub-sa
  namespace: integration
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::ACCOUNT:role/IntegrationHubRole
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  namespace: integration
  name: integration-hub-role
rules:
- apiGroups: [""]
  resources: ["pods", "services", "configmaps", "secrets"]
  verbs: ["get", "list", "watch", "create", "update", "patch"]
- apiGroups: ["apps"]
  resources: ["deployments", "replicasets"]
  verbs: ["get", "list", "watch", "create", "update", "patch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: integration-hub-binding
  namespace: integration
subjects:
- kind: ServiceAccount
  name: integration-hub-sa
  namespace: integration
roleRef:
  kind: Role
  name: integration-hub-role
  apiGroup: rbac.authorization.k8s.io
```

#### Integration Hub Deployment
```plantuml
Deployment_Node(integration_deployment, "Integration Hub Deployment", "Kubernetes Deployment")
```

**Реализация Deployment**:
```yaml
# k8s/deployments/integration-hub.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: integration-hub
  namespace: integration
  labels:
    app: integration-hub
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
      app: integration-hub
  template:
    metadata:
      labels:
        app: integration-hub
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "3000"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: integration-hub-sa
      tolerations:
      - key: workload
        value: integration
        effect: NoSchedule
      nodeSelector:
        role: integration
      
      containers:
      - name: integration-hub
        image: auto-ru/integration-hub:v1.0.0
        ports:
        - containerPort: 3000
          name: http
        - containerPort: 3001
          name: websocket
        
        env:
        - name: NODE_ENV
          value: "production"
        - name: PORT
          value: "3000"
        - name: WEBSOCKET_PORT
          value: "3001"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: integration-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: integration-secrets
              key: redis-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: integration-secrets
              key: jwt-secret
        - name: PROMETHEUS_URL
          value: "http://prometheus.monitoring.svc.cluster.local:9090"
        - name: JAEGER_ENDPOINT
          value: "http://jaeger.monitoring.svc.cluster.local:14268/api/traces"
        
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
            port: 3000
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        
        volumeMounts:
        - name: config-volume
          mountPath: /app/config
          readOnly: true
        - name: logs-volume
          mountPath: /app/logs
      
      volumes:
      - name: config-volume
        configMap:
          name: integration-hub-config
      - name: logs-volume
        emptyDir: {}
      
      # Security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      
      # Pod disruption budget
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
                  - integration-hub
              topologyKey: kubernetes.io/hostname
---
apiVersion: v1
kind: Service
metadata:
  name: integration-hub-service
  namespace: integration
  labels:
    app: integration-hub
spec:
  type: ClusterIP
  ports:
  - port: 80
    targetPort: 3000
    protocol: TCP
    name: http
  - port: 3001
    targetPort: 3001
    protocol: TCP
    name: websocket
  selector:
    app: integration-hub
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: integration-hub-pdb
  namespace: integration
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: integration-hub
```

#### Apollo Gateway Deployment
```plantuml
Deployment_Node(gateway_deployment, "Apollo Gateway Deployment", "Kubernetes Deployment")
```

**Конфигурация Gateway**:
```yaml
# k8s/deployments/apollo-gateway.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apollo-gateway
  namespace: integration
spec:
  replicas: 2
  selector:
    matchLabels:
      app: apollo-gateway
  template:
    metadata:
      labels:
        app: apollo-gateway
    spec:
      containers:
      - name: apollo-gateway
        image: auto-ru/apollo-gateway:v2.0.0
        ports:
        - containerPort: 4000
        
        env:
        - name: NODE_ENV
          value: "production"
        - name: APOLLO_GRAPH_REF
          valueFrom:
            secretKeyRef:
              name: apollo-secrets
              key: graph-ref
        - name: APOLLO_KEY
          valueFrom:
            secretKeyRef:
              name: apollo-secrets
              key: api-key
        - name: SUBGRAPH_UGC_URL
          value: "http://ugc-subgraph.subgraphs.svc.cluster.local:4001/graphql"
        - name: SUBGRAPH_USERS_URL
          value: "http://users-subgraph.subgraphs.svc.cluster.local:4002/graphql"
        - name: SUBGRAPH_OFFERS_URL
          value: "http://offers-subgraph.subgraphs.svc.cluster.local:4003/graphql"
        
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        
        livenessProbe:
          httpGet:
            path: /.well-known/apollo/server-health
            port: 4000
          initialDelaySeconds: 30
          periodSeconds: 10
        
        readinessProbe:
          httpGet:
            path: /.well-known/apollo/server-health
            port: 4000
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: apollo-gateway-service
  namespace: integration
spec:
  type: ClusterIP
  ports:
  - port: 4000
    targetPort: 4000
  selector:
    app: apollo-gateway
```

### 2. Monitoring Infrastructure

#### Prometheus Deployment
```plantuml
Deployment_Node(prometheus_deployment, "Prometheus Deployment", "Kubernetes StatefulSet")
```

**Конфигурация Prometheus**:
```yaml
# k8s/monitoring/prometheus.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: prometheus
  namespace: monitoring
spec:
  serviceName: prometheus
  replicas: 1
  selector:
    matchLabels:
      app: prometheus
  template:
    metadata:
      labels:
        app: prometheus
    spec:
      serviceAccountName: prometheus-sa
      containers:
      - name: prometheus
        image: prom/prometheus:v2.45.0
        ports:
        - containerPort: 9090
        
        args:
        - '--config.file=/etc/prometheus/prometheus.yml'
        - '--storage.tsdb.path=/prometheus'
        - '--web.console.libraries=/etc/prometheus/console_libraries'
        - '--web.console.templates=/etc/prometheus/consoles'
        - '--storage.tsdb.retention.time=30d'
        - '--web.enable-lifecycle'
        - '--web.enable-admin-api'
        
        resources:
          requests:
            memory: "2Gi"
            cpu: "500m"
          limits:
            memory: "4Gi"
            cpu: "1000m"
        
        volumeMounts:
        - name: prometheus-config
          mountPath: /etc/prometheus
        - name: prometheus-storage
          mountPath: /prometheus
        
        livenessProbe:
          httpGet:
            path: /-/healthy
            port: 9090
          initialDelaySeconds: 30
          periodSeconds: 15
        
        readinessProbe:
          httpGet:
            path: /-/ready
            port: 9090
          initialDelaySeconds: 5
          periodSeconds: 5
      
      volumes:
      - name: prometheus-config
        configMap:
          name: prometheus-config
  
  volumeClaimTemplates:
  - metadata:
      name: prometheus-storage
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: gp3
      resources:
        requests:
          storage: 100Gi
---
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
    # Integration Hub metrics
    - job_name: 'integration-hub'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - integration
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
    
    # Apollo Gateway metrics
    - job_name: 'apollo-gateway'
      kubernetes_sd_configs:
      - role: service
        namespaces:
          names:
          - integration
      relabel_configs:
      - source_labels: [__meta_kubernetes_service_name]
        action: keep
        regex: apollo-gateway-service
    
    # Subgraph metrics
    - job_name: 'subgraphs'
      kubernetes_sd_configs:
      - role: service
        namespaces:
          names:
          - subgraphs
      relabel_configs:
      - source_labels: [__meta_kubernetes_service_label_app]
        action: keep
        regex: .*-subgraph
    
    alerting:
      alertmanagers:
      - kubernetes_sd_configs:
        - role: service
          namespaces:
            names:
            - monitoring
        relabel_configs:
        - source_labels: [__meta_kubernetes_service_name]
          action: keep
          regex: alertmanager
```

### 3. Managed Services

#### RDS Cluster
```plantuml
Deployment_Node(rds_cluster, "RDS Cluster", "AWS RDS/Cloud SQL")
```

**Конфигурация управляемой базы данных**:
```yaml
# terraform/rds.tf
resource "aws_rds_cluster" "integration_cluster" {
  cluster_identifier      = "auto-ru-integration-cluster"
  engine                 = "aurora-postgresql"
  engine_version         = "15.4"
  database_name          = "integration_db"
  master_username        = "integration_admin"
  manage_master_user_password = true
  
  # High availability
  availability_zones = ["us-west-2a", "us-west-2b", "us-west-2c"]
  
  # Backup configuration
  backup_retention_period = 30
  preferred_backup_window = "03:00-04:00"
  preferred_maintenance_window = "sun:04:00-sun:05:00"
  
  # Security
  vpc_security_group_ids = [aws_security_group.rds_sg.id]
  db_subnet_group_name   = aws_db_subnet_group.integration_subnet_group.name
  
  # Performance
  storage_encrypted = true
  kms_key_id       = aws_kms_key.rds_key.arn
  
  # Monitoring
  enabled_cloudwatch_logs_exports = ["postgresql"]
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_monitoring_role.arn
  
  # Performance Insights
  performance_insights_enabled = true
  performance_insights_retention_period = 7
  
  tags = {
    Name        = "auto-ru-integration-cluster"
    Environment = "production"
    Team        = "platform"
  }
}

resource "aws_rds_cluster_instance" "integration_instances" {
  count              = 2
  identifier         = "integration-instance-${count.index + 1}"
  cluster_identifier = aws_rds_cluster.integration_cluster.id
  instance_class     = "db.r6g.large"
  engine             = aws_rds_cluster.integration_cluster.engine
  engine_version     = aws_rds_cluster.integration_cluster.engine_version
  
  performance_insights_enabled = true
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_monitoring_role.arn
  
  tags = {
    Name = "integration-instance-${count.index + 1}"
  }
}

# Read replica for analytics
resource "aws_rds_cluster" "integration_read_replica" {
  cluster_identifier = "auto-ru-integration-read-replica"
  
  # Cross-region read replica
  source_region               = "us-west-2"
  replication_source_identifier = aws_rds_cluster.integration_cluster.cluster_resource_id
  
  engine         = "aurora-postgresql"
  instance_class = "db.r6g.large"
  
  tags = {
    Name = "integration-read-replica"
    Purpose = "analytics"
  }
}
```

#### ElastiCache Cluster
```plantuml
Deployment_Node(elasticache_cluster, "ElastiCache Cluster", "AWS ElastiCache/Memorystore")
```

**Конфигурация управляемого кеша**:
```yaml
# terraform/elasticache.tf
resource "aws_elasticache_replication_group" "integration_cache" {
  replication_group_id       = "auto-ru-integration-cache"
  description                = "Redis cluster for integration hub"
  
  # Redis configuration
  engine               = "redis"
  engine_version       = "7.0"
  node_type           = "cache.r6g.large"
  port                = 6379
  parameter_group_name = aws_elasticache_parameter_group.integration_params.name
  
  # Cluster configuration
  num_cache_clusters = 3
  
  # High availability
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  # Security
  subnet_group_name = aws_elasticache_subnet_group.integration_cache_subnet.name
  security_group_ids = [aws_security_group.elasticache_sg.id]
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  auth_token = random_password.redis_auth_token.result
  
  # Backup
  snapshot_retention_limit = 7
  snapshot_window         = "03:00-05:00"
  maintenance_window      = "sun:05:00-sun:07:00"
  
  # Monitoring
  notification_topic_arn = aws_sns_topic.cache_notifications.arn
  
  tags = {
    Name        = "integration-cache"
    Environment = "production"
  }
}

resource "aws_elasticache_parameter_group" "integration_params" {
  family = "redis7"
  name   = "integration-cache-params"
  
  parameter {
    name  = "maxmemory-policy"
    value = "allkeys-lru"
  }
  
  parameter {
    name  = "timeout"
    value = "300"
  }
  
  parameter {
    name  = "tcp-keepalive"
    value = "300"
  }
}
```

## Сетевая архитектура и безопасность

### Network Security Groups
```yaml
# terraform/security-groups.tf
resource "aws_security_group" "integration_hub_sg" {
  name_prefix = "integration-hub-"
  vpc_id      = aws_vpc.main.id
  
  # Inbound rules
  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = [aws_vpc.main.cidr_block]
  }
  
  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = [aws_vpc.main.cidr_block]
  }
  
  # Outbound rules
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
  
  tags = {
    Name = "integration-hub-sg"
  }
}

resource "aws_security_group" "rds_sg" {
  name_prefix = "rds-"
  vpc_id      = aws_vpc.main.id
  
  ingress {
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [aws_security_group.integration_hub_sg.id]
  }
  
  tags = {
    Name = "rds-sg"
  }
}
```

### Ingress Configuration
```yaml
# k8s/ingress/integration-ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: integration-ingress
  namespace: integration
  annotations:
    kubernetes.io/ingress.class: "nginx"
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - integration.auto.ru
    secretName: integration-tls
  
  rules:
  - host: integration.auto.ru
    http:
      paths:
      - path: /api
        pathType: Prefix
        backend:
          service:
            name: integration-hub-service
            port:
              number: 80
      - path: /graphql
        pathType: Prefix
        backend:
          service:
            name: apollo-gateway-service
            port:
              number: 4000
      - path: /
        pathType: Prefix
        backend:
          service:
            name: integration-hub-service
            port:
              number: 80
```

## Мониторинг и алертинг

### Alerting Rules
```yaml
# k8s/monitoring/alert-rules.yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: integration-alerts
  namespace: monitoring
spec:
  groups:
  - name: integration.rules
    rules:
    - alert: IntegrationHubDown
      expr: up{job="integration-hub"} == 0
      for: 1m
      labels:
        severity: critical
        team: platform
      annotations:
        summary: "Integration Hub is down"
        description: "Integration Hub has been down for more than 1 minute"
        runbook_url: "https://runbooks.auto.ru/integration-hub-down"
    
    - alert: HighFederationLatency
      expr: histogram_quantile(0.95, rate(graphql_request_duration_seconds_bucket{job="apollo-gateway"}[5m])) > 2
      for: 5m
      labels:
        severity: warning
        team: platform
      annotations:
        summary: "High federation latency detected"
        description: "95th percentile latency is {{ $value }} seconds"
    
    - alert: SubgraphUnhealthy
      expr: subgraph_health_status == 0
      for: 2m
      labels:
        severity: critical
        team: platform
      annotations:
        summary: "Subgraph {{ $labels.subgraph_name }} is unhealthy"
        description: "Subgraph {{ $labels.subgraph_name }} has been unhealthy for more than 2 minutes"
```

## Выводы

Диаграмма развертывания Task 3 демонстрирует:

1. **Production-ready архитектуру** с высокой доступностью и отказоустойчивостью
2. **Облачную инфраструктуру** с управляемыми сервисами
3. **Комплексную систему мониторинга** и алертинга
4. **Безопасную сетевую архитектуру** с изоляцией компонентов
5. **Автоматическое масштабирование** и управление ресурсами
6. **Готовность к продакшену** с резервным копированием и восстановлением

Архитектура обеспечивает надежную эксплуатацию системы интеграции и мониторинга федеративной GraphQL системы в облачной среде.