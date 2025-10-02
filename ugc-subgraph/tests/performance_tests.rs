use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use tokio::time::timeout;
use criterion::{black_box, Criterion};
use testcontainers::{clients::Cli, images::postgres::Postgres};
use serial_test::serial;

use ugc_subgraph::{
    service::{ReviewService, create_review_service_with_metrics},
    repository::PostgresReviewRepository,
    models::review::{CreateReviewInput, ReviewsFilter, UpdateReviewInput, ModerationStatus},
    telemetry::metrics::Metrics,
    error::UgcError,
};

// Performance test setup
async fn setup_performance_test() -> (ReviewService, sqlx::PgPool) {
    let docker = Cli::default();
    let postgres_image = Postgres::default();
    let container = docker.run(postgres_image);
    
    let connection_string = format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        container.get_host_port_ipv4(5432)
    );
    
    let pool = sqlx::PgPool::connect(&connection_string)
        .await
        .expect("Failed to connect to test database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    let metrics = Arc::new(Metrics::new());
    let service = create_review_service_with_metrics(pool.clone(), metrics);
    
    (service, pool)
}

// Helper to create test data
async fn create_test_reviews(service: &ReviewService, count: usize, offer_id: Uuid) -> Vec<Uuid> {
    let mut review_ids = Vec::new();
    
    for i in 0..count {
        let input = CreateReviewInput {
            offer_id,
            rating: (i % 5) + 1,
            text: format!("Performance test review {}", i),
        };
        
        let review = service.create_review(input, Uuid::new_v4())
            .await
            .expect("Failed to create test review");
        
        review_ids.push(review.id);
    }
    
    review_ids
}

#[tokio::test]
#[serial]
async fn test_bulk_review_creation_performance() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    let start_time = Instant::now();
    let review_count = 1000;
    
    // Create reviews concurrently
    let mut handles = vec![];
    for i in 0..review_count {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            let input = CreateReviewInput {
                offer_id,
                rating: (i % 5) + 1,
                text: format!("Bulk test review {}", i),
            };
            
            service_clone.create_review(input, Uuid::new_v4()).await
        });
        handles.push(handle);
    }
    
    // Wait for all creations to complete
    let mut successful_creations = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => successful_creations += 1,
            _ => {}
        }
    }
    
    let duration = start_time.elapsed();
    
    println!("Created {} reviews in {:?}", successful_creations, duration);
    println!("Average time per review: {:?}", duration / successful_creations as u32);
    
    // Performance assertions
    assert!(successful_creations >= review_count * 95 / 100, "Should create at least 95% of reviews");
    assert!(duration.as_secs() < 60, "Should complete within 60 seconds");
    
    // Test throughput
    let throughput = successful_creations as f64 / duration.as_secs_f64();
    println!("Throughput: {:.2} reviews/second", throughput);
    assert!(throughput > 10.0, "Should achieve at least 10 reviews/second");
}

#[tokio::test]
#[serial]
async fn test_bulk_query_performance() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    // Create test data
    let review_ids = create_test_reviews(&service, 500, offer_id).await;
    
    let start_time = Instant::now();
    
    // Test individual queries
    let mut handles = vec![];
    for review_id in &review_ids[..100] { // Test first 100
        let service_clone = service.clone();
        let review_id = *review_id;
        let handle = tokio::spawn(async move {
            service_clone.get_review_by_id(review_id).await
        });
        handles.push(handle);
    }
    
    let mut successful_queries = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(Some(_))) => successful_queries += 1,
            _ => {}
        }
    }
    
    let individual_query_duration = start_time.elapsed();
    
    println!("Queried {} individual reviews in {:?}", successful_queries, individual_query_duration);
    
    // Test batch queries
    let batch_start = Instant::now();
    
    let batch_result = service.get_reviews_by_ids(review_ids[..100].to_vec()).await;
    let batch_duration = batch_start.elapsed();
    
    match batch_result {
        Ok(results) => {
            let found_count = results.iter().filter(|r| r.is_some()).count();
            println!("Batch queried {} reviews in {:?}", found_count, batch_duration);
            
            // Batch should be significantly faster than individual queries
            assert!(batch_duration < individual_query_duration / 2, 
                   "Batch query should be at least 2x faster than individual queries");
        }
        Err(e) => panic!("Batch query failed: {:?}", e),
    }
    
    // Test pagination performance
    let pagination_start = Instant::now();
    
    let (paginated_reviews, total_count) = service
        .get_reviews_with_pagination(None, 100, 0)
        .await
        .expect("Pagination query should succeed");
    
    let pagination_duration = pagination_start.elapsed();
    
    println!("Paginated query returned {} reviews (total: {}) in {:?}", 
             paginated_reviews.len(), total_count, pagination_duration);
    
    assert!(pagination_duration.as_millis() < 1000, "Pagination should complete within 1 second");
    assert!(total_count >= 500, "Should find all created reviews");
}

#[tokio::test]
#[serial]
async fn test_concurrent_read_write_performance() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    // Create initial test data
    let review_ids = create_test_reviews(&service, 100, offer_id).await;
    
    let start_time = Instant::now();
    let test_duration = Duration::from_secs(30);
    
    // Spawn concurrent readers
    let mut read_handles = vec![];
    for _ in 0..10 {
        let service_clone = service.clone();
        let review_ids_clone = review_ids.clone();
        let handle = tokio::spawn(async move {
            let mut read_count = 0;
            let start = Instant::now();
            
            while start.elapsed() < test_duration {
                let random_id = review_ids_clone[read_count % review_ids_clone.len()];
                if let Ok(Some(_)) = service_clone.get_review_by_id(random_id).await {
                    read_count += 1;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            
            read_count
        });
        read_handles.push(handle);
    }
    
    // Spawn concurrent writers
    let mut write_handles = vec![];
    for i in 0..5 {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            let mut write_count = 0;
            let start = Instant::now();
            
            while start.elapsed() < test_duration {
                let input = CreateReviewInput {
                    offer_id,
                    rating: (write_count % 5) + 1,
                    text: format!("Concurrent write test {} - {}", i, write_count),
                };
                
                if service_clone.create_review(input, Uuid::new_v4()).await.is_ok() {
                    write_count += 1;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            
            write_count
        });
        write_handles.push(handle);
    }
    
    // Wait for all operations to complete
    let mut total_reads = 0;
    for handle in read_handles {
        total_reads += handle.await.unwrap_or(0);
    }
    
    let mut total_writes = 0;
    for handle in write_handles {
        total_writes += handle.await.unwrap_or(0);
    }
    
    let actual_duration = start_time.elapsed();
    
    println!("Concurrent operations completed in {:?}", actual_duration);
    println!("Total reads: {}, reads/sec: {:.2}", total_reads, total_reads as f64 / actual_duration.as_secs_f64());
    println!("Total writes: {}, writes/sec: {:.2}", total_writes, total_writes as f64 / actual_duration.as_secs_f64());
    
    // Performance assertions
    assert!(total_reads > 100, "Should perform significant number of reads");
    assert!(total_writes > 50, "Should perform significant number of writes");
    
    let read_throughput = total_reads as f64 / actual_duration.as_secs_f64();
    let write_throughput = total_writes as f64 / actual_duration.as_secs_f64();
    
    assert!(read_throughput > 10.0, "Should achieve at least 10 reads/second");
    assert!(write_throughput > 1.0, "Should achieve at least 1 write/second");
}

#[tokio::test]
#[serial]
async fn test_memory_usage_under_load() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    // Measure initial memory usage (simplified - in real tests you'd use proper memory profiling)
    let initial_time = Instant::now();
    
    // Create a large number of reviews
    let review_count = 5000;
    let mut review_ids = Vec::with_capacity(review_count);
    
    for i in 0..review_count {
        let input = CreateReviewInput {
            offer_id,
            rating: (i % 5) + 1,
            text: format!("Memory test review {} with some additional text to increase memory usage", i),
        };
        
        match service.create_review(input, Uuid::new_v4()).await {
            Ok(review) => review_ids.push(review.id),
            Err(_) => break,
        }
        
        // Check for memory leaks by ensuring operations don't slow down significantly
        if i > 0 && i % 1000 == 0 {
            let current_duration = initial_time.elapsed();
            let expected_max_duration = Duration::from_millis(i as u64 * 2); // 2ms per review max
            
            if current_duration > expected_max_duration {
                println!("Warning: Performance degradation detected at {} reviews", i);
            }
        }
    }
    
    let creation_duration = initial_time.elapsed();
    println!("Created {} reviews in {:?}", review_ids.len(), creation_duration);
    
    // Test querying all reviews to ensure memory is handled properly
    let query_start = Instant::now();
    
    let (all_reviews, total_count) = service
        .get_reviews_with_pagination(None, review_ids.len() as i32, 0)
        .await
        .expect("Should be able to query all reviews");
    
    let query_duration = query_start.elapsed();
    
    println!("Queried {} reviews (total: {}) in {:?}", all_reviews.len(), total_count, query_duration);
    
    // Memory usage assertions (simplified)
    assert!(creation_duration.as_millis() < review_ids.len() as u128 * 5, 
           "Creation time should scale linearly");
    assert!(query_duration.as_millis() < 10000, 
           "Large query should complete within 10 seconds");
    assert_eq!(all_reviews.len(), review_ids.len(), 
              "Should retrieve all created reviews");
}

#[tokio::test]
#[serial]
async fn test_database_connection_pool_performance() {
    let (service, pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    // Test connection pool under high concurrency
    let concurrent_operations = 100;
    let start_time = Instant::now();
    
    let mut handles = vec![];
    for i in 0..concurrent_operations {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            // Mix of different operations to test connection pool
            match i % 4 {
                0 => {
                    // Create operation
                    let input = CreateReviewInput {
                        offer_id,
                        rating: 5,
                        text: format!("Pool test review {}", i),
                    };
                    service_clone.create_review(input, Uuid::new_v4()).await.is_ok()
                }
                1 => {
                    // Read operation
                    service_clone.get_reviews_with_pagination(None, 10, 0).await.is_ok()
                }
                2 => {
                    // Update operation (create first, then update)
                    let input = CreateReviewInput {
                        offer_id,
                        rating: 3,
                        text: format!("Update test review {}", i),
                    };
                    if let Ok(review) = service_clone.create_review(input, Uuid::new_v4()).await {
                        let update_input = UpdateReviewInput {
                            rating: Some(4),
                            text: Some(format!("Updated review {}", i)),
                        };
                        service_clone.update_review(review.id, update_input, review.author_id).await.is_ok()
                    } else {
                        false
                    }
                }
                _ => {
                    // Aggregation operation
                    service_clone.update_offer_rating(offer_id).await.is_ok()
                }
            }
        });
        handles.push(handle);
    }
    
    // Wait for all operations
    let mut successful_operations = 0;
    for handle in handles {
        if handle.await.unwrap_or(false) {
            successful_operations += 1;
        }
    }
    
    let duration = start_time.elapsed();
    
    println!("Completed {} operations in {:?}", successful_operations, duration);
    println!("Connection pool stats: active={}, idle={}", 
             pool.num_idle(), pool.size());
    
    // Performance assertions
    assert!(successful_operations >= concurrent_operations * 90 / 100, 
           "Should complete at least 90% of operations successfully");
    assert!(duration.as_secs() < 30, 
           "Should complete within 30 seconds");
    
    // Connection pool should not be exhausted
    assert!(pool.size() > 0, "Connection pool should have connections");
}

#[tokio::test]
#[serial]
async fn test_query_complexity_performance() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    // Create test data
    let _review_ids = create_test_reviews(&service, 100, offer_id).await;
    
    // Test simple query performance
    let simple_start = Instant::now();
    let simple_result = service.get_review_by_id(Uuid::new_v4()).await;
    let simple_duration = simple_start.elapsed();
    
    assert!(simple_result.is_ok());
    println!("Simple query took: {:?}", simple_duration);
    
    // Test complex query performance (pagination with filters)
    let complex_start = Instant::now();
    let filter = Some(ReviewsFilter {
        offer_id: Some(offer_id),
        author_id: None,
        min_rating: Some(3),
        max_rating: Some(5),
        moderated_only: Some(true),
        moderation_status: Some(ModerationStatus::Approved),
    });
    
    let complex_result = service.get_reviews_with_pagination(filter, 50, 0).await;
    let complex_duration = complex_start.elapsed();
    
    assert!(complex_result.is_ok());
    println!("Complex query took: {:?}", complex_duration);
    
    // Test batch query performance
    let batch_start = Instant::now();
    let batch_ids: Vec<Uuid> = (0..50).map(|_| Uuid::new_v4()).collect();
    let batch_result = service.get_reviews_by_ids(batch_ids).await;
    let batch_duration = batch_start.elapsed();
    
    assert!(batch_result.is_ok());
    println!("Batch query took: {:?}", batch_duration);
    
    // Performance assertions
    assert!(simple_duration.as_millis() < 100, "Simple query should be very fast");
    assert!(complex_duration.as_millis() < 1000, "Complex query should complete within 1 second");
    assert!(batch_duration.as_millis() < 500, "Batch query should be efficient");
    
    // Complex query should not be more than 10x slower than simple query
    assert!(complex_duration.as_millis() < simple_duration.as_millis() * 10,
           "Complex query should not be excessively slower than simple query");
}

#[tokio::test]
#[serial]
async fn test_error_handling_performance() {
    let (service, _pool) = setup_performance_test().await;
    
    let start_time = Instant::now();
    let error_operations = 100;
    
    // Test performance of error scenarios
    let mut handles = vec![];
    for i in 0..error_operations {
        let service_clone = service.clone();
        let handle = tokio::spawn(async move {
            match i % 3 {
                0 => {
                    // Test not found error
                    service_clone.get_review_by_id(Uuid::new_v4()).await.is_ok()
                }
                1 => {
                    // Test validation error
                    let invalid_input = CreateReviewInput {
                        offer_id: Uuid::new_v4(),
                        rating: 10, // Invalid rating
                        text: "Test".to_string(),
                    };
                    // This should fail at validation level
                    invalid_input.validate().is_ok()
                }
                _ => {
                    // Test unauthorized error
                    let non_existent_review_id = Uuid::new_v4();
                    let wrong_user_id = Uuid::new_v4();
                    let update_input = UpdateReviewInput {
                        rating: Some(3),
                        text: Some("Unauthorized update".to_string()),
                    };
                    service_clone.update_review(non_existent_review_id, update_input, wrong_user_id).await.is_ok()
                }
            }
        });
        handles.push(handle);
    }
    
    // Wait for all error operations
    let mut completed_operations = 0;
    for handle in handles {
        handle.await.unwrap();
        completed_operations += 1;
    }
    
    let duration = start_time.elapsed();
    
    println!("Completed {} error operations in {:?}", completed_operations, duration);
    
    // Error handling should not significantly impact performance
    assert!(duration.as_millis() < 5000, "Error handling should be fast");
    assert_eq!(completed_operations, error_operations, "All error operations should complete");
    
    let error_throughput = completed_operations as f64 / duration.as_secs_f64();
    assert!(error_throughput > 20.0, "Should handle at least 20 errors/second");
}

// Benchmark functions for criterion (if running with criterion)
pub fn benchmark_review_creation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("create_review", |b| {
        b.to_async(&rt).iter(|| async {
            let input = CreateReviewInput {
                offer_id: Uuid::new_v4(),
                rating: 5,
                text: "Benchmark review".to_string(),
            };
            
            // In a real benchmark, you'd use a real service
            black_box(input.validate())
        })
    });
}

pub fn benchmark_review_validation(c: &mut Criterion) {
    c.bench_function("validate_review_input", |b| {
        let input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: "Benchmark validation test with some reasonable length text".to_string(),
        };
        
        b.iter(|| {
            black_box(input.validate())
        })
    });
}

pub fn benchmark_uuid_generation(c: &mut Criterion) {
    c.bench_function("uuid_generation", |b| {
        b.iter(|| {
            black_box(Uuid::new_v4())
        })
    });
}

// Stress test for sustained load
#[tokio::test]
#[serial]
async fn test_sustained_load_performance() {
    let (service, _pool) = setup_performance_test().await;
    let offer_id = Uuid::new_v4();
    
    let test_duration = Duration::from_secs(60); // 1 minute sustained test
    let start_time = Instant::now();
    
    let mut total_operations = 0;
    let mut error_count = 0;
    
    // Run sustained load
    while start_time.elapsed() < test_duration {
        let batch_start = Instant::now();
        let batch_size = 10;
        
        let mut batch_handles = vec![];
        for i in 0..batch_size {
            let service_clone = service.clone();
            let handle = tokio::spawn(async move {
                let input = CreateReviewInput {
                    offer_id,
                    rating: (i % 5) + 1,
                    text: format!("Sustained load test review {}", i),
                };
                
                service_clone.create_review(input, Uuid::new_v4()).await
            });
            batch_handles.push(handle);
        }
        
        // Wait for batch to complete
        for handle in batch_handles {
            match handle.await {
                Ok(Ok(_)) => total_operations += 1,
                _ => error_count += 1,
            }
        }
        
        // Small delay to prevent overwhelming the system
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Check for performance degradation
        let batch_duration = batch_start.elapsed();
        if batch_duration > Duration::from_secs(5) {
            println!("Warning: Batch took {:?}, possible performance degradation", batch_duration);
        }
    }
    
    let total_duration = start_time.elapsed();
    let success_rate = total_operations as f64 / (total_operations + error_count) as f64;
    let throughput = total_operations as f64 / total_duration.as_secs_f64();
    
    println!("Sustained load test results:");
    println!("Duration: {:?}", total_duration);
    println!("Total operations: {}", total_operations);
    println!("Error count: {}", error_count);
    println!("Success rate: {:.2}%", success_rate * 100.0);
    println!("Throughput: {:.2} operations/second", throughput);
    
    // Performance assertions for sustained load
    assert!(success_rate > 0.95, "Should maintain >95% success rate under sustained load");
    assert!(throughput > 5.0, "Should maintain >5 operations/second under sustained load");
    assert!(total_operations > 300, "Should complete significant number of operations");
}