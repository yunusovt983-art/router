#!/usr/bin/env python3
"""
Security Scanner for Apollo Router Federation

This script performs comprehensive security testing including:
- OWASP Top 10 vulnerabilities
- GraphQL-specific security issues
- Authentication and authorization testing
- Input validation testing
- Rate limiting verification
"""

import requests
import json
import time
import argparse
import sys
from typing import Dict, List, Any, Optional
from urllib.parse import urljoin
import base64
import hashlib
import random
import string

class SecurityScanner:
    def __init__(self, base_url: str, auth_token: Optional[str] = None):
        self.base_url = base_url.rstrip('/')
        self.graphql_url = urljoin(self.base_url, '/graphql')
        self.auth_token = auth_token
        self.session = requests.Session()
        self.vulnerabilities = []
        self.test_results = {}
        
        # Set default headers
        self.session.headers.update({
            'Content-Type': 'application/json',
            'User-Agent': 'SecurityScanner/1.0'
        })
        
        if auth_token:
            self.session.headers['Authorization'] = f'Bearer {auth_token}'
    
    def log_vulnerability(self, severity: str, title: str, description: str, evidence: str = ""):
        """Log a discovered vulnerability"""
        vulnerability = {
            'severity': severity,
            'title': title,
            'description': description,
            'evidence': evidence,
            'timestamp': time.time()
        }
        self.vulnerabilities.append(vulnerability)
        
        # Color coding for console output
        colors = {
            'CRITICAL': '\033[91m',  # Red
            'HIGH': '\033[93m',      # Yellow
            'MEDIUM': '\033[94m',    # Blue
            'LOW': '\033[92m',       # Green
            'INFO': '\033[96m'       # Cyan
        }
        reset_color = '\033[0m'
        
        color = colors.get(severity, '')
        print(f"{color}[{severity}] {title}{reset_color}")
        print(f"  {description}")
        if evidence:
            print(f"  Evidence: {evidence}")
        print()
    
    def make_graphql_request(self, query: str, variables: Dict = None, headers: Dict = None) -> requests.Response:
        """Make a GraphQL request"""
        payload = {
            'query': query,
            'variables': variables or {}
        }
        
        request_headers = self.session.headers.copy()
        if headers:
            request_headers.update(headers)
        
        return self.session.post(self.graphql_url, json=payload, headers=request_headers)
    
    def test_graphql_introspection(self):
        """Test GraphQL introspection availability"""
        print("🔍 Testing GraphQL Introspection...")
        
        introspection_query = """
        query IntrospectionQuery {
          __schema {
            queryType { name }
            mutationType { name }
            subscriptionType { name }
            types {
              ...FullType
            }
            directives {
              name
              description
              locations
              args {
                ...InputValue
              }
            }
          }
        }
        
        fragment FullType on __Type {
          kind
          name
          description
          fields(includeDeprecated: true) {
            name
            description
            args {
              ...InputValue
            }
            type {
              ...TypeRef
            }
            isDeprecated
            deprecationReason
          }
          inputFields {
            ...InputValue
          }
          interfaces {
            ...TypeRef
          }
          enumValues(includeDeprecated: true) {
            name
            description
            isDeprecated
            deprecationReason
          }
          possibleTypes {
            ...TypeRef
          }
        }
        
        fragment InputValue on __InputValue {
          name
          description
          type { ...TypeRef }
          defaultValue
        }
        
        fragment TypeRef on __Type {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
                ofType {
                  kind
                  name
                  ofType {
                    kind
                    name
                    ofType {
                      kind
                      name
                      ofType {
                        kind
                        name
                      }
                    }
                  }
                }
              }
            }
          }
        }
        """
        
        response = self.make_graphql_request(introspection_query)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and '__schema' in data['data']:
                    self.log_vulnerability(
                        'MEDIUM',
                        'GraphQL Introspection Enabled',
                        'GraphQL introspection is enabled, which exposes the entire schema structure. '
                        'This should be disabled in production environments.',
                        f'Schema types discovered: {len(data["data"]["__schema"]["types"])}'
                    )
                    return data['data']['__schema']
            except json.JSONDecodeError:
                pass
        
        self.log_vulnerability(
            'INFO',
            'GraphQL Introspection Disabled',
            'GraphQL introspection is properly disabled.',
            ''
        )
        return None
    
    def test_query_depth_limiting(self):
        """Test query depth limiting"""
        print("🔍 Testing Query Depth Limiting...")
        
        # Create a deeply nested query
        deep_query = """
        query DeepQuery {
          offers(first: 1) {
            edges {
              node {
                reviews(first: 1) {
                  edges {
                    node {
                      author {
                        reviews(first: 1) {
                          edges {
                            node {
                              offer {
                                reviews(first: 1) {
                                  edges {
                                    node {
                                      author {
                                        reviews(first: 1) {
                                          edges {
                                            node {
                                              offer {
                                                reviews(first: 1) {
                                                  edges {
                                                    node {
                                                      id
                                                    }
                                                  }
                                                }
                                              }
                                            }
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
        """
        
        response = self.make_graphql_request(deep_query)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'errors' in data:
                    # Check if error is related to query depth
                    for error in data['errors']:
                        if 'depth' in error.get('message', '').lower():
                            self.log_vulnerability(
                                'INFO',
                                'Query Depth Limiting Enabled',
                                'Query depth limiting is properly configured.',
                                f'Depth limit error: {error["message"]}'
                            )
                            return
                
                # If no depth limit error, it might be vulnerable
                if 'data' in data:
                    self.log_vulnerability(
                        'HIGH',
                        'No Query Depth Limiting',
                        'Deep nested queries are allowed, which could lead to DoS attacks.',
                        'Deep query executed successfully without depth limit error'
                    )
            except json.JSONDecodeError:
                pass
    
    def test_query_complexity_limiting(self):
        """Test query complexity limiting"""
        print("🔍 Testing Query Complexity Limiting...")
        
        # Create a complex query with many fields
        complex_query = """
        query ComplexQuery {
          offers(first: 100) {
            edges {
              node {
                id
                title
                price
                description
                createdAt
                updatedAt
                reviews(first: 50) {
                  edges {
                    node {
                      id
                      rating
                      text
                      createdAt
                      updatedAt
                      author {
                        id
                        name
                        email
                        createdAt
                        reviews(first: 20) {
                          edges {
                            node {
                              id
                              rating
                              text
                              createdAt
                            }
                          }
                        }
                      }
                    }
                  }
                }
                seller {
                  id
                  name
                  email
                  phone
                  createdAt
                  offers(first: 30) {
                    edges {
                      node {
                        id
                        title
                        price
                        reviews(first: 10) {
                          edges {
                            node {
                              id
                              rating
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
        """
        
        response = self.make_graphql_request(complex_query)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'errors' in data:
                    for error in data['errors']:
                        if any(keyword in error.get('message', '').lower() 
                              for keyword in ['complexity', 'cost', 'limit']):
                            self.log_vulnerability(
                                'INFO',
                                'Query Complexity Limiting Enabled',
                                'Query complexity limiting is properly configured.',
                                f'Complexity limit error: {error["message"]}'
                            )
                            return
                
                if 'data' in data:
                    self.log_vulnerability(
                        'HIGH',
                        'No Query Complexity Limiting',
                        'Complex queries are allowed without limits, which could lead to resource exhaustion.',
                        'Complex query executed successfully without complexity limit error'
                    )
            except json.JSONDecodeError:
                pass
    
    def test_rate_limiting(self):
        """Test rate limiting"""
        print("🔍 Testing Rate Limiting...")
        
        simple_query = """
        query SimpleQuery {
          __typename
        }
        """
        
        # Send rapid requests to test rate limiting
        request_count = 100
        successful_requests = 0
        rate_limited_requests = 0
        
        start_time = time.time()
        
        for i in range(request_count):
            response = self.make_graphql_request(simple_query)
            
            if response.status_code == 200:
                successful_requests += 1
            elif response.status_code == 429:  # Too Many Requests
                rate_limited_requests += 1
            elif response.status_code == 403:  # Forbidden (might be rate limiting)
                try:
                    data = response.json()
                    if 'rate' in str(data).lower() or 'limit' in str(data).lower():
                        rate_limited_requests += 1
                except:
                    pass
            
            # Small delay to avoid overwhelming the server
            time.sleep(0.01)
        
        end_time = time.time()
        duration = end_time - start_time
        requests_per_second = request_count / duration
        
        if rate_limited_requests > 0:
            self.log_vulnerability(
                'INFO',
                'Rate Limiting Enabled',
                f'Rate limiting is working. {rate_limited_requests} out of {request_count} requests were rate limited.',
                f'RPS attempted: {requests_per_second:.2f}, Rate limited: {rate_limited_requests}'
            )
        else:
            self.log_vulnerability(
                'MEDIUM',
                'No Rate Limiting Detected',
                f'No rate limiting detected. All {successful_requests} requests succeeded.',
                f'RPS achieved: {requests_per_second:.2f} without rate limiting'
            )
    
    def test_authentication_bypass(self):
        """Test authentication bypass vulnerabilities"""
        print("🔍 Testing Authentication Bypass...")
        
        # Test without authentication token
        protected_query = """
        mutation CreateReview($input: CreateReviewInput!) {
          createReview(input: $input) {
            id
            rating
            text
          }
        }
        """
        
        variables = {
            "input": {
                "offerId": "550e8400-e29b-41d4-a716-446655440001",
                "rating": 5,
                "text": "Security test review"
            }
        }
        
        # Remove auth token temporarily
        original_auth = self.session.headers.get('Authorization')
        if 'Authorization' in self.session.headers:
            del self.session.headers['Authorization']
        
        response = self.make_graphql_request(protected_query, variables)
        
        # Restore auth token
        if original_auth:
            self.session.headers['Authorization'] = original_auth
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('createReview'):
                    self.log_vulnerability(
                        'CRITICAL',
                        'Authentication Bypass',
                        'Protected mutations can be executed without authentication.',
                        f'Created review without auth: {data["data"]["createReview"]["id"]}'
                    )
                elif 'errors' in data:
                    # Check if error is authentication-related
                    for error in data['errors']:
                        if any(keyword in error.get('message', '').lower() 
                              for keyword in ['unauthorized', 'authentication', 'token']):
                            self.log_vulnerability(
                                'INFO',
                                'Authentication Required',
                                'Authentication is properly enforced for protected operations.',
                                f'Auth error: {error["message"]}'
                            )
                            return
            except json.JSONDecodeError:
                pass
    
    def test_authorization_bypass(self):
        """Test authorization bypass vulnerabilities"""
        print("🔍 Testing Authorization Bypass...")
        
        # Test accessing other user's data
        other_user_query = """
        query GetUserReviews($userId: ID!) {
          user(id: $userId) {
            id
            email
            phone
            reviews {
              edges {
                node {
                  id
                  text
                }
              }
            }
          }
        }
        """
        
        # Try to access another user's sensitive data
        variables = {"userId": "660e8400-e29b-41d4-a716-446655440999"}
        
        response = self.make_graphql_request(other_user_query, variables)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('user'):
                    user_data = data['data']['user']
                    if user_data.get('email') or user_data.get('phone'):
                        self.log_vulnerability(
                            'HIGH',
                            'Authorization Bypass - Sensitive Data Exposure',
                            'Sensitive user data (email, phone) can be accessed without proper authorization.',
                            f'Exposed data: email={user_data.get("email")}, phone={user_data.get("phone")}'
                        )
            except json.JSONDecodeError:
                pass
    
    def test_sql_injection(self):
        """Test SQL injection vulnerabilities"""
        print("🔍 Testing SQL Injection...")
        
        # SQL injection payloads
        sql_payloads = [
            "' OR '1'='1",
            "'; DROP TABLE reviews; --",
            "' UNION SELECT * FROM users --",
            "1' OR 1=1 --",
            "'; SELECT pg_sleep(5); --"
        ]
        
        for payload in sql_payloads:
            query = """
            query TestSQLInjection($id: ID!) {
              review(id: $id) {
                id
                rating
                text
              }
            }
            """
            
            variables = {"id": payload}
            
            start_time = time.time()
            response = self.make_graphql_request(query, variables)
            end_time = time.time()
            
            # Check for SQL injection indicators
            if response.status_code == 200:
                try:
                    data = response.json()
                    response_text = json.dumps(data).lower()
                    
                    # Check for SQL error messages
                    sql_errors = [
                        'syntax error', 'mysql', 'postgresql', 'ora-', 'microsoft',
                        'driver', 'odbc', 'jdbc', 'sql server', 'pg_'
                    ]
                    
                    for error_pattern in sql_errors:
                        if error_pattern in response_text:
                            self.log_vulnerability(
                                'CRITICAL',
                                'SQL Injection Vulnerability',
                                f'SQL injection detected with payload: {payload}',
                                f'Error pattern found: {error_pattern}'
                            )
                            return
                    
                    # Check for time-based SQL injection (sleep payload)
                    if 'pg_sleep' in payload and (end_time - start_time) > 4:
                        self.log_vulnerability(
                            'CRITICAL',
                            'Time-based SQL Injection',
                            f'Time-based SQL injection detected with payload: {payload}',
                            f'Response time: {end_time - start_time:.2f}s'
                        )
                        return
                        
                except json.JSONDecodeError:
                    pass
    
    def test_nosql_injection(self):
        """Test NoSQL injection vulnerabilities"""
        print("🔍 Testing NoSQL Injection...")
        
        # NoSQL injection payloads
        nosql_payloads = [
            {"$ne": None},
            {"$gt": ""},
            {"$regex": ".*"},
            {"$where": "1==1"},
            {"$or": [{"rating": 1}, {"rating": 5}]}
        ]
        
        for payload in nosql_payloads:
            query = """
            query TestNoSQLInjection($filter: ReviewsFilter) {
              reviews(filter: $filter, first: 10) {
                edges {
                  node {
                    id
                    rating
                  }
                }
              }
            }
            """
            
            variables = {"filter": payload}
            
            response = self.make_graphql_request(query, variables)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('reviews'):
                        reviews = data['data']['reviews']['edges']
                        if len(reviews) > 0:
                            self.log_vulnerability(
                                'HIGH',
                                'Potential NoSQL Injection',
                                f'NoSQL injection payload returned data: {payload}',
                                f'Returned {len(reviews)} reviews'
                            )
                except json.JSONDecodeError:
                    pass
    
    def test_xss_vulnerabilities(self):
        """Test Cross-Site Scripting vulnerabilities"""
        print("🔍 Testing XSS Vulnerabilities...")
        
        xss_payloads = [
            "<script>alert('XSS')</script>",
            "javascript:alert('XSS')",
            "<img src=x onerror=alert('XSS')>",
            "';alert('XSS');//",
            "<svg onload=alert('XSS')>"
        ]
        
        for payload in xss_payloads:
            # Test XSS in review creation
            mutation = """
            mutation CreateReview($input: CreateReviewInput!) {
              createReview(input: $input) {
                id
                text
              }
            }
            """
            
            variables = {
                "input": {
                    "offerId": "550e8400-e29b-41d4-a716-446655440001",
                    "rating": 5,
                    "text": payload
                }
            }
            
            response = self.make_graphql_request(mutation, variables)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('createReview'):
                        review_text = data['data']['createReview'].get('text', '')
                        if payload in review_text:
                            self.log_vulnerability(
                                'HIGH',
                                'Stored XSS Vulnerability',
                                f'XSS payload was stored without sanitization: {payload}',
                                f'Review ID: {data["data"]["createReview"]["id"]}'
                            )
                except json.JSONDecodeError:
                    pass
    
    def test_information_disclosure(self):
        """Test information disclosure vulnerabilities"""
        print("🔍 Testing Information Disclosure...")
        
        # Test error message disclosure
        invalid_queries = [
            "query { nonExistentField }",
            "query { user(id: \"invalid-uuid\") { id } }",
            "mutation { nonExistentMutation }",
            "query { review(id: null) { id } }"
        ]
        
        for query in invalid_queries:
            response = self.make_graphql_request(query)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'errors' in data:
                        for error in data['errors']:
                            error_message = error.get('message', '').lower()
                            
                            # Check for sensitive information in error messages
                            sensitive_patterns = [
                                'database', 'sql', 'connection', 'server',
                                'internal', 'stack trace', 'file path',
                                'password', 'secret', 'token'
                            ]
                            
                            for pattern in sensitive_patterns:
                                if pattern in error_message:
                                    self.log_vulnerability(
                                        'MEDIUM',
                                        'Information Disclosure in Error Messages',
                                        f'Error message contains sensitive information: {pattern}',
                                        f'Error: {error["message"]}'
                                    )
                except json.JSONDecodeError:
                    pass
    
    def test_csrf_protection(self):
        """Test CSRF protection"""
        print("🔍 Testing CSRF Protection...")
        
        # Test if mutations can be executed via GET request
        mutation_query = """
        mutation CreateReview($input: CreateReviewInput!) {
          createReview(input: $input) {
            id
          }
        }
        """
        
        variables = {
            "input": {
                "offerId": "550e8400-e29b-41d4-a716-446655440001",
                "rating": 5,
                "text": "CSRF test review"
            }
        }
        
        # Try to execute mutation via GET request
        params = {
            'query': mutation_query,
            'variables': json.dumps(variables)
        }
        
        response = self.session.get(self.graphql_url, params=params)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('createReview'):
                    self.log_vulnerability(
                        'HIGH',
                        'CSRF Vulnerability',
                        'Mutations can be executed via GET requests, making CSRF attacks possible.',
                        f'Created review via GET: {data["data"]["createReview"]["id"]}'
                    )
            except json.JSONDecodeError:
                pass
    
    def test_dos_vulnerabilities(self):
        """Test Denial of Service vulnerabilities"""
        print("🔍 Testing DoS Vulnerabilities...")
        
        # Test resource exhaustion with large queries
        large_query = """
        query LargeQuery {
          offers(first: 1000) {
            edges {
              node {
                id
                title
                description
                reviews(first: 100) {
                  edges {
                    node {
                      id
                      text
                      author {
                        id
                        name
                        reviews(first: 50) {
                          edges {
                            node {
                              id
                              text
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
        """
        
        start_time = time.time()
        response = self.make_graphql_request(large_query)
        end_time = time.time()
        
        response_time = end_time - start_time
        
        if response.status_code == 200 and response_time > 10:
            self.log_vulnerability(
                'MEDIUM',
                'Potential DoS via Resource Exhaustion',
                f'Large query took {response_time:.2f} seconds to execute, indicating potential DoS vulnerability.',
                f'Query response time: {response_time:.2f}s'
            )
    
    def test_jwt_vulnerabilities(self):
        """Test JWT token vulnerabilities"""
        print("🔍 Testing JWT Vulnerabilities...")
        
        if not self.auth_token:
            self.log_vulnerability(
                'INFO',
                'No JWT Token Provided',
                'JWT vulnerability tests skipped - no token provided.',
                ''
            )
            return
        
        # Test with modified JWT token
        try:
            # Decode JWT (without verification for testing)
            parts = self.auth_token.split('.')
            if len(parts) == 3:
                header = json.loads(base64.urlsafe_b64decode(parts[0] + '=='))
                payload = json.loads(base64.urlsafe_b64decode(parts[1] + '=='))
                
                # Test with modified payload
                modified_payload = payload.copy()
                modified_payload['roles'] = ['admin', 'moderator']  # Privilege escalation
                
                modified_payload_b64 = base64.urlsafe_b64encode(
                    json.dumps(modified_payload).encode()
                ).decode().rstrip('=')
                
                modified_token = f"{parts[0]}.{modified_payload_b64}.{parts[2]}"
                
                # Test with modified token
                test_query = """
                query TestPrivilegeEscalation {
                  __typename
                }
                """
                
                response = self.make_graphql_request(
                    test_query, 
                    headers={'Authorization': f'Bearer {modified_token}'}
                )
                
                if response.status_code == 200:
                    try:
                        data = response.json()
                        if 'data' in data:
                            self.log_vulnerability(
                                'CRITICAL',
                                'JWT Signature Not Verified',
                                'Modified JWT tokens are accepted, indicating signature verification is not implemented.',
                                f'Modified token accepted with roles: {modified_payload["roles"]}'
                            )
                    except json.JSONDecodeError:
                        pass
                        
        except Exception as e:
            self.log_vulnerability(
                'INFO',
                'JWT Token Analysis Failed',
                f'Could not analyze JWT token: {str(e)}',
                ''
            )
    
    def generate_report(self) -> Dict[str, Any]:
        """Generate security audit report"""
        
        # Categorize vulnerabilities by severity
        severity_counts = {'CRITICAL': 0, 'HIGH': 0, 'MEDIUM': 0, 'LOW': 0, 'INFO': 0}
        for vuln in self.vulnerabilities:
            severity_counts[vuln['severity']] += 1
        
        # Calculate security score (0-100)
        total_issues = sum(severity_counts.values()) - severity_counts['INFO']
        if total_issues == 0:
            security_score = 100
        else:
            # Weight different severities
            weighted_score = (
                severity_counts['CRITICAL'] * 10 +
                severity_counts['HIGH'] * 5 +
                severity_counts['MEDIUM'] * 2 +
                severity_counts['LOW'] * 1
            )
            security_score = max(0, 100 - weighted_score)
        
        report = {
            'scan_timestamp': time.time(),
            'target_url': self.base_url,
            'security_score': security_score,
            'severity_counts': severity_counts,
            'total_vulnerabilities': len(self.vulnerabilities),
            'vulnerabilities': self.vulnerabilities,
            'recommendations': self.generate_recommendations()
        }
        
        return report
    
    def generate_recommendations(self) -> List[str]:
        """Generate security recommendations based on findings"""
        recommendations = []
        
        # Check for critical issues
        critical_issues = [v for v in self.vulnerabilities if v['severity'] == 'CRITICAL']
        if critical_issues:
            recommendations.append("🚨 URGENT: Address all CRITICAL vulnerabilities immediately")
            recommendations.append("🔒 Implement proper input validation and sanitization")
            recommendations.append("🛡️ Enable JWT signature verification")
        
        # Check for high issues
        high_issues = [v for v in self.vulnerabilities if v['severity'] == 'HIGH']
        if high_issues:
            recommendations.append("⚠️ Address HIGH severity vulnerabilities as priority")
            recommendations.append("🔐 Implement proper authorization checks")
            recommendations.append("🚫 Add query complexity and depth limiting")
        
        # General recommendations
        recommendations.extend([
            "🔍 Disable GraphQL introspection in production",
            "⏱️ Implement rate limiting for all endpoints",
            "🛡️ Add CSRF protection for mutations",
            "📝 Sanitize all user inputs to prevent XSS",
            "🔒 Use parameterized queries to prevent SQL injection",
            "📊 Implement comprehensive security monitoring",
            "🔄 Regular security audits and penetration testing",
            "📚 Security training for development team"
        ])
        
        return recommendations
    
    def run_full_scan(self):
        """Run complete security scan"""
        print("🔒 Starting Security Audit...")
        print(f"Target: {self.base_url}")
        print("=" * 50)
        
        # Run all security tests
        test_methods = [
            self.test_graphql_introspection,
            self.test_query_depth_limiting,
            self.test_query_complexity_limiting,
            self.test_rate_limiting,
            self.test_authentication_bypass,
            self.test_authorization_bypass,
            self.test_sql_injection,
            self.test_nosql_injection,
            self.test_xss_vulnerabilities,
            self.test_information_disclosure,
            self.test_csrf_protection,
            self.test_dos_vulnerabilities,
            self.test_jwt_vulnerabilities
        ]
        
        for test_method in test_methods:
            try:
                test_method()
            except Exception as e:
                print(f"❌ Error in {test_method.__name__}: {str(e)}")
        
        print("=" * 50)
        print("🔒 Security Audit Complete")
        
        return self.generate_report()

def main():
    parser = argparse.ArgumentParser(description='Security Scanner for Apollo Router Federation')
    parser.add_argument('url', help='Base URL of the Apollo Router')
    parser.add_argument('--token', '-t', help='JWT authentication token')
    parser.add_argument('--output', '-o', help='Output file for security report (JSON)')
    parser.add_argument('--format', '-f', choices=['json', 'html'], default='json', help='Output format')
    
    args = parser.parse_args()
    
    scanner = SecurityScanner(args.url, args.token)
    report = scanner.run_full_scan()
    
    # Print summary
    print(f"\n📊 Security Score: {report['security_score']}/100")
    print(f"🔍 Total Issues Found: {report['total_vulnerabilities']}")
    print(f"🚨 Critical: {report['severity_counts']['CRITICAL']}")
    print(f"⚠️  High: {report['severity_counts']['HIGH']}")
    print(f"📋 Medium: {report['severity_counts']['MEDIUM']}")
    print(f"ℹ️  Low: {report['severity_counts']['LOW']}")
    
    # Save report
    if args.output:
        if args.format == 'json':
            with open(args.output, 'w') as f:
                json.dump(report, f, indent=2)
        elif args.format == 'html':
            # Generate HTML report (simplified)
            html_content = f"""
            <!DOCTYPE html>
            <html>
            <head>
                <title>Security Audit Report</title>
                <style>
                    body {{ font-family: Arial, sans-serif; margin: 40px; }}
                    .critical {{ color: #dc3545; }}
                    .high {{ color: #fd7e14; }}
                    .medium {{ color: #ffc107; }}
                    .low {{ color: #28a745; }}
                    .info {{ color: #17a2b8; }}
                </style>
            </head>
            <body>
                <h1>Security Audit Report</h1>
                <p><strong>Target:</strong> {report['target_url']}</p>
                <p><strong>Security Score:</strong> {report['security_score']}/100</p>
                <h2>Vulnerabilities Found</h2>
                <ul>
            """
            
            for vuln in report['vulnerabilities']:
                severity_class = vuln['severity'].lower()
                html_content += f"""
                    <li class="{severity_class}">
                        <strong>[{vuln['severity']}] {vuln['title']}</strong><br>
                        {vuln['description']}<br>
                        <em>Evidence: {vuln['evidence']}</em>
                    </li>
                """
            
            html_content += """
                </ul>
                <h2>Recommendations</h2>
                <ul>
            """
            
            for rec in report['recommendations']:
                html_content += f"<li>{rec}</li>"
            
            html_content += """
                </ul>
            </body>
            </html>
            """
            
            with open(args.output, 'w') as f:
                f.write(html_content)
        
        print(f"📄 Report saved to: {args.output}")
    
    # Exit with error code if critical or high vulnerabilities found
    if report['severity_counts']['CRITICAL'] > 0 or report['severity_counts']['HIGH'] > 0:
        sys.exit(1)

if __name__ == '__main__':
    main()