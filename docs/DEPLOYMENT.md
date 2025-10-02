# Руководство по развертыванию в продакшене / Production Deployment Guide

Этот документ предоставляет comprehensive руководство по развертыванию Apollo Router Federation в продакшн окружениях.

*This document provides comprehensive guidance for deploying the Apollo Router Federation to production environments.*

## 📋 Содержание / Table of Contents

1. [Обзор развертывания / Deployment Overview](#обзор-развертывания--deployment-overview)
2. [Предварительные требования / Prerequisites](#предварительные-требования--prerequisites)
3. [Настройка инфраструктуры / Infrastructure Setup](#настройка-инфраструктуры--infrastructure-setup)
4. [Развертывание приложения / Application Deployment](#развертывание-приложения--application-deployment)
5. [Мониторинг и алертинг / Monitoring and Alerting](#мониторинг-и-алертинг--monitoring-and-alerting)
6. [Конфигурация безопасности / Security Configuration](#конфигурация-безопасности--security-configuration)
7. [Оптимизация производительности / Performance Optimization](#оптимизация-производительности--performance-optimization)
8. [Резервное копирование и восстановление / Backup and Recovery](#резервное-копирование-и-восстановление--backup-and-recovery)
9. [Устранение неполадок / Troubleshooting](#устранение-неполадок--troubleshooting)
10. [Операционные процедуры / Operational Procedures](#операционные-процедуры--operational-procedures)

## Обзор развертывания / Deployment Overview

Apollo Router Federation развертывается из нескольких компонентов на Kubernetes:

*The Apollo Router Federation deployment consists of multiple components deployed on Kubernetes:*

```mermaid
graph TB
    subgraph "Продакшн окружение / Production Environment"
        subgraph "Kubernetes кластер / Kubernetes Cluster"
            Router[Apollo Router<br/>3 реплики / 3 replicas]
            UGC[UGC Subgraph<br/>3 реплики / 3 replicas]
            Users[Users Subgraph<br/>3 реплики / 3 replicas]
            Offers[Offers Subgraph<br/>3 реплики / 3 replicas]
            Catalog[Catalog Subgraph<br/>3 реплики / 3 replicas]
            Search[Search Subgraph<br/>3 реплики / 3 replicas]
        end
        
        subgraph "Слой данных / Data Layer"
            PostgreSQL[(PostgreSQL<br/>Primary + Replicas)]
            Redis[(Redis Cluster)]
            Elasticsearch[(Elasticsearch<br/>3 узла / 3 nodes)]
        end
        
        subgraph "Мониторинг / Monitoring"
            Prometheus[Prometheus]
            Grafana[Grafana]
            Jaeger[Jaeger]
        end
    end
    
    subgraph "Внешние сервисы / External Services"
        LB[Load Balancer]
        CDN[CDN]
        DNS[DNS]
    end
    
    LB --> Router
    CDN --> LB
    DNS --> CDN
    
    Router --> UGC
    Router --> Users
    Router --> Offers
    Router --> Catalog
    Router --> Search
    
    UGC --> PostgreSQL
    UGC --> Redis
    Search --> Elasticsearch
    
    Router --> Prometheus
    Prometheus --> Grafana
    Router --> Jaeger
```

### Стратегия развертывания / Deployment Strategy

- **Blue-Green Deployment**: Развертывание без простоя / Zero-downtime deployments
- **Rolling Updates**: Постепенное обновление с проверками здоровья / Gradual rollout with health checks
- **Canary Releases**: Постепенное переключение трафика для крупных изменений / Gradual traffic shifting for major changes
- **Автоматический откат / Automated Rollback**: Автоматический откат при обнаружении сбоев / Automatic rollback on failure detection

## Предварительные требования / Prerequisites

### Требования к инфраструктуре / Infrastructure Requirements

- **Kubernetes кластер / Kubernetes Cluster**: v1.24+
- **Требования к узлам / Node Requirements**: 
  - Минимум 3 узла / Minimum 3 nodes
  - 8 CPU ядер на узел / 8 CPU cores per node
  - 32GB RAM на узел / 32GB RAM per node
  - 100GB SSD хранилище на узел / 100GB SSD storage per node
- **Сеть / Network**: CNI плагин (Calico/Flannel) / CNI plugin (Calico/Flannel)
- **Хранилище / Storage**: Динамическое выделение с SSD storage class / Dynamic provisioning with SSD storage class

### Необходимые инструменты / Required Tools

```bash
# Установка необходимых инструментов / Install required tools
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Установка Helm / Install Helm
curl https://get.helm.sh/helm-v3.12.0-linux-amd64.tar.gz | tar xz
sudo mv linux-amd64/helm /usr/local/bin/

# Установка Docker / Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

### Требования к доступу / Access Requirements

- **Kubernetes кластер / Kubernetes Cluster**: Административный доступ / Admin access
- **Docker Registry**: Права на push/pull / Push/pull permissions
- **Управление DNS / DNS Management**: Возможность создания/изменения DNS записей / Ability to create/modify DNS records
- **SSL сертификаты / SSL Certificates**: Доступ к управлению сертификатами / Certificate management access

## Настройка инфраструктуры / Infrastructure Setup

### 1. Настройка Kubernetes кластера / Kubernetes Cluster Setup

```bash
# Создание namespace / Create namespace
kubectl create namespace auto-ru-federation

# Настройка RBAC / Set up RBAC
kubectl apply -f - << EOF
apiVersion: v1
kind: ServiceAccount
metadata:
  name: apollo-federation-sa
  namespace: auto-ru-federation
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: apollo-federation-role
rules:
- apiGroups: [""]
  resources: ["pods", "services", "endpoints"]
  verbs: ["get", "list", "watch"]
- apiGroups: ["apps"]
  resources: ["deployments"]
  verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: apollo-federation-binding
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name: apollo-federation-role
subjects:
- kind: ServiceAccount
  name: apollo-federation-sa
  namespace: auto-ru-federation
EOF
```

### 2. Конфигурация хранилища / Storage Configuration

```bash
# Создание storage class для высокопроизводительных рабочих нагрузок
# Create storage class for high-performance workloads
kubectl apply -f - << EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: fast-ssd
provisioner: kubernetes.io/aws-ebs  # Настройте для вашего облачного провайдера / Adjust for your cloud provider
parameters:
  type: gp3
  iops: "3000"
  throughput: "125"
volumeBindingMode: WaitForFirstConsumer
allowVolumeExpansion: true
EOF
```

### 3. Сетевые политики / Network Policies

```bash
# Создание сетевых политик для безопасности / Create network policies for security
kubectl apply -f - << EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: apollo-federation-netpol
  namespace: auto-ru-federation
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    - podSelector: {}
  egress:
  - to:
    - podSelector: {}
  - to: []
    ports:
    - protocol: TCP
      port: 53
    - protocol: UDP
      port: 53
  - to: []
    ports:
    - protocol: TCP
      port: 443
    - protocol: TCP
      port: 80
EOF
```

## Развертывание приложения / Application Deployment

### 1. Автоматизированное развертывание / Automated Deployment

```bash
# Установка переменных окружения / Set environment variables
export ENVIRONMENT=production
export NAMESPACE=auto-ru-federation
export DOCKER_REGISTRY=registry.auto.ru
export VERSION=v1.0.0

# Запуск скрипта развертывания / Run deployment script
./scripts/production/deploy-production.sh
```

### 2. Ручные шаги развертывания / Manual Deployment Steps

#### Шаг 1: Развертывание компонентов инфраструктуры / Step 1: Deploy Infrastructure Components

```bash
# Развертывание PostgreSQL / Deploy PostgreSQL
helm upgrade --install postgresql \
  oci://registry-1.docker.io/bitnamicharts/postgresql \
  --namespace auto-ru-federation \
  --values - << EOF
auth:
  postgresPassword: "secure-postgres-password"
primary:
  persistence:
    size: 100Gi
    storageClass: fast-ssd
  resources:
    requests:
      memory: 4Gi
      cpu: 2000m
    limits:
      memory: 8Gi
      cpu: 4000m
  configuration: |
    shared_buffers = 2GB
    effective_cache_size = 6GB
    work_mem = 256MB
    maintenance_work_mem = 1GB
    max_connections = 200
metrics:
  enabled: true
  serviceMonitor:
    enabled: true
EOF

# Развертывание Redis / Deploy Redis
helm upgrade --install redis \
  oci://registry-1.docker.io/bitnamicharts/redis \
  --namespace auto-ru-federation \
  --values - << EOF
auth:
  password: "secure-redis-password"
master:
  persistence:
    size: 50Gi
    storageClass: fast-ssd
  resources:
    requests:
      memory: 2Gi
      cpu: 1000m
    limits:
      memory: 4Gi
      cpu: 2000m
replica:
  replicaCount: 2
  persistence:
    size: 50Gi
    storageClass: fast-ssd
metrics:
  enabled: true
  serviceMonitor:
    enabled: true
EOF
```#### Ша
г 2: Развертывание сервисов приложения / Step 2: Deploy Application Services

```bash
# Развертывание UGC Subgraph / Deploy UGC Subgraph
kubectl apply -f - << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: auto-ru-federation
  labels:
    app: ugc-subgraph
    version: v1.0.0
spec:
  replicas: 3
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
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "4001"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: apollo-federation-sa
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: ugc-subgraph
        image: registry.auto.ru/ugc-subgraph:v1.0.0
        ports:
        - containerPort: 4001
          name: http
        env:
        - name: PORT
          value: "4001"
        - name: ENVIRONMENT
          value: "production"
        - name: LOG_LEVEL
          value: "info"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-credentials
              key: ugc-database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: redis-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: jwt-secret
              key: secret
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
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        volumeMounts:
        - name: tmp
          mountPath: /tmp
      volumes:
      - name: tmp
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: ugc-subgraph
  namespace: auto-ru-federation
  labels:
    app: ugc-subgraph
spec:
  selector:
    app: ugc-subgraph
  ports:
  - port: 4001
    targetPort: 4001
    name: http
  type: ClusterIP
EOF
```

#### Шаг 3: Развертывание Apollo Router / Step 3: Deploy Apollo Router

```bash
# Создание конфигурации роутера / Create router configuration
kubectl create configmap apollo-router-config \
  --from-file=router.yaml \
  --namespace=auto-ru-federation

# Развертывание Apollo Router / Deploy Apollo Router
kubectl apply -f - << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apollo-router
  namespace: auto-ru-federation
  labels:
    app: apollo-router
    version: v1.0.0
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
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "4000"
        prometheus.io/path: "/metrics"
    spec:
      serviceAccountName: apollo-federation-sa
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      containers:
      - name: apollo-router
        image: registry.auto.ru/apollo-router:v1.0.0
        ports:
        - containerPort: 4000
          name: http
        env:
        - name: APOLLO_ROUTER_CONFIG_PATH
          value: "/etc/router/router.yaml"
        - name: APOLLO_ROUTER_LOG
          value: "info"
        - name: APOLLO_ROUTER_SUPERGRAPH_PATH
          value: "/etc/router/supergraph.graphql"
        volumeMounts:
        - name: config
          mountPath: /etc/router
          readOnly: true
        - name: tmp
          mountPath: /tmp
        resources:
          requests:
            memory: 2Gi
            cpu: 1000m
          limits:
            memory: 4Gi
            cpu: 2000m
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
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
      volumes:
      - name: config
        configMap:
          name: apollo-router-config
      - name: tmp
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: apollo-router
  namespace: auto-ru-federation
  labels:
    app: apollo-router
spec:
  selector:
    app: apollo-router
  ports:
  - port: 4000
    targetPort: 4000
    name: http
  type: ClusterIP
EOF
```

#### Шаг 4: Настройка Ingress / Step 4: Configure Ingress

```bash
# Развертывание Ingress / Deploy Ingress
kubectl apply -f - << EOF
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: apollo-router-ingress
  namespace: auto-ru-federation
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/rate-limit: "1000"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
    nginx.ingress.kubernetes.io/proxy-body-size: "10m"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "60"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "60"
    nginx.ingress.kubernetes.io/enable-cors: "true"
    nginx.ingress.kubernetes.io/cors-allow-origin: "https://auto.ru,https://m.auto.ru"
    nginx.ingress.kubernetes.io/cors-allow-methods: "GET,POST,OPTIONS"
    nginx.ingress.kubernetes.io/cors-allow-headers: "DNT,User-Agent,X-Requested-With,If-Modified-Since,Cache-Control,Content-Type,Range,Authorization"
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
```

## Мониторинг и алертинг / Monitoring and Alerting

### 1. Конфигурация Prometheus / Prometheus Configuration

```bash
# Развертывание Prometheus Operator / Deploy Prometheus Operator
helm upgrade --install prometheus-operator \
  prometheus-community/kube-prometheus-stack \
  --namespace auto-ru-federation \
  --values - << EOF
prometheus:
  prometheusSpec:
    retention: 30d
    storageSpec:
      volumeClaimTemplate:
        spec:
          storageClassName: fast-ssd
          resources:
            requests:
              storage: 100Gi
    resources:
      requests:
        memory: 4Gi
        cpu: 2000m
      limits:
        memory: 8Gi
        cpu: 4000m
    additionalScrapeConfigs:
    - job_name: 'apollo-federation'
      kubernetes_sd_configs:
      - role: pod
        namespaces:
          names:
          - auto-ru-federation
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

grafana:
  adminPassword: "secure-grafana-password"
  persistence:
    enabled: true
    storageClassName: fast-ssd
    size: 10Gi
  resources:
    requests:
      memory: 1Gi
      cpu: 500m
    limits:
      memory: 2Gi
      cpu: 1000m

alertmanager:
  alertmanagerSpec:
    storage:
      volumeClaimTemplate:
        spec:
          storageClassName: fast-ssd
          resources:
            requests:
              storage: 10Gi
    resources:
      requests:
        memory: 512Mi
        cpu: 250m
      limits:
        memory: 1Gi
        cpu: 500m
EOF
```### 2. П
равила алертинга / Alert Rules

```bash
# Создание PrometheusRule для Apollo Federation / Create PrometheusRule for Apollo Federation
kubectl apply -f - << EOF
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: apollo-federation-alerts
  namespace: auto-ru-federation
  labels:
    app: apollo-federation
    prometheus: kube-prometheus
    role: alert-rules
spec:
  groups:
  - name: apollo-federation.rules
    interval: 30s
    rules:
    # Высокий уровень ошибок / High Error Rate
    - alert: ApolloFederationHighErrorRate
      expr: |
        (
          rate(graphql_errors_total[5m]) / 
          rate(graphql_requests_total[5m])
        ) > 0.05
      for: 2m
      labels:
        severity: critical
        service: apollo-federation
      annotations:
        summary: "Обнаружен высокий уровень ошибок GraphQL / High GraphQL error rate detected"
        description: "Уровень ошибок GraphQL составляет {{ \$value | humanizePercentage }} для {{ \$labels.instance }} / GraphQL error rate is {{ \$value | humanizePercentage }} for {{ \$labels.instance }}"
        runbook_url: "https://runbook.auto.ru/apollo-federation/high-error-rate"
    
    # Высокое время отклика / High Response Time
    - alert: ApolloFederationHighResponseTime
      expr: |
        histogram_quantile(0.95, 
          rate(graphql_request_duration_seconds_bucket[5m])
        ) > 1
      for: 5m
      labels:
        severity: warning
        service: apollo-federation
      annotations:
        summary: "Высокое время отклика GraphQL / High GraphQL response time"
        description: "95-й процентиль времени отклика составляет {{ \$value }}с для {{ \$labels.instance }} / 95th percentile response time is {{ \$value }}s for {{ \$labels.instance }}"
        runbook_url: "https://runbook.auto.ru/apollo-federation/high-response-time"
    
    # Сервис недоступен / Service Down
    - alert: ApolloFederationServiceDown
      expr: up{job="apollo-federation"} == 0
      for: 1m
      labels:
        severity: critical
        service: apollo-federation
      annotations:
        summary: "Сервис Apollo Federation недоступен / Apollo Federation service is down"
        description: "{{ \$labels.instance }} недоступен более 1 минуты / {{ \$labels.instance }} has been down for more than 1 minute"
        runbook_url: "https://runbook.auto.ru/apollo-federation/service-down"
    
    # Высокое использование памяти / High Memory Usage
    - alert: ApolloFederationHighMemoryUsage
      expr: |
        (
          process_resident_memory_bytes / 
          container_spec_memory_limit_bytes
        ) > 0.9
      for: 5m
      labels:
        severity: warning
        service: apollo-federation
      annotations:
        summary: "Обнаружено высокое использование памяти / High memory usage detected"
        description: "Использование памяти составляет {{ \$value | humanizePercentage }} для {{ \$labels.instance }} / Memory usage is {{ \$value | humanizePercentage }} for {{ \$labels.instance }}"
        runbook_url: "https://runbook.auto.ru/apollo-federation/high-memory-usage"
EOF
```

### 3. Трассировка Jaeger / Jaeger Tracing

```bash
# Развертывание Jaeger Operator / Deploy Jaeger Operator
kubectl apply -f https://github.com/jaegertracing/jaeger-operator/releases/download/v1.47.0/jaeger-operator.yaml

# Создание экземпляра Jaeger / Create Jaeger instance
kubectl apply -f - << EOF
apiVersion: jaegertracing.io/v1
kind: Jaeger
metadata:
  name: jaeger-production
  namespace: auto-ru-federation
spec:
  strategy: production
  storage:
    type: elasticsearch
    elasticsearch:
      nodeCount: 3
      resources:
        requests:
          memory: 4Gi
          cpu: 2000m
        limits:
          memory: 8Gi
          cpu: 4000m
      redundancyPolicy: SingleRedundancy
  collector:
    resources:
      requests:
        memory: 1Gi
        cpu: 500m
      limits:
        memory: 2Gi
        cpu: 1000m
  query:
    resources:
      requests:
        memory: 512Mi
        cpu: 250m
      limits:
        memory: 1Gi
        cpu: 500m
EOF
```

## Конфигурация безопасности / Security Configuration

### 1. Управление секретами / Secrets Management

```bash
# Создание секрета с учетными данными базы данных / Create database credentials secret
kubectl create secret generic database-credentials \
  --from-literal=ugc-database-url="postgresql://ugc_user:secure_password@postgresql:5432/ugc_db" \
  --from-literal=users-database-url="postgresql://users_user:secure_password@postgresql:5432/users_db" \
  --from-literal=offers-database-url="postgresql://offers_user:secure_password@postgresql:5432/offers_db" \
  --namespace=auto-ru-federation

# Создание секрета с учетными данными Redis / Create Redis credentials secret
kubectl create secret generic redis-credentials \
  --from-literal=redis-url="redis://:secure-redis-password@redis-master:6379" \
  --namespace=auto-ru-federation

# Создание JWT секрета / Create JWT secret
kubectl create secret generic jwt-secret \
  --from-literal=secret="your-super-secure-jwt-secret-key-here" \
  --namespace=auto-ru-federation
```

### 2. Стандарты безопасности Pod / Pod Security Standards

```bash
# Применение стандартов безопасности Pod / Apply Pod Security Standards
kubectl label namespace auto-ru-federation \
  pod-security.kubernetes.io/enforce=restricted \
  pod-security.kubernetes.io/audit=restricted \
  pod-security.kubernetes.io/warn=restricted
```

## Оптимизация производительности / Performance Optimization

### 1. Распределение ресурсов / Resource Allocation

```yaml
# Рекомендуемое распределение ресурсов / Recommended resource allocation
resources:
  apollo-router:
    requests:
      memory: 2Gi
      cpu: 1000m
    limits:
      memory: 4Gi
      cpu: 2000m
  
  subgraphs:
    requests:
      memory: 1Gi
      cpu: 500m
    limits:
      memory: 2Gi
      cpu: 1000m
  
  postgresql:
    requests:
      memory: 4Gi
      cpu: 2000m
    limits:
      memory: 8Gi
      cpu: 4000m
  
  redis:
    requests:
      memory: 2Gi
      cpu: 1000m
    limits:
      memory: 4Gi
      cpu: 2000m
```

### 2. Горизонтальное автомасштабирование Pod / Horizontal Pod Autoscaling

```bash
# Создание HPA для Apollo Router / Create HPA for Apollo Router
kubectl apply -f - << EOF
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: apollo-router-hpa
  namespace: auto-ru-federation
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: apollo-router
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
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
EOF
```

### 3. Бюджеты прерывания Pod / Pod Disruption Budgets

```bash
# Создание PDB для высокой доступности / Create PDB for high availability
kubectl apply -f - << EOF
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: apollo-router-pdb
  namespace: auto-ru-federation
spec:
  minAvailable: 2
  selector:
    matchLabels:
      app: apollo-router
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: subgraphs-pdb
  namespace: auto-ru-federation
spec:
  minAvailable: 1
  selector:
    matchExpressions:
    - key: app
      operator: In
      values: ["ugc-subgraph", "users-subgraph", "offers-subgraph", "catalog-subgraph", "search-subgraph"]
EOF
```#
# Резервное копирование и восстановление / Backup and Recovery

### 1. Резервное копирование баз данных / Database Backups

```bash
# Создание CronJob для резервного копирования / Create backup CronJob
kubectl apply -f - << EOF
apiVersion: batch/v1
kind: CronJob
metadata:
  name: postgresql-backup
  namespace: auto-ru-federation
spec:
  schedule: "0 2 * * *"  # Ежедневно в 2:00 / Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: postgres-backup
            image: postgres:15
            command:
            - /bin/bash
            - -c
            - |
              pg_dump -h postgresql -U postgres -d ugc_db > /backup/ugc_db_$(date +%Y%m%d_%H%M%S).sql
              pg_dump -h postgresql -U postgres -d users_db > /backup/users_db_$(date +%Y%m%d_%H%M%S).sql
              pg_dump -h postgresql -U postgres -d offers_db > /backup/offers_db_$(date +%Y%m%d_%H%M%S).sql
              # Загрузка в S3 или другое хранилище резервных копий / Upload to S3 or other backup storage
              aws s3 cp /backup/ s3://auto-ru-backups/postgresql/ --recursive
            env:
            - name: PGPASSWORD
              valueFrom:
                secretKeyRef:
                  name: database-credentials
                  key: postgres-password
            volumeMounts:
            - name: backup-storage
              mountPath: /backup
          volumes:
          - name: backup-storage
            emptyDir: {}
          restartPolicy: OnFailure
EOF
```

### 2. Резервное копирование конфигураций / Configuration Backups

```bash
# Резервное копирование ресурсов Kubernetes / Backup Kubernetes resources
kubectl get all,configmaps,secrets,ingress,networkpolicies \
  -n auto-ru-federation \
  -o yaml > k8s-backup-$(date +%Y%m%d).yaml

# Сохранение в системе контроля версий или хранилище резервных копий
# Store in version control or backup storage
git add k8s-backup-$(date +%Y%m%d).yaml
git commit -m "Резервная копия Kubernetes $(date +%Y%m%d) / Kubernetes backup $(date +%Y%m%d)"
git push origin main
```

## Устранение неполадок / Troubleshooting

### Распространенные проблемы и решения / Common Issues and Solutions

#### 1. Проблемы запуска Pod / Pod Startup Issues

```bash
# Проверка статуса pod / Check pod status
kubectl get pods -n auto-ru-federation

# Проверка логов pod / Check pod logs
kubectl logs -n auto-ru-federation deployment/apollo-router --tail=100

# Описание pod для событий / Describe pod for events
kubectl describe pod -n auto-ru-federation <pod-name>

# Проверка ограничений ресурсов / Check resource constraints
kubectl top pods -n auto-ru-federation
```

#### 2. Проблемы обнаружения сервисов / Service Discovery Issues

```bash
# Проверка endpoints сервисов / Check service endpoints
kubectl get endpoints -n auto-ru-federation

# Тестирование подключения к сервису / Test service connectivity
kubectl exec -it deployment/apollo-router -n auto-ru-federation -- \
  curl http://ugc-subgraph:4001/health

# Проверка разрешения DNS / Check DNS resolution
kubectl exec -it deployment/apollo-router -n auto-ru-federation -- \
  nslookup ugc-subgraph.auto-ru-federation.svc.cluster.local
```

#### 3. Проблемы подключения к базе данных / Database Connection Issues

```bash
# Проверка статуса pod базы данных / Check database pod status
kubectl get pods -n auto-ru-federation -l app.kubernetes.io/name=postgresql

# Проверка логов базы данных / Check database logs
kubectl logs -n auto-ru-federation postgresql-0 --tail=100

# Тестирование подключения к базе данных / Test database connectivity
kubectl exec -it postgresql-0 -n auto-ru-federation -- \
  psql -U postgres -c "SELECT version();"
```

#### 4. Проблемы производительности / Performance Issues

```bash
# Проверка использования ресурсов / Check resource usage
kubectl top pods -n auto-ru-federation
kubectl top nodes

# Проверка статуса HPA / Check HPA status
kubectl get hpa -n auto-ru-federation

# Просмотр метрик в Grafana / Review metrics in Grafana
# Проверка медленных запросов в базе данных / Check slow queries in database
kubectl exec -it postgresql-0 -n auto-ru-federation -- \
  psql -U postgres -c "SELECT query, mean_time, calls FROM pg_stat_statements ORDER BY mean_time DESC LIMIT 10;"
```

## Операционные процедуры / Operational Procedures

### 1. Чек-лист развертывания / Deployment Checklist

- [ ] **Перед развертыванием / Pre-deployment**
  - [ ] Все тесты проходят в CI/CD / All tests passing in CI/CD
  - [ ] Сканирование безопасности завершено / Security scan completed
  - [ ] Тесты производительности пройдены / Performance tests passed
  - [ ] Миграции базы данных проверены / Database migrations reviewed
  - [ ] План отката подготовлен / Rollback plan prepared
  - [ ] Заинтересованные стороны уведомлены / Stakeholders notified

- [ ] **Развертывание / Deployment**
  - [ ] Развертывание в staging сначала / Deploy to staging first
  - [ ] Запуск тестов staging / Run staging tests
  - [ ] Развертывание в продакшн / Deploy to production
  - [ ] Мониторинг прогресса развертывания / Monitor deployment progress
  - [ ] Проверка health checks / Verify health checks

- [ ] **После развертывания / Post-deployment**
  - [ ] Запуск smoke тестов / Run smoke tests
  - [ ] Проверка уровня ошибок / Check error rates
  - [ ] Мониторинг метрик производительности / Monitor performance metrics
  - [ ] Проверка бизнес-функциональности / Verify business functionality
  - [ ] Обновление документации / Update documentation

### 2. Процедуры отката / Rollback Procedures

```bash
# Автоматический откат (если скрипт развертывания обнаруживает проблемы)
# Automatic rollback (if deployment script detects issues)
./scripts/production/deploy-production.sh rollback

# Ручной откат / Manual rollback
kubectl rollout undo deployment/apollo-router -n auto-ru-federation
kubectl rollout undo deployment/ugc-subgraph -n auto-ru-federation
# Повторить для других сервисов / Repeat for other services

# Проверка статуса отката / Check rollback status
kubectl rollout status deployment/apollo-router -n auto-ru-federation
```

### 3. Процедуры масштабирования / Scaling Procedures

```bash
# Масштабирование вверх для высокого трафика / Scale up for high traffic
kubectl scale deployment apollo-router --replicas=10 -n auto-ru-federation
kubectl scale deployment ugc-subgraph --replicas=5 -n auto-ru-federation

# Масштабирование вниз во время низкого трафика / Scale down during low traffic
kubectl scale deployment apollo-router --replicas=3 -n auto-ru-federation
kubectl scale deployment ugc-subgraph --replicas=3 -n auto-ru-federation
```

### 4. Процедуры обслуживания / Maintenance Procedures

```bash
# Вывод узла из эксплуатации для обслуживания / Drain node for maintenance
kubectl drain <node-name> --ignore-daemonsets --delete-emptydir-data

# Обновление узла и возврат в работу / Update node and bring back online
kubectl uncordon <node-name>

# Перезапуск сервисов с обновлением / Rolling restart of services
kubectl rollout restart deployment/apollo-router -n auto-ru-federation
```

### 5. Аварийные процедуры / Emergency Procedures

Для критических проблем следуйте [Production Runbook](production-runbook.md):

*For critical issues, follow the [Production Runbook](production-runbook.md):*

1. **Немедленная реакция / Immediate Response**
   - Проверка статуса системы / Check system status
   - Идентификация затронутых компонентов / Identify affected components
   - Реализация немедленных исправлений / Implement immediate fixes

2. **Эскалация / Escalation**
   - Связь с дежурным инженером / Contact on-call engineer
   - Уведомление заинтересованных сторон / Notify stakeholders
   - Активация команды реагирования на инциденты / Activate incident response team

3. **Восстановление / Recovery**
   - Реализация исправлений / Implement fixes
   - Мониторинг восстановления системы / Monitor system recovery
   - Проведение post-incident review / Conduct post-incident review

---

## 📝 Заключение / Conclusion

Это руководство по развертыванию предоставляет comprehensive инструкции по развертыванию и эксплуатации Apollo Router Federation в продакшене. Регулярный пересмотр и обновление этой документации необходимы по мере развития системы.

*This deployment guide provides comprehensive instructions for deploying and operating the Apollo Router Federation in production. Regular reviews and updates of this documentation are essential as the system evolves.*

### 🎯 Ключевые особенности / Key Features

- **Двуязычная документация** - полная поддержка русского и английского языков / **Bilingual documentation** - full support for Russian and English languages
- **Production-ready конфигурации** - готовые к использованию YAML манифесты / **Production-ready configurations** - ready-to-use YAML manifests
- **Comprehensive мониторинг** - полная настройка observability stack / **Comprehensive monitoring** - complete observability stack setup
- **Безопасность по умолчанию** - все security best practices включены / **Security by default** - all security best practices included
- **Операционная готовность** - полные runbook процедуры / **Operational readiness** - complete runbook procedures

### 🚀 Практическая ценность / Practical Value

- **Готовность к продакшену** - все аспекты продакшн развертывания покрыты / **Production readiness** - all aspects of production deployment covered
- **Масштабируемость** - поддержка автомасштабирования и high availability / **Scalability** - support for autoscaling and high availability
- **Надежность** - comprehensive backup и disaster recovery процедуры / **Reliability** - comprehensive backup and disaster recovery procedures
- **Мониторинг** - полная observability с метриками, трейсами и логами / **Monitoring** - complete observability with metrics, traces, and logs