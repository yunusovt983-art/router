#!/usr/bin/env python3
"""
OWASP Top 10 Security Tests for Apollo Router Federation

This script specifically tests for OWASP Top 10 vulnerabilities:
1. A01:2021 – Broken Access Control
2. A02:2021 – Cryptographic Failures
3. A03:2021 – Injection
4. A04:2021 – Insecure Design
5. A05:2021 – Security Misconfiguration
6. A06:2021 – Vulnerable and Outdated Components
7. A07:2021 – Identification and Authentication Failures
8. A08:2021 – Software and Data Integrity Failures
9. A09:2021 – Security Logging and Monitoring Failures
10. A10:2021 – Server-Side Request Forgery (SSRF)
"""

import requests
import json
import time
import base64
import hashlib
import random
import string
from typing import Dict, List, Any, Optional
from urllib.parse import urljoin, urlparse
import subprocess
import os

class OWASPTop10Tester:
    def __init__(self, base_url: str, auth_token: Optional[str] = None):
        self.base_url = base_url.rstrip('/')
        self.graphql_url = urljoin(self.base_url, '/graphql')
        self.auth_token = auth_token
        self.session = requests.Session()
        self.findings = []
        
        self.session.headers.update({
            'Content-Type': 'application/json',
            'User-Agent': 'OWASP-Top10-Tester/1.0'
        })
        
        if auth_token:
            self.session.headers['Authorization'] = f'Bearer {auth_token}'
    
    def log_finding(self, owasp_category: str, severity: str, title: str, description: str, evidence: str = ""):
        """Log an OWASP Top 10 finding"""
        finding = {
            'owasp_category': owasp_category,
            'severity': severity,
            'title': title,
            'description': description,
            'evidence': evidence,
            'timestamp': time.time()
        }
        self.findings.append(finding)
        
        print(f"[{owasp_category}] [{severity}] {title}")
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
    
    def test_a01_broken_access_control(self):
        """A01:2021 – Broken Access Control"""
        print("🔍 Testing A01:2021 – Broken Access Control")
        
        # Test 1: Vertical privilege escalation
        admin_query = """
        mutation ModerateReview($reviewId: ID!, $status: ModerationStatus!) {
          moderateReview(reviewId: $reviewId, status: $status) {
            id
            moderationStatus
          }
        }
        """
        
        variables = {
            "reviewId": "770e8400-e29b-41d4-a716-446655440001",
            "status": "APPROVED"
        }
        
        response = self.make_graphql_request(admin_query, variables)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('moderateReview'):
                    self.log_finding(
                        'A01:2021',
                        'CRITICAL',
                        'Vertical Privilege Escalation',
                        'Regular users can perform admin operations (review moderation)',
                        f'Moderated review: {data["data"]["moderateReview"]["id"]}'
                    )
            except json.JSONDecodeError:
                pass
        
        # Test 2: Horizontal privilege escalation
        other_user_data_query = """
        query GetUserPrivateData($userId: ID!) {
          user(id: $userId) {
            id
            email
            phone
            address
            paymentMethods {
              id
              cardNumber
            }
          }
        }
        """
        
        # Try to access another user's private data
        variables = {"userId": "660e8400-e29b-41d4-a716-446655440999"}
        response = self.make_graphql_request(other_user_data_query, variables)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('user'):
                    user_data = data['data']['user']
                    sensitive_fields = ['email', 'phone', 'address', 'paymentMethods']
                    exposed_fields = [field for field in sensitive_fields if user_data.get(field)]
                    
                    if exposed_fields:
                        self.log_finding(
                            'A01:2021',
                            'HIGH',
                            'Horizontal Privilege Escalation',
                            'Users can access other users\' private data',
                            f'Exposed fields: {exposed_fields}'
                        )
            except json.JSONDecodeError:
                pass
        
        # Test 3: Direct object reference
        direct_access_query = """
        query DirectAccess($id: ID!) {
          review(id: $id) {
            id
            text
            author {
              email
              phone
            }
          }
        }
        """
        
        # Try to access reviews directly without authorization
        test_ids = [
            "770e8400-e29b-41d4-a716-446655440001",
            "770e8400-e29b-41d4-a716-446655440002",
            "770e8400-e29b-41d4-a716-446655440003"
        ]
        
        accessible_reviews = 0
        for review_id in test_ids:
            response = self.make_graphql_request(direct_access_query, {"id": review_id})
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('review'):
                        accessible_reviews += 1
                except json.JSONDecodeError:
                    pass
        
        if accessible_reviews > 0:
            self.log_finding(
                'A01:2021',
                'MEDIUM',
                'Insecure Direct Object References',
                'Reviews can be accessed directly without proper authorization checks',
                f'Accessible reviews: {accessible_reviews}/{len(test_ids)}'
            )
    
    def test_a02_cryptographic_failures(self):
        """A02:2021 – Cryptographic Failures"""
        print("🔍 Testing A02:2021 – Cryptographic Failures")
        
        # Test 1: Check if HTTPS is enforced
        if self.base_url.startswith('http://'):
            self.log_finding(
                'A02:2021',
                'HIGH',
                'Unencrypted Communication',
                'Application is accessible over HTTP instead of HTTPS',
                f'URL: {self.base_url}'
            )
        
        # Test 2: Weak JWT implementation
        if self.auth_token:
            try:
                parts = self.auth_token.split('.')
                if len(parts) == 3:
                    header = json.loads(base64.urlsafe_b64decode(parts[0] + '=='))
                    
                    # Check for weak algorithms
                    alg = header.get('alg', '').upper()
                    weak_algorithms = ['NONE', 'HS256']  # HS256 can be weak if secret is compromised
                    
                    if alg in weak_algorithms:
                        self.log_finding(
                            'A02:2021',
                            'MEDIUM',
                            'Weak JWT Algorithm',
                            f'JWT uses potentially weak algorithm: {alg}',
                            f'Algorithm: {alg}'
                        )
            except Exception:
                pass
        
        # Test 3: Password storage (if applicable)
        password_test_mutation = """
        mutation ChangePassword($oldPassword: String!, $newPassword: String!) {
          changePassword(oldPassword: $oldPassword, newPassword: $newPassword) {
            success
            message
          }
        }
        """
        
        # Test with weak password
        variables = {
            "oldPassword": "oldpass123",
            "newPassword": "123"  # Very weak password
        }
        
        response = self.make_graphql_request(password_test_mutation, variables)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('changePassword', {}).get('success'):
                    self.log_finding(
                        'A02:2021',
                        'MEDIUM',
                        'Weak Password Policy',
                        'System accepts very weak passwords',
                        'Accepted password: "123"'
                    )
            except json.JSONDecodeError:
                pass
    
    def test_a03_injection(self):
        """A03:2021 – Injection"""
        print("🔍 Testing A03:2021 – Injection")
        
        # Test 1: SQL Injection
        sql_payloads = [
            "' OR '1'='1' --",
            "'; DROP TABLE reviews; --",
            "' UNION SELECT password FROM users --",
            "1' OR 1=1 --"
        ]
        
        for payload in sql_payloads:
            query = """
            query SQLInjectionTest($id: ID!) {
              review(id: $id) {
                id
                text
              }
            }
            """
            
            response = self.make_graphql_request(query, {"id": payload})
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    response_text = json.dumps(data).lower()
                    
                    # Check for SQL error indicators
                    sql_errors = ['syntax error', 'mysql', 'postgresql', 'sql server', 'ora-']
                    for error in sql_errors:
                        if error in response_text:
                            self.log_finding(
                                'A03:2021',
                                'CRITICAL',
                                'SQL Injection Vulnerability',
                                f'SQL injection detected with payload: {payload}',
                                f'Error indicator: {error}'
                            )
                            break
                except json.JSONDecodeError:
                    pass
        
        # Test 2: NoSQL Injection
        nosql_payloads = [
            {"$ne": None},
            {"$regex": ".*"},
            {"$where": "1==1"},
            {"$gt": ""}
        ]
        
        for payload in nosql_payloads:
            query = """
            query NoSQLInjectionTest($filter: ReviewsFilter) {
              reviews(filter: $filter, first: 10) {
                edges {
                  node {
                    id
                  }
                }
              }
            }
            """
            
            response = self.make_graphql_request(query, {"filter": payload})
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('reviews', {}).get('edges'):
                        self.log_finding(
                            'A03:2021',
                            'HIGH',
                            'NoSQL Injection Vulnerability',
                            f'NoSQL injection payload returned data: {payload}',
                            f'Payload: {json.dumps(payload)}'
                        )
                except json.JSONDecodeError:
                    pass
        
        # Test 3: GraphQL Injection
        graphql_injection_payloads = [
            '") { id } user(id: "1',
            '") { password } user(id: "1',
            '") { __schema { types { name } } } user(id: "1'
        ]
        
        for payload in graphql_injection_payloads:
            malicious_query = f"""
            query GraphQLInjectionTest {{
              user(id: "{payload}") {{
                id
                name
              }}
            }}
            """
            
            response = self.make_graphql_request(malicious_query)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and ('password' in str(data) or '__schema' in str(data)):
                        self.log_finding(
                            'A03:2021',
                            'HIGH',
                            'GraphQL Injection Vulnerability',
                            f'GraphQL injection successful with payload: {payload}',
                            f'Response contains sensitive data'
                        )
                except json.JSONDecodeError:
                    pass
    
    def test_a04_insecure_design(self):
        """A04:2021 – Insecure Design"""
        print("🔍 Testing A04:2021 – Insecure Design")
        
        # Test 1: Business logic flaws
        # Test creating multiple reviews for the same offer by the same user
        create_review_mutation = """
        mutation CreateReview($input: CreateReviewInput!) {
          createReview(input: $input) {
            id
            rating
          }
        }
        """
        
        offer_id = "550e8400-e29b-41d4-a716-446655440001"
        
        # Try to create multiple reviews
        successful_reviews = 0
        for i in range(3):
            variables = {
                "input": {
                    "offerId": offer_id,
                    "rating": 5,
                    "text": f"Business logic test review {i}"
                }
            }
            
            response = self.make_graphql_request(create_review_mutation, variables)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('createReview'):
                        successful_reviews += 1
                except json.JSONDecodeError:
                    pass
        
        if successful_reviews > 1:
            self.log_finding(
                'A04:2021',
                'MEDIUM',
                'Business Logic Flaw',
                'Users can create multiple reviews for the same offer',
                f'Created {successful_reviews} reviews for the same offer'
            )
        
        # Test 2: Race condition in review creation
        import threading
        import time
        
        race_condition_results = []
        
        def create_review_thread():
            variables = {
                "input": {
                    "offerId": offer_id,
                    "rating": 4,
                    "text": "Race condition test"
                }
            }
            response = self.make_graphql_request(create_review_mutation, variables)
            race_condition_results.append(response.status_code == 200)
        
        # Start multiple threads simultaneously
        threads = []
        for _ in range(5):
            thread = threading.Thread(target=create_review_thread)
            threads.append(thread)
        
        # Start all threads at once
        for thread in threads:
            thread.start()
        
        # Wait for all threads to complete
        for thread in threads:
            thread.join()
        
        successful_concurrent_reviews = sum(race_condition_results)
        if successful_concurrent_reviews > 1:
            self.log_finding(
                'A04:2021',
                'MEDIUM',
                'Race Condition Vulnerability',
                'Concurrent review creation is not properly handled',
                f'Successfully created {successful_concurrent_reviews} concurrent reviews'
            )
    
    def test_a05_security_misconfiguration(self):
        """A05:2021 – Security Misconfiguration"""
        print("🔍 Testing A05:2021 – Security Misconfiguration")
        
        # Test 1: GraphQL introspection enabled
        introspection_query = """
        query IntrospectionQuery {
          __schema {
            queryType { name }
            mutationType { name }
            types { name }
          }
        }
        """
        
        response = self.make_graphql_request(introspection_query)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and '__schema' in data['data']:
                    self.log_finding(
                        'A05:2021',
                        'MEDIUM',
                        'GraphQL Introspection Enabled',
                        'GraphQL introspection is enabled in production',
                        f'Schema types exposed: {len(data["data"]["__schema"]["types"])}'
                    )
            except json.JSONDecodeError:
                pass
        
        # Test 2: Verbose error messages
        invalid_query = "query { nonExistentField }"
        response = self.make_graphql_request(invalid_query)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'errors' in data:
                    for error in data['errors']:
                        error_message = error.get('message', '').lower()
                        sensitive_info = ['database', 'internal', 'server', 'path', 'file']
                        
                        for info in sensitive_info:
                            if info in error_message:
                                self.log_finding(
                                    'A05:2021',
                                    'LOW',
                                    'Information Disclosure in Error Messages',
                                    'Error messages contain sensitive information',
                                    f'Error: {error["message"]}'
                                )
                                break
            except json.JSONDecodeError:
                pass
        
        # Test 3: Default credentials (if applicable)
        default_creds = [
            ("admin", "admin"),
            ("admin", "password"),
            ("root", "root"),
            ("test", "test")
        ]
        
        login_mutation = """
        mutation Login($username: String!, $password: String!) {
          login(username: $username, password: $password) {
            token
            user {
              id
              roles
            }
          }
        }
        """
        
        for username, password in default_creds:
            variables = {"username": username, "password": password}
            response = self.make_graphql_request(login_mutation, variables)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data and data['data'].get('login', {}).get('token'):
                        self.log_finding(
                            'A05:2021',
                            'CRITICAL',
                            'Default Credentials',
                            f'Default credentials work: {username}/{password}',
                            f'User roles: {data["data"]["login"]["user"]["roles"]}'
                        )
                except json.JSONDecodeError:
                    pass
    
    def test_a06_vulnerable_components(self):
        """A06:2021 – Vulnerable and Outdated Components"""
        print("🔍 Testing A06:2021 – Vulnerable and Outdated Components")
        
        # Test 1: Check server headers for version information
        response = self.session.get(self.base_url)
        
        server_header = response.headers.get('Server', '')
        if server_header:
            # Check for known vulnerable versions (simplified check)
            vulnerable_patterns = [
                'nginx/1.14',  # Example of potentially outdated version
                'Apache/2.2',  # Very old Apache version
                'Express',     # Exposing framework information
            ]
            
            for pattern in vulnerable_patterns:
                if pattern in server_header:
                    self.log_finding(
                        'A06:2021',
                        'MEDIUM',
                        'Server Version Disclosure',
                        f'Server header exposes potentially vulnerable version: {server_header}',
                        f'Server: {server_header}'
                    )
                    break
        
        # Test 2: Check for common vulnerable endpoints
        vulnerable_endpoints = [
            '/admin',
            '/debug',
            '/test',
            '/api/v1',
            '/graphiql',
            '/playground'
        ]
        
        for endpoint in vulnerable_endpoints:
            try:
                response = self.session.get(urljoin(self.base_url, endpoint))
                if response.status_code == 200:
                    self.log_finding(
                        'A06:2021',
                        'LOW',
                        'Exposed Development Endpoint',
                        f'Development/admin endpoint is accessible: {endpoint}',
                        f'Status: {response.status_code}'
                    )
            except requests.RequestException:
                pass
    
    def test_a07_identification_authentication_failures(self):
        """A07:2021 – Identification and Authentication Failures"""
        print("🔍 Testing A07:2021 – Identification and Authentication Failures")
        
        # Test 1: Brute force protection
        login_mutation = """
        mutation Login($username: String!, $password: String!) {
          login(username: $username, password: $password) {
            token
          }
        }
        """
        
        # Attempt multiple failed logins
        failed_attempts = 0
        for i in range(10):
            variables = {
                "username": "testuser",
                "password": f"wrongpassword{i}"
            }
            
            response = self.make_graphql_request(login_mutation, variables)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'errors' in data:
                        failed_attempts += 1
                    elif 'data' in data and data['data'].get('login', {}).get('token'):
                        # Successful login with wrong password - major issue
                        self.log_finding(
                            'A07:2021',
                            'CRITICAL',
                            'Authentication Bypass',
                            'Login succeeded with incorrect password',
                            f'Password: wrongpassword{i}'
                        )
                        break
                except json.JSONDecodeError:
                    pass
            
            time.sleep(0.1)  # Small delay between attempts
        
        if failed_attempts == 10:
            self.log_finding(
                'A07:2021',
                'MEDIUM',
                'No Brute Force Protection',
                'No account lockout or rate limiting detected after 10 failed login attempts',
                f'Failed attempts: {failed_attempts}'
            )
        
        # Test 2: Session management
        if self.auth_token:
            # Test token expiration
            old_token = self.auth_token
            
            # Try using token after a delay (simplified test)
            time.sleep(1)
            
            test_query = "query { __typename }"
            response = self.make_graphql_request(test_query)
            
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'data' in data:
                        # Token still works - check if it has proper expiration
                        try:
                            parts = self.auth_token.split('.')
                            if len(parts) == 3:
                                payload = json.loads(base64.urlsafe_b64decode(parts[1] + '=='))
                                exp = payload.get('exp', 0)
                                current_time = time.time()
                                
                                # Check if token expires in more than 24 hours
                                if exp - current_time > 86400:  # 24 hours
                                    self.log_finding(
                                        'A07:2021',
                                        'MEDIUM',
                                        'Long Token Expiration',
                                        'JWT tokens have very long expiration times',
                                        f'Token expires in {(exp - current_time) / 3600:.1f} hours'
                                    )
                        except Exception:
                            pass
                except json.JSONDecodeError:
                    pass
    
    def test_a08_software_data_integrity_failures(self):
        """A08:2021 – Software and Data Integrity Failures"""
        print("🔍 Testing A08:2021 – Software and Data Integrity Failures")
        
        # Test 1: JWT signature verification
        if self.auth_token:
            try:
                parts = self.auth_token.split('.')
                if len(parts) == 3:
                    # Modify the signature
                    modified_signature = base64.urlsafe_b64encode(b'modified_signature').decode().rstrip('=')
                    modified_token = f"{parts[0]}.{parts[1]}.{modified_signature}"
                    
                    test_query = "query { __typename }"
                    response = self.make_graphql_request(
                        test_query,
                        headers={'Authorization': f'Bearer {modified_token}'}
                    )
                    
                    if response.status_code == 200:
                        try:
                            data = response.json()
                            if 'data' in data:
                                self.log_finding(
                                    'A08:2021',
                                    'CRITICAL',
                                    'JWT Signature Not Verified',
                                    'Modified JWT tokens are accepted without signature verification',
                                    'Modified signature accepted'
                                )
                        except json.JSONDecodeError:
                            pass
            except Exception:
                pass
        
        # Test 2: Data integrity in mutations
        create_review_mutation = """
        mutation CreateReview($input: CreateReviewInput!) {
          createReview(input: $input) {
            id
            rating
            text
          }
        }
        """
        
        # Test with manipulated data
        variables = {
            "input": {
                "offerId": "550e8400-e29b-41d4-a716-446655440001",
                "rating": 10,  # Invalid rating (should be 1-5)
                "text": "Data integrity test"
            }
        }
        
        response = self.make_graphql_request(create_review_mutation, variables)
        
        if response.status_code == 200:
            try:
                data = response.json()
                if 'data' in data and data['data'].get('createReview'):
                    review = data['data']['createReview']
                    if review.get('rating') == 10:
                        self.log_finding(
                            'A08:2021',
                            'HIGH',
                            'Data Validation Bypass',
                            'Invalid data values are accepted without proper validation',
                            f'Invalid rating accepted: {review["rating"]}'
                        )
            except json.JSONDecodeError:
                pass
    
    def test_a09_security_logging_monitoring_failures(self):
        """A09:2021 – Security Logging and Monitoring Failures"""
        print("🔍 Testing A09:2021 – Security Logging and Monitoring Failures")
        
        # Test 1: Check if security events are logged
        # This is difficult to test externally, but we can check for indicators
        
        # Perform suspicious activities and see if there's any response
        suspicious_queries = [
            "query { __schema { types { name } } }",  # Introspection
            "query { user(id: \"' OR 1=1 --\") { id } }",  # SQL injection attempt
            "mutation { deleteAllReviews }",  # Non-existent dangerous mutation
        ]
        
        for query in suspicious_queries:
            response = self.make_graphql_request(query)
            # In a real system, these should be logged
            # We can only check if the system responds appropriately
        
        # Test 2: Rate limiting response (indicates monitoring)
        rapid_requests = 0
        for i in range(50):
            response = self.make_graphql_request("query { __typename }")
            if response.status_code == 429:  # Rate limited
                self.log_finding(
                    'A09:2021',
                    'INFO',
                    'Rate Limiting Detected',
                    'System has rate limiting in place (good security practice)',
                    f'Rate limited after {i} requests'
                )
                break
            rapid_requests += 1
            time.sleep(0.01)
        
        if rapid_requests == 50:
            self.log_finding(
                'A09:2021',
                'MEDIUM',
                'No Rate Limiting Detected',
                'No rate limiting detected - security events may not be monitored',
                'Completed 50 rapid requests without rate limiting'
            )
    
    def test_a10_server_side_request_forgery(self):
        """A10:2021 – Server-Side Request Forgery (SSRF)"""
        print("🔍 Testing A10:2021 – Server-Side Request Forgery (SSRF)")
        
        # Test 1: URL-based SSRF
        ssrf_payloads = [
            "http://localhost:22",  # SSH port
            "http://127.0.0.1:3306",  # MySQL port
            "http://169.254.169.254/latest/meta-data/",  # AWS metadata
            "file:///etc/passwd",  # Local file access
            "http://internal-service:8080",  # Internal service
        ]
        
        # Test if there are any fields that accept URLs
        url_test_mutation = """
        mutation UpdateProfile($input: UpdateProfileInput!) {
          updateProfile(input: $input) {
            id
            avatarUrl
          }
        }
        """
        
        for payload in ssrf_payloads:
            variables = {
                "input": {
                    "avatarUrl": payload
                }
            }
            
            start_time = time.time()
            response = self.make_graphql_request(url_test_mutation, variables)
            end_time = time.time()
            
            # Check for SSRF indicators
            if response.status_code == 200:
                try:
                    data = response.json()
                    if 'errors' in data:
                        for error in data['errors']:
                            error_message = error.get('message', '').lower()
                            ssrf_indicators = [
                                'connection refused', 'timeout', 'unreachable',
                                'internal server', 'network error'
                            ]
                            
                            for indicator in ssrf_indicators:
                                if indicator in error_message:
                                    self.log_finding(
                                        'A10:2021',
                                        'HIGH',
                                        'Server-Side Request Forgery (SSRF)',
                                        f'SSRF vulnerability detected with payload: {payload}',
                                        f'Error: {error["message"]}'
                                    )
                                    break
                    
                    # Check for time-based SSRF (long response times)
                    if end_time - start_time > 5:
                        self.log_finding(
                            'A10:2021',
                            'MEDIUM',
                            'Potential Time-based SSRF',
                            f'Long response time with URL payload: {payload}',
                            f'Response time: {end_time - start_time:.2f}s'
                        )
                        
                except json.JSONDecodeError:
                    pass
    
    def generate_owasp_report(self) -> Dict[str, Any]:
        """Generate OWASP Top 10 compliance report"""
        
        # Group findings by OWASP category
        owasp_categories = {}
        for finding in self.findings:
            category = finding['owasp_category']
            if category not in owasp_categories:
                owasp_categories[category] = []
            owasp_categories[category].append(finding)
        
        # Calculate compliance score
        total_categories = 10
        compliant_categories = 0
        
        category_names = {
            'A01:2021': 'Broken Access Control',
            'A02:2021': 'Cryptographic Failures',
            'A03:2021': 'Injection',
            'A04:2021': 'Insecure Design',
            'A05:2021': 'Security Misconfiguration',
            'A06:2021': 'Vulnerable and Outdated Components',
            'A07:2021': 'Identification and Authentication Failures',
            'A08:2021': 'Software and Data Integrity Failures',
            'A09:2021': 'Security Logging and Monitoring Failures',
            'A10:2021': 'Server-Side Request Forgery (SSRF)'
        }
        
        for category in category_names.keys():
            category_findings = owasp_categories.get(category, [])
            critical_high_findings = [f for f in category_findings if f['severity'] in ['CRITICAL', 'HIGH']]
            
            if not critical_high_findings:
                compliant_categories += 1
        
        compliance_score = (compliant_categories / total_categories) * 100
        
        report = {
            'scan_timestamp': time.time(),
            'target_url': self.base_url,
            'owasp_compliance_score': compliance_score,
            'compliant_categories': compliant_categories,
            'total_categories': total_categories,
            'findings_by_category': owasp_categories,
            'category_names': category_names,
            'total_findings': len(self.findings),
            'findings': self.findings,
            'recommendations': self.generate_owasp_recommendations()
        }
        
        return report
    
    def generate_owasp_recommendations(self) -> List[str]:
        """Generate OWASP-specific recommendations"""
        recommendations = []
        
        # Check which categories have issues
        categories_with_issues = set()
        for finding in self.findings:
            if finding['severity'] in ['CRITICAL', 'HIGH']:
                categories_with_issues.add(finding['owasp_category'])
        
        # Category-specific recommendations
        if 'A01:2021' in categories_with_issues:
            recommendations.extend([
                "🔒 Implement proper access control checks for all operations",
                "🔐 Use role-based access control (RBAC) consistently",
                "🛡️ Validate user permissions at the API level"
            ])
        
        if 'A02:2021' in categories_with_issues:
            recommendations.extend([
                "🔐 Enforce HTTPS for all communications",
                "🔑 Use strong cryptographic algorithms (AES-256, RSA-2048+)",
                "🛡️ Implement proper key management"
            ])
        
        if 'A03:2021' in categories_with_issues:
            recommendations.extend([
                "💉 Use parameterized queries to prevent SQL injection",
                "🛡️ Implement input validation and sanitization",
                "🔍 Use GraphQL query complexity analysis"
            ])
        
        if 'A04:2021' in categories_with_issues:
            recommendations.extend([
                "🏗️ Implement proper business logic validation",
                "🔄 Add concurrency controls for critical operations",
                "📋 Conduct threat modeling for business processes"
            ])
        
        if 'A05:2021' in categories_with_issues:
            recommendations.extend([
                "⚙️ Disable GraphQL introspection in production",
                "🔧 Remove verbose error messages",
                "🛡️ Implement security headers and configurations"
            ])
        
        if 'A06:2021' in categories_with_issues:
            recommendations.extend([
                "📦 Keep all dependencies up to date",
                "🔍 Regular vulnerability scanning of components",
                "🚫 Remove unused dependencies and features"
            ])
        
        if 'A07:2021' in categories_with_issues:
            recommendations.extend([
                "🔐 Implement strong authentication mechanisms",
                "🛡️ Add brute force protection",
                "⏰ Use appropriate session timeouts"
            ])
        
        if 'A08:2021' in categories_with_issues:
            recommendations.extend([
                "✅ Implement JWT signature verification",
                "🔍 Add data integrity checks",
                "🛡️ Use secure software update mechanisms"
            ])
        
        if 'A09:2021' in categories_with_issues:
            recommendations.extend([
                "📊 Implement comprehensive security logging",
                "🚨 Set up security monitoring and alerting",
                "📈 Monitor for suspicious activities"
            ])
        
        if 'A10:2021' in categories_with_issues:
            recommendations.extend([
                "🌐 Validate and sanitize all URLs",
                "🚫 Implement URL whitelist for external requests",
                "🔒 Use network segmentation to limit SSRF impact"
            ])
        
        return recommendations
    
    def run_owasp_top10_scan(self):
        """Run complete OWASP Top 10 security scan"""
        print("🔒 Starting OWASP Top 10 Security Scan...")
        print(f"Target: {self.base_url}")
        print("=" * 60)
        
        # Run all OWASP Top 10 tests
        test_methods = [
            self.test_a01_broken_access_control,
            self.test_a02_cryptographic_failures,
            self.test_a03_injection,
            self.test_a04_insecure_design,
            self.test_a05_security_misconfiguration,
            self.test_a06_vulnerable_components,
            self.test_a07_identification_authentication_failures,
            self.test_a08_software_data_integrity_failures,
            self.test_a09_security_logging_monitoring_failures,
            self.test_a10_server_side_request_forgery
        ]
        
        for test_method in test_methods:
            try:
                test_method()
            except Exception as e:
                print(f"❌ Error in {test_method.__name__}: {str(e)}")
        
        print("=" * 60)
        print("🔒 OWASP Top 10 Security Scan Complete")
        
        return self.generate_owasp_report()

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='OWASP Top 10 Security Tester for Apollo Router Federation')
    parser.add_argument('url', help='Base URL of the Apollo Router')
    parser.add_argument('--token', '-t', help='JWT authentication token')
    parser.add_argument('--output', '-o', help='Output file for OWASP report (JSON)')
    
    args = parser.parse_args()
    
    tester = OWASPTop10Tester(args.url, args.token)
    report = tester.run_owasp_top10_scan()
    
    # Print summary
    print(f"\n📊 OWASP Compliance Score: {report['owasp_compliance_score']:.1f}%")
    print(f"✅ Compliant Categories: {report['compliant_categories']}/{report['total_categories']}")
    print(f"🔍 Total Findings: {report['total_findings']}")
    
    # Print findings by category
    for category, name in report['category_names'].items():
        findings = report['findings_by_category'].get(category, [])
        critical_high = [f for f in findings if f['severity'] in ['CRITICAL', 'HIGH']]
        
        status = "✅" if not critical_high else "❌"
        print(f"{status} {category}: {name} ({len(critical_high)} critical/high issues)")
    
    # Save report
    if args.output:
        with open(args.output, 'w') as f:
            json.dump(report, f, indent=2)
        print(f"📄 OWASP report saved to: {args.output}")
    
    # Exit with error code if critical issues found
    critical_findings = [f for f in report['findings'] if f['severity'] == 'CRITICAL']
    if critical_findings:
        print(f"\n🚨 {len(critical_findings)} CRITICAL vulnerabilities found!")
        return 1
    
    return 0

if __name__ == '__main__':
    exit(main())