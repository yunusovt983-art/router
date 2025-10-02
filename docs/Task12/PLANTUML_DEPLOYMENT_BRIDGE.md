# Task 12: Deployment Diagram - Architecture to Code Bridge
## C4_ARCHITECTURE_DEPLOYMENT.puml - Мост между дизайном и реализацией

### Обзор диаграммы

Диаграмма развертывания Task 12 показывает физическое размещение компонентов системы в различных окружениях и их инфраструктурную реализацию. Каждый deployment node имеет конкретную реализацию в виде конфигурационных файлов, скриптов развертывания и инфраструктурного кода.

### Developer Machine Environment

#### Docker Desktop Runtime
**PlantUML элемент:**
```plantuml
Deployment_Node(docker_desktop, "Docker Desktop", "Container Runtime") {
    Container(local_ugc, "UGC Subgraph", "Rust/Docker", "Port 4001")
    Container(local_users, "Users Subgraph", "Rust/Docker", "Port 4002") 
    Container(local_offers, "Offers Subgraph", "Rust/Docker", "Port 4004")
    Container(local_router, "Apollo Router", "Node.js/Docker", "Port 4000")
    Container(local_postgres, "PostgreSQL", "Docker", "Port 5432")
    Container(local_redis, "Redis", "Docker", "Port 6379")
}
```

**Физическая реализация:**

##### System Requirements
```bash
# system-requirements.sh - проверка системных требований
#!/bin/bash

echo "🔍 Checking system requirements for Auto.ru GraphQL Federation..."

# Check Docker
if command -v docker &> /dev/null; then
    DOCKER_VERSION=$(docker --version | cut -d' ' -f3 | cut -d',' -f1)
    echo "✅ Docker: $DOCKER_VERSION"
else
    echo "❌ Docker not installed"
    exit 1
fi

# Check Docker Compose
if command -v docker-compose &> /dev/null; then
    COMPOSE_VERSION=$(docker-compose --version | cut -d' ' -f3 | cut -d',' -f1)
    echo "✅ Docker Compose: $COMPOSE_VERSION"
else
    echo "❌ Docker Compose not installed"
    exit 1
fi

# Check available resources
TOTAL_RAM=$(free -h | awk '/^Mem:/ {print $2}')
AVAILABLE_RAM=$(free -h | awk '/^Mem:/ {print $7}')
CPU_CORES=$(nproc)
DISK_SPACE=$(df -h . | awk 'NR==2 {print $4}')

echo "💻 System Resources:"
echo "   RAM: $AVAILABLE_RAM / $TOTAL_RAM available"
echo "   CPU Cores: $CPU_CORES"
echo "   Disk Space: $DISK_SPACE available"

# Minimum requirements check
RAM_GB=$(free -g | awk '/^Mem:/ {print $2}')
if [ "$RAM_GB" -lt 8 ]; then
    echo "⚠️  Warning: Minimum 8GB RAM recommended, found ${RAM_GB}GB"
fi

if [ "$CPU_CORES" -lt 4 ]; then
    echo "⚠️  Warning: Minimum 4 CPU cores recommended, found $CPU_CORES"
fi

echo "✅ System requirements check completed"
```

##### Docker Desktop Configuration
```json
// ~/.docker/daemon.json - Docker Desktop настройки
{
  "builder": {
    "gc": {
      "enabled": true,
      "defaultKeepStorage": "20GB"
    }
  },
  "experimental": false,
  "features": {
    "buildkit": true
  },
  "insecure-registries": [],
  "registry-mirrors": [],
  "storage-driver": "overlay2",
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  },
  "default-address-pools": [
    {
      "base": "172.20.0.0/16",
      "size": 24
    }
  ]
}
```

##### Local Resource Allocation
```yaml
# docker-compose.override.yml - локальные resource limits
version: '3.8'

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
    
  users-subgraph:
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
        reservations:
          cpus: '0.1'
          memory: 64M
    
  offers-subgraph:
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 256M
        reservations:
          cpus: '0.1'
          memory: 64M
    
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
    
  redis:
    deploy:
      resources:
        limits:
          cpus: '0.25'
          memory: 128M
        reservations:
          cpus: '0.05'
          memory: 32M
```

#### Development Tools Node
**PlantUML элемент:**
```plantuml
Deployment_Node(dev_tools, "Development Tools", "Native Applications") {
    Container(ide, "IDE/Editor", "VS Code/IntelliJ", "Code editing")
    Container(git_client, "Git Client", "Git", "Version control")
    Container(make_tool, "Make", "GNU Make", "Task automation")
}
```

**Tool Configuration реализация:**

##### VS Code Configuration
```json
// .vscode/settings.json - IDE настройки для проекта
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.checkOnSave.extraArgs": [
    "--all-targets",
    "--all-features"
  ],
  "rust-analyzer.cargo.buildScripts.enable": true,
  "rust-analyzer.procMacro.enable": true,
  
  // Docker integration
  "docker.defaultRegistryPath": "ghcr.io/auto-ru",
  "docker.commands.build": "${containerCommand} build -f ${dockerfile} -t ${tag} ${context}",
  
  // File watching exclusions
  "files.watcherExclude": {
    "**/target/**": true,
    "**/.git/**": true,
    "**/node_modules/**": true,
    "**/data/**": true
  },
  
  // GraphQL support
  "graphql-config.load.configName": "graphql",
  "graphql.useSchemaFileForCompletion": true,
  
  // Formatting
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.eslint": true
  },
  
  // Terminal integration
  "terminal.integrated.env.linux": {
    "RUST_LOG": "debug",
    "RUST_BACKTRACE": "1"
  }
}
```

```json
// .vscode/tasks.json - VS Code tasks
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Start Development Environment",
      "type": "shell",
      "command": "make",
      "args": ["dev"],
      "group": "build",
      "presentation": {
        "echo": true,
        "reveal": "always",
        "focus": false,
        "panel": "shared"
      },
      "problemMatcher": []
    },
    {
      "label": "Run Tests",
      "type": "shell",
      "command": "cargo",
      "args": ["test", "--all-features", "--workspace"],
      "group": "test",
      "presentation": {
        "echo": true,
        "reveal": "always",
        "focus": false,
        "panel": "shared"
      }
    },
    {
      "label": "Check Health",
      "type": "shell",
      "command": "make",
      "args": ["health"],
      "group": "build"
    },
    {
      "label": "View Logs",
      "type": "shell",
      "command": "make",
      "args": ["logs"],
      "group": "build"
    }
  ]
}
```

##### Git Configuration
```bash
# .gitconfig - проект-специфичная Git конфигурация
[core]
    autocrlf = input
    filemode = false
    
[push]
    default = current
    followTags = true
    
[pull]
    rebase = true
    
[branch]
    autosetupmerge = always
    autosetuprebase = always
    
[alias]
    co = checkout
    br = branch
    ci = commit
    st = status
    unstage = reset HEAD --
    last = log -1 HEAD
    visual = !gitk
    
    # Development workflow aliases
    dev-setup = !make dev
    dev-test = !make test
    dev-health = !make health
    
[commit]
    template = .gitmessage
    
[merge]
    tool = vscode
    
[diff]
    tool = vscode
```

```bash
# .gitmessage - commit message template
# <type>(<scope>): <subject>
#
# <body>
#
# <footer>

# Type должен быть одним из:
# feat: новая функциональность
# fix: исправление бага
# docs: изменения в документации
# style: форматирование, отсутствующие точки с запятой и т.д.
# refactor: рефакторинг кода
# test: добавление тестов
# chore: обновление задач сборки, настроек и т.д.

# Scope (опционально):
# ugc, users, offers, router, ci, docs

# Subject:
# - используйте повелительное наклонение: "change", а не "changed" или "changes"
# - не используйте заглавную букву в начале
# - не ставьте точку в конце

# Body (опционально):
# - используйте повелительное наклонение
# - включите мотивацию для изменения и сравните с предыдущим поведением

# Footer (опционально):
# - ссылки на issues: "Closes #123, #456"
# - BREAKING CHANGE: описание breaking changes
```

### GitHub Cloud Infrastructure

#### GitHub Actions Runtime Environment
**PlantUML элемент:**
```plantuml
Deployment_Node(github_actions, "GitHub Actions", "Serverless CI/CD") {
    Container(ci_runner, "CI Runner", "Ubuntu VM", "Testing and building")
    Container(build_runner, "Build Runner", "Ubuntu VM", "Docker image building")
    Container(deploy_runner, "Deploy Runner", "Ubuntu VM", "Deployment automation")
}
```

**GitHub Actions Infrastructure реализация:**

##### Runner Configuration
```yaml
# .github/workflows/runner-config.yml - runner specifications
name: Runner Configuration

# Runner specifications соответствуют архитектурным требованиям
defaults:
  run:
    shell: bash

env:
  # Global environment variables
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1
  DOCKER_BUILDKIT: 1
  COMPOSE_DOCKER_CLI_BUILD: 1

jobs:
  setup-runner:
    runs-on: ubuntu-latest
    
    # Runner specifications
    # CPU: 2-core x86_64
    # RAM: 7 GB
    # Storage: 14 GB SSD
    # Network: High-speed internet
    
    steps:
      - name: System Information
        run: |
          echo "🖥️ Runner System Information:"
          echo "OS: $(lsb_release -d | cut -f2)"
          echo "Kernel: $(uname -r)"
          echo "CPU: $(nproc) cores"
          echo "RAM: $(free -h | awk '/^Mem:/ {print $2}')"
          echo "Disk: $(df -h / | awk 'NR==2 {print $4}') available"
          echo "Docker: $(docker --version)"
          echo "Docker Compose: $(docker-compose --version)"
      
      - name: Setup Rust Environment
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          profile: minimal
          override: true
          components: rustfmt, clippy
      
      - name: Setup Node.js Environment
        uses: actions/setup-node@v3
        with:
          node-version: '18'
          cache: 'npm'
      
      - name: Setup Docker Buildx
        uses: docker/setup-buildx-action@v3
        with:
          platforms: linux/amd64,linux/arm64
      
      - name: Cache Setup
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

##### Build Matrix Configuration
```yaml
# .github/workflows/build-matrix.yml - multi-service build strategy
name: Build Matrix

on:
  workflow_call:
    inputs:
      services:
        required: true
        type: string
        default: '["ugc-subgraph", "users-subgraph", "offers-subgraph", "apollo-router"]'
      platforms:
        required: true
        type: string
        default: '["linux/amd64", "linux/arm64"]'

jobs:
  build-matrix:
    runs-on: ubuntu-latest
    timeout-minutes: 45
    
    strategy:
      fail-fast: false
      matrix:
        service: ${{ fromJson(inputs.services) }}
        platform: ${{ fromJson(inputs.platforms) }}
        include:
          # Service-specific configurations
          - service: ugc-subgraph
            dockerfile: ugc-subgraph/Dockerfile
            context: .
            build-args: |
              CARGO_FEATURES=default
              RUST_VERSION=1.75
          
          - service: users-subgraph
            dockerfile: users-subgraph/Dockerfile
            context: .
            build-args: |
              CARGO_FEATURES=default
              RUST_VERSION=1.75
          
          - service: offers-subgraph
            dockerfile: offers-subgraph/Dockerfile
            context: .
            build-args: |
              CARGO_FEATURES=default
              RUST_VERSION=1.75
          
          - service: apollo-router
            dockerfile: apollo-router/Dockerfile
            context: .
            build-args: |
              NODE_VERSION=18
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
        with:
          platforms: ${{ matrix.platform }}
      
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ghcr.io/auto-ru/${{ matrix.service }}
          tags: |
            type=ref,event=branch
            type=ref,event=pr
            type=sha,prefix={{branch}}-
            type=raw,value=latest,enable={{is_default_branch}}
      
      - name: Build and export
        uses: docker/build-push-action@v5
        with:
          context: ${{ matrix.context }}
          file: ${{ matrix.dockerfile }}
          platforms: ${{ matrix.platform }}
          build-args: ${{ matrix.build-args }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha,scope=${{ matrix.service }}-${{ matrix.platform }}
          cache-to: type=gha,mode=max,scope=${{ matrix.service }}-${{ matrix.platform }}
          outputs: type=docker,dest=/tmp/${{ matrix.service }}-${{ matrix.platform }}.tar
      
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: docker-images
          path: /tmp/${{ matrix.service }}-${{ matrix.platform }}.tar
```

#### GitHub Container Registry
**PlantUML элемент:**
```plantuml
Deployment_Node(github_registry, "GitHub Container Registry", "Container Storage") {
    Container(image_ugc, "UGC Image", "Docker Image", "auto-ru/ugc:tag")
    Container(image_users, "Users Image", "Docker Image", "auto-ru/users:tag")
    Container(image_offers, "Offers Image", "Docker Image", "auto-ru/offers:tag")
}
```

**Registry Management реализация:**

##### Registry Configuration
```yaml
# .github/workflows/registry-management.yml
name: Container Registry Management

on:
  push:
    branches: [main, develop]
  release:
    types: [published]

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: auto-ru

jobs:
  push-to-registry:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      
      - name: Log in to Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}/${{ matrix.service }}
          tags: |
            # Branch-based tags
            type=ref,event=branch
            type=ref,event=pr
            
            # Semantic versioning
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            
            # SHA-based tags
            type=sha,prefix={{branch}}-,format=short
            
            # Latest tag for main branch
            type=raw,value=latest,enable={{is_default_branch}}
            
            # Date-based tags
            type=schedule,pattern={{date 'YYYYMMDD'}}
      
      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./${{ matrix.service }}/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          platforms: linux/amd64,linux/arm64
          cache-from: type=gha
          cache-to: type=gha,mode=max
      
      - name: Generate SBOM
        uses: anchore/sbom-action@v0
        with:
          image: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}/${{ matrix.service }}:${{ github.sha }}
          format: spdx-json
          output-file: sbom-${{ matrix.service }}.spdx.json
      
      - name: Upload SBOM
        uses: actions/upload-artifact@v3
        with:
          name: sbom-${{ matrix.service }}
          path: sbom-${{ matrix.service }}.spdx.json
```

##### Registry Cleanup
```yaml
# .github/workflows/registry-cleanup.yml
name: Registry Cleanup

on:
  schedule:
    - cron: '0 2 * * 0'  # Weekly on Sunday at 2 AM
  workflow_dispatch:

jobs:
  cleanup:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    
    strategy:
      matrix:
        service: [ugc-subgraph, users-subgraph, offers-subgraph, apollo-router]
    
    steps:
      - name: Delete old images
        uses: actions/delete-package-versions@v4
        with:
          package-name: ${{ matrix.service }}
          package-type: container
          min-versions-to-keep: 10
          delete-only-untagged-versions: true
```

### AWS Cloud Infrastructure

#### Staging Environment (AWS EKS)
**PlantUML элемент:**
```plantuml
Deployment_Node(aws_staging, "Staging Environment", "AWS EKS Cluster") {
    Deployment_Node(staging_nodes, "EKS Worker Nodes", "EC2 t3.medium") {
        Container(staging_ugc, "UGC Pod", "Kubernetes Pod", "Replicas: 2")
        Container(staging_users, "Users Pod", "Kubernetes Pod", "Replicas: 1")
        Container(staging_offers, "Offers Pod", "Kubernetes Pod", "Replicas: 1")
        Container(staging_router, "Router Pod", "Kubernetes Pod", "Replicas: 2")
    }
}
```

**AWS EKS Staging реализация:**

##### Terraform EKS Configuration
```hcl
# terraform/staging/eks.tf - EKS cluster configuration
terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.20"
    }
  }
}

# EKS Cluster
resource "aws_eks_cluster" "staging" {
  name     = "auto-ru-staging"
  role_arn = aws_iam_role.eks_cluster.arn
  version  = "1.28"

  vpc_config {
    subnet_ids              = aws_subnet.private[*].id
    endpoint_private_access = true
    endpoint_public_access  = true
    public_access_cidrs     = ["0.0.0.0/0"]
    
    security_group_ids = [aws_security_group.eks_cluster.id]
  }

  # Logging configuration
  enabled_cluster_log_types = [
    "api",
    "audit",
    "authenticator",
    "controllerManager",
    "scheduler"
  ]

  # Encryption configuration
  encryption_config {
    provider {
      key_arn = aws_kms_key.eks.arn
    }
    resources = ["secrets"]
  }

  depends_on = [
    aws_iam_role_policy_attachment.eks_cluster_policy,
    aws_iam_role_policy_attachment.eks_vpc_resource_controller,
  ]

  tags = {
    Environment = "staging"
    Project     = "auto-ru-federation"
    ManagedBy   = "terraform"
  }
}

# EKS Node Group
resource "aws_eks_node_group" "staging_workers" {
  cluster_name    = aws_eks_cluster.staging.name
  node_group_name = "staging-workers"
  node_role_arn   = aws_iam_role.eks_node_group.arn
  subnet_ids      = aws_subnet.private[*].id

  # Instance configuration - соответствует архитектурным требованиям
  instance_types = ["t3.medium"]
  capacity_type  = "ON_DEMAND"
  disk_size      = 50

  # Scaling configuration
  scaling_config {
    desired_size = 3
    max_size     = 5
    min_size     = 2
  }

  # Update configuration
  update_config {
    max_unavailable_percentage = 25
  }

  # Launch template
  launch_template {
    id      = aws_launch_template.eks_nodes.id
    version = aws_launch_template.eks_nodes.latest_version
  }

  # Labels
  labels = {
    Environment = "staging"
    NodeType    = "worker"
  }

  # Taints for workload isolation
  taint {
    key    = "workload-type"
    value  = "web-services"
    effect = "NO_SCHEDULE"
  }

  depends_on = [
    aws_iam_role_policy_attachment.eks_worker_node_policy,
    aws_iam_role_policy_attachment.eks_cni_policy,
    aws_iam_role_policy_attachment.eks_container_registry_policy,
  ]

  tags = {
    Environment = "staging"
    Project     = "auto-ru-federation"
  }
}

# Launch Template for EKS Nodes
resource "aws_launch_template" "eks_nodes" {
  name_prefix   = "auto-ru-staging-"
  image_id      = data.aws_ami.eks_worker.id
  instance_type = "t3.medium"

  vpc_security_group_ids = [aws_security_group.eks_nodes.id]

  user_data = base64encode(templatefile("${path.module}/user-data.sh", {
    cluster_name        = aws_eks_cluster.staging.name
    cluster_endpoint    = aws_eks_cluster.staging.endpoint
    cluster_ca          = aws_eks_cluster.staging.certificate_authority[0].data
    bootstrap_arguments = "--container-runtime containerd"
  }))

  block_device_mappings {
    device_name = "/dev/xvda"
    ebs {
      volume_size           = 50
      volume_type          = "gp3"
      iops                 = 3000
      throughput           = 125
      encrypted            = true
      delete_on_termination = true
    }
  }

  monitoring {
    enabled = true
  }

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name        = "auto-ru-staging-worker"
      Environment = "staging"
    }
  }
}
```

##### Kubernetes Deployment Manifests
```yaml
# k8s/staging/ugc-deployment.yaml - UGC service deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: staging
  labels:
    app: ugc-subgraph
    version: v1
    environment: staging
spec:
  replicas: 2  # Соответствует архитектурной диаграмме
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
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9001"
        prometheus.io/path: "/metrics"
    
    spec:
      serviceAccountName: ugc-service-account
      
      # Security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      
      # Anti-affinity для распределения по узлам
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
              topologyKey: kubernetes.io/hostname
      
      # Tolerations для node taints
      tolerations:
      - key: workload-type
        operator: Equal
        value: web-services
        effect: NoSchedule
      
      containers:
      - name: ugc-subgraph
        image: ghcr.io/auto-ru/ugc-subgraph:latest
        imagePullPolicy: Always
        
        ports:
        - name: http
          containerPort: 4001
          protocol: TCP
        - name: metrics
          containerPort: 9001
          protocol: TCP
        
        # Resource allocation - соответствует архитектурным требованиям
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
        - name: RUST_BACKTRACE
          value: "0"
        - name: ENVIRONMENT
          value: "staging"
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
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: ugc-secrets
              key: jwt-secret
        
        # Health checks
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
          successThreshold: 1
        
        readinessProbe:
          httpGet:
            path: /ready
            port: http
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
          successThreshold: 1
        
        # Startup probe для медленного старта
        startupProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 30
        
        # Security context
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        
        # Volume mounts
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
      
      # DNS configuration
      dnsPolicy: ClusterFirst
      dnsConfig:
        options:
        - name: ndots
          value: "2"
        - name: edns0

---
apiVersion: v1
kind: Service
metadata:
  name: ugc-subgraph
  namespace: staging
  labels:
    app: ugc-subgraph
spec:
  type: ClusterIP
  ports:
  - name: http
    port: 4001
    targetPort: http
    protocol: TCP
  - name: metrics
    port: 9001
    targetPort: metrics
    protocol: TCP
  selector:
    app: ugc-subgraph

---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: ugc-service-account
  namespace: staging
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/UGCServiceRole
```

##### Staging Data Layer
```hcl
# terraform/staging/rds.tf - PostgreSQL RDS configuration
resource "aws_db_instance" "staging_postgres" {
  identifier = "auto-ru-staging-postgres"
  
  # Engine configuration
  engine         = "postgres"
  engine_version = "14.9"
  instance_class = "db.t3.micro"  # Соответствует архитектурным требованиям
  
  # Storage configuration
  allocated_storage     = 20
  max_allocated_storage = 100
  storage_type         = "gp3"
  storage_encrypted    = true
  kms_key_id          = aws_kms_key.rds.arn
  
  # Database configuration
  db_name  = "ugc_db"
  username = "ugc_user"
  password = random_password.db_password.result
  
  # Network configuration
  vpc_security_group_ids = [aws_security_group.rds_staging.id]
  db_subnet_group_name   = aws_db_subnet_group.staging.name
  publicly_accessible    = false
  
  # Backup configuration
  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  copy_tags_to_snapshot  = true
  delete_automated_backups = false
  
  # Maintenance configuration
  maintenance_window         = "sun:04:00-sun:05:00"
  auto_minor_version_upgrade = true
  
  # Monitoring configuration
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_monitoring.arn
  
  # Performance Insights
  performance_insights_enabled          = true
  performance_insights_retention_period = 7
  
  # Logging
  enabled_cloudwatch_logs_exports = ["postgresql", "upgrade"]
  
  # Deletion protection
  deletion_protection = false  # staging environment
  skip_final_snapshot = true   # staging environment
  
  tags = {
    Name        = "auto-ru-staging-postgres"
    Environment = "staging"
    Service     = "auto-ru-federation"
  }
}

# ElastiCache Redis configuration
resource "aws_elasticache_replication_group" "staging_redis" {
  replication_group_id       = "auto-ru-staging-redis"
  description                = "Redis cluster for staging environment"
  
  # Node configuration - соответствует архитектурным требованиям
  node_type                  = "cache.t3.micro"
  port                       = 6379
  parameter_group_name       = "default.redis7"
  
  # Cluster configuration
  num_cache_clusters         = 2
  automatic_failover_enabled = true
  multi_az_enabled          = true
  
  # Network configuration
  subnet_group_name = aws_elasticache_subnet_group.staging.name
  security_group_ids = [aws_security_group.elasticache_staging.id]
  
  # Backup configuration
  snapshot_retention_limit = 3
  snapshot_window         = "03:00-05:00"
  
  # Maintenance configuration
  maintenance_window = "sun:05:00-sun:07:00"
  
  # Encryption
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  
  # Logging
  log_delivery_configuration {
    destination      = aws_cloudwatch_log_group.redis_slow_log.name
    destination_type = "cloudwatch-logs"
    log_format      = "text"
    log_type        = "slow-log"
  }
  
  tags = {
    Name        = "auto-ru-staging-redis"
    Environment = "staging"
    Service     = "auto-ru-federation"
  }
}
```

#### Production Environment (AWS EKS)
**PlantUML элемент:**
```plantuml
Deployment_Node(aws_production, "Production Environment", "AWS EKS Cluster") {
    Deployment_Node(prod_nodes, "EKS Worker Nodes", "EC2 c5.large") {
        Container(prod_ugc, "UGC Pod", "Kubernetes Pod", "Replicas: 3")
        Container(prod_users, "Users Pod", "Kubernetes Pod", "Replicas: 2")
        Container(prod_offers, "Offers Pod", "Kubernetes Pod", "Replicas: 2")
        Container(prod_router, "Router Pod", "Kubernetes Pod", "Replicas: 3")
    }
}
```

**Production Infrastructure реализация:**

##### Production EKS Configuration
```hcl
# terraform/production/eks.tf - Production EKS cluster
resource "aws_eks_cluster" "production" {
  name     = "auto-ru-production"
  role_arn = aws_iam_role.eks_cluster.arn
  version  = "1.28"

  vpc_config {
    subnet_ids              = aws_subnet.private[*].id
    endpoint_private_access = true
    endpoint_public_access  = false  # Production security
    
    security_group_ids = [aws_security_group.eks_cluster.id]
  }

  # Enhanced logging for production
  enabled_cluster_log_types = [
    "api",
    "audit",
    "authenticator",
    "controllerManager",
    "scheduler"
  ]

  # Encryption configuration
  encryption_config {
    provider {
      key_arn = aws_kms_key.eks.arn
    }
    resources = ["secrets"]
  }

  tags = {
    Environment = "production"
    Project     = "auto-ru-federation"
    Backup      = "required"
  }
}

# Production Node Groups with multiple instance types
resource "aws_eks_node_group" "production_workers" {
  cluster_name    = aws_eks_cluster.production.name
  node_group_name = "production-workers"
  node_role_arn   = aws_iam_role.eks_node_group.arn
  subnet_ids      = aws_subnet.private[*].id

  # Instance configuration - соответствует архитектурным требованиям
  instance_types = ["c5.large", "c5.xlarge", "m5.large"]
  capacity_type  = "SPOT"  # Cost optimization
  disk_size      = 100

  # Scaling configuration
  scaling_config {
    desired_size = 6
    max_size     = 12
    min_size     = 3
  }

  # Update configuration
  update_config {
    max_unavailable_percentage = 25
  }

  # Remote access
  remote_access {
    ec2_ssh_key = aws_key_pair.eks_nodes.key_name
    source_security_group_ids = [aws_security_group.bastion.id]
  }

  tags = {
    Environment = "production"
    Project     = "auto-ru-federation"
  }
}

# Spot instance configuration for cost optimization
resource "aws_eks_node_group" "production_spot" {
  cluster_name    = aws_eks_cluster.production.name
  node_group_name = "production-spot"
  node_role_arn   = aws_iam_role.eks_node_group.arn
  subnet_ids      = aws_subnet.private[*].id

  capacity_type  = "SPOT"
  instance_types = ["c5.large", "c5.xlarge", "m5.large", "m5.xlarge"]

  scaling_config {
    desired_size = 3
    max_size     = 8
    min_size     = 0
  }

  # Taints for spot instances
  taint {
    key    = "spot-instance"
    value  = "true"
    effect = "NO_SCHEDULE"
  }

  tags = {
    Environment = "production"
    NodeType    = "spot"
  }
}
```

##### Production Deployment with High Availability
```yaml
# k8s/production/ugc-deployment.yaml - Production UGC deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ugc-subgraph
  namespace: production
  labels:
    app: ugc-subgraph
    version: v1
    environment: production
spec:
  replicas: 3  # Соответствует архитектурной диаграмме
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxUnavailable: 1
      maxSurge: 2  # Faster rollouts in production
  
  selector:
    matchLabels:
      app: ugc-subgraph
  
  template:
    metadata:
      labels:
        app: ugc-subgraph
        version: v1
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9001"
        prometheus.io/path: "/metrics"
    
    spec:
      serviceAccountName: ugc-service-account
      
      # Security context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
      
      # Multi-AZ distribution
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchExpressions:
              - key: app
                operator: In
                values:
                - ugc-subgraph
            topologyKey: topology.kubernetes.io/zone
        
        nodeAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            preference:
              matchExpressions:
              - key: node.kubernetes.io/instance-type
                operator: In
                values:
                - c5.large
                - c5.xlarge
      
      # Tolerations
      tolerations:
      - key: workload-type
        operator: Equal
        value: web-services
        effect: NoSchedule
      
      containers:
      - name: ugc-subgraph
        image: ghcr.io/auto-ru/ugc-subgraph:v1.0.0  # Pinned version for production
        imagePullPolicy: IfNotPresent
        
        ports:
        - name: http
          containerPort: 4001
          protocol: TCP
        - name: metrics
          containerPort: 9001
          protocol: TCP
        
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
          value: "warn"  # Reduced logging in production
        - name: RUST_BACKTRACE
          value: "0"
        - name: ENVIRONMENT
          value: "production"
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
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: ugc-secrets
              key: jwt-secret
        
        # Enhanced health checks for production
        livenessProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 60
          periodSeconds: 30
          timeoutSeconds: 10
          failureThreshold: 3
          successThreshold: 1
        
        readinessProbe:
          httpGet:
            path: /ready
            port: http
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
          successThreshold: 1
        
        startupProbe:
          httpGet:
            path: /health
            port: http
          initialDelaySeconds: 30
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 60  # Allow longer startup time
        
        # Security context
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
        
        # Volume mounts
        volumeMounts:
        - name: tmp
          mountPath: /tmp
        - name: cache
          mountPath: /app/cache
      
      volumes:
      - name: tmp
        emptyDir:
          sizeLimit: 100Mi
      - name: cache
        emptyDir:
          sizeLimit: 500Mi
      
      # Priority class for production workloads
      priorityClassName: high-priority
      
      # Termination grace period
      terminationGracePeriodSeconds: 60

---
# Horizontal Pod Autoscaler
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: ugc-subgraph-hpa
  namespace: production
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: ugc-subgraph
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
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
      - type: Percent
        value: 10
        periodSeconds: 60
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
      - type: Percent
        value: 50
        periodSeconds: 60
```

### Deployment Automation Scripts

#### Deployment Pipeline
```bash
#!/bin/bash
# scripts/deploy.sh - Deployment automation script

set -euo pipefail

# Configuration
ENVIRONMENT=${1:-staging}
SERVICE=${2:-all}
IMAGE_TAG=${3:-latest}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

success() {
    echo -e "${GREEN}✅ $1${NC}"
}

warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ $1${NC}"
    exit 1
}

# Validate environment
validate_environment() {
    log "Validating deployment environment: $ENVIRONMENT"
    
    case $ENVIRONMENT in
        staging|production)
            log "Environment $ENVIRONMENT is valid"
            ;;
        *)
            error "Invalid environment: $ENVIRONMENT. Must be 'staging' or 'production'"
            ;;
    esac
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check kubectl
    if ! command -v kubectl &> /dev/null; then
        error "kubectl is not installed"
    fi
    
    # Check AWS CLI
    if ! command -v aws &> /dev/null; then
        error "AWS CLI is not installed"
    fi
    
    # Check Helm
    if ! command -v helm &> /dev/null; then
        error "Helm is not installed"
    fi
    
    success "All prerequisites are installed"
}

# Configure kubectl
configure_kubectl() {
    log "Configuring kubectl for $ENVIRONMENT environment..."
    
    case $ENVIRONMENT in
        staging)
            aws eks update-kubeconfig --region us-east-1 --name auto-ru-staging
            ;;
        production)
            aws eks update-kubeconfig --region us-east-1 --name auto-ru-production
            ;;
    esac
    
    # Verify connection
    if kubectl cluster-info &> /dev/null; then
        success "kubectl configured successfully"
    else
        error "Failed to configure kubectl"
    fi
}

# Deploy service
deploy_service() {
    local service_name=$1
    local namespace=$ENVIRONMENT
    
    log "Deploying $service_name to $namespace namespace..."
    
    # Apply Kubernetes manifests
    kubectl apply -f k8s/$ENVIRONMENT/$service_name-deployment.yaml
    kubectl apply -f k8s/$ENVIRONMENT/$service_name-service.yaml
    
    # Update image if specified
    if [[ $IMAGE_TAG != "latest" ]]; then
        kubectl set image deployment/$service_name \
            $service_name=ghcr.io/auto-ru/$service_name:$IMAGE_TAG \
            -n $namespace
    fi
    
    # Wait for rollout
    log "Waiting for $service_name rollout to complete..."
    if kubectl rollout status deployment/$service_name -n $namespace --timeout=300s; then
        success "$service_name deployed successfully"
    else
        error "Failed to deploy $service_name"
    fi
}

# Deploy all services
deploy_all() {
    local services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "apollo-router")
    
    for service in "${services[@]}"; do
        deploy_service $service
    done
}

# Health check
health_check() {
    log "Performing health checks..."
    
    local namespace=$ENVIRONMENT
    local services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "apollo-router")
    
    for service in "${services[@]}"; do
        log "Checking health of $service..."
        
        # Get service endpoint
        local endpoint=$(kubectl get service $service -n $namespace -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
        
        if [[ -z $endpoint ]]; then
            # Use port-forward for ClusterIP services
            kubectl port-forward service/$service 8080:4001 -n $namespace &
            local port_forward_pid=$!
            sleep 5
            endpoint="localhost:8080"
        fi
        
        # Health check
        if curl -f http://$endpoint/health &> /dev/null; then
            success "$service is healthy"
        else
            warning "$service health check failed"
        fi
        
        # Cleanup port-forward
        if [[ -n ${port_forward_pid:-} ]]; then
            kill $port_forward_pid &> /dev/null || true
        fi
    done
}

# Rollback function
rollback() {
    local service_name=${1:-all}
    local namespace=$ENVIRONMENT
    
    log "Rolling back $service_name in $namespace..."
    
    if [[ $service_name == "all" ]]; then
        local services=("ugc-subgraph" "users-subgraph" "offers-subgraph" "apollo-router")
        for service in "${services[@]}"; do
            kubectl rollout undo deployment/$service -n $namespace
        done
    else
        kubectl rollout undo deployment/$service_name -n $namespace
    fi
    
    success "Rollback completed"
}

# Main deployment function
main() {
    log "Starting deployment process..."
    log "Environment: $ENVIRONMENT"
    log "Service: $SERVICE"
    log "Image Tag: $IMAGE_TAG"
    
    validate_environment
    check_prerequisites
    configure_kubectl
    
    case $SERVICE in
        all)
            deploy_all
            ;;
        rollback)
            rollback
            ;;
        *)
            deploy_service $SERVICE
            ;;
    esac
    
    health_check
    
    success "Deployment completed successfully!"
}

# Handle script arguments
case ${1:-help} in
    staging|production)
        main
        ;;
    rollback)
        ENVIRONMENT=${2:-staging}
        SERVICE="rollback"
        main
        ;;
    help|*)
        echo "Usage: $0 <environment> [service] [image_tag]"
        echo "       $0 rollback <environment> [service]"
        echo ""
        echo "Environments: staging, production"
        echo "Services: ugc-subgraph, users-subgraph, offers-subgraph, apollo-router, all"
        echo "Image Tag: Docker image tag (default: latest)"
        echo ""
        echo "Examples:"
        echo "  $0 staging all v1.0.0"
        echo "  $0 production ugc-subgraph latest"
        echo "  $0 rollback staging ugc-subgraph"
        ;;
esac
```

### Заключение

Диаграмма развертывания Task 12 обеспечивает полную трассируемость между архитектурными решениями развертывания и их физической реализацией:

1. **Local Development** → Docker Desktop конфигурации и development tools
2. **CI/CD Infrastructure** → GitHub Actions workflows и runner configurations
3. **Container Registry** → Image management и distribution strategies
4. **AWS Staging** → EKS cluster configuration и Kubernetes manifests
5. **AWS Production** → High-availability deployment с auto-scaling
6. **Deployment Automation** → Scripts и pipelines для automated deployment

Каждый deployment node в диаграмме имеет конкретную реализацию в виде инфраструктурного кода, конфигурационных файлов и automation scripts, что обеспечивает полную согласованность между архитектурным дизайном и физической инфраструктурой.