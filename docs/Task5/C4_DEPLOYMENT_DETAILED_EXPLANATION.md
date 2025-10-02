# Task 5: Deployment Diagram - Подробное объяснение AI-driven production инфраструктуры

## 🎯 Цель диаграммы

Deployment диаграмма Task 5 демонстрирует **production-ready развертывание AI-driven федеративной системы**, показывая как ML компоненты интегрируются с облачной инфраструктурой для обеспечения высокой доступности, масштабируемости и производительности. Диаграмма служит руководством для DevOps команд по развертыванию интеллектуальной системы в production.

## ☁️ AWS Cloud AI Platform: Облачная ML инфраструктура

### Production VPC AI - Сетевая архитектура с AI оптимизацией

#### Terraform конфигурация для AI-enhanced VPC
```hcl
# infrastructure/terraform/vpc-ai.tf
resource "aws_vpc" "production_vpc_ai" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name        = "production-vpc-ai"
    Environment = "production"
    AIEnabled   = "true"
    Project     = "auto-ru-federation-ai"
  }
}

# Подсети для различных AI компонентов
resource "aws_subnet" "public_subnet_ai" {
  count             = 2
  vpc_id            = aws_vpc.production_vpc_ai.id
  cidr_block        = "10.0.${count.index + 1}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]
  
  map_public_ip_on_launch = true

  tags = {
    Name = "public-subnet-ai-${count.index + 1}"
    Type = "public"
    AIWorkload = "gateway-inference"
  }
}

resource "aws_subnet" "private_subnet_ai" {
  count             = 3
  vpc_id            = aws_vpc.production_vpc_ai.id
  cidr_block        = "10.0.${count.index + 10}.0/24"
  availability_zone = data.aws_availability_zones.available.names[count.index]

  tags = {
    Name = "private-subnet-ai-${count.index + 1}"
    Type = "private"
    AIWorkload = count.index == 0 ? "ml-inference" : count.index == 1 ? "ml-training" : "data-processing"
  }
}

# NAT Gateway для приватных подсетей с AI трафиком
resource "aws_nat_gateway" "ai_nat_gateway" {
  count         = 2
  allocation_id = aws_eip.ai_nat_eip[count.index].id
  subnet_id     = aws_subnet.public_subnet_ai[count.index].id

  tags = {
    Name = "ai-nat-gateway-${count.index + 1}"
    AIOptimized = "true"
  }
}

# Security Groups для AI компонентов
resource "aws_security_group" "ai_gateway_sg" {
  name_description = "Security group for AI Gateway"
  vpc_id          = aws_vpc.production_vpc_ai.id

  # HTTP/HTTPS трафик
  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # GraphQL порт
  ingress {
    from_port   = 4000
    to_port     = 4000
    protocol    = "tcp"
    cidr_blocks = [aws_vpc.production_vpc_ai.cidr_block]
  }

  # ML inference порты
  ingress {
    from_port   = 8080
    to_port     = 8090
    protocol    = "tcp"
    cidr_blocks = [aws_vpc.production_vpc_ai.cidr_block]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "ai-gateway-sg"
    Component = "ai-gateway"
  }
}
```

### Intelligent ALB - ML-enhanced Load Balancer

#### Application Load Balancer с AI маршрутизацией
```yaml
# kubernetes/manifests/alb-ai-controller.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alb-ai-config
  namespace: kube-system
data:
  ai-routing-config.json: |
    {
      "mlRoutingEnabled": true,
      "performancePredictionModel": {
        "endpoint": "http://model-server:8080/predict-latency",
        "timeout": 100,
        "fallbackStrategy": "round-robin"
      },
      "adaptiveWeights": {
        "enabled": true,
        "learningRate": 0.01,
        "updateInterval": 30
      },
      "healthCheckAI": {
        "enabled": true,
        "anomalyDetection": true,
        "predictiveFailover": true
      }
    }
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: alb-ai-controller
  namespace: kube-system
spec:
  replicas: 2
  selector:
    matchLabels:
      app: alb-ai-controller
  template:
    metadata:
      labels:
        app: alb-ai-controller
    spec:
      containers:
      - name: alb-ai-controller
        image: auto-ru/alb-ai-controller:v1.0.0
        ports:
        - containerPort: 8080
        env:
        - name: AWS_REGION
          value: "us-east-1"
        - name: ML_MODEL_ENDPOINT
          value: "http://model-server:8080"
        - name: PROMETHEUS_ENDPOINT
          value: "http://prometheus:9090"
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

## 🚀 EKS AI Cluster: Kubernetes с AI операторами

### AI-Enhanced Kubernetes Configuration

#### EKS кластер с ML оптимизациями
```yaml
# kubernetes/cluster/eks-ai-cluster.yaml
apiVersion: eksctl.io/v1alpha5
kind: ClusterConfig

metadata:
  name: auto-ru-ai-cluster
  region: us-east-1
  version: "1.28"

vpc:
  id: vpc-ai-production
  subnets:
    private:
      us-east-1a: { id: subnet-ai-private-1 }
      us-east-1b: { id: subnet-ai-private-2 }
      us-east-1c: { id: subnet-ai-private-3 }
    public:
      us-east-1a: { id: subnet-ai-public-1 }
      us-east-1b: { id: subnet-ai-public-2 }

# Node groups для различных AI workloads
nodeGroups:
  # Gateway nodes - оптимизированы для inference
  - name: ai-gateway-nodes
    instanceType: c5.2xlarge
    desiredCapacity: 3
    minSize: 2
    maxSize: 10
    volumeSize: 100
    volumeType: gp3
    labels:
      workload-type: "ai-gateway"
      node-class: "inference-optimized"
    taints:
      - key: "ai-gateway"
        value: "true"
        effect: "NoSchedule"
    tags:
      AIWorkload: "gateway-inference"
      AutoScaling: "enabled"

  # ML inference nodes - GPU enabled
  - name: ml-inference-nodes
    instanceType: g4dn.xlarge
    desiredCapacity: 2
    minSize: 1
    maxSize: 5
    volumeSize: 200
    volumeType: gp3
    labels:
      workload-type: "ml-inference"
      node-class: "gpu-enabled"
      nvidia.com/gpu: "true"
    taints:
      - key: "ml-inference"
        value: "true"
        effect: "NoSchedule"
    tags:
      AIWorkload: "ml-inference"
      GPUEnabled: "true"

  # Training nodes - высокопроизводительные
  - name: ml-training-nodes
    instanceType: p3.2xlarge
    desiredCapacity: 0
    minSize: 0
    maxSize: 3
    volumeSize: 500
    volumeType: gp3
    labels:
      workload-type: "ml-training"
      node-class: "training-optimized"
    taints:
      - key: "ml-training"
        value: "true"
        effect: "NoSchedule"
    tags:
      AIWorkload: "ml-training"
      SpotInstance: "true"

# Addons для AI workloads
addons:
  - name: vpc-cni
    version: latest
  - name: coredns
    version: latest
  - name: kube-proxy
    version: latest
  - name: aws-ebs-csi-driver
    version: latest
  - name: nvidia-device-plugin
    version: latest

# IAM роли для AI сервисов
iam:
  withOIDC: true
  serviceAccounts:
    - metadata:
        name: ai-gateway-sa
        namespace: default
      attachPolicyARNs:
        - arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess
        - arn:aws:iam::aws:policy/AmazonSageMakerReadOnly
      tags:
        Component: "ai-gateway"
    
    - metadata:
        name: ml-inference-sa
        namespace: default
      attachPolicyARNs:
        - arn:aws:iam::aws:policy/AmazonSageMakerFullAccess
        - arn:aws:iam::aws:policy/AmazonS3FullAccess
      tags:
        Component: "ml-inference"

# CloudWatch logging для AI метрик
cloudWatch:
  clusterLogging:
    enableTypes: ["api", "audit", "authenticator", "controllerManager", "scheduler"]
    logRetentionInDays: 30
```

### Apollo Gateway AI Pod - Интеллектуальный Gateway

#### Kubernetes Deployment с ML оптимизациями
```yaml
# kubernetes/deployments/apollo-gateway-ai.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apollo-gateway-ai
  namespace: default
  labels:
    app: apollo-gateway-ai
    component: ai-gateway
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
      app: apollo-gateway-ai
  template:
    metadata:
      labels:
        app: apollo-gateway-ai
        component: ai-gateway
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "4000"
        prometheus.io/path: "/metrics"
    spec:
      nodeSelector:
        workload-type: "ai-gateway"
      tolerations:
      - key: "ai-gateway"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
      
      serviceAccountName: ai-gateway-sa
      
      initContainers:
      # Инициализация ML моделей
      - name: model-downloader
        image: auto-ru/model-downloader:v1.0.0
        command:
        - /bin/sh
        - -c
        - |
          echo "Downloading ML models..."
          aws s3 sync s3://auto-ru-ml-models/gateway-ai/ /models/
          echo "Models downloaded successfully"
        volumeMounts:
        - name: ml-models
          mountPath: /models
        env:
        - name: AWS_REGION
          value: "us-east-1"
      
      containers:
      - name: apollo-gateway-ai
        image: auto-ru/apollo-gateway-ai:v1.0.0
        ports:
        - containerPort: 4000
          name: graphql
        - containerPort: 8080
          name: metrics
        
        env:
        # GraphQL конфигурация
        - name: GRAPHQL_PORT
          value: "4000"
        - name: GRAPHQL_PATH
          value: "/graphql"
        
        # ML модели конфигурация
        - name: ML_MODELS_PATH
          value: "/models"
        - name: PERFORMANCE_PREDICTOR_MODEL
          value: "/models/performance-predictor.json"
        - name: QUERY_CLASSIFIER_MODEL
          value: "/models/query-classifier.json"
        - name: ROUTING_OPTIMIZER_MODEL
          value: "/models/routing-optimizer.json"
        
        # AI сервисы endpoints
        - name: MODEL_SERVER_ENDPOINT
          value: "http://model-server:8080"
        - name: FEATURE_STORE_ENDPOINT
          value: "http://feature-store:8080"
        
        # Subgraph endpoints
        - name: USER_SUBGRAPH_URL
          value: "http://user-subgraph-ai:4001/graphql"
        - name: OFFER_SUBGRAPH_URL
          value: "http://offer-subgraph-ai:4002/graphql"
        - name: REVIEW_SUBGRAPH_URL
          value: "http://review-subgraph-ai:4003/graphql"
        
        # Мониторинг и логирование
        - name: LOG_LEVEL
          value: "info"
        - name: ENABLE_TRACING
          value: "true"
        - name: JAEGER_ENDPOINT
          value: "http://jaeger-collector:14268/api/traces"
        
        # AI конфигурация
        - name: AI_ENABLED
          value: "true"
        - name: ML_INFERENCE_TIMEOUT
          value: "100"
        - name: ADAPTIVE_ROUTING_ENABLED
          value: "true"
        - name: PERFORMANCE_PREDICTION_ENABLED
          value: "true"
        
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        
        volumeMounts:
        - name: ml-models
          mountPath: /models
          readOnly: true
        - name: config
          mountPath: /app/config
          readOnly: true
        
        livenessProbe:
          httpGet:
            path: /health
            port: 4000
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 10
          failureThreshold: 3
        
        readinessProbe:
          httpGet:
            path: /ready
            port: 4000
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 2
        
        # Graceful shutdown
        lifecycle:
          preStop:
            exec:
              command:
              - /bin/sh
              - -c
              - |
                echo "Gracefully shutting down Apollo Gateway AI..."
                # Дождаться завершения текущих запросов
                sleep 15
      
      volumes:
      - name: ml-models
        emptyDir: {}
      - name: config
        configMap:
          name: apollo-gateway-ai-config

---
apiVersion: v1
kind: Service
metadata:
  name: apollo-gateway-ai
  labels:
    app: apollo-gateway-ai
spec:
  type: ClusterIP
  ports:
  - port: 4000
    targetPort: 4000
    name: graphql
  - port: 8080
    targetPort: 8080
    name: metrics
  selector:
    app: apollo-gateway-ai

---
apiVersion: v1
kind: ConfigMap
metadata:
  name: apollo-gateway-ai-config
data:
  gateway-config.json: |
    {
      "ai": {
        "enabled": true,
        "models": {
          "performancePredictor": {
            "path": "/models/performance-predictor.json",
            "timeout": 100,
            "cacheSize": 1000
          },
          "queryClassifier": {
            "path": "/models/query-classifier.json",
            "timeout": 50,
            "cacheSize": 2000
          },
          "routingOptimizer": {
            "path": "/models/routing-optimizer.json",
            "learningRate": 0.01,
            "updateInterval": 300
          }
        },
        "features": {
          "adaptiveRouting": true,
          "performancePrediction": true,
          "queryOptimization": true,
          "anomalyDetection": true
        }
      },
      "federation": {
        "introspectionEnabled": false,
        "queryPlanCaching": true,
        "subscriptions": false
      },
      "caching": {
        "enabled": true,
        "redis": {
          "host": "redis-ai-primary",
          "port": 6379,
          "db": 0
        },
        "ttl": {
          "default": 300,
          "query": 60,
          "schema": 3600
        }
      }
    }
```

### Model Server - ML модели в production

#### TorchServe deployment для ML inference
```yaml
# kubernetes/deployments/model-server.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: model-server
  namespace: default
  labels:
    app: model-server
    component: ml-inference
spec:
  replicas: 2
  selector:
    matchLabels:
      app: model-server
  template:
    metadata:
      labels:
        app: model-server
        component: ml-inference
    spec:
      nodeSelector:
        workload-type: "ml-inference"
      tolerations:
      - key: "ml-inference"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
      
      serviceAccountName: ml-inference-sa
      
      initContainers:
      - name: model-loader
        image: auto-ru/model-loader:v1.0.0
        command:
        - /bin/sh
        - -c
        - |
          echo "Loading ML models from S3..."
          aws s3 sync s3://auto-ru-ml-models/inference/ /models/
          
          # Создание model store для TorchServe
          torch-model-archiver --model-name performance_predictor \
            --version 1.0 \
            --model-file /models/performance_predictor.py \
            --serialized-file /models/performance_predictor.pth \
            --handler /models/performance_predictor_handler.py \
            --export-path /model-store/
          
          torch-model-archiver --model-name query_classifier \
            --version 1.0 \
            --model-file /models/query_classifier.py \
            --serialized-file /models/query_classifier.pth \
            --handler /models/query_classifier_handler.py \
            --export-path /model-store/
          
          echo "Models loaded and archived successfully"
        volumeMounts:
        - name: model-store
          mountPath: /model-store
        - name: models-cache
          mountPath: /models
      
      containers:
      - name: torchserve
        image: pytorch/torchserve:0.8.2-gpu
        ports:
        - containerPort: 8080
          name: inference
        - containerPort: 8081
          name: management
        - containerPort: 8082
          name: metrics
        
        env:
        - name: TS_CONFIG_FILE
          value: "/config/config.properties"
        
        command:
        - torchserve
        - --start
        - --model-store=/model-store
        - --models=performance_predictor=performance_predictor.mar
        - --models=query_classifier=query_classifier.mar
        - --ts-config=/config/config.properties
        
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
            nvidia.com/gpu: 1
          limits:
            memory: "4Gi"
            cpu: "2000m"
            nvidia.com/gpu: 1
        
        volumeMounts:
        - name: model-store
          mountPath: /model-store
          readOnly: true
        - name: torchserve-config
          mountPath: /config
          readOnly: true
        - name: logs
          mountPath: /logs
        
        livenessProbe:
          httpGet:
            path: /ping
            port: 8080
          initialDelaySeconds: 120
          periodSeconds: 30
          timeoutSeconds: 10
        
        readinessProbe:
          httpGet:
            path: /ping
            port: 8080
          initialDelaySeconds: 60
          periodSeconds: 10
          timeoutSeconds: 5
      
      # Sidecar для мониторинга моделей
      - name: model-monitor
        image: auto-ru/model-monitor:v1.0.0
        ports:
        - containerPort: 9090
          name: monitor-metrics
        
        env:
        - name: TORCHSERVE_MANAGEMENT_API
          value: "http://localhost:8081"
        - name: PROMETHEUS_ENDPOINT
          value: "http://prometheus:9090"
        - name: MODEL_DRIFT_THRESHOLD
          value: "0.1"
        - name: PERFORMANCE_THRESHOLD
          value: "0.05"
        
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "200m"
        
        volumeMounts:
        - name: logs
          mountPath: /logs
          readOnly: true
      
      volumes:
      - name: model-store
        emptyDir: {}
      - name: models-cache
        emptyDir: {}
      - name: logs
        emptyDir: {}
      - name: torchserve-config
        configMap:
          name: torchserve-config

---
apiVersion: v1
kind: ConfigMap
metadata:
  name: torchserve-config
data:
  config.properties: |
    inference_address=http://0.0.0.0:8080
    management_address=http://0.0.0.0:8081
    metrics_address=http://0.0.0.0:8082
    
    # Performance settings
    number_of_netty_threads=8
    job_queue_size=1000
    number_of_gpu=1
    
    # Model settings
    default_workers_per_model=2
    max_workers=4
    batch_size=8
    max_batch_delay=100
    
    # Logging
    default_response_timeout=120
    unregister_model_timeout=120
    decode_input_request=true
    
    # Metrics
    enable_metrics_api=true
    metrics_format=prometheus
    
    # CORS
    cors_allowed_origin=*
    cors_allowed_methods=GET,POST,PUT,DELETE
    cors_allowed_headers=*
```

## 🗄️ Intelligent Storage Layer: AI-оптимизированные базы данных

### PostgreSQL AI - ML-enhanced база данных

#### RDS PostgreSQL с ML расширениями
```yaml
# kubernetes/statefulsets/postgres-ai.yaml
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: postgres-ai-cluster
  namespace: default
spec:
  instances: 3
  
  # PostgreSQL конфигурация с ML расширениями
  postgresql:
    parameters:
      # Основные параметры производительности
      shared_preload_libraries: "pg_stat_statements,auto_explain,pg_ml"
      max_connections: "200"
      shared_buffers: "256MB"
      effective_cache_size: "1GB"
      work_mem: "4MB"
      maintenance_work_mem: "64MB"
      
      # ML-specific параметры
      pg_ml.enabled: "on"
      pg_ml.model_cache_size: "128MB"
      pg_ml.inference_timeout: "5s"
      
      # Query optimization
      auto_explain.log_min_duration: "1s"
      auto_explain.log_analyze: "on"
      auto_explain.log_buffers: "on"
      
      # Статистика для ML анализа
      pg_stat_statements.track: "all"
      pg_stat_statements.max: "10000"
      pg_stat_statements.save: "on"
    
    # Инициализация ML расширений
    initdb:
      database: auto_ru_ai
      owner: auto_ru_user
      secret:
        name: postgres-credentials
    
    # Дополнительные базы данных
    databases:
      - name: ml_features
        owner: auto_ru_user
      - name: model_metadata
        owner: auto_ru_user
  
  # Мониторинг
  monitoring:
    enabled: true
    prometheusRule:
      enabled: true
    
  # Backup конфигурация
  backup:
    retentionPolicy: "30d"
    barmanObjectStore:
      destinationPath: "s3://auto-ru-postgres-backups"
      s3Credentials:
        accessKeyId:
          name: postgres-backup-credentials
          key: ACCESS_KEY_ID
        secretAccessKey:
          name: postgres-backup-credentials
          key: SECRET_ACCESS_KEY
      wal:
        retention: "7d"
      data:
        retention: "30d"
  
  # Ресурсы
  resources:
    requests:
      memory: "2Gi"
      cpu: "1000m"
    limits:
      memory: "4Gi"
      cpu: "2000m"
  
  # Хранилище
  storage:
    size: "100Gi"
    storageClass: "gp3-encrypted"
  
  # Affinity для распределения по AZ
  affinity:
    podAntiAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchLabels:
            cnpg.io/cluster: postgres-ai-cluster
        topologyKey: topology.kubernetes.io/zone

---
# ML функции и процедуры
apiVersion: v1
kind: ConfigMap
metadata:
  name: postgres-ml-functions
data:
  ml-functions.sql: |
    -- Создание схемы для ML функций
    CREATE SCHEMA IF NOT EXISTS ml_ops;
    
    -- Функция для предсказания производительности запросов
    CREATE OR REPLACE FUNCTION ml_ops.predict_query_performance(
        query_text TEXT,
        query_params JSONB DEFAULT '{}'::jsonb
    ) RETURNS TABLE(
        estimated_duration_ms FLOAT,
        estimated_memory_mb FLOAT,
        confidence_score FLOAT
    ) AS $$
    BEGIN
        -- Вызов ML модели через pg_ml расширение
        RETURN QUERY
        SELECT 
            (ml_predict('query_performance_model', 
                        ml_ops.extract_query_features(query_text, query_params))).duration_ms,
            (ml_predict('query_performance_model', 
                        ml_ops.extract_query_features(query_text, query_params))).memory_mb,
            (ml_predict('query_performance_model', 
                        ml_ops.extract_query_features(query_text, query_params))).confidence;
    END;
    $$ LANGUAGE plpgsql;
    
    -- Функция извлечения признаков из запроса
    CREATE OR REPLACE FUNCTION ml_ops.extract_query_features(
        query_text TEXT,
        query_params JSONB DEFAULT '{}'::jsonb
    ) RETURNS FLOAT[] AS $$
    DECLARE
        features FLOAT[];
        query_plan JSONB;
    BEGIN
        -- Получение плана выполнения
        EXECUTE 'EXPLAIN (FORMAT JSON) ' || query_text 
        INTO query_plan;
        
        -- Извлечение признаков из плана
        features := ARRAY[
            -- Структурные признаки
            (query_plan->'Plan'->>'Total Cost')::FLOAT,
            (query_plan->'Plan'->>'Plan Rows')::FLOAT,
            (query_plan->'Plan'->>'Plan Width')::FLOAT,
            
            -- Сложность запроса
            ml_ops.calculate_query_complexity(query_text),
            ml_ops.count_joins(query_text),
            ml_ops.count_subqueries(query_text),
            
            -- Параметры запроса
            jsonb_array_length(COALESCE(query_params, '{}'::jsonb))::FLOAT
        ];
        
        RETURN features;
    END;
    $$ LANGUAGE plpgsql;
    
    -- Автоматическая оптимизация индексов на основе ML
    CREATE OR REPLACE FUNCTION ml_ops.suggest_indexes()
    RETURNS TABLE(
        table_name TEXT,
        column_names TEXT[],
        index_type TEXT,
        expected_improvement FLOAT
    ) AS $$
    BEGIN
        -- Анализ статистики запросов и предложение индексов
        RETURN QUERY
        WITH query_stats AS (
            SELECT 
                schemaname,
                tablename,
                attname,
                n_distinct,
                correlation,
                most_common_vals
            FROM pg_stats
            WHERE schemaname NOT IN ('information_schema', 'pg_catalog')
        ),
        ml_suggestions AS (
            SELECT 
                qs.tablename,
                ARRAY[qs.attname] as columns,
                CASE 
                    WHEN qs.n_distinct > 1000 THEN 'btree'
                    WHEN qs.correlation < 0.1 THEN 'hash'
                    ELSE 'btree'
                END as idx_type,
                ml_predict('index_performance_model', 
                          ARRAY[qs.n_distinct, qs.correlation])::FLOAT as improvement
            FROM query_stats qs
            WHERE NOT EXISTS (
                SELECT 1 FROM pg_indexes pi 
                WHERE pi.tablename = qs.tablename 
                AND pi.indexdef LIKE '%' || qs.attname || '%'
            )
        )
        SELECT 
            ms.tablename,
            ms.columns,
            ms.idx_type,
            ms.improvement
        FROM ml_suggestions ms
        WHERE ms.improvement > 0.1
        ORDER BY ms.improvement DESC;
    END;
    $$ LANGUAGE plpgsql;
```

### Redis AI - Интеллектуальное кеширование

#### Redis с RedisAI для ML inference
```yaml
# kubernetes/deployments/redis-ai.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-ai-primary
  namespace: default
spec:
  serviceName: redis-ai-primary
  replicas: 1
  selector:
    matchLabels:
      app: redis-ai-primary
  template:
    metadata:
      labels:
        app: redis-ai-primary
        component: cache-ai
    spec:
      containers:
      - name: redis-ai
        image: redislabs/redisai:1.2.7-gpu
        ports:
        - containerPort: 6379
          name: redis
        
        command:
        - redis-server
        - /config/redis.conf
        
        env:
        - name: REDIS_PASSWORD
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: password
        
        resources:
          requests:
            memory: "2Gi"
            cpu: "500m"
          limits:
            memory: "4Gi"
            cpu: "1000m"
        
        volumeMounts:
        - name: redis-config
          mountPath: /config
        - name: redis-data
          mountPath: /data
        - name: ai-models
          mountPath: /models
        
        livenessProbe:
          exec:
            command:
            - redis-cli
            - --no-auth-warning
            - -a
            - $(REDIS_PASSWORD)
            - ping
          initialDelaySeconds: 30
          periodSeconds: 10
        
        readinessProbe:
          exec:
            command:
            - redis-cli
            - --no-auth-warning
            - -a
            - $(REDIS_PASSWORD)
            - ping
          initialDelaySeconds: 5
          periodSeconds: 5
      
      # Sidecar для загрузки AI моделей
      - name: model-loader
        image: auto-ru/redis-ai-loader:v1.0.0
        command:
        - /bin/sh
        - -c
        - |
          echo "Loading AI models into Redis..."
          
          # Загрузка модели предсказания TTL
          redis-cli -h localhost -a $REDIS_PASSWORD \
            AI.MODELSTORE cache_ttl_predictor TF CPU \
            BLOB < /models/cache_ttl_predictor.pb
          
          # Загрузка модели предсказания паттернов доступа
          redis-cli -h localhost -a $REDIS_PASSWORD \
            AI.MODELSTORE access_pattern_predictor TF CPU \
            BLOB < /models/access_pattern_predictor.pb
          
          # Загрузка модели персонализации кеша
          redis-cli -h localhost -a $REDIS_PASSWORD \
            AI.MODELSTORE cache_personalization TF CPU \
            BLOB < /models/cache_personalization.pb
          
          echo "AI models loaded successfully"
          
          # Мониторинг и переодическое обновление моделей
          while true; do
            sleep 3600  # Проверка каждый час
            # Проверка новых версий моделей и обновление при необходимости
          done
        
        env:
        - name: REDIS_PASSWORD
          valueFrom:
            secretKeyRef:
              name: redis-credentials
              key: password
        
        volumeMounts:
        - name: ai-models
          mountPath: /models
        
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "200m"
      
      initContainers:
      - name: model-downloader
        image: auto-ru/model-downloader:v1.0.0
        command:
        - /bin/sh
        - -c
        - |
          echo "Downloading Redis AI models..."
          aws s3 sync s3://auto-ru-ml-models/redis-ai/ /models/
          echo "Models downloaded successfully"
        
        volumeMounts:
        - name: ai-models
          mountPath: /models
      
      volumes:
      - name: redis-config
        configMap:
          name: redis-ai-config
      - name: ai-models
        emptyDir: {}
  
  volumeClaimTemplates:
  - metadata:
      name: redis-data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "gp3-encrypted"
      resources:
        requests:
          storage: 50Gi

---
apiVersion: v1
kind: ConfigMap
metadata:
  name: redis-ai-config
data:
  redis.conf: |
    # Основные настройки Redis
    bind 0.0.0.0
    port 6379
    requirepass ${REDIS_PASSWORD}
    
    # Память и производительность
    maxmemory 3gb
    maxmemory-policy allkeys-lru
    
    # Персистентность
    save 900 1
    save 300 10
    save 60 10000
    
    # RedisAI конфигурация
    loadmodule /usr/lib/redis/modules/redisai.so
    
    # AI-specific настройки
    # Размер пула потоков для AI операций
    ai-threads 4
    
    # Таймаут для AI операций
    ai-timeout 5000
    
    # Логирование AI операций
    loglevel notice
    logfile /data/redis-ai.log
    
    # Мониторинг
    latency-monitor-threshold 100
    
    # Сеть
    tcp-keepalive 300
    timeout 0
    
    # Клиенты
    maxclients 10000
    
    # Медленные запросы
    slowlog-log-slower-than 10000
    slowlog-max-len 128
```

## 🔍 AI Services Region: Managed AI сервисы

### SageMaker Integration - Managed ML платформа

#### SageMaker Endpoints для production inference
```python
# infrastructure/sagemaker/model_deployment.py
import boto3
import json
from datetime import datetime
from typing import Dict, List, Optional

class SageMakerAIDeployment:
    """
    Управление SageMaker endpoints для Auto.ru AI системы
    """
    
    def __init__(self, region_name: str = 'us-east-1'):
        self.sagemaker = boto3.client('sagemaker', region_name=region_name)
        self.runtime = boto3.client('sagemaker-runtime', region_name=region_name)
        
    def deploy_performance_predictor(
        self, 
        model_data_url: str,
        instance_type: str = 'ml.c5.xlarge',
        instance_count: int = 2
    ) -> str:
        """
        Развертывание модели предсказания производительности
        """
        model_name = f"performance-predictor-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
        endpoint_config_name = f"{model_name}-config"
        endpoint_name = f"{model_name}-endpoint"
        
        # Создание модели
        self.sagemaker.create_model(
            ModelName=model_name,
            PrimaryContainer={
                'Image': '763104351884.dkr.ecr.us-east-1.amazonaws.com/pytorch-inference:1.12.0-gpu-py38',
                'ModelDataUrl': model_data_url,
                'Environment': {
                    'SAGEMAKER_PROGRAM': 'inference.py',
                    'SAGEMAKER_SUBMIT_DIRECTORY': '/opt/ml/code',
                    'SAGEMAKER_CONTAINER_LOG_LEVEL': '20',
                    'SAGEMAKER_REGION': 'us-east-1',
                    'MODEL_NAME': 'performance_predictor',
                    'BATCH_SIZE': '32',
                    'MAX_SEQUENCE_LENGTH': '512'
                }
            },
            ExecutionRoleArn='arn:aws:iam::ACCOUNT:role/SageMakerExecutionRole',
            Tags=[
                {'Key': 'Project', 'Value': 'auto-ru-ai'},
                {'Key': 'Component', 'Value': 'performance-predictor'},
                {'Key': 'Environment', 'Value': 'production'}
            ]
        )
        
        # Конфигурация endpoint
        self.sagemaker.create_endpoint_config(
            EndpointConfigName=endpoint_config_name,
            ProductionVariants=[
                {
                    'VariantName': 'primary',
                    'ModelName': model_name,
                    'InitialInstanceCount': instance_count,
                    'InstanceType': instance_type,
                    'InitialVariantWeight': 1.0,
                    'AcceleratorType': 'ml.eia2.medium'  # Elastic Inference для оптимизации
                }
            ],
            DataCaptureConfig={
                'EnableCapture': True,
                'InitialSamplingPercentage': 10,
                'DestinationS3Uri': 's3://auto-ru-ml-data-capture/performance-predictor/',
                'CaptureOptions': [
                    {'CaptureMode': 'Input'},
                    {'CaptureMode': 'Output'}
                ]
            },
            Tags=[
                {'Key': 'Project', 'Value': 'auto-ru-ai'},
                {'Key': 'Component', 'Value': 'performance-predictor'}
            ]
        )
        
        # Создание endpoint
        self.sagemaker.create_endpoint(
            EndpointName=endpoint_name,
            EndpointConfigName=endpoint_config_name,
            Tags=[
                {'Key': 'Project', 'Value': 'auto-ru-ai'},
                {'Key': 'Component', 'Value': 'performance-predictor'},
                {'Key': 'AutoScaling', 'Value': 'enabled'}
            ]
        )
        
        # Настройка автомасштабирования
        self._setup_auto_scaling(endpoint_name, 'primary')
        
        return endpoint_name
    
    def deploy_multi_model_endpoint(
        self,
        models: List[Dict[str, str]],
        instance_type: str = 'ml.c5.2xlarge'
    ) -> str:
        """
        Развертывание multi-model endpoint для нескольких AI моделей
        """
        endpoint_name = f"auto-ru-multi-model-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
        model_name = f"{endpoint_name}-model"
        endpoint_config_name = f"{endpoint_name}-config"
        
        # Создание multi-model
        self.sagemaker.create_model(
            ModelName=model_name,
            PrimaryContainer={
                'Image': '763104351884.dkr.ecr.us-east-1.amazonaws.com/pytorch-inference:1.12.0-gpu-py38',
                'Mode': 'MultiModel',
                'ModelDataUrl': 's3://auto-ru-ml-models/multi-model/',
                'Environment': {
                    'SAGEMAKER_PROGRAM': 'multi_model_inference.py',
                    'SAGEMAKER_SUBMIT_DIRECTORY': '/opt/ml/code',
                    'SAGEMAKER_MULTI_MODEL': 'true',
                    'SAGEMAKER_CONTAINER_LOG_LEVEL': '20'
                }
            },
            ExecutionRoleArn='arn:aws:iam::ACCOUNT:role/SageMakerExecutionRole'
        )
        
        # Конфигурация с A/B тестированием
        production_variants = []
        for i, model_config in enumerate(models):
            variant_name = model_config.get('name', f'variant-{i}')
            weight = model_config.get('weight', 1.0 / len(models))
            
            production_variants.append({
                'VariantName': variant_name,
                'ModelName': model_name,
                'InitialInstanceCount': 1,
                'InstanceType': instance_type,
                'InitialVariantWeight': weight
            })
        
        self.sagemaker.create_endpoint_config(
            EndpointConfigName=endpoint_config_name,
            ProductionVariants=production_variants,
            DataCaptureConfig={
                'EnableCapture': True,
                'InitialSamplingPercentage': 20,
                'DestinationS3Uri': 's3://auto-ru-ml-data-capture/multi-model/',
                'CaptureOptions': [
                    {'CaptureMode': 'Input'},
                    {'CaptureMode': 'Output'}
                ]
            }
        )
        
        # Создание endpoint
        self.sagemaker.create_endpoint(
            EndpointName=endpoint_name,
            EndpointConfigName=endpoint_config_name
        )
        
        return endpoint_name
    
    def _setup_auto_scaling(
        self, 
        endpoint_name: str, 
        variant_name: str,
        min_capacity: int = 1,
        max_capacity: int = 10
    ):
        """
        Настройка автомасштабирования для SageMaker endpoint
        """
        autoscaling = boto3.client('application-autoscaling')
        
        # Регистрация scalable target
        autoscaling.register_scalable_target(
            ServiceNamespace='sagemaker',
            ResourceId=f'endpoint/{endpoint_name}/variant/{variant_name}',
            ScalableDimension='sagemaker:variant:DesiredInstanceCount',
            MinCapacity=min_capacity,
            MaxCapacity=max_capacity,
            RoleArn='arn:aws:iam::ACCOUNT:role/SageMakerAutoScalingRole'
        )
        
        # Политика масштабирования на основе инвокаций
        autoscaling.put_scaling_policy(
            PolicyName=f'{endpoint_name}-invocations-scaling-policy',
            ServiceNamespace='sagemaker',
            ResourceId=f'endpoint/{endpoint_name}/variant/{variant_name}',
            ScalableDimension='sagemaker:variant:DesiredInstanceCount',
            PolicyType='TargetTrackingScaling',
            TargetTrackingScalingPolicyConfiguration={
                'TargetValue': 70.0,
                'PredefinedMetricSpecification': {
                    'PredefinedMetricType': 'SageMakerVariantInvocationsPerInstance'
                },
                'ScaleOutCooldown': 300,
                'ScaleInCooldown': 300
            }
        )
        
        # Политика масштабирования на основе латентности
        autoscaling.put_scaling_policy(
            PolicyName=f'{endpoint_name}-latency-scaling-policy',
            ServiceNamespace='sagemaker',
            ResourceId=f'endpoint/{endpoint_name}/variant/{variant_name}',
            ScalableDimension='sagemaker:variant:DesiredInstanceCount',
            PolicyType='TargetTrackingScaling',
            TargetTrackingScalingPolicyConfiguration={
                'TargetValue': 100.0,  # 100ms target latency
                'PredefinedMetricSpecification': {
                    'PredefinedMetricType': 'SageMakerVariantModelLatency'
                },
                'ScaleOutCooldown': 180,
                'ScaleInCooldown': 300
            }
        )

# Terraform конфигурация для SageMaker
# infrastructure/terraform/sagemaker.tf
resource "aws_sagemaker_model" "performance_predictor" {
  name               = "auto-ru-performance-predictor"
  execution_role_arn = aws_iam_role.sagemaker_execution_role.arn

  primary_container {
    image          = "763104351884.dkr.ecr.us-east-1.amazonaws.com/pytorch-inference:1.12.0-gpu-py38"
    model_data_url = "s3://auto-ru-ml-models/performance-predictor/model.tar.gz"
    
    environment = {
      SAGEMAKER_PROGRAM           = "inference.py"
      SAGEMAKER_SUBMIT_DIRECTORY  = "/opt/ml/code"
      MODEL_NAME                  = "performance_predictor"
      BATCH_SIZE                  = "32"
    }
  }

  tags = {
    Project     = "auto-ru-ai"
    Component   = "performance-predictor"
    Environment = "production"
  }
}

resource "aws_sagemaker_endpoint_configuration" "performance_predictor" {
  name = "auto-ru-performance-predictor-config"

  production_variants {
    variant_name           = "primary"
    model_name            = aws_sagemaker_model.performance_predictor.name
    initial_instance_count = 2
    instance_type         = "ml.c5.xlarge"
    initial_variant_weight = 1.0
    accelerator_type      = "ml.eia2.medium"
  }

  data_capture_config {
    enable_capture                = true
    initial_sampling_percentage   = 10
    destination_s3_uri           = "s3://auto-ru-ml-data-capture/performance-predictor/"
    
    capture_options {
      capture_mode = "Input"
    }
    
    capture_options {
      capture_mode = "Output"
    }
  }

  tags = {
    Project   = "auto-ru-ai"
    Component = "performance-predictor"
  }
}

resource "aws_sagemaker_endpoint" "performance_predictor" {
  name                 = "auto-ru-performance-predictor"
  endpoint_config_name = aws_sagemaker_endpoint_configuration.performance_predictor.name

  tags = {
    Project     = "auto-ru-ai"
    Component   = "performance-predictor"
    Environment = "production"
  }
}
```

## 🎯 Ключевые принципы Deployment архитектуры

### 1. Multi-Region AI Deployment
Географически распределенное развертывание для минимизации латентности:
- **Edge AI**: CloudFront + Lambda@Edge для inference на границе сети
- **Regional Models**: Локальные копии моделей в каждом регионе
- **Global Model Registry**: Централизованное управление версиями моделей

### 2. Auto-Scaling AI Workloads
Интеллектуальное масштабирование на основе ML предсказаний:
- **Predictive Scaling**: Предсказание нагрузки и проактивное масштабирование
- **GPU Resource Management**: Эффективное использование дорогих GPU ресурсов
- **Cost Optimization**: Автоматическое переключение между instance типами

### 3. ML Model Lifecycle Management
Полный жизненный цикл ML моделей в production:
- **Continuous Deployment**: Автоматический деплой новых версий моделей
- **A/B Testing**: Тестирование моделей на production трафике
- **Rollback Capabilities**: Быстрый откат к предыдущим версиям при проблемах

### 4. Observability и Monitoring
Комплексный мониторинг AI системы:
- **Model Performance**: Мониторинг качества и производительности моделей
- **Infrastructure Metrics**: Отслеживание ресурсов и производительности
- **Business Impact**: Измерение влияния AI на бизнес-метрики

### 5. Security и Compliance
Безопасность AI системы на всех уровнях:
- **Model Security**: Защита ML моделей от атак и утечек
- **Data Privacy**: Соблюдение требований по защите персональных данных
- **Access Control**: Ролевая модель доступа к AI ресурсам

Эта Deployment диаграмма демонстрирует enterprise-grade развертывание AI-driven системы, обеспечивающее высокую доступность, масштабируемость и производительность в production среде.