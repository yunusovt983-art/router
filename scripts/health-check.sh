#!/bin/bash

# Auto.ru GraphQL Federation - Health Check Script

echo "🏥 Checking service health..."

# Check if Docker is running
if ! sudo docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running"
    exit 1
fi

# Check container status
echo "📊 Container Status:"
sudo docker compose ps --format "table {{.Name}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "🔍 Service Health Checks:"

# Check Elasticsearch
if sudo docker compose exec -T elasticsearch curl -s http://localhost:9200/_cluster/health > /dev/null 2>&1; then
    echo "✅ Elasticsearch: Healthy"
else
    echo "❌ Elasticsearch: Unhealthy"
fi

# Check PostgreSQL databases
for db in ugc users offers catalog; do
    if sudo docker compose exec -T ${db}-postgres pg_isready -U postgres > /dev/null 2>&1; then
        echo "✅ PostgreSQL (${db}): Healthy"
    else
        echo "❌ PostgreSQL (${db}): Unhealthy"
    fi
done

# Check Redis
if sudo docker compose exec -T redis redis-cli ping > /dev/null 2>&1; then
    echo "✅ Redis: Healthy"
else
    echo "❌ Redis: Unhealthy"
fi

# Check Prometheus
if sudo docker compose exec -T prometheus wget -q --spider http://localhost:9090/-/healthy > /dev/null 2>&1; then
    echo "✅ Prometheus: Healthy"
else
    echo "❌ Prometheus: Unhealthy"
fi

# Check Grafana
if sudo docker compose exec -T grafana curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
    echo "✅ Grafana: Healthy"
else
    echo "❌ Grafana: Unhealthy"
fi

# Check Jaeger
if sudo docker compose exec -T jaeger wget -q --spider http://localhost:16686 > /dev/null 2>&1; then
    echo "✅ Jaeger: Healthy"
else
    echo "❌ Jaeger: Unhealthy"
fi

echo ""
echo "🌐 Access URLs:"
echo "- Elasticsearch: http://localhost:9200"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo "- Jaeger UI: http://localhost:16686"
echo "- Prometheus: http://localhost:9091"
echo ""
echo "📊 Database Connections:"
echo "- UGC DB: postgresql://postgres:password@localhost:5432/ugc_db"
echo "- Users DB: postgresql://postgres:password@localhost:5433/users_db"
echo "- Offers DB: postgresql://postgres:password@localhost:5435/offers_db"
echo "- Catalog DB: postgresql://postgres:password@localhost:5434/catalog_db"
echo "- Redis: redis://localhost:6379"