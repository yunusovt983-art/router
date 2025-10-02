use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{Result, UgcError};
use crate::models::{OfferRating, Review};
use crate::repository::ReviewRepository;

/// Generic DataLoader trait for batching operations
#[async_trait]
pub trait DataLoader<K, V>: Send + Sync {
    async fn load(&self, key: K) -> Result<Option<V>>;
    async fn load_many(&self, keys: Vec<K>) -> Result<Vec<Option<V>>>;
    async fn clear(&self, key: &K);
    async fn clear_all(&self);
}

/// Batch loading function signature
pub type BatchLoadFn<K, V> = Arc<dyn Fn(Vec<K>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<Option<V>>>> + Send>> + Send + Sync>;

/// Generic DataLoader implementation with caching
pub struct GenericDataLoader<K, V> {
    cache: Arc<RwLock<HashMap<K, V>>>,
    batch_load_fn: BatchLoadFn<K, V>,
    max_batch_size: usize,
}

impl<K, V> GenericDataLoader<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(batch_load_fn: BatchLoadFn<K, V>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            batch_load_fn,
            max_batch_size: 100,
        }
    }

    pub fn with_max_batch_size(mut self, max_batch_size: usize) -> Self {
        self.max_batch_size = max_batch_size;
        self
    }
}

#[async_trait]
impl<K, V> DataLoader<K, V> for GenericDataLoader<K, V>
where
    K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    async fn load(&self, key: K) -> Result<Option<V>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(value) = cache.get(&key) {
                debug!("DataLoader cache hit for key");
                return Ok(Some(value.clone()));
            }
        }

        // Load single item (could be optimized to batch with other concurrent requests)
        let results = self.load_many(vec![key]).await?;
        Ok(results.into_iter().next().unwrap_or(None))
    }

    async fn load_many(&self, keys: Vec<K>) -> Result<Vec<Option<V>>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }

        let mut uncached_keys = Vec::new();
        let mut cached_results = HashMap::new();

        // Check cache for existing values
        {
            let cache = self.cache.read().await;
            for key in &keys {
                if let Some(value) = cache.get(key) {
                    cached_results.insert(key.clone(), Some(value.clone()));
                } else {
                    uncached_keys.push(key.clone());
                }
            }
        }

        // Batch load uncached keys
        if !uncached_keys.is_empty() {
            // Split into batches if necessary
            let mut all_loaded_results = HashMap::new();
            
            for chunk in uncached_keys.chunks(self.max_batch_size) {
                let chunk_keys = chunk.to_vec();
                let loaded_values = (self.batch_load_fn)(chunk_keys.clone()).await?;
                
                for (key, value) in chunk_keys.into_iter().zip(loaded_values.into_iter()) {
                    all_loaded_results.insert(key, value);
                }
            }

            // Update cache with loaded values
            {
                let mut cache = self.cache.write().await;
                for (key, value) in &all_loaded_results {
                    if let Some(v) = value {
                        cache.insert(key.clone(), v.clone());
                    }
                }
            }

            // Merge with cached results
            cached_results.extend(all_loaded_results);
        }

        // Return results in the same order as requested keys
        let results = keys
            .into_iter()
            .map(|key| cached_results.get(&key).cloned().unwrap_or(None))
            .collect();

        Ok(results)
    }

    async fn clear(&self, key: &K) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
        debug!("Cleared DataLoader cache for key");
    }

    async fn clear_all(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cleared all DataLoader cache");
    }
}

/// Review DataLoader for batching review lookups
pub struct ReviewDataLoader {
    loader: GenericDataLoader<Uuid, Review>,
}

impl ReviewDataLoader {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        let batch_load_fn: BatchLoadFn<Uuid, Review> = Arc::new(move |ids| {
            let repo = Arc::clone(&repository);
            Box::pin(async move {
                repo.get_reviews_by_ids(ids).await
            })
        });

        Self {
            loader: GenericDataLoader::new(batch_load_fn).with_max_batch_size(50),
        }
    }

    pub async fn load_review(&self, id: Uuid) -> Result<Option<Review>> {
        self.loader.load(id).await
    }

    pub async fn load_reviews(&self, ids: Vec<Uuid>) -> Result<Vec<Option<Review>>> {
        self.loader.load_many(ids).await
    }

    pub async fn clear_review(&self, id: &Uuid) {
        self.loader.clear(id).await
    }
}

/// Offer Rating DataLoader for batching rating lookups
pub struct OfferRatingDataLoader {
    loader: GenericDataLoader<Uuid, OfferRating>,
}

impl OfferRatingDataLoader {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        let batch_load_fn: BatchLoadFn<Uuid, OfferRating> = Arc::new(move |offer_ids| {
            let repo = Arc::clone(&repository);
            Box::pin(async move {
                repo.get_offer_ratings_by_ids(offer_ids).await
            })
        });

        Self {
            loader: GenericDataLoader::new(batch_load_fn).with_max_batch_size(50),
        }
    }

    pub async fn load_offer_rating(&self, offer_id: Uuid) -> Result<Option<OfferRating>> {
        self.loader.load(offer_id).await
    }

    pub async fn load_offer_ratings(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<OfferRating>>> {
        self.loader.load_many(offer_ids).await
    }

    pub async fn clear_offer_rating(&self, offer_id: &Uuid) {
        self.loader.clear(offer_id).await
    }
}

/// Reviews by Offer DataLoader for batching reviews by offer lookups
pub struct ReviewsByOfferDataLoader {
    loader: GenericDataLoader<Uuid, Vec<Review>>,
}

impl ReviewsByOfferDataLoader {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        let batch_load_fn: BatchLoadFn<Uuid, Vec<Review>> = Arc::new(move |offer_ids| {
            let repo = Arc::clone(&repository);
            Box::pin(async move {
                repo.get_reviews_by_offer_ids(offer_ids).await
            })
        });

        Self {
            loader: GenericDataLoader::new(batch_load_fn).with_max_batch_size(20),
        }
    }

    pub async fn load_reviews_for_offer(&self, offer_id: Uuid) -> Result<Option<Vec<Review>>> {
        self.loader.load(offer_id).await
    }

    pub async fn load_reviews_for_offers(&self, offer_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>> {
        self.loader.load_many(offer_ids).await
    }

    pub async fn clear_reviews_for_offer(&self, offer_id: &Uuid) {
        self.loader.clear(offer_id).await
    }
}

/// Reviews by Author DataLoader for batching reviews by author lookups
pub struct ReviewsByAuthorDataLoader {
    loader: GenericDataLoader<Uuid, Vec<Review>>,
}

impl ReviewsByAuthorDataLoader {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        let batch_load_fn: BatchLoadFn<Uuid, Vec<Review>> = Arc::new(move |author_ids| {
            let repo = Arc::clone(&repository);
            Box::pin(async move {
                repo.get_reviews_by_author_ids(author_ids).await
            })
        });

        Self {
            loader: GenericDataLoader::new(batch_load_fn).with_max_batch_size(20),
        }
    }

    pub async fn load_reviews_for_author(&self, author_id: Uuid) -> Result<Option<Vec<Review>>> {
        self.loader.load(author_id).await
    }

    pub async fn load_reviews_for_authors(&self, author_ids: Vec<Uuid>) -> Result<Vec<Option<Vec<Review>>>> {
        self.loader.load_many(author_ids).await
    }

    pub async fn clear_reviews_for_author(&self, author_id: &Uuid) {
        self.loader.clear(author_id).await
    }
}

/// Combined DataLoader service that manages all loaders
#[derive(Clone)]
pub struct DataLoaderService {
    pub reviews: Arc<ReviewDataLoader>,
    pub offer_ratings: Arc<OfferRatingDataLoader>,
    pub reviews_by_offer: Arc<ReviewsByOfferDataLoader>,
    pub reviews_by_author: Arc<ReviewsByAuthorDataLoader>,
}

impl DataLoaderService {
    pub fn new(repository: Arc<dyn ReviewRepository>) -> Self {
        Self {
            reviews: Arc::new(ReviewDataLoader::new(Arc::clone(&repository))),
            offer_ratings: Arc::new(OfferRatingDataLoader::new(Arc::clone(&repository))),
            reviews_by_offer: Arc::new(ReviewsByOfferDataLoader::new(Arc::clone(&repository))),
            reviews_by_author: Arc::new(ReviewsByAuthorDataLoader::new(Arc::clone(&repository))),
        }
    }

    /// Clear all caches (useful after mutations)
    pub async fn clear_all(&self) {
        self.reviews.loader.clear_all().await;
        self.offer_ratings.loader.clear_all().await;
        self.reviews_by_offer.loader.clear_all().await;
        self.reviews_by_author.loader.clear_all().await;
    }

    /// Clear caches related to a specific review
    pub async fn invalidate_review(&self, review: &Review) {
        self.reviews.clear_review(&review.id).await;
        self.reviews_by_offer.clear_reviews_for_offer(&review.offer_id).await;
        self.reviews_by_author.clear_reviews_for_author(&review.author_id).await;
        self.offer_ratings.clear_offer_rating(&review.offer_id).await;
    }

    /// Clear caches related to a specific offer
    pub async fn invalidate_offer(&self, offer_id: Uuid) {
        self.reviews_by_offer.clear_reviews_for_offer(&offer_id).await;
        self.offer_ratings.clear_offer_rating(&offer_id).await;
    }

    /// Clear caches related to a specific author
    pub async fn invalidate_author(&self, author_id: Uuid) {
        self.reviews_by_author.clear_reviews_for_author(&author_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_generic_dataloader() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let batch_load_fn: BatchLoadFn<i32, String> = Arc::new(move |keys| {
            let count = Arc::clone(&call_count_clone);
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                let results = keys
                    .into_iter()
                    .map(|k| Some(format!("value_{}", k)))
                    .collect();
                Ok(results)
            })
        });

        let loader = GenericDataLoader::new(batch_load_fn);

        // First load should call batch function
        let result1 = loader.load(1).await.unwrap();
        assert_eq!(result1, Some("value_1".to_string()));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second load of same key should use cache
        let result2 = loader.load(1).await.unwrap();
        assert_eq!(result2, Some("value_1".to_string()));
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // No additional call

        // Load different key should call batch function again
        let result3 = loader.load(2).await.unwrap();
        assert_eq!(result3, Some("value_2".to_string()));
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_batch_loading() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let batch_load_fn: BatchLoadFn<i32, String> = Arc::new(move |keys| {
            let count = Arc::clone(&call_count_clone);
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                let results = keys
                    .into_iter()
                    .map(|k| Some(format!("value_{}", k)))
                    .collect();
                Ok(results)
            })
        });

        let loader = GenericDataLoader::new(batch_load_fn);

        // Load multiple keys at once
        let results = loader.load_many(vec![1, 2, 3]).await.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], Some("value_1".to_string()));
        assert_eq!(results[1], Some("value_2".to_string()));
        assert_eq!(results[2], Some("value_3".to_string()));
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Single batch call

        // Subsequent loads should use cache
        let result = loader.load(2).await.unwrap();
        assert_eq!(result, Some("value_2".to_string()));
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // No additional call
    }

    #[tokio::test]
    async fn test_cache_clearing() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let batch_load_fn: BatchLoadFn<i32, String> = Arc::new(move |keys| {
            let count = Arc::clone(&call_count_clone);
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                let results = keys
                    .into_iter()
                    .map(|k| Some(format!("value_{}", k)))
                    .collect();
                Ok(results)
            })
        });

        let loader = GenericDataLoader::new(batch_load_fn);

        // Load and cache
        let _result1 = loader.load(1).await.unwrap();
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Clear cache
        loader.clear(&1).await;

        // Load again should call batch function
        let _result2 = loader.load(1).await.unwrap();
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}