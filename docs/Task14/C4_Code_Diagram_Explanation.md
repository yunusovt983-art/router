# C4 Code Diagram - Подробное объяснение

## 📋 Обзор диаграммы
**Файл:** `C4_Code_Diagram.puml`  
**Уровень:** Code (Level 4)  
**Цель:** Детальная структура классов и их взаимодействий в системе оптимизации производительности Task 14

## 🎯 Архитектурное назначение

Эта диаграмма показывает **конкретную реализацию Task 14 на уровне кода**, демонстрируя классы, методы и их взаимосвязи для достижения оптимальной производительности GraphQL federation.

## 🏗️ Системы классов

### 1. DataLoader System

#### 1.1 DataLoaderManager
```rust
// Фактическая реализация: src/performance/dataloader/manager.rs
pub struct DataLoaderManager {
    review_loader: Arc<ReviewDataLoader>,
    rating_loader: Arc<RatingDataLoader>,
    offer_loader: Arc<OfferDataLoader>,
    user_loader: Arc<UserDataLoader>,
    metrics: Arc<MetricsCollector>,
    config: DataLoaderConfig,
}

impl DataLoaderManager {
    pub fn new(
        db_pool: PgPool,
        external_clients: ExternalClients,
        config: DataLoaderConfig
    ) -> Self {
        Self {
            review_loader: Arc::new(ReviewDataLoader::new(
                ReviewRepository::new(db_pool.clone()),
                config.clone()
            )),
            rating_loader: Arc::new(RatingDataLoader::new(
                RatingRepository::new(db_pool.clone()),
                config.clone()
            )),
            offer_loader: Arc::new(OfferDataLoader::new(
                external_clients.offers_client,
                config.clone()
            )),
            user_loader: Arc::new(UserDataLoader::new(
                external_clients.users_client,
                config.clone()
            )),
            metrics: Arc::new(MetricsCollector::new()),
            config,
        }
    }
    
    // Task 14: Typed access to specific loaders
    pub fn get_review_loader(&self) -> &ReviewDataLoader {
        &self.review_loader
    }
    
    pub fn get_rating_loader(&self) -> &RatingDataLoader {
        &self.rating_loader
    }
    
    // Task 14: Request context creation for isolation
    pub fn create_request_context(&self) -> DataLoaderRequestContext {
        DataLoaderRequestContext::new(
            self.review_loader.clone(),
            self.rating_loader.clone(),
            self.offer_loader.clone(),
            self.user_loader.clone(),
        )
    }
}
```#
### 1.2 ReviewDataLoader
```rust
// Фактическая реализация: src/performance/dataloader/review_loader.rs
pub struct ReviewDataLoader {
    batch_fn: BatchFn<ReviewId, Review>,
    cache: Arc<RwLock<HashMap<ReviewId, Review>>>,
    repository: Arc<ReviewRepository>,
    config: DataLoaderConfig,
    metrics: Arc<MetricsCollector>,
}

impl ReviewDataLoader {
    pub fn new(repository: ReviewRepository, config: DataLoaderConfig) -> Self {
        let repo = Arc::new(repository);
        let batch_fn = {
            let repo = repo.clone();
            BatchFn::new(
                move |ids: Vec<ReviewId>| {
                    let repo = repo.clone();
                    async move { repo.find_by_ids(ids).await }
                },
                config.max_batch_size
            )
        };
        
        Self {
            batch_fn,
            cache: Arc::new(RwLock::new(HashMap::new())),
            repository: repo,
            config,
            metrics: Arc::new(MetricsCollector::new()),
        }
    }
    
    // Task 14: Single item loading with batching
    pub async fn load(&self, id: ReviewId) -> Result<Review> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(review) = cache.get(&id) {
                self.metrics.record_cache_hit("dataloader_memory");
                return Ok(review.clone());
            }
        }
        
        // Load through batch function
        self.batch_fn.load(id).await
    }
    
    // Task 14: Multiple items loading
    pub async fn load_many(&self, ids: Vec<ReviewId>) -> Result<Vec<Review>> {
        self.batch_fn.load_many(ids).await
    }
    
    // Task 14: Clear request-scoped cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.try_write() {
            cache.clear();
        }
    }
}
```