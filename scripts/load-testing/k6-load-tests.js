// K6 Load Testing Scripts for Apollo Router Federation
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const responseTime = new Trend('response_time');
const federatedQueries = new Counter('federated_queries');
const complexQueries = new Counter('complex_queries');

// Test configuration
export const options = {
  scenarios: {
    // Scenario 1: Baseline load test
    baseline_load: {
      executor: 'constant-vus',
      vus: 10,
      duration: '5m',
      tags: { scenario: 'baseline' },
    },
    
    // Scenario 2: Spike test
    spike_test: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 50 },
        { duration: '1m', target: 100 },
        { duration: '2m', target: 50 },
        { duration: '1m', target: 0 },
      ],
      tags: { scenario: 'spike' },
    },
    
    // Scenario 3: Stress test
    stress_test: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '5m', target: 100 },
        { duration: '10m', target: 100 },
        { duration: '5m', target: 0 },
      ],
      tags: { scenario: 'stress' },
    },
    
    // Scenario 4: Soak test (long duration)
    soak_test: {
      executor: 'constant-vus',
      vus: 20,
      duration: '30m',
      tags: { scenario: 'soak' },
    },
  },
  
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% of requests should be below 500ms
    http_req_failed: ['rate<0.05'],   // Error rate should be below 5%
    errors: ['rate<0.05'],
    response_time: ['p(95)<1000'],
  },
};

// Test data
const ROUTER_URL = __ENV.ROUTER_URL || 'http://localhost:4000/graphql';
const JWT_TOKEN = __ENV.JWT_TOKEN || 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...'; // Mock token

// Sample UUIDs for testing
const OFFER_IDS = [
  '550e8400-e29b-41d4-a716-446655440001',
  '550e8400-e29b-41d4-a716-446655440002',
  '550e8400-e29b-41d4-a716-446655440003',
  '550e8400-e29b-41d4-a716-446655440004',
  '550e8400-e29b-41d4-a716-446655440005',
];

const USER_IDS = [
  '660e8400-e29b-41d4-a716-446655440001',
  '660e8400-e29b-41d4-a716-446655440002',
  '660e8400-e29b-41d4-a716-446655440003',
];

// GraphQL queries for testing
const QUERIES = {
  // Simple query - single subgraph
  simple_review: `
    query GetReview($id: ID!) {
      review(id: $id) {
        id
        rating
        text
        createdAt
      }
    }
  `,
  
  // Federated query - spans multiple subgraphs
  federated_offer_with_reviews: `
    query GetOfferWithReviews($offerId: ID!) {
      offer(id: $offerId) {
        id
        title
        price
        reviews(first: 10) {
          edges {
            node {
              id
              rating
              text
              author {
                id
                name
                email
              }
            }
          }
        }
        averageRating
        reviewsCount
      }
    }
  `,
  
  // Complex federated query - deep nesting
  complex_federated: `
    query ComplexFederatedQuery($first: Int!) {
      offers(first: $first) {
        edges {
          node {
            id
            title
            price
            reviews(first: 5) {
              edges {
                node {
                  id
                  rating
                  text
                  createdAt
                  author {
                    id
                    name
                    reviews(first: 3) {
                      edges {
                        node {
                          id
                          rating
                          offer {
                            id
                            title
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
            averageRating
          }
        }
      }
    }
  `,
  
  // Mutation test
  create_review: `
    mutation CreateReview($input: CreateReviewInput!) {
      createReview(input: $input) {
        id
        rating
        text
        createdAt
        author {
          id
          name
        }
        offer {
          id
          title
          averageRating
        }
      }
    }
  `,
  
  // Subscription test (if supported)
  review_updates: `
    subscription ReviewUpdates($offerId: ID!) {
      reviewUpdated(offerId: $offerId) {
        id
        rating
        text
        moderationStatus
      }
    }
  `,
};

// Helper function to make GraphQL request
function makeGraphQLRequest(query, variables = {}, operationName = null) {
  const payload = {
    query,
    variables,
    operationName,
  };
  
  const params = {
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${JWT_TOKEN}`,
    },
    timeout: '30s',
  };
  
  const response = http.post(ROUTER_URL, JSON.stringify(payload), params);
  
  // Record metrics
  responseTime.add(response.timings.duration);
  
  const success = check(response, {
    'status is 200': (r) => r.status === 200,
    'response has data': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.data !== null && body.data !== undefined;
      } catch (e) {
        return false;
      }
    },
    'no GraphQL errors': (r) => {
      try {
        const body = JSON.parse(r.body);
        return !body.errors || body.errors.length === 0;
      } catch (e) {
        return false;
      }
    },
  });
  
  if (!success) {
    errorRate.add(1);
    console.log(`Request failed: ${response.status} - ${response.body}`);
  } else {
    errorRate.add(0);
  }
  
  return response;
}

// Test scenarios
export default function () {
  const scenario = __ENV.K6_SCENARIO || 'mixed';
  
  switch (scenario) {
    case 'simple':
      testSimpleQueries();
      break;
    case 'federated':
      testFederatedQueries();
      break;
    case 'complex':
      testComplexQueries();
      break;
    case 'mutations':
      testMutations();
      break;
    case 'mixed':
    default:
      testMixedWorkload();
      break;
  }
  
  sleep(1);
}

function testSimpleQueries() {
  // Test simple review queries
  const reviewId = '770e8400-e29b-41d4-a716-446655440001';
  makeGraphQLRequest(QUERIES.simple_review, { id: reviewId });
}

function testFederatedQueries() {
  // Test federated queries that span multiple subgraphs
  const offerId = OFFER_IDS[Math.floor(Math.random() * OFFER_IDS.length)];
  
  const response = makeGraphQLRequest(QUERIES.federated_offer_with_reviews, { 
    offerId 
  });
  
  federatedQueries.add(1);
  
  // Additional checks for federated queries
  check(response, {
    'federated data is complete': (r) => {
      try {
        const body = JSON.parse(r.body);
        const offer = body.data?.offer;
        return offer && offer.reviews && offer.averageRating !== null;
      } catch (e) {
        return false;
      }
    },
  });
}

function testComplexQueries() {
  // Test complex nested federated queries
  const response = makeGraphQLRequest(QUERIES.complex_federated, { 
    first: 10 
  });
  
  complexQueries.add(1);
  
  // Check for query complexity handling
  check(response, {
    'complex query completes': (r) => r.status === 200,
    'complex query has reasonable response time': (r) => r.timings.duration < 2000,
  });
}

function testMutations() {
  // Test review creation mutations
  const offerId = OFFER_IDS[Math.floor(Math.random() * OFFER_IDS.length)];
  const rating = Math.floor(Math.random() * 5) + 1;
  const text = `Load test review ${Math.random().toString(36).substring(7)}`;
  
  const input = {
    offerId,
    rating,
    text,
  };
  
  const response = makeGraphQLRequest(QUERIES.create_review, { input });
  
  check(response, {
    'mutation succeeds': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.data?.createReview?.id !== null;
      } catch (e) {
        return false;
      }
    },
  });
}

function testMixedWorkload() {
  // Simulate realistic mixed workload
  const workloadType = Math.random();
  
  if (workloadType < 0.4) {
    // 40% simple queries
    testSimpleQueries();
  } else if (workloadType < 0.7) {
    // 30% federated queries
    testFederatedQueries();
  } else if (workloadType < 0.9) {
    // 20% complex queries
    testComplexQueries();
  } else {
    // 10% mutations
    testMutations();
  }
}

// Setup function
export function setup() {
  console.log('Starting load test setup...');
  
  // Verify router is accessible
  const healthCheck = http.get(`${ROUTER_URL.replace('/graphql', '')}/health`);
  if (healthCheck.status !== 200) {
    console.error('Router health check failed');
    return null;
  }
  
  console.log('Router is healthy, starting load test...');
  return { routerHealthy: true };
}

// Teardown function
export function teardown(data) {
  console.log('Load test completed');
  console.log(`Total federated queries: ${federatedQueries.count}`);
  console.log(`Total complex queries: ${complexQueries.count}`);
}

// Handle different test phases
export function handleSummary(data) {
  return {
    'load-test-results.json': JSON.stringify(data, null, 2),
    'load-test-summary.txt': textSummary(data, { indent: ' ', enableColors: true }),
  };
}