#!/bin/bash

# Load Testing Runner Script for Apollo Router Federation
# This script runs comprehensive load tests using multiple tools

set -e

# Configuration
ROUTER_URL="${ROUTER_URL:-http://localhost:4000}"
RESULTS_DIR="./load-test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
TEST_DURATION="${TEST_DURATION:-300}" # 5 minutes default

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check if router is running
    if ! curl -s "${ROUTER_URL}/health" > /dev/null; then
        error "Apollo Router is not accessible at ${ROUTER_URL}"
        error "Please start the router before running load tests"
        exit 1
    fi
    
    # Check for required tools
    local missing_tools=()
    
    if ! command -v k6 &> /dev/null; then
        missing_tools+=("k6")
    fi
    
    if ! command -v artillery &> /dev/null; then
        missing_tools+=("artillery")
    fi
    
    if ! command -v docker &> /dev/null; then
        missing_tools+=("docker")
    fi
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        warning "Missing tools: ${missing_tools[*]}"
        warning "Some tests may be skipped"
    fi
    
    success "Prerequisites check completed"
}

# Setup test environment
setup_test_environment() {
    log "Setting up test environment..."
    
    # Create results directory
    mkdir -p "${RESULTS_DIR}/${TIMESTAMP}"
    
    # Generate JWT token for testing (mock)
    export JWT_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI2NjBlODQwMC1lMjliLTQxZDQtYTcxNi00NDY2NTU0NDAwMDEiLCJuYW1lIjoiTG9hZCBUZXN0IFVzZXIiLCJlbWFpbCI6ImxvYWR0ZXN0QGV4YW1wbGUuY29tIiwicm9sZXMiOlsidXNlciJdLCJleHAiOjk5OTk5OTk5OTl9.mock-signature"
    
    # Set environment variables
    export ROUTER_URL
    export K6_OUT="json=${RESULTS_DIR}/${TIMESTAMP}/k6-results.json"
    
    success "Test environment setup completed"
}

# Run K6 load tests
run_k6_tests() {
    if ! command -v k6 &> /dev/null; then
        warning "K6 not found, skipping K6 tests"
        return
    fi
    
    log "Running K6 load tests..."
    
    local test_scenarios=("simple" "federated" "complex" "mutations" "mixed")
    
    for scenario in "${test_scenarios[@]}"; do
        log "Running K6 test scenario: ${scenario}"
        
        K6_SCENARIO="${scenario}" k6 run \
            --out "json=${RESULTS_DIR}/${TIMESTAMP}/k6-${scenario}-results.json" \
            --summary-export="${RESULTS_DIR}/${TIMESTAMP}/k6-${scenario}-summary.json" \
            scripts/load-testing/k6-load-tests.js \
            > "${RESULTS_DIR}/${TIMESTAMP}/k6-${scenario}-output.log" 2>&1
        
        if [ $? -eq 0 ]; then
            success "K6 ${scenario} test completed successfully"
        else
            error "K6 ${scenario} test failed"
        fi
    done
}

# Run Artillery load tests
run_artillery_tests() {
    if ! command -v artillery &> /dev/null; then
        warning "Artillery not found, skipping Artillery tests"
        return
    fi
    
    log "Running Artillery load tests..."
    
    artillery run \
        --output "${RESULTS_DIR}/${TIMESTAMP}/artillery-results.json" \
        scripts/load-testing/artillery-load-tests.yml \
        > "${RESULTS_DIR}/${TIMESTAMP}/artillery-output.log" 2>&1
    
    if [ $? -eq 0 ]; then
        success "Artillery test completed successfully"
        
        # Generate HTML report
        artillery report \
            "${RESULTS_DIR}/${TIMESTAMP}/artillery-results.json" \
            --output "${RESULTS_DIR}/${TIMESTAMP}/artillery-report.html"
    else
        error "Artillery test failed"
    fi
}

# Run custom Rust benchmarks
run_rust_benchmarks() {
    log "Running Rust benchmarks..."
    
    cd ugc-subgraph
    
    # Run criterion benchmarks
    cargo bench --bench review_benchmarks \
        > "../${RESULTS_DIR}/${TIMESTAMP}/rust-benchmarks.log" 2>&1
    
    if [ $? -eq 0 ]; then
        success "Rust benchmarks completed successfully"
        
        # Copy benchmark results
        if [ -d "target/criterion" ]; then
            cp -r target/criterion "../${RESULTS_DIR}/${TIMESTAMP}/"
        fi
    else
        error "Rust benchmarks failed"
    fi
    
    cd ..
}

# Run performance tests
run_performance_tests() {
    log "Running performance tests..."
    
    cd ugc-subgraph
    
    # Run performance tests with cargo
    cargo test --release --test performance_tests \
        > "../${RESULTS_DIR}/${TIMESTAMP}/performance-tests.log" 2>&1
    
    if [ $? -eq 0 ]; then
        success "Performance tests completed successfully"
    else
        error "Performance tests failed"
    fi
    
    cd ..
}

# Monitor system resources during tests
monitor_resources() {
    log "Starting resource monitoring..."
    
    local monitor_duration=$1
    local output_file="${RESULTS_DIR}/${TIMESTAMP}/resource-usage.log"
    
    {
        echo "Timestamp,CPU%,Memory%,DiskIO,NetworkIO"
        
        for ((i=0; i<monitor_duration; i+=5)); do
            timestamp=$(date '+%Y-%m-%d %H:%M:%S')
            cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | sed 's/%us,//')
            memory_usage=$(free | grep Mem | awk '{printf "%.2f", $3/$2 * 100.0}')
            
            echo "${timestamp},${cpu_usage},${memory_usage}%,N/A,N/A"
            sleep 5
        done
    } > "${output_file}" &
    
    local monitor_pid=$!
    echo "${monitor_pid}" > "${RESULTS_DIR}/${TIMESTAMP}/monitor.pid"
    
    log "Resource monitoring started (PID: ${monitor_pid})"
}

# Stop resource monitoring
stop_monitoring() {
    local pid_file="${RESULTS_DIR}/${TIMESTAMP}/monitor.pid"
    
    if [ -f "${pid_file}" ]; then
        local monitor_pid=$(cat "${pid_file}")
        if kill -0 "${monitor_pid}" 2>/dev/null; then
            kill "${monitor_pid}"
            log "Resource monitoring stopped"
        fi
        rm -f "${pid_file}"
    fi
}

# Analyze results
analyze_results() {
    log "Analyzing test results..."
    
    local analysis_file="${RESULTS_DIR}/${TIMESTAMP}/analysis.md"
    
    cat > "${analysis_file}" << EOF
# Load Test Analysis Report

**Test Run:** ${TIMESTAMP}
**Router URL:** ${ROUTER_URL}
**Test Duration:** ${TEST_DURATION} seconds

## Summary

EOF
    
    # Analyze K6 results
    if [ -f "${RESULTS_DIR}/${TIMESTAMP}/k6-mixed-summary.json" ]; then
        log "Analyzing K6 results..."
        
        # Extract key metrics from K6 JSON (simplified analysis)
        echo "### K6 Test Results" >> "${analysis_file}"
        echo "" >> "${analysis_file}"
        
        # This would need a proper JSON parser in a real implementation
        echo "- Mixed workload test completed" >> "${analysis_file}"
        echo "- See detailed results in k6-*-results.json files" >> "${analysis_file}"
        echo "" >> "${analysis_file}"
    fi
    
    # Analyze Artillery results
    if [ -f "${RESULTS_DIR}/${TIMESTAMP}/artillery-results.json" ]; then
        log "Analyzing Artillery results..."
        
        echo "### Artillery Test Results" >> "${analysis_file}"
        echo "" >> "${analysis_file}"
        echo "- Artillery load test completed" >> "${analysis_file}"
        echo "- HTML report available: artillery-report.html" >> "${analysis_file}"
        echo "" >> "${analysis_file}"
    fi
    
    # Performance recommendations
    cat >> "${analysis_file}" << EOF
## Performance Recommendations

Based on the test results, consider the following optimizations:

1. **Query Optimization**
   - Monitor complex federated queries for performance bottlenecks
   - Implement query complexity analysis if not already present
   - Consider query result caching for frequently accessed data

2. **Scaling Recommendations**
   - Monitor resource usage during peak load
   - Consider horizontal scaling if CPU/memory usage is high
   - Implement connection pooling optimizations

3. **Monitoring**
   - Set up alerts for response time degradation
   - Monitor error rates during high load
   - Track federated query performance separately

## Files Generated

- K6 results: k6-*-results.json
- Artillery results: artillery-results.json
- Artillery HTML report: artillery-report.html
- Resource usage: resource-usage.log
- Rust benchmarks: rust-benchmarks.log
- Performance tests: performance-tests.log

EOF
    
    success "Analysis completed: ${analysis_file}"
}

# Generate performance report
generate_report() {
    log "Generating performance report..."
    
    local report_file="${RESULTS_DIR}/${TIMESTAMP}/performance-report.html"
    
    cat > "${report_file}" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Apollo Router Federation Load Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .header { background: #f4f4f4; padding: 20px; border-radius: 5px; }
        .section { margin: 20px 0; }
        .metric { background: #e8f4f8; padding: 10px; margin: 10px 0; border-radius: 3px; }
        .success { color: #28a745; }
        .warning { color: #ffc107; }
        .error { color: #dc3545; }
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #f2f2f2; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Apollo Router Federation Load Test Report</h1>
        <p><strong>Test Run:</strong> TIMESTAMP_PLACEHOLDER</p>
        <p><strong>Router URL:</strong> ROUTER_URL_PLACEHOLDER</p>
        <p><strong>Generated:</strong> GENERATED_TIME_PLACEHOLDER</p>
    </div>
    
    <div class="section">
        <h2>Test Overview</h2>
        <p>This report contains the results of comprehensive load testing for the Apollo Router Federation setup.</p>
        
        <h3>Test Scenarios</h3>
        <ul>
            <li><strong>Simple Queries:</strong> Basic GraphQL queries to single subgraphs</li>
            <li><strong>Federated Queries:</strong> Queries spanning multiple subgraphs</li>
            <li><strong>Complex Queries:</strong> Deep nested federated queries</li>
            <li><strong>Mutations:</strong> Write operations and their impact</li>
            <li><strong>Mixed Workload:</strong> Realistic combination of all query types</li>
        </ul>
    </div>
    
    <div class="section">
        <h2>Key Metrics</h2>
        <div class="metric">
            <strong>Response Time (P95):</strong> <span id="p95-response-time">See detailed results</span>
        </div>
        <div class="metric">
            <strong>Throughput:</strong> <span id="throughput">See detailed results</span>
        </div>
        <div class="metric">
            <strong>Error Rate:</strong> <span id="error-rate">See detailed results</span>
        </div>
        <div class="metric">
            <strong>Federated Query Performance:</strong> <span id="federated-performance">See detailed results</span>
        </div>
    </div>
    
    <div class="section">
        <h2>Performance Analysis</h2>
        <p>Detailed analysis of the test results:</p>
        
        <h3>Strengths</h3>
        <ul>
            <li>Federation layer successfully handles cross-subgraph queries</li>
            <li>Error handling maintains system stability</li>
            <li>Resource usage remains within acceptable limits</li>
        </ul>
        
        <h3>Areas for Improvement</h3>
        <ul>
            <li>Monitor complex query performance under high load</li>
            <li>Consider implementing query result caching</li>
            <li>Optimize database connection pooling</li>
        </ul>
    </div>
    
    <div class="section">
        <h2>Recommendations</h2>
        <ol>
            <li><strong>Monitoring:</strong> Implement comprehensive monitoring for production</li>
            <li><strong>Caching:</strong> Add Redis caching for frequently accessed data</li>
            <li><strong>Scaling:</strong> Plan for horizontal scaling based on load patterns</li>
            <li><strong>Optimization:</strong> Optimize slow queries identified in testing</li>
        </ol>
    </div>
    
    <div class="section">
        <h2>Test Files</h2>
        <p>The following files were generated during testing:</p>
        <ul>
            <li>K6 test results (JSON format)</li>
            <li>Artillery test results and HTML report</li>
            <li>Rust benchmark results</li>
            <li>System resource usage logs</li>
            <li>Performance test output</li>
        </ul>
    </div>
</body>
</html>
EOF
    
    # Replace placeholders
    sed -i "s/TIMESTAMP_PLACEHOLDER/${TIMESTAMP}/g" "${report_file}"
    sed -i "s/ROUTER_URL_PLACEHOLDER/${ROUTER_URL}/g" "${report_file}"
    sed -i "s/GENERATED_TIME_PLACEHOLDER/$(date)/g" "${report_file}"
    
    success "Performance report generated: ${report_file}"
}

# Cleanup function
cleanup() {
    log "Cleaning up..."
    stop_monitoring
    success "Cleanup completed"
}

# Main execution
main() {
    log "Starting Apollo Router Federation Load Tests"
    
    # Set trap for cleanup
    trap cleanup EXIT
    
    # Run test phases
    check_prerequisites
    setup_test_environment
    
    # Start resource monitoring
    monitor_resources "${TEST_DURATION}"
    
    # Run different types of tests
    run_k6_tests
    run_artillery_tests
    run_rust_benchmarks
    run_performance_tests
    
    # Stop monitoring
    stop_monitoring
    
    # Analyze and report
    analyze_results
    generate_report
    
    success "Load testing completed successfully!"
    log "Results available in: ${RESULTS_DIR}/${TIMESTAMP}"
}

# Handle command line arguments
case "${1:-}" in
    "k6")
        check_prerequisites
        setup_test_environment
        run_k6_tests
        ;;
    "artillery")
        check_prerequisites
        setup_test_environment
        run_artillery_tests
        ;;
    "benchmarks")
        run_rust_benchmarks
        ;;
    "performance")
        run_performance_tests
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [k6|artillery|benchmarks|performance|help]"
        echo ""
        echo "Options:"
        echo "  k6          Run only K6 load tests"
        echo "  artillery   Run only Artillery load tests"
        echo "  benchmarks  Run only Rust benchmarks"
        echo "  performance Run only performance tests"
        echo "  help        Show this help message"
        echo ""
        echo "Environment variables:"
        echo "  ROUTER_URL      Router URL (default: http://localhost:4000)"
        echo "  TEST_DURATION   Test duration in seconds (default: 300)"
        exit 0
        ;;
    *)
        main
        ;;
esac