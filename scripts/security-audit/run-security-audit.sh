#!/bin/bash

# Security Audit Runner for Apollo Router Federation
# This script runs comprehensive security testing including OWASP Top 10 and penetration testing

set -e

# Configuration
ROUTER_URL="${ROUTER_URL:-http://localhost:4000}"
RESULTS_DIR="./security-audit-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
JWT_TOKEN="${JWT_TOKEN:-}"

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

# Check prerequisites
check_prerequisites() {
    log "Checking security audit prerequisites..."
    
    # Check if router is accessible
    if ! curl -s "${ROUTER_URL}/health" > /dev/null; then
        error "Apollo Router is not accessible at ${ROUTER_URL}"
        error "Please start the router before running security audit"
        exit 1
    fi
    
    # Check for required tools
    local missing_tools=()
    
    if ! command -v python3 &> /dev/null; then
        missing_tools+=("python3")
    fi
    
    if ! command -v nmap &> /dev/null; then
        missing_tools+=("nmap")
    fi
    
    if ! command -v nikto &> /dev/null; then
        missing_tools+=("nikto")
    fi
    
    if ! command -v sqlmap &> /dev/null; then
        missing_tools+=("sqlmap")
    fi
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        warning "Missing security tools: ${missing_tools[*]}"
        warning "Some tests may be skipped. Install with:"
        warning "  sudo apt-get install nmap nikto sqlmap"
        warning "  pip3 install requests"
    fi
    
    success "Prerequisites check completed"
}

# Setup audit environment
setup_audit_environment() {
    log "Setting up security audit environment..."
    
    # Create results directory
    mkdir -p "${RESULTS_DIR}/${TIMESTAMP}"
    
    # Generate test JWT token if not provided
    if [ -z "$JWT_TOKEN" ]; then
        warning "No JWT token provided, generating test token..."
        # This is a mock token for testing - in real scenarios, use proper authentication
        JWT_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI2NjBlODQwMC1lMjliLTQxZDQtYTcxNi00NDY2NTU0NDAwMDEiLCJuYW1lIjoiU2VjdXJpdHkgVGVzdCBVc2VyIiwiZW1haWwiOiJzZWN1cml0eUBleGFtcGxlLmNvbSIsInJvbGVzIjpbInVzZXIiXSwiZXhwIjo5OTk5OTk5OTk5fQ.mock-signature-for-testing"
        export JWT_TOKEN
    fi
    
    success "Audit environment setup completed"
}

# Run network reconnaissance
run_network_reconnaissance() {
    if ! command -v nmap &> /dev/null; then
        warning "Nmap not found, skipping network reconnaissance"
        return
    fi
    
    log "Running network reconnaissance..."
    
    # Extract host and port from URL
    local host=$(echo "$ROUTER_URL" | sed -E 's|^https?://([^:/]+).*|\1|')
    local port=$(echo "$ROUTER_URL" | sed -E 's|^https?://[^:/]+:?([0-9]+)?.*|\1|')
    
    if [ -z "$port" ]; then
        if [[ "$ROUTER_URL" == https://* ]]; then
            port=443
        else
            port=80
        fi
    fi
    
    # Port scan
    nmap -sS -O -sV -p 1-10000 "$host" > "${RESULTS_DIR}/${TIMESTAMP}/nmap-scan.txt" 2>&1
    
    # Service detection
    nmap -sC -sV -p "$port" "$host" > "${RESULTS_DIR}/${TIMESTAMP}/nmap-service-detection.txt" 2>&1
    
    # Vulnerability scan
    nmap --script vuln -p "$port" "$host" > "${RESULTS_DIR}/${TIMESTAMP}/nmap-vuln-scan.txt" 2>&1
    
    success "Network reconnaissance completed"
}

# Run web application scanning
run_web_app_scanning() {
    if ! command -v nikto &> /dev/null; then
        warning "Nikto not found, skipping web application scanning"
        return
    fi
    
    log "Running web application scanning..."
    
    # Nikto scan
    nikto -h "$ROUTER_URL" -Format txt -output "${RESULTS_DIR}/${TIMESTAMP}/nikto-scan.txt"
    
    success "Web application scanning completed"
}

# Run GraphQL-specific security tests
run_graphql_security_tests() {
    log "Running GraphQL-specific security tests..."
    
    python3 scripts/security-audit/security-scanner.py \
        "$ROUTER_URL" \
        --token "$JWT_TOKEN" \
        --output "${RESULTS_DIR}/${TIMESTAMP}/graphql-security-report.json" \
        --format json
    
    if [ $? -eq 0 ]; then
        success "GraphQL security tests completed"
    else
        error "GraphQL security tests failed"
    fi
}

# Run OWASP Top 10 tests
run_owasp_top10_tests() {
    log "Running OWASP Top 10 security tests..."
    
    python3 scripts/security-audit/owasp-top10-tests.py \
        "$ROUTER_URL" \
        --token "$JWT_TOKEN" \
        --output "${RESULTS_DIR}/${TIMESTAMP}/owasp-top10-report.json"
    
    if [ $? -eq 0 ]; then
        success "OWASP Top 10 tests completed"
    else
        error "OWASP Top 10 tests failed"
    fi
}

# Run SQL injection tests
run_sql_injection_tests() {
    if ! command -v sqlmap &> /dev/null; then
        warning "SQLMap not found, skipping SQL injection tests"
        return
    fi
    
    log "Running SQL injection tests..."
    
    # Create a simple GraphQL request file for sqlmap
    cat > "${RESULTS_DIR}/${TIMESTAMP}/graphql-request.txt" << EOF
POST ${ROUTER_URL}/graphql HTTP/1.1
Host: $(echo "$ROUTER_URL" | sed -E 's|^https?://([^:/]+).*|\1|')
Content-Type: application/json
Authorization: Bearer ${JWT_TOKEN}

{"query":"query GetReview(\$id: ID!) { review(id: \$id) { id rating text } }","variables":{"id":"*"}}
EOF
    
    # Run sqlmap
    sqlmap -r "${RESULTS_DIR}/${TIMESTAMP}/graphql-request.txt" \
           --batch \
           --level=3 \
           --risk=2 \
           --output-dir="${RESULTS_DIR}/${TIMESTAMP}/sqlmap" \
           > "${RESULTS_DIR}/${TIMESTAMP}/sqlmap-output.txt" 2>&1
    
    success "SQL injection tests completed"
}

# Run authentication and authorization tests
run_auth_tests() {
    log "Running authentication and authorization tests..."
    
    # Test without authentication
    info "Testing unauthenticated access..."
    curl -X POST \
         -H "Content-Type: application/json" \
         -d '{"query":"mutation { createReview(input: {offerId: \"test\", rating: 5, text: \"test\"}) { id } }"}' \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/unauth-test.json" 2>&1
    
    # Test with invalid token
    info "Testing with invalid token..."
    curl -X POST \
         -H "Content-Type: application/json" \
         -H "Authorization: Bearer invalid-token" \
         -d '{"query":"query { __typename }"}' \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/invalid-token-test.json" 2>&1
    
    # Test with expired token (if applicable)
    info "Testing with potentially expired token..."
    EXPIRED_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyLCJleHAiOjE1MTYyMzkwMjJ9.invalid"
    curl -X POST \
         -H "Content-Type: application/json" \
         -H "Authorization: Bearer $EXPIRED_TOKEN" \
         -d '{"query":"query { __typename }"}' \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/expired-token-test.json" 2>&1
    
    success "Authentication and authorization tests completed"
}

# Run rate limiting tests
run_rate_limiting_tests() {
    log "Running rate limiting tests..."
    
    info "Testing rate limiting with rapid requests..."
    
    # Create a simple script to test rate limiting
    cat > "${RESULTS_DIR}/${TIMESTAMP}/rate-limit-test.py" << 'EOF'
import requests
import time
import json
import sys

url = sys.argv[1]
token = sys.argv[2] if len(sys.argv) > 2 else None

headers = {'Content-Type': 'application/json'}
if token:
    headers['Authorization'] = f'Bearer {token}'

query = {"query": "query { __typename }"}

results = []
for i in range(100):
    start_time = time.time()
    try:
        response = requests.post(url, json=query, headers=headers, timeout=5)
        end_time = time.time()
        
        results.append({
            'request_number': i + 1,
            'status_code': response.status_code,
            'response_time': end_time - start_time,
            'rate_limited': response.status_code == 429
        })
        
        if response.status_code == 429:
            print(f"Rate limited at request {i + 1}")
            break
            
    except requests.RequestException as e:
        results.append({
            'request_number': i + 1,
            'error': str(e)
        })
    
    time.sleep(0.01)  # Small delay

with open('rate-limit-results.json', 'w') as f:
    json.dump(results, f, indent=2)

print(f"Completed {len(results)} requests")
rate_limited_count = sum(1 for r in results if r.get('rate_limited', False))
print(f"Rate limited requests: {rate_limited_count}")
EOF
    
    cd "${RESULTS_DIR}/${TIMESTAMP}"
    python3 rate-limit-test.py "$ROUTER_URL/graphql" "$JWT_TOKEN"
    cd - > /dev/null
    
    success "Rate limiting tests completed"
}

# Run input validation tests
run_input_validation_tests() {
    log "Running input validation tests..."
    
    # Test XSS payloads
    info "Testing XSS payloads..."
    XSS_PAYLOADS=(
        "<script>alert('XSS')</script>"
        "javascript:alert('XSS')"
        "<img src=x onerror=alert('XSS')>"
        "<svg onload=alert('XSS')>"
    )
    
    for payload in "${XSS_PAYLOADS[@]}"; do
        curl -X POST \
             -H "Content-Type: application/json" \
             -H "Authorization: Bearer $JWT_TOKEN" \
             -d "{\"query\":\"mutation { createReview(input: {offerId: \\\"test\\\", rating: 5, text: \\\"$payload\\\"}) { id text } }\"}" \
             "$ROUTER_URL/graphql" \
             >> "${RESULTS_DIR}/${TIMESTAMP}/xss-test-results.json" 2>&1
        echo "" >> "${RESULTS_DIR}/${TIMESTAMP}/xss-test-results.json"
    done
    
    # Test injection payloads
    info "Testing injection payloads..."
    INJECTION_PAYLOADS=(
        "' OR '1'='1"
        "'; DROP TABLE reviews; --"
        "' UNION SELECT * FROM users --"
        "\${7*7}"
        "{{7*7}}"
    )
    
    for payload in "${INJECTION_PAYLOADS[@]}"; do
        curl -X POST \
             -H "Content-Type: application/json" \
             -H "Authorization: Bearer $JWT_TOKEN" \
             -d "{\"query\":\"query { review(id: \\\"$payload\\\") { id } }\"}" \
             "$ROUTER_URL/graphql" \
             >> "${RESULTS_DIR}/${TIMESTAMP}/injection-test-results.json" 2>&1
        echo "" >> "${RESULTS_DIR}/${TIMESTAMP}/injection-test-results.json"
    done
    
    success "Input validation tests completed"
}

# Run DoS tests
run_dos_tests() {
    log "Running Denial of Service tests..."
    
    # Test query depth
    info "Testing query depth limits..."
    DEEP_QUERY='{"query":"query { offers(first: 1) { edges { node { reviews(first: 1) { edges { node { author { reviews(first: 1) { edges { node { offer { reviews(first: 1) { edges { node { id } } } } } } } } } } } } } } }"}'
    
    curl -X POST \
         -H "Content-Type: application/json" \
         -H "Authorization: Bearer $JWT_TOKEN" \
         -d "$DEEP_QUERY" \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/deep-query-test.json" 2>&1
    
    # Test query complexity
    info "Testing query complexity limits..."
    COMPLEX_QUERY='{"query":"query { offers(first: 100) { edges { node { id title price description reviews(first: 50) { edges { node { id rating text author { id name email reviews(first: 20) { edges { node { id rating text } } } } } } } } } } }"}'
    
    curl -X POST \
         -H "Content-Type: application/json" \
         -H "Authorization: Bearer $JWT_TOKEN" \
         -d "$COMPLEX_QUERY" \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/complex-query-test.json" 2>&1
    
    # Test large payload
    info "Testing large payload handling..."
    LARGE_TEXT=$(python3 -c "print('A' * 10000)")
    LARGE_PAYLOAD="{\"query\":\"mutation { createReview(input: {offerId: \\\"test\\\", rating: 5, text: \\\"$LARGE_TEXT\\\"}) { id } }\"}"
    
    curl -X POST \
         -H "Content-Type: application/json" \
         -H "Authorization: Bearer $JWT_TOKEN" \
         -d "$LARGE_PAYLOAD" \
         "$ROUTER_URL/graphql" \
         > "${RESULTS_DIR}/${TIMESTAMP}/large-payload-test.json" 2>&1
    
    success "DoS tests completed"
}

# Generate security report
generate_security_report() {
    log "Generating comprehensive security report..."
    
    local report_file="${RESULTS_DIR}/${TIMESTAMP}/security-audit-report.md"
    
    cat > "$report_file" << EOF
# Security Audit Report

**Target:** ${ROUTER_URL}
**Date:** $(date)
**Audit ID:** ${TIMESTAMP}

## Executive Summary

This report contains the results of a comprehensive security audit of the Apollo Router Federation setup, including tests for OWASP Top 10 vulnerabilities, GraphQL-specific security issues, and general web application security.

## Audit Scope

The security audit covered the following areas:

1. **Network Reconnaissance**
   - Port scanning and service detection
   - Vulnerability scanning
   - Network security assessment

2. **Web Application Security**
   - General web application vulnerabilities
   - HTTP security headers
   - SSL/TLS configuration

3. **GraphQL-Specific Security**
   - Introspection exposure
   - Query depth and complexity limits
   - Rate limiting
   - Authentication and authorization

4. **OWASP Top 10 (2021)**
   - A01: Broken Access Control
   - A02: Cryptographic Failures
   - A03: Injection
   - A04: Insecure Design
   - A05: Security Misconfiguration
   - A06: Vulnerable and Outdated Components
   - A07: Identification and Authentication Failures
   - A08: Software and Data Integrity Failures
   - A09: Security Logging and Monitoring Failures
   - A10: Server-Side Request Forgery (SSRF)

5. **Input Validation**
   - XSS prevention
   - Injection attack prevention
   - Data validation

6. **Denial of Service Protection**
   - Query complexity limits
   - Rate limiting
   - Resource exhaustion protection

## Test Results Summary

EOF
    
    # Add results from different test files
    if [ -f "${RESULTS_DIR}/${TIMESTAMP}/graphql-security-report.json" ]; then
        echo "### GraphQL Security Test Results" >> "$report_file"
        echo "" >> "$report_file"
        python3 -c "
import json
try:
    with open('${RESULTS_DIR}/${TIMESTAMP}/graphql-security-report.json') as f:
        data = json.load(f)
    print(f'- Security Score: {data.get(\"security_score\", \"N/A\")}/100')
    print(f'- Total Vulnerabilities: {data.get(\"total_vulnerabilities\", \"N/A\")}')
    severity_counts = data.get('severity_counts', {})
    for severity, count in severity_counts.items():
        if count > 0:
            print(f'- {severity}: {count}')
except:
    print('- GraphQL security report not available')
" >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    if [ -f "${RESULTS_DIR}/${TIMESTAMP}/owasp-top10-report.json" ]; then
        echo "### OWASP Top 10 Test Results" >> "$report_file"
        echo "" >> "$report_file"
        python3 -c "
import json
try:
    with open('${RESULTS_DIR}/${TIMESTAMP}/owasp-top10-report.json') as f:
        data = json.load(f)
    print(f'- OWASP Compliance Score: {data.get(\"owasp_compliance_score\", \"N/A\"):.1f}%')
    print(f'- Compliant Categories: {data.get(\"compliant_categories\", \"N/A\")}/{data.get(\"total_categories\", \"N/A\")}')
    print(f'- Total Findings: {data.get(\"total_findings\", \"N/A\")}')
except:
    print('- OWASP Top 10 report not available')
" >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    # Add recommendations
    cat >> "$report_file" << EOF
## Security Recommendations

Based on the audit findings, the following security improvements are recommended:

### High Priority
1. 🚨 Address all CRITICAL and HIGH severity vulnerabilities immediately
2. 🔒 Implement proper authentication and authorization controls
3. 🛡️ Enable input validation and sanitization
4. 🔐 Ensure JWT tokens are properly validated

### Medium Priority
1. ⚙️ Disable GraphQL introspection in production
2. ⏱️ Implement comprehensive rate limiting
3. 🔍 Add query complexity and depth limiting
4. 📊 Implement security monitoring and logging

### Low Priority
1. 🔧 Remove verbose error messages
2. 📦 Keep all dependencies up to date
3. 🛡️ Implement security headers
4. 📋 Regular security audits

## Detailed Findings

Detailed findings and evidence can be found in the following files:

- GraphQL Security Report: \`graphql-security-report.json\`
- OWASP Top 10 Report: \`owasp-top10-report.json\`
- Network Scan Results: \`nmap-*.txt\`
- Web Application Scan: \`nikto-scan.txt\`
- Authentication Tests: \`*-test.json\`
- Rate Limiting Tests: \`rate-limit-results.json\`

## Conclusion

This security audit provides a comprehensive assessment of the Apollo Router Federation security posture. All identified vulnerabilities should be addressed according to their severity levels, with critical and high-severity issues taking immediate priority.

Regular security audits should be conducted to maintain a strong security posture as the application evolves.

---

**Audit conducted by:** Security Audit Script
**Report generated:** $(date)
EOF
    
    success "Security report generated: $report_file"
}

# Generate HTML report
generate_html_report() {
    log "Generating HTML security report..."
    
    local html_file="${RESULTS_DIR}/${TIMESTAMP}/security-audit-report.html"
    
    cat > "$html_file" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Security Audit Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; line-height: 1.6; }
        .header { background: #f4f4f4; padding: 20px; border-radius: 5px; margin-bottom: 20px; }
        .critical { color: #dc3545; font-weight: bold; }
        .high { color: #fd7e14; font-weight: bold; }
        .medium { color: #ffc107; font-weight: bold; }
        .low { color: #28a745; font-weight: bold; }
        .info { color: #17a2b8; font-weight: bold; }
        .section { margin: 20px 0; padding: 15px; border-left: 4px solid #007bff; background: #f8f9fa; }
        .finding { margin: 10px 0; padding: 10px; border-radius: 3px; background: #fff; border: 1px solid #ddd; }
        .score { font-size: 24px; font-weight: bold; }
        .good-score { color: #28a745; }
        .medium-score { color: #ffc107; }
        .bad-score { color: #dc3545; }
        table { border-collapse: collapse; width: 100%; margin: 20px 0; }
        th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }
        th { background-color: #f2f2f2; }
        .recommendation { background: #e7f3ff; padding: 10px; margin: 5px 0; border-radius: 3px; }
    </style>
</head>
<body>
    <div class="header">
        <h1>🔒 Security Audit Report</h1>
        <p><strong>Target:</strong> TARGET_URL_PLACEHOLDER</p>
        <p><strong>Date:</strong> DATE_PLACEHOLDER</p>
        <p><strong>Audit ID:</strong> TIMESTAMP_PLACEHOLDER</p>
    </div>
    
    <div class="section">
        <h2>📊 Executive Summary</h2>
        <p>This report presents the findings of a comprehensive security audit conducted on the Apollo Router Federation setup. The audit covered OWASP Top 10 vulnerabilities, GraphQL-specific security issues, and general web application security best practices.</p>
        
        <div style="display: flex; gap: 20px; margin: 20px 0;">
            <div style="text-align: center; padding: 20px; background: white; border-radius: 5px; border: 1px solid #ddd;">
                <div class="score" id="overall-score">N/A</div>
                <div>Overall Security Score</div>
            </div>
            <div style="text-align: center; padding: 20px; background: white; border-radius: 5px; border: 1px solid #ddd;">
                <div class="score" id="owasp-score">N/A</div>
                <div>OWASP Compliance</div>
            </div>
            <div style="text-align: center; padding: 20px; background: white; border-radius: 5px; border: 1px solid #ddd;">
                <div class="score" id="total-findings">N/A</div>
                <div>Total Findings</div>
            </div>
        </div>
    </div>
    
    <div class="section">
        <h2>🎯 Audit Scope</h2>
        <ul>
            <li><strong>Network Security:</strong> Port scanning, service detection, vulnerability assessment</li>
            <li><strong>Web Application Security:</strong> General web vulnerabilities, HTTP headers, SSL/TLS</li>
            <li><strong>GraphQL Security:</strong> Introspection, query limits, authentication, authorization</li>
            <li><strong>OWASP Top 10:</strong> Comprehensive testing against OWASP Top 10 2021</li>
            <li><strong>Input Validation:</strong> XSS, injection attacks, data validation</li>
            <li><strong>DoS Protection:</strong> Query complexity, rate limiting, resource exhaustion</li>
        </ul>
    </div>
    
    <div class="section">
        <h2>🔍 Key Findings</h2>
        <div id="findings-summary">
            <p>Loading findings...</p>
        </div>
    </div>
    
    <div class="section">
        <h2>📋 OWASP Top 10 Assessment</h2>
        <table>
            <thead>
                <tr>
                    <th>Category</th>
                    <th>Description</th>
                    <th>Status</th>
                    <th>Findings</th>
                </tr>
            </thead>
            <tbody id="owasp-table">
                <tr><td colspan="4">Loading OWASP assessment...</td></tr>
            </tbody>
        </table>
    </div>
    
    <div class="section">
        <h2>🛡️ Security Recommendations</h2>
        <div id="recommendations">
            <div class="recommendation">
                <strong>🚨 Critical Priority:</strong> Address all CRITICAL and HIGH severity vulnerabilities immediately
            </div>
            <div class="recommendation">
                <strong>🔒 High Priority:</strong> Implement proper authentication and authorization controls
            </div>
            <div class="recommendation">
                <strong>🛡️ Medium Priority:</strong> Enable comprehensive input validation and sanitization
            </div>
            <div class="recommendation">
                <strong>📊 Ongoing:</strong> Implement security monitoring and regular audits
            </div>
        </div>
    </div>
    
    <div class="section">
        <h2>📁 Detailed Reports</h2>
        <p>The following detailed reports are available:</p>
        <ul>
            <li><a href="graphql-security-report.json">GraphQL Security Report (JSON)</a></li>
            <li><a href="owasp-top10-report.json">OWASP Top 10 Report (JSON)</a></li>
            <li><a href="nmap-scan.txt">Network Scan Results</a></li>
            <li><a href="nikto-scan.txt">Web Application Scan</a></li>
            <li><a href="rate-limit-results.json">Rate Limiting Test Results</a></li>
        </ul>
    </div>
    
    <div class="section">
        <h2>📞 Next Steps</h2>
        <ol>
            <li><strong>Immediate Action:</strong> Address all critical and high-severity vulnerabilities</li>
            <li><strong>Short Term:</strong> Implement recommended security controls</li>
            <li><strong>Medium Term:</strong> Establish security monitoring and alerting</li>
            <li><strong>Long Term:</strong> Regular security audits and penetration testing</li>
        </ol>
    </div>
    
    <script>
        // Load and display security data
        async function loadSecurityData() {
            try {
                // Load GraphQL security report
                const graphqlResponse = await fetch('graphql-security-report.json');
                if (graphqlResponse.ok) {
                    const graphqlData = await graphqlResponse.json();
                    document.getElementById('overall-score').textContent = graphqlData.security_score + '/100';
                    
                    const scoreElement = document.getElementById('overall-score');
                    if (graphqlData.security_score >= 80) {
                        scoreElement.className = 'score good-score';
                    } else if (graphqlData.security_score >= 60) {
                        scoreElement.className = 'score medium-score';
                    } else {
                        scoreElement.className = 'score bad-score';
                    }
                }
                
                // Load OWASP report
                const owaspResponse = await fetch('owasp-top10-report.json');
                if (owaspResponse.ok) {
                    const owaspData = await owaspResponse.json();
                    document.getElementById('owasp-score').textContent = owaspData.owasp_compliance_score.toFixed(1) + '%';
                    document.getElementById('total-findings').textContent = owaspData.total_findings;
                    
                    // Update OWASP table
                    const owaspTable = document.getElementById('owasp-table');
                    owaspTable.innerHTML = '';
                    
                    for (const [category, name] of Object.entries(owaspData.category_names)) {
                        const findings = owaspData.findings_by_category[category] || [];
                        const criticalHigh = findings.filter(f => ['CRITICAL', 'HIGH'].includes(f.severity));
                        
                        const row = owaspTable.insertRow();
                        row.innerHTML = `
                            <td>${category}</td>
                            <td>${name}</td>
                            <td>${criticalHigh.length === 0 ? '<span class="info">✅ Compliant</span>' : '<span class="critical">❌ Issues Found</span>'}</td>
                            <td>${criticalHigh.length} critical/high</td>
                        `;
                    }
                }
            } catch (error) {
                console.error('Error loading security data:', error);
            }
        }
        
        // Load data when page loads
        document.addEventListener('DOMContentLoaded', loadSecurityData);
    </script>
</body>
</html>
EOF
    
    # Replace placeholders
    sed -i "s/TARGET_URL_PLACEHOLDER/${ROUTER_URL//\//\\/}/g" "$html_file"
    sed -i "s/DATE_PLACEHOLDER/$(date)/g" "$html_file"
    sed -i "s/TIMESTAMP_PLACEHOLDER/${TIMESTAMP}/g" "$html_file"
    
    success "HTML report generated: $html_file"
}

# Cleanup function
cleanup() {
    log "Cleaning up security audit..."
    # Remove temporary files if needed
    success "Cleanup completed"
}

# Main execution
main() {
    log "Starting Apollo Router Federation Security Audit"
    
    # Set trap for cleanup
    trap cleanup EXIT
    
    # Run audit phases
    check_prerequisites
    setup_audit_environment
    
    # Run different types of security tests
    run_network_reconnaissance
    run_web_app_scanning
    run_graphql_security_tests
    run_owasp_top10_tests
    run_sql_injection_tests
    run_auth_tests
    run_rate_limiting_tests
    run_input_validation_tests
    run_dos_tests
    
    # Generate reports
    generate_security_report
    generate_html_report
    
    success "Security audit completed successfully!"
    log "Results available in: ${RESULTS_DIR}/${TIMESTAMP}"
    
    # Check for critical findings
    if [ -f "${RESULTS_DIR}/${TIMESTAMP}/graphql-security-report.json" ]; then
        CRITICAL_COUNT=$(python3 -c "
import json
try:
    with open('${RESULTS_DIR}/${TIMESTAMP}/graphql-security-report.json') as f:
        data = json.load(f)
    print(data.get('severity_counts', {}).get('CRITICAL', 0))
except:
    print(0)
")
        
        if [ "$CRITICAL_COUNT" -gt 0 ]; then
            error "🚨 $CRITICAL_COUNT CRITICAL vulnerabilities found!"
            error "Immediate action required!"
            exit 1
        fi
    fi
}

# Handle command line arguments
case "${1:-}" in
    "graphql")
        check_prerequisites
        setup_audit_environment
        run_graphql_security_tests
        ;;
    "owasp")
        check_prerequisites
        setup_audit_environment
        run_owasp_top10_tests
        ;;
    "network")
        check_prerequisites
        setup_audit_environment
        run_network_reconnaissance
        ;;
    "webapp")
        check_prerequisites
        setup_audit_environment
        run_web_app_scanning
        ;;
    "help"|"-h"|"--help")
        echo "Usage: $0 [graphql|owasp|network|webapp|help]"
        echo ""
        echo "Options:"
        echo "  graphql   Run only GraphQL security tests"
        echo "  owasp     Run only OWASP Top 10 tests"
        echo "  network   Run only network reconnaissance"
        echo "  webapp    Run only web application scanning"
        echo "  help      Show this help message"
        echo ""
        echo "Environment variables:"
        echo "  ROUTER_URL    Router URL (default: http://localhost:4000)"
        echo "  JWT_TOKEN     JWT token for authentication"
        exit 0
        ;;
    *)
        main
        ;;
esac