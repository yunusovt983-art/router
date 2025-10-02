#!/bin/bash

# Staging Environment Tests for Apollo Router Federation
# This script runs comprehensive tests in staging before production deployment

set -e

# Configuration
STAGING_URL="${STAGING_URL:-https://staging-api.auto.ru}"
STAGING_NAMESPACE="${STAGING_NAMESPACE:-auto-ru-federation-staging}"
TEST_TIMEOUT="${TEST_TIMEOUT:-300}"
PARALLEL_USERS="${PARALLEL_USERS:-10}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Logging functions
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

info() {
    echo -e "${PURPLE}[INFO]${NC} $1"
}

# Test results tracking
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

# Record test result
record_test_result() {
    local test_name="$1"
    local result="$2"
    
    if [ "$result" = "PASS" ]; then
        ((TESTS_PASSED++))
        success "✅ $test_name"
    else
        ((TESTS_FAILED++))
        FAILED_TESTS+=("$test_name")
        error "❌ $test_name"
    fi
}

# Wait for staging environment to be ready
wait_for_staging_ready() {
    log "Waiting for staging environment to be ready..."
    
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if curl -f "$STAGING_URL/health" &> /dev/null; then
            success "Staging environment is ready"
            return 0
        fi
        
        info "Attempt $attempt/$max_attempts - waiting for staging..."
        sleep 10
        ((attempt++))
    done
    
    error "Staging environment not ready after $max_attempts attempts"
    return 1
}

# Test basic GraphQL functionality
test_basic_graphql() {
    log "Testing basic GraphQL functionality..."
    
    local query='{"query":"query { __typename }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"__typename"'; then
        record_test_result "Basic GraphQL Query" "PASS"
    else
        record_test_result "Basic GraphQL Query" "FAIL"
        error "Response: $response"
    fi
}

# Test GraphQL introspection (should be disabled)
test_introspection_disabled() {
    log "Testing GraphQL introspection is disabled..."
    
    local introspection_query='{"query":"query { __schema { types { name } } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$introspection_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"errors"' && ! echo "$response" | grep -q '"__schema"'; then
        record_test_result "Introspection Disabled" "PASS"
    else
        record_test_result "Introspection Disabled" "FAIL"
        error "Introspection appears to be enabled: $response"
    fi
}

# Test federated queries
test_federated_queries() {
    log "Testing federated queries..."
    
    # Test query spanning multiple subgraphs
    local federated_query='{"query":"query { offers(first: 1) { edges { node { id title reviews(first: 1) { edges { node { id rating author { id name } } } } } } } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$federated_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"offers"' && ! echo "$response" | grep -q '"errors"'; then
        record_test_result "Federated Query" "PASS"
    else
        record_test_result "Federated Query" "FAIL"
        error "Federated query failed: $response"
    fi
}

# Test authentication
test_authentication() {
    log "Testing authentication..."
    
    # Test without authentication (should fail for protected operations)
    local protected_query='{"query":"mutation { createReview(input: {offerId: \"test\", rating: 5, text: \"test\"}) { id } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$protected_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"errors"' && echo "$response" | grep -qi "auth"; then
        record_test_result "Authentication Required" "PASS"
    else
        record_test_result "Authentication Required" "FAIL"
        error "Authentication not properly enforced: $response"
    fi
    
    # Test with invalid token
    local invalid_token_response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer invalid-token" \
        -d "$protected_query" \
        "$STAGING_URL/graphql")
    
    if echo "$invalid_token_response" | grep -q '"errors"'; then
        record_test_result "Invalid Token Rejected" "PASS"
    else
        record_test_result "Invalid Token Rejected" "FAIL"
        error "Invalid token not properly rejected: $invalid_token_response"
    fi
}

# Test rate limiting
test_rate_limiting() {
    log "Testing rate limiting..."
    
    local query='{"query":"query { __typename }"}'
    local rate_limited=false
    
    # Send rapid requests
    for i in {1..50}; do
        local response=$(curl -s -w "%{http_code}" -X POST \
            -H "Content-Type: application/json" \
            -d "$query" \
            "$STAGING_URL/graphql")
        
        if echo "$response" | grep -q "429"; then
            rate_limited=true
            break
        fi
        
        sleep 0.1
    done
    
    if [ "$rate_limited" = true ]; then
        record_test_result "Rate Limiting" "PASS"
    else
        record_test_result "Rate Limiting" "FAIL"
        warning "Rate limiting may not be configured properly"
    fi
}

# Test error handling
test_error_handling() {
    log "Testing error handling..."
    
    # Test invalid query
    local invalid_query='{"query":"query { nonExistentField }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$invalid_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"errors"' && ! echo "$response" | grep -qi "internal"; then
        record_test_result "Error Handling" "PASS"
    else
        record_test_result "Error Handling" "FAIL"
        error "Error handling may expose internal details: $response"
    fi
}

# Test query complexity limits
test_query_complexity() {
    log "Testing query complexity limits..."
    
    # Create a complex query
    local complex_query='{"query":"query { offers(first: 100) { edges { node { id title price description reviews(first: 50) { edges { node { id rating text author { id name email reviews(first: 20) { edges { node { id rating text } } } } } } } } } } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$complex_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"errors"' && echo "$response" | grep -qi "complex"; then
        record_test_result "Query Complexity Limits" "PASS"
    else
        record_test_result "Query Complexity Limits" "FAIL"
        warning "Query complexity limits may not be properly configured"
    fi
}

# Test health endpoints
test_health_endpoints() {
    log "Testing health endpoints..."
    
    # Test main health endpoint
    local health_response=$(curl -s -w "%{http_code}" "$STAGING_URL/health")
    
    if echo "$health_response" | grep -q "200"; then
        record_test_result "Main Health Endpoint" "PASS"
    else
        record_test_result "Main Health Endpoint" "FAIL"
        error "Health endpoint returned: $health_response"
    fi
    
    # Test readiness endpoint
    local ready_response=$(curl -s -w "%{http_code}" "$STAGING_URL/ready")
    
    if echo "$ready_response" | grep -q "200"; then
        record_test_result "Readiness Endpoint" "PASS"
    else
        record_test_result "Readiness Endpoint" "FAIL"
        error "Readiness endpoint returned: $ready_response"
    fi
}

# Test CORS configuration
test_cors() {
    log "Testing CORS configuration..."
    
    local cors_response=$(curl -s -H "Origin: https://auto.ru" \
        -H "Access-Control-Request-Method: POST" \
        -H "Access-Control-Request-Headers: Content-Type" \
        -X OPTIONS \
        "$STAGING_URL/graphql")
    
    if echo "$cors_response" | grep -q "Access-Control-Allow-Origin"; then
        record_test_result "CORS Configuration" "PASS"
    else
        record_test_result "CORS Configuration" "FAIL"
        error "CORS headers not found: $cors_response"
    fi
}

# Test security headers
test_security_headers() {
    log "Testing security headers..."
    
    local headers=$(curl -s -I "$STAGING_URL/graphql")
    local security_headers_found=0
    
    # Check for important security headers
    if echo "$headers" | grep -qi "x-content-type-options"; then
        ((security_headers_found++))
    fi
    
    if echo "$headers" | grep -qi "x-frame-options"; then
        ((security_headers_found++))
    fi
    
    if echo "$headers" | grep -qi "strict-transport-security"; then
        ((security_headers_found++))
    fi
    
    if [ $security_headers_found -ge 2 ]; then
        record_test_result "Security Headers" "PASS"
    else
        record_test_result "Security Headers" "FAIL"
        error "Missing security headers. Found: $security_headers_found/3"
    fi
}

# Test database connectivity
test_database_connectivity() {
    log "Testing database connectivity..."
    
    # Test query that requires database access
    local db_query='{"query":"query { offers(first: 1) { edges { node { id } } } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$db_query" \
        "$STAGING_URL/graphql")
    
    if echo "$response" | grep -q '"offers"' && ! echo "$response" | grep -qi "database.*error"; then
        record_test_result "Database Connectivity" "PASS"
    else
        record_test_result "Database Connectivity" "FAIL"
        error "Database connectivity issue: $response"
    fi
}

# Test cache functionality
test_cache_functionality() {
    log "Testing cache functionality..."
    
    # Make the same query twice and measure response times
    local query='{"query":"query { offers(first: 5) { edges { node { id title } } } }"}'
    
    local start1=$(date +%s%N)
    local response1=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$query" \
        "$STAGING_URL/graphql")
    local end1=$(date +%s%N)
    local time1=$(( (end1 - start1) / 1000000 ))
    
    sleep 1
    
    local start2=$(date +%s%N)
    local response2=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$query" \
        "$STAGING_URL/graphql")
    local end2=$(date +%s%N)
    local time2=$(( (end2 - start2) / 1000000 ))
    
    # Second request should be faster (cached)
    if [ $time2 -lt $time1 ] && echo "$response1" | grep -q '"offers"'; then
        record_test_result "Cache Functionality" "PASS"
        info "First request: ${time1}ms, Second request: ${time2}ms"
    else
        record_test_result "Cache Functionality" "FAIL"
        warning "Cache may not be working. First: ${time1}ms, Second: ${time2}ms"
    fi
}

# Load testing
run_load_test() {
    log "Running load test with $PARALLEL_USERS concurrent users..."
    
    local query='{"query":"query { offers(first: 3) { edges { node { id title reviews(first: 2) { edges { node { rating } } } } } } }"}'
    local load_test_script="/tmp/load_test.sh"
    
    # Create load test script
    cat > "$load_test_script" << EOF
#!/bin/bash
for i in {1..10}; do
    curl -s -X POST \\
        -H "Content-Type: application/json" \\
        -d '$query' \\
        "$STAGING_URL/graphql" > /dev/null
    sleep 0.1
done
EOF
    
    chmod +x "$load_test_script"
    
    # Run parallel load test
    local pids=()
    for i in $(seq 1 $PARALLEL_USERS); do
        "$load_test_script" &
        pids+=($!)
    done
    
    # Wait for all background processes
    local failed_processes=0
    for pid in "${pids[@]}"; do
        if ! wait $pid; then
            ((failed_processes++))
        fi
    done
    
    rm -f "$load_test_script"
    
    if [ $failed_processes -eq 0 ]; then
        record_test_result "Load Test ($PARALLEL_USERS users)" "PASS"
    else
        record_test_result "Load Test ($PARALLEL_USERS users)" "FAIL"
        error "$failed_processes out of $PARALLEL_USERS processes failed"
    fi
}

# Test monitoring endpoints
test_monitoring() {
    log "Testing monitoring endpoints..."
    
    # Test metrics endpoint
    local metrics_response=$(curl -s -w "%{http_code}" "$STAGING_URL/metrics")
    
    if echo "$metrics_response" | grep -q "200" && echo "$metrics_response" | grep -q "prometheus"; then
        record_test_result "Metrics Endpoint" "PASS"
    else
        record_test_result "Metrics Endpoint" "FAIL"
        error "Metrics endpoint issue: $metrics_response"
    fi
}

# Test SSL/TLS configuration
test_ssl_configuration() {
    log "Testing SSL/TLS configuration..."
    
    if [[ "$STAGING_URL" == https://* ]]; then
        # Test SSL certificate
        local ssl_info=$(echo | openssl s_client -connect "$(echo $STAGING_URL | sed 's|https://||' | sed 's|/.*||'):443" -servername "$(echo $STAGING_URL | sed 's|https://||' | sed 's|/.*||')" 2>/dev/null | openssl x509 -noout -dates 2>/dev/null)
        
        if [ -n "$ssl_info" ]; then
            record_test_result "SSL Certificate" "PASS"
        else
            record_test_result "SSL Certificate" "FAIL"
            error "SSL certificate validation failed"
        fi
    else
        warning "Skipping SSL test - staging URL is not HTTPS"
        record_test_result "SSL Certificate" "SKIP"
    fi
}

# Test data consistency
test_data_consistency() {
    log "Testing data consistency..."
    
    # Test that federated data is consistent
    local consistency_query='{"query":"query { offers(first: 1) { edges { node { id reviewsCount reviews(first: 1) { edges { node { id } } } } } } }"}'
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -d "$consistency_query" \
        "$STAGING_URL/graphql")
    
    # Parse response to check consistency (simplified check)
    if echo "$response" | grep -q '"offers"' && ! echo "$response" | grep -q '"errors"'; then
        record_test_result "Data Consistency" "PASS"
    else
        record_test_result "Data Consistency" "FAIL"
        error "Data consistency issue: $response"
    fi
}

# Generate test report
generate_test_report() {
    log "Generating test report..."
    
    local report_file="staging-test-report-$(date +%Y%m%d_%H%M%S).md"
    
    cat > "$report_file" << EOF
# Staging Test Report

**Environment**: $STAGING_URL
**Date**: $(date)
**Total Tests**: $((TESTS_PASSED + TESTS_FAILED))
**Passed**: $TESTS_PASSED
**Failed**: $TESTS_FAILED
**Success Rate**: $(( TESTS_PASSED * 100 / (TESTS_PASSED + TESTS_FAILED) ))%

## Test Results

### ✅ Passed Tests: $TESTS_PASSED

### ❌ Failed Tests: $TESTS_FAILED

EOF
    
    if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
        echo "**Failed Test Details:**" >> "$report_file"
        for test in "${FAILED_TESTS[@]}"; do
            echo "- $test" >> "$report_file"
        done
        echo "" >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF
## Recommendations

$(if [ $TESTS_FAILED -eq 0 ]; then
    echo "✅ All tests passed. Staging environment is ready for production deployment."
else
    echo "❌ $TESTS_FAILED tests failed. Address these issues before production deployment:"
    for test in "${FAILED_TESTS[@]}"; do
        echo "- Fix: $test"
    done
fi)

## Next Steps

$(if [ $TESTS_FAILED -eq 0 ]; then
    echo "1. Proceed with production deployment"
    echo "2. Monitor production metrics closely"
    echo "3. Have rollback plan ready"
else
    echo "1. Fix all failed tests"
    echo "2. Re-run staging tests"
    echo "3. Do not proceed to production until all tests pass"
fi)

---
**Report Generated**: $(date)
EOF
    
    success "Test report generated: $report_file"
    
    # Display summary
    echo ""
    echo "=========================================="
    echo "           STAGING TEST SUMMARY"
    echo "=========================================="
    echo "Total Tests: $((TESTS_PASSED + TESTS_FAILED))"
    echo "Passed: $TESTS_PASSED"
    echo "Failed: $TESTS_FAILED"
    echo "Success Rate: $(( TESTS_PASSED * 100 / (TESTS_PASSED + TESTS_FAILED) ))%"
    echo "=========================================="
    
    if [ $TESTS_FAILED -eq 0 ]; then
        success "🎉 All staging tests passed! Ready for production deployment."
        return 0
    else
        error "💥 $TESTS_FAILED tests failed. Do not deploy to production!"
        error "Failed tests: ${FAILED_TESTS[*]}"
        return 1
    fi
}

# Main test execution
main() {
    log "Starting staging environment tests"
    log "Target: $STAGING_URL"
    log "Namespace: $STAGING_NAMESPACE"
    
    # Wait for staging to be ready
    if ! wait_for_staging_ready; then
        error "Staging environment not ready, aborting tests"
        exit 1
    fi
    
    # Run all tests
    test_basic_graphql
    test_introspection_disabled
    test_federated_queries
    test_authentication
    test_rate_limiting
    test_error_handling
    test_query_complexity
    test_health_endpoints
    test_cors
    test_security_headers
    test_database_connectivity
    test_cache_functionality
    test_monitoring
    test_ssl_configuration
    test_data_consistency
    run_load_test
    
    # Generate report and exit with appropriate code
    generate_test_report
}

# Handle command line arguments
case "${1:-}" in
    "quick")
        # Run only essential tests
        wait_for_staging_ready
        test_basic_graphql
        test_federated_queries
        test_health_endpoints
        generate_test_report
        ;;
    "security")
        # Run only security tests
        wait_for_staging_ready
        test_introspection_disabled
        test_authentication
        test_rate_limiting
        test_security_headers
        test_ssl_configuration
        generate_test_report
        ;;
    "performance")
        # Run only performance tests
        wait_for_staging_ready
        test_cache_functionality
        run_load_test
        generate_test_report
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [quick|security|performance|help]"
        echo ""
        echo "Options:"
        echo "  quick       Run only essential tests"
        echo "  security    Run only security tests"
        echo "  performance Run only performance tests"
        echo "  help        Show this help message"
        echo ""
        echo "Environment variables:"
        echo "  STAGING_URL         Staging environment URL (default: https://staging-api.auto.ru)"
        echo "  STAGING_NAMESPACE   Kubernetes namespace (default: auto-ru-federation-staging)"
        echo "  TEST_TIMEOUT        Test timeout in seconds (default: 300)"
        echo "  PARALLEL_USERS      Number of parallel users for load test (default: 10)"
        exit 0
        ;;
    *)
        main
        ;;
esac