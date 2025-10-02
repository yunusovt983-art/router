use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use uuid::Uuid;
use std::time::Duration;

use ugc_subgraph::models::review::{CreateReviewInput, UpdateReviewInput, ModerationStatus};

fn benchmark_review_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("review_validation");
    
    // Benchmark different text lengths
    for text_length in [10, 100, 1000, 5000].iter() {
        let input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: "a".repeat(*text_length),
        };
        
        group.bench_with_input(
            BenchmarkId::new("create_review_validation", text_length),
            text_length,
            |b, _| {
                b.iter(|| {
                    black_box(input.validate())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_update_review_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_review_validation");
    
    let inputs = vec![
        UpdateReviewInput {
            rating: Some(4),
            text: None,
        },
        UpdateReviewInput {
            rating: None,
            text: Some("Updated review text".to_string()),
        },
        UpdateReviewInput {
            rating: Some(3),
            text: Some("Updated review with both fields".to_string()),
        },
    ];
    
    for (i, input) in inputs.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("update_validation", i),
            input,
            |b, input| {
                b.iter(|| {
                    black_box(input.validate())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_moderation_status_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("moderation_status");
    
    let statuses = vec!["pending", "approved", "rejected", "flagged"];
    
    for status in statuses {
        group.bench_with_input(
            BenchmarkId::new("parse_status", status),
            &status,
            |b, status| {
                b.iter(|| {
                    black_box(status.parse::<ModerationStatus>())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_uuid_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("uuid_operations");
    
    group.bench_function("uuid_generation", |b| {
        b.iter(|| {
            black_box(Uuid::new_v4())
        })
    });
    
    let uuid = Uuid::new_v4();
    group.bench_function("uuid_to_string", |b| {
        b.iter(|| {
            black_box(uuid.to_string())
        })
    });
    
    let uuid_string = uuid.to_string();
    group.bench_function("uuid_from_string", |b| {
        b.iter(|| {
            black_box(uuid_string.parse::<Uuid>())
        })
    });
    
    group.finish();
}

fn benchmark_review_creation_input(c: &mut Criterion) {
    let mut group = c.benchmark_group("review_creation");
    
    // Benchmark creating review input with different parameters
    let offer_ids: Vec<Uuid> = (0..100).map(|_| Uuid::new_v4()).collect();
    let author_ids: Vec<Uuid> = (0..100).map(|_| Uuid::new_v4()).collect();
    
    group.bench_function("create_input_struct", |b| {
        b.iter(|| {
            let offer_id = offer_ids[black_box(0)];
            let input = CreateReviewInput {
                offer_id,
                rating: black_box(5),
                text: black_box("Benchmark review text".to_string()),
            };
            black_box(input)
        })
    });
    
    group.finish();
}

fn benchmark_text_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_validation");
    
    // Test different validation scenarios
    let test_cases = vec![
        ("empty", ""),
        ("short", "Good"),
        ("medium", &"a".repeat(100)),
        ("long", &"a".repeat(1000)),
        ("max_length", &"a".repeat(5000)),
        ("too_long", &"a".repeat(5001)),
        ("whitespace_only", "   "),
        ("with_newlines", "Line 1\nLine 2\nLine 3"),
    ];
    
    for (name, text) in test_cases {
        let input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating: 5,
            text: text.to_string(),
        };
        
        group.bench_with_input(
            BenchmarkId::new("text_validation", name),
            &input,
            |b, input| {
                b.iter(|| {
                    black_box(input.validate())
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_rating_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("rating_validation");
    
    // Test different rating values
    for rating in [0, 1, 3, 5, 6, 10] {
        let input = CreateReviewInput {
            offer_id: Uuid::new_v4(),
            rating,
            text: "Test review".to_string(),
        };
        
        group.bench_with_input(
            BenchmarkId::new("rating_validation", rating),
            &rating,
            |b, _| {
                b.iter(|| {
                    black_box(input.validate())
                })
            },
        );
    }
    
    group.finish();
}

// Benchmark serialization/deserialization
fn benchmark_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    
    let input = CreateReviewInput {
        offer_id: Uuid::new_v4(),
        rating: 5,
        text: "Serialization benchmark test review".to_string(),
    };
    
    group.bench_function("serialize_create_input", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&input))
        })
    });
    
    let serialized = serde_json::to_string(&input).unwrap();
    group.bench_function("deserialize_create_input", |b| {
        b.iter(|| {
            black_box(serde_json::from_str::<CreateReviewInput>(&serialized))
        })
    });
    
    group.finish();
}

// Benchmark string operations commonly used in reviews
fn benchmark_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");
    
    let text = "This is a sample review text with some content to benchmark string operations";
    
    group.bench_function("string_clone", |b| {
        b.iter(|| {
            black_box(text.to_string())
        })
    });
    
    group.bench_function("string_trim", |b| {
        let padded_text = format!("   {}   ", text);
        b.iter(|| {
            black_box(padded_text.trim())
        })
    });
    
    group.bench_function("string_length", |b| {
        b.iter(|| {
            black_box(text.len())
        })
    });
    
    group.bench_function("string_is_empty", |b| {
        b.iter(|| {
            black_box(text.is_empty())
        })
    });
    
    group.finish();
}

// Configure benchmark groups
criterion_group!(
    benches,
    benchmark_review_validation,
    benchmark_update_review_validation,
    benchmark_moderation_status_parsing,
    benchmark_uuid_operations,
    benchmark_review_creation_input,
    benchmark_text_validation,
    benchmark_rating_validation,
    benchmark_serialization,
    benchmark_string_operations
);

criterion_main!(benches);