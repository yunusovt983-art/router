#!/bin/bash

# Setup logging infrastructure for Auto.ru GraphQL Federation

set -e

echo "Setting up logging infrastructure..."

# Create necessary directories
mkdir -p logs
mkdir -p filebeat
mkdir -p logstash/pipeline
mkdir -p logstash/config

# Set permissions for Elasticsearch data
sudo chown -R 1000:1000 logs/ || true

# Start logging services
echo "Starting Elasticsearch, Kibana, and Logstash..."
docker-compose -f docker-compose.logging.yml up -d

# Wait for Elasticsearch to be ready
echo "Waiting for Elasticsearch to be ready..."
until curl -s http://localhost:9200/_cluster/health | grep -q '"status":"green\|yellow"'; do
  echo "Waiting for Elasticsearch..."
  sleep 5
done

echo "Elasticsearch is ready!"

# Create index template for structured logs
echo "Creating index template..."
curl -X PUT "localhost:9200/_index_template/auto-ru-federation-logs" \
  -H "Content-Type: application/json" \
  -d '{
    "index_patterns": ["auto-ru-federation-logs-*"],
    "template": {
      "settings": {
        "number_of_shards": 1,
        "number_of_replicas": 0,
        "index.refresh_interval": "5s"
      },
      "mappings": {
        "properties": {
          "@timestamp": {
            "type": "date"
          },
          "level": {
            "type": "keyword"
          },
          "service_name": {
            "type": "keyword"
          },
          "correlation_id": {
            "type": "keyword"
          },
          "trace_id": {
            "type": "keyword"
          },
          "user_id": {
            "type": "keyword"
          },
          "event_type": {
            "type": "keyword"
          },
          "graphql_operation": {
            "type": "keyword"
          },
          "target": {
            "type": "keyword"
          },
          "module": {
            "type": "keyword"
          },
          "file": {
            "type": "keyword"
          },
          "line": {
            "type": "integer"
          },
          "fields": {
            "type": "object",
            "dynamic": true
          },
          "message": {
            "type": "text",
            "analyzer": "standard"
          }
        }
      }
    }
  }'

echo "Index template created!"

# Wait for Kibana to be ready
echo "Waiting for Kibana to be ready..."
until curl -s http://localhost:5601/api/status | grep -q '"overall":{"level":"available"'; do
  echo "Waiting for Kibana..."
  sleep 5
done

echo "Kibana is ready!"

# Create Kibana index pattern
echo "Creating Kibana index pattern..."
curl -X POST "localhost:5601/api/saved_objects/index-pattern/auto-ru-federation-logs" \
  -H "Content-Type: application/json" \
  -H "kbn-xsrf: true" \
  -d '{
    "attributes": {
      "title": "auto-ru-federation-logs-*",
      "timeFieldName": "@timestamp"
    }
  }' || echo "Index pattern might already exist"

echo "Logging infrastructure setup complete!"
echo ""
echo "Access points:"
echo "- Kibana: http://localhost:5601"
echo "- Elasticsearch: http://localhost:9200"
echo ""
echo "To view logs in Kibana:"
echo "1. Go to http://localhost:5601"
echo "2. Navigate to 'Discover'"
echo "3. Select the 'auto-ru-federation-logs-*' index pattern"
echo ""
echo "Useful Kibana queries:"
echo "- service_name:ugc-subgraph"
echo "- level:ERROR"
echo "- event_type:review_created"
echo "- correlation_id:\"your-correlation-id\""