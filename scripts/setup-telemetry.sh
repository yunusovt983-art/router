#!/bin/bash

# Setup complete telemetry infrastructure for Auto.ru GraphQL Federation

set -e

echo "Setting up telemetry infrastructure..."

# Load environment variables
if [ -f .env.telemetry ]; then
    export $(cat .env.telemetry | grep -v '^#' | xargs)
fi

# Start telemetry services
echo "Starting Jaeger, Prometheus, and Grafana..."
docker-compose -f docker-compose.telemetry.yml up -d

# Wait for services to be ready
echo "Waiting for services to be ready..."
sleep 30

# Check Jaeger
until curl -s http://localhost:16686/api/services | grep -q '\[\]'; do
  echo "Waiting for Jaeger..."
  sleep 5
done
echo "Jaeger is ready!"

# Check Prometheus
until curl -s http://localhost:9090/-/ready | grep -q 'Prometheus is Ready'; do
  echo "Waiting for Prometheus..."
  sleep 5
done
echo "Prometheus is ready!"

# Check Grafana
until curl -s http://localhost:3000/api/health | grep -q '"database":"ok"'; do
  echo "Waiting for Grafana..."
  sleep 5
done
echo "Grafana is ready!"

echo "Telemetry infrastructure setup complete!"
echo ""
echo "Access points:"
echo "- Jaeger UI: http://localhost:16686"
echo "- Prometheus: http://localhost:9090"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo "- OTEL Collector: http://localhost:4318"