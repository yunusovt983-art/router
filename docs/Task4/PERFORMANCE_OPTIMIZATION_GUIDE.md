# Task 4: Performance Optimization Guide

## Обзор

Этот документ содержит comprehensive guide по оптимизации производительности федеративной GraphQL системы Auto.ru в рамках Task 4. Руководство покрывает все уровни системы от Gateway до database optimization с конкретными техниками и best practices.

## 🚀 Gateway Layer Optimization

### Query Planning Optimization

#### Intelligent Query Plan Caching
```typescript
// Оптимизированное кеширование планов запросов
class OptimizedQueryPlanner {
  private planCache = new Redis({
    host: 'redis-cluster',
    keyPrefix: 'qplan:',
    ttl: 3600, // 1 hour cache
  });

  async planQuery(query: DocumentNode): Promise<QueryPlan> {
    const queryHash = this.generateQueryHash(query);
    
    // Попытка получить из кеша
    let plan = await this.planCache.get(queryHash);
    if (plan) {
      this.metrics.incrementCounter('query_plan_cache_hit');
      return plan;
    }

    // Создание нового плана
    plan = await this.createOptimizedPlan(query);
    
    // Кеширование с TTL на основе сложности
    const ttl = this.calculateCacheTTL(plan.complexity);
    await this.planCache.set(queryHash, plan, ttl);
    
    this.metrics.incrementCounter('query_plan_cache_miss');
    return plan;
  }

  private calculateCacheTTL(complexity: number): number {
    // Более сложные запросы кешируются дольше
    return Math.min(3600, complexity * 60);
  }
}
```

#### Query Complexity Analysis
```typescript
// Анализ сложности запросов для оптимизации
class QueryComplexityAnalyzer {
  analyzeComplexity(query: DocumentNode): ComplexityAnalysis {
    const analysis = {
      depth: this.calculateDepth(query),
      fieldCount: this.countFields(query),
      joinComplexity: this.calculateJoinComplexity(query),
      estimatedCost: 0,
      recommendations: []
    };

    analysis.estimatedCost = this.calculateEstimatedCost(analysis);
    analysis.recommendations = this.generateOptimizationRecommendations(analysis);

    return analysis;
  }

  private generateOptimizationRecommendations(analysis: ComplexityAnalysis): string[] {
    const recommendations = [];

    if (analysis.depth > 10) {
      recommendations.push('Consider reducing query depth or using pagination');
    }

    if (analysis.fieldCount > 100) {
      recommendations.push('Consider field selection optimization');
    }

    if (analysis.joinComplexity > 50) {
      recommendations.push('Consider DataLoader batching for related entities');
    }

    return recommendations;
  }
}
```

### Response Caching Strategy

#### Multi-Level Caching
```typescript
// Многоуровневое кеширование ответов
class ResponseCacheManager {
  private l1Cache = new Map(); // In-memory cache
  private l2Cache = new Redis(); // Redis cache
  private cdnCache = new CloudFront(); // CDN cache

  async get(key: string): Promise<any> {
    // L1: Memory cache (fastest)
    let result = this.l1Cache.get(key);
    if (result) {
      this.metrics.incrementCounter('l1_cache_hit');
      return result;
    }

    // L2: Redis cache (fast)
    result = await this.l2Cache.get(key);
    if (result) {
      this.l1Cache.set(key, result, { ttl: 60 }); // 1 minute L1 cache
      this.metrics.incrementCounter('l2_cache_hit');
      return result;
    }

    // L3: CDN cache (for static/semi-static data)
    if (this.isStaticData(key)) {
      result = await this.cdnCache.get(key);
      if (result) {
        this.l2Cache.set(key, result, { ttl: 300 }); // 5 minutes L2 cache
        this.l1Cache.set(key, result, { ttl: 60 });
        this.metrics.incrementCounter('cdn_cache_hit');
        return result;
      }
    }

    this.metrics.incrementCounter('cache_miss');
    return null;
  }

  async set(key: string, value: any, options: CacheOptions): Promise<void> {
    // Определение TTL на основе типа данных
    const ttl = this.calculateOptimalTTL(key, value, options);

    // Кеширование на всех уровнях
    this.l1Cache.set(key, value, { ttl: Math.min(ttl, 60) });
    await this.l2Cache.set(key, value, { ttl });

    // CDN кеширование для статических данных
    if (this.isStaticData(key)) {
      await this.cdnCache.set(key, value, { ttl: ttl * 10 });
    }
  }
}
```

#### Field-Level Caching
```typescript
// Кеширование на уровне полей
class FieldLevelCache {
  async cacheFieldResult(
    typename: string,
    fieldName: string,
    args: any,
    result: any,
    ttl: number
  ): Promise<void> {
    const cacheKey = this.generateFieldCacheKey(typename, fieldName, args);
    
    // Кеширование с метаданными
    const cacheEntry = {
      result,
      timestamp: Date.now(),
      ttl,
      typename,
      fieldName,
      dependencies: this.extractDependencies(result)
    };

    await this.cache.set(cacheKey, cacheEntry, ttl);
    
    // Индексирование для invalidation
    await this.addToInvalidationIndex(typename, fieldName, cacheKey);
  }

  async invalidateByType(typename: string): Promise<void> {
    const keys = await this.getKeysForType(typename);
    await Promise.all(keys.map(key => this.cache.del(key)));
    
    this.metrics.incrementCounter('cache_invalidation', { type: typename });
  }
}
```

### Rate Limiting Optimization

#### Intelligent Rate Limiting
```typescript
// Интеллектуальное ограничение частоты запросов
class IntelligentRateLimiter {
  async checkRateLimit(userId: string, operation: string): Promise<RateLimitResult> {
    const userTier = await this.getUserTier(userId);
    const operationCost = this.getOperationCost(operation);
    
    // Динамические лимиты на основе пользователя и операции
    const limits = this.calculateDynamicLimits(userTier, operationCost);
    
    // Sliding window с Redis
    const current = await this.incrementSlidingWindow(userId, operationCost);
    
    const result = {
      allowed: current <= limits.maxRequests,
      remaining: Math.max(0, limits.maxRequests - current),
      resetTime: this.getResetTime(),
      retryAfter: current > limits.maxRequests ? this.calculateRetryAfter(current, limits) : null
    };

    // Adaptive limits на основе системной нагрузки
    if (this.isSystemUnderLoad()) {
      result.allowed = result.allowed && current <= limits.maxRequests * 0.7;
    }

    return result;
  }

  private calculateDynamicLimits(userTier: UserTier, operationCost: number): RateLimits {
    const baseLimits = this.getBaseLimits(userTier);
    
    return {
      maxRequests: Math.floor(baseLimits.maxRequests / Math.max(1, operationCost / 10)),
      windowSize: baseLimits.windowSize,
      burstAllowance: baseLimits.burstAllowance
    };
  }
}
```

## 🔧 Subgraph Optimization

### DataLoader Optimization

#### Advanced DataLoader Patterns
```typescript
// Оптимизированные DataLoader'ы с кешированием
class OptimizedDataLoader<K, V> {
  private loader: DataLoader<K, V>;
  private cache: Redis;

  constructor(batchFn: BatchLoadFn<K, V>, options: DataLoaderOptions) {
    this.loader = new DataLoader(
      this.createOptimizedBatchFn(batchFn),
      {
        ...options,
        cacheMap: this.createCacheMap(),
        maxBatchSize: this.calculateOptimalBatchSize(),
      }
    );
  }

  private createOptimizedBatchFn(originalBatchFn: BatchLoadFn<K, V>): BatchLoadFn<K, V> {
    return async (keys: readonly K[]): Promise<(V | Error)[]> => {
      // Проверка кеша перед batch запросом
      const cachedResults = await this.getCachedResults(keys);
      const uncachedKeys = keys.filter((_, index) => !cachedResults[index]);

      if (uncachedKeys.length === 0) {
        return cachedResults;
      }

      // Batch запрос только для некешированных ключей
      const freshResults = await originalBatchFn(uncachedKeys);
      
      // Кеширование новых результатов
      await this.cacheResults(uncachedKeys, freshResults);

      // Объединение кешированных и свежих результатов
      return this.mergeResults(keys, cachedResults, uncachedKeys, freshResults);
    };
  }

  private calculateOptimalBatchSize(): number {
    // Динамический расчет оптимального размера batch'а
    const systemLoad = this.getSystemLoad();
    const networkLatency = this.getNetworkLatency();
    
    if (systemLoad > 0.8) return 10; // Меньшие batch'и при высокой нагрузке
    if (networkLatency > 100) return 50; // Большие batch'и при высокой латентности
    
    return 25; // Оптимальный размер по умолчанию
  }
}
```

#### Geo-Spatial DataLoader
```typescript
// Специализированный DataLoader для геопространственных запросов
class GeoSpatialDataLoader {
  private loader: DataLoader<GeoQuery, Offer[]>;

  constructor() {
    this.loader = new DataLoader(
      this.batchGeoQueries.bind(this),
      {
        maxBatchSize: 10,
        cacheKeyFn: this.createGeoKey.bind(this)
      }
    );
  }

  private async batchGeoQueries(queries: readonly GeoQuery[]): Promise<Offer[][]> {
    // Группировка запросов по географическим регионам
    const regionGroups = this.groupByRegion(queries);
    
    const results = await Promise.all(
      regionGroups.map(async (group) => {
        // Оптимизированный запрос для региона
        const regionOffers = await this.queryRegion(group.region, group.queries);
        
        // Распределение результатов по исходным запросам
        return this.distributeResults(group.queries, regionOffers);
      })
    );

    return results.flat();
  }

  private groupByRegion(queries: readonly GeoQuery[]): RegionGroup[] {
    const groups = new Map<string, GeoQuery[]>();
    
    queries.forEach(query => {
      const region = this.getRegionForCoordinates(query.lat, query.lng);
      if (!groups.has(region)) {
        groups.set(region, []);
      }
      groups.get(region)!.push(query);
    });

    return Array.from(groups.entries()).map(([region, queries]) => ({
      region,
      queries
    }));
  }
}
```

### Database Query Optimization

#### Connection Pool Optimization
```typescript
// Оптимизированное управление пулом соединений
class OptimizedConnectionPool {
  private pools: Map<string, Pool> = new Map();
  private metrics: MetricsCollector;

  createPool(config: PoolConfig): Pool {
    const optimizedConfig = this.optimizePoolConfig(config);
    
    const pool = new Pool({
      ...optimizedConfig,
      // Динамическое управление размером пула
      min: this.calculateMinConnections(),
      max: this.calculateMaxConnections(),
      
      // Оптимизированные таймауты
      acquireTimeoutMillis: 30000,
      createTimeoutMillis: 30000,
      destroyTimeoutMillis: 5000,
      idleTimeoutMillis: 30000,
      reapIntervalMillis: 1000,
      
      // Валидация соединений
      validate: this.validateConnection.bind(this),
      
      // Обработчики событий для мониторинга
      afterCreate: this.onConnectionCreate.bind(this),
      beforeDestroy: this.onConnectionDestroy.bind(this)
    });

    this.setupPoolMonitoring(pool);
    return pool;
  }

  private calculateMaxConnections(): number {
    const cpuCount = os.cpus().length;
    const memoryGB = os.totalmem() / (1024 ** 3);
    
    // Формула на основе ресурсов системы
    return Math.min(
      cpuCount * 4, // CPU-based limit
      Math.floor(memoryGB * 2), // Memory-based limit
      100 // Absolute maximum
    );
  }

  private async validateConnection(connection: any): Promise<boolean> {
    try {
      const result = await connection.query('SELECT 1');
      return result.rows.length === 1;
    } catch (error) {
      this.metrics.incrementCounter('connection_validation_failed');
      return false;
    }
  }
}
```

#### Query Optimization
```typescript
// Оптимизация SQL запросов
class QueryOptimizer {
  optimizeQuery(query: string, params: any[]): OptimizedQuery {
    let optimizedQuery = query;
    let optimizedParams = params;

    // Анализ и оптимизация запроса
    const analysis = this.analyzeQuery(query);
    
    if (analysis.hasUnnecessaryJoins) {
      optimizedQuery = this.removeUnnecessaryJoins(optimizedQuery);
    }

    if (analysis.canUseIndex) {
      optimizedQuery = this.addIndexHints(optimizedQuery, analysis.suggestedIndexes);
    }

    if (analysis.canUsePagination) {
      optimizedQuery = this.addOptimalPagination(optimizedQuery);
    }

    // Подготовленные запросы для лучшей производительности
    const preparedQuery = this.createPreparedStatement(optimizedQuery);

    return {
      query: optimizedQuery,
      params: optimizedParams,
      preparedStatement: preparedQuery,
      estimatedCost: analysis.estimatedCost,
      optimizations: analysis.appliedOptimizations
    };
  }

  private analyzeQuery(query: string): QueryAnalysis {
    // Парсинг и анализ SQL запроса
    const ast = this.parseSQL(query);
    
    return {
      hasUnnecessaryJoins: this.detectUnnecessaryJoins(ast),
      canUseIndex: this.canBenefitFromIndex(ast),
      suggestedIndexes: this.suggestIndexes(ast),
      canUsePagination: this.canOptimizePagination(ast),
      estimatedCost: this.estimateQueryCost(ast),
      appliedOptimizations: []
    };
  }
}
```

### Search Optimization

#### Elasticsearch Query Optimization
```typescript
// Оптимизация поисковых запросов
class SearchQueryOptimizer {
  optimizeSearchQuery(query: SearchQuery): OptimizedSearchQuery {
    const optimizedQuery = {
      ...query,
      // Оптимизация размера результатов
      size: this.calculateOptimalSize(query.size),
      
      // Оптимизация полей для поиска
      _source: this.optimizeSourceFields(query.fields),
      
      // Оптимизация сортировки
      sort: this.optimizeSort(query.sort),
      
      // Добавление кеширования
      request_cache: true,
      
      // Оптимизация агрегаций
      aggs: this.optimizeAggregations(query.aggs)
    };

    // Добавление suggest для автодополнения
    if (query.suggest) {
      optimizedQuery.suggest = this.optimizeSuggest(query.suggest);
    }

    return optimizedQuery;
  }

  private optimizeSourceFields(fields?: string[]): string[] | boolean {
    if (!fields || fields.length === 0) {
      // Возвращаем только необходимые поля по умолчанию
      return ['id', 'title', 'price', 'location', 'images'];
    }

    // Исключаем тяжелые поля если они не нужны
    const excludeFields = ['full_description', 'raw_data', 'internal_notes'];
    return fields.filter(field => !excludeFields.includes(field));
  }

  private optimizeAggregations(aggs?: any): any {
    if (!aggs) return undefined;

    // Оптимизация агрегаций для производительности
    const optimizedAggs = { ...aggs };

    Object.keys(optimizedAggs).forEach(key => {
      const agg = optimizedAggs[key];
      
      // Добавление кеширования для агрегаций
      if (agg.terms) {
        agg.terms.execution_hint = 'map';
        agg.terms.collect_mode = 'breadth_first';
      }

      // Ограничение размера агрегаций
      if (agg.terms && !agg.terms.size) {
        agg.terms.size = 10;
      }
    });

    return optimizedAggs;
  }
}
```

#### Search Index Optimization
```typescript
// Оптимизация индексов Elasticsearch
class SearchIndexOptimizer {
  async optimizeIndex(indexName: string): Promise<void> {
    // Анализ текущего состояния индекса
    const stats = await this.getIndexStats(indexName);
    const mapping = await this.getIndexMapping(indexName);

    // Оптимизация маппинга
    const optimizedMapping = this.optimizeMapping(mapping);
    
    // Оптимизация настроек индекса
    const optimizedSettings = this.optimizeIndexSettings(stats);

    // Применение оптимизаций
    await this.updateIndexSettings(indexName, optimizedSettings);
    
    // Reindex если необходимо
    if (this.needsReindex(mapping, optimizedMapping)) {
      await this.reindexWithOptimizations(indexName, optimizedMapping);
    }

    // Оптимизация сегментов
    await this.optimizeSegments(indexName);
  }

  private optimizeMapping(mapping: any): any {
    const optimized = { ...mapping };

    // Оптимизация полей для поиска
    Object.keys(optimized.properties).forEach(field => {
      const fieldMapping = optimized.properties[field];

      // Отключение индексации для полей, которые не используются в поиске
      if (this.isNonSearchableField(field)) {
        fieldMapping.index = false;
      }

      // Оптимизация текстовых полей
      if (fieldMapping.type === 'text') {
        // Добавление keyword subfield для сортировки и агрегаций
        fieldMapping.fields = {
          keyword: {
            type: 'keyword',
            ignore_above: 256
          }
        };
      }

      // Оптимизация числовых полей
      if (fieldMapping.type === 'integer' && this.isRangeField(field)) {
        fieldMapping.type = 'integer_range';
      }
    });

    return optimized;
  }

  private optimizeIndexSettings(stats: any): any {
    return {
      // Оптимизация refresh интервала
      refresh_interval: this.calculateOptimalRefreshInterval(stats),
      
      // Оптимизация количества шардов
      number_of_shards: this.calculateOptimalShards(stats),
      
      // Оптимизация реплик
      number_of_replicas: this.calculateOptimalReplicas(),
      
      // Оптимизация merge policy
      merge: {
        policy: {
          max_merge_at_once: 10,
          segments_per_tier: 10
        }
      },
      
      // Оптимизация кеширования
      requests: {
        cache: {
          enable: true
        }
      }
    };
  }
}
```

## 💾 Database Layer Optimization

### PostgreSQL Optimization

#### Index Optimization
```sql
-- Оптимизированные индексы для Auto.ru
-- Составные индексы для частых запросов
CREATE INDEX CONCURRENTLY idx_offers_location_price_status 
ON offers (location_id, price, status) 
WHERE status = 'active';

-- Частичные индексы для активных объявлений
CREATE INDEX CONCURRENTLY idx_offers_active_created 
ON offers (created_at DESC) 
WHERE status = 'active';

-- GIN индексы для полнотекстового поиска
CREATE INDEX CONCURRENTLY idx_offers_search_vector 
ON offers USING gin(search_vector);

-- Индексы для геопространственных запросов
CREATE INDEX CONCURRENTLY idx_offers_location_gist 
ON offers USING gist(location_point);

-- Индексы для JSON полей
CREATE INDEX CONCURRENTLY idx_offers_features_gin 
ON offers USING gin(features jsonb_path_ops);
```

#### Query Performance Tuning
```sql
-- Оптимизация конфигурации PostgreSQL
-- postgresql.conf оптимизации

-- Memory settings
shared_buffers = '4GB'                    -- 25% от RAM
effective_cache_size = '12GB'             -- 75% от RAM
work_mem = '256MB'                        -- Для сложных запросов
maintenance_work_mem = '1GB'              -- Для maintenance операций

-- Connection settings
max_connections = 200                     -- Оптимальное количество соединений
superuser_reserved_connections = 3

-- Checkpoint settings
checkpoint_completion_target = 0.9
wal_buffers = '64MB'
checkpoint_timeout = '15min'

-- Query planner settings
random_page_cost = 1.1                    -- Для SSD
effective_io_concurrency = 200            -- Для SSD

-- Logging settings для мониторинга
log_min_duration_statement = 1000         -- Логировать медленные запросы
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on
```

#### Partitioning Strategy
```sql
-- Партиционирование таблиц для улучшения производительности
-- Партиционирование offers по дате создания
CREATE TABLE offers_partitioned (
    id BIGSERIAL,
    title VARCHAR(255) NOT NULL,
    price DECIMAL(10,2),
    created_at TIMESTAMP NOT NULL,
    -- другие поля
) PARTITION BY RANGE (created_at);

-- Создание партиций по месяцам
CREATE TABLE offers_2024_01 PARTITION OF offers_partitioned
FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

CREATE TABLE offers_2024_02 PARTITION OF offers_partitioned
FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');

-- Автоматическое создание партиций
CREATE OR REPLACE FUNCTION create_monthly_partition(table_name text, start_date date)
RETURNS void AS $$
DECLARE
    partition_name text;
    end_date date;
BEGIN
    partition_name := table_name || '_' || to_char(start_date, 'YYYY_MM');
    end_date := start_date + interval '1 month';
    
    EXECUTE format('CREATE TABLE %I PARTITION OF %I FOR VALUES FROM (%L) TO (%L)',
                   partition_name, table_name, start_date, end_date);
END;
$$ LANGUAGE plpgsql;
```

### Redis Optimization

#### Memory Optimization
```redis
# Redis конфигурация для оптимальной производительности
# redis.conf оптимизации

# Memory management
maxmemory 8gb
maxmemory-policy allkeys-lru
maxmemory-samples 10

# Persistence optimization
save 900 1
save 300 10
save 60 10000
stop-writes-on-bgsave-error yes
rdbcompression yes
rdbchecksum yes

# Network optimization
tcp-keepalive 300
timeout 0
tcp-backlog 511

# Client optimization
maxclients 10000

# Slow log
slowlog-log-slower-than 10000
slowlog-max-len 128

# Latency monitoring
latency-monitor-threshold 100
```

#### Cache Strategies
```typescript
// Оптимизированные стратегии кеширования
class OptimizedCacheStrategy {
  // Write-through кеширование для критических данных
  async writeThrough(key: string, value: any): Promise<void> {
    // Одновременная запись в БД и кеш
    await Promise.all([
      this.database.set(key, value),
      this.cache.set(key, value, { ttl: 3600 })
    ]);
  }

  // Write-behind кеширование для некритических данных
  async writeBehind(key: string, value: any): Promise<void> {
    // Немедленная запись в кеш
    await this.cache.set(key, value, { ttl: 3600 });
    
    // Асинхронная запись в БД
    this.writeQueue.add({
      operation: 'set',
      key,
      value,
      timestamp: Date.now()
    });
  }

  // Cache-aside с intelligent prefetching
  async cacheAside(key: string): Promise<any> {
    // Попытка получить из кеша
    let value = await this.cache.get(key);
    if (value) {
      // Prefetch связанных данных
      this.prefetchRelatedData(key);
      return value;
    }

    // Загрузка из БД
    value = await this.database.get(key);
    if (value) {
      // Кеширование с адаптивным TTL
      const ttl = this.calculateAdaptiveTTL(key, value);
      await this.cache.set(key, value, { ttl });
    }

    return value;
  }

  private calculateAdaptiveTTL(key: string, value: any): number {
    // TTL на основе частоты доступа и типа данных
    const accessFrequency = this.getAccessFrequency(key);
    const dataType = this.getDataType(value);
    
    let baseTTL = 3600; // 1 hour default
    
    // Часто используемые данные кешируются дольше
    if (accessFrequency > 100) baseTTL *= 2;
    
    // Статические данные кешируются дольше
    if (dataType === 'static') baseTTL *= 24;
    
    return baseTTL;
  }
}
```

## 📊 Monitoring & Performance Metrics

### Key Performance Indicators

#### Gateway Metrics
```typescript
// Ключевые метрики производительности Gateway
class GatewayMetrics {
  private metrics = {
    // Latency metrics
    requestDuration: new Histogram({
      name: 'graphql_request_duration_seconds',
      help: 'Duration of GraphQL requests',
      labelNames: ['operation', 'status'],
      buckets: [0.01, 0.05, 0.1, 0.2, 0.5, 1, 2, 5]
    }),

    // Throughput metrics
    requestsTotal: new Counter({
      name: 'graphql_requests_total',
      help: 'Total number of GraphQL requests',
      labelNames: ['operation', 'status']
    }),

    // Cache metrics
    cacheHitRate: new Gauge({
      name: 'cache_hit_rate',
      help: 'Cache hit rate percentage',
      labelNames: ['cache_type']
    }),

    // Error metrics
    errorRate: new Gauge({
      name: 'error_rate',
      help: 'Error rate percentage',
      labelNames: ['error_type']
    }),

    // Query complexity metrics
    queryComplexity: new Histogram({
      name: 'query_complexity',
      help: 'GraphQL query complexity score',
      buckets: [1, 5, 10, 25, 50, 100, 200, 500]
    })
  };

  recordRequest(operation: string, duration: number, status: string): void {
    this.metrics.requestDuration
      .labels(operation, status)
      .observe(duration);
    
    this.metrics.requestsTotal
      .labels(operation, status)
      .inc();
  }

  recordCacheHit(cacheType: string, hit: boolean): void {
    const currentRate = this.getCacheHitRate(cacheType);
    const newRate = hit ? currentRate + 0.1 : currentRate - 0.1;
    
    this.metrics.cacheHitRate
      .labels(cacheType)
      .set(Math.max(0, Math.min(100, newRate)));
  }
}
```

#### Database Performance Metrics
```sql
-- Мониторинг производительности PostgreSQL
-- Медленные запросы
SELECT 
  query,
  calls,
  total_time,
  mean_time,
  rows,
  100.0 * shared_blks_hit / nullif(shared_blks_hit + shared_blks_read, 0) AS hit_percent
FROM pg_stat_statements 
ORDER BY total_time DESC 
LIMIT 10;

-- Использование индексов
SELECT 
  schemaname,
  tablename,
  indexname,
  idx_tup_read,
  idx_tup_fetch,
  idx_scan
FROM pg_stat_user_indexes 
ORDER BY idx_scan DESC;

-- Блокировки
SELECT 
  blocked_locks.pid AS blocked_pid,
  blocked_activity.usename AS blocked_user,
  blocking_locks.pid AS blocking_pid,
  blocking_activity.usename AS blocking_user,
  blocked_activity.query AS blocked_statement,
  blocking_activity.query AS current_statement_in_blocking_process
FROM pg_catalog.pg_locks blocked_locks
JOIN pg_catalog.pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid
JOIN pg_catalog.pg_locks blocking_locks ON blocking_locks.locktype = blocked_locks.locktype
JOIN pg_catalog.pg_stat_activity blocking_activity ON blocking_activity.pid = blocking_locks.pid
WHERE NOT blocked_locks.granted;
```

### Performance Alerting

#### Intelligent Alerting Rules
```yaml
# Prometheus alerting rules для производительности
groups:
- name: performance.rules
  rules:
  # High latency alert
  - alert: HighLatency
    expr: histogram_quantile(0.95, graphql_request_duration_seconds) > 0.5
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High GraphQL request latency"
      description: "95th percentile latency is {{ $value }}s"

  # Low cache hit rate
  - alert: LowCacheHitRate
    expr: cache_hit_rate < 70
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Low cache hit rate"
      description: "Cache hit rate is {{ $value }}%"

  # High error rate
  - alert: HighErrorRate
    expr: error_rate > 1
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "High error rate detected"
      description: "Error rate is {{ $value }}%"

  # Database connection pool exhaustion
  - alert: DatabaseConnectionPoolHigh
    expr: database_connections_active / database_connections_max > 0.8
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Database connection pool usage high"
      description: "Connection pool usage is {{ $value }}%"
```

## 🎯 Performance Targets & SLAs

### Service Level Objectives

#### Response Time Targets
- **P50 latency**: < 50ms для простых запросов
- **P95 latency**: < 200ms для сложных запросов
- **P99 latency**: < 500ms для всех запросов
- **Timeout**: 30 секунд максимум

#### Throughput Targets
- **Gateway throughput**: > 10,000 RPS
- **Database throughput**: > 5,000 QPS
- **Cache throughput**: > 50,000 RPS
- **Search throughput**: > 1,000 QPS

#### Availability Targets
- **System availability**: 99.9% uptime
- **Database availability**: 99.95% uptime
- **Cache availability**: 99.9% uptime
- **Search availability**: 99.5% uptime

#### Resource Utilization Targets
- **CPU utilization**: < 70% average
- **Memory utilization**: < 80% average
- **Disk I/O**: < 80% capacity
- **Network I/O**: < 70% capacity

### Performance Testing Strategy

#### Load Testing Scenarios
```javascript
// K6 load testing сценарии
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  stages: [
    { duration: '5m', target: 100 },   // Warm-up
    { duration: '10m', target: 500 },  // Normal load
    { duration: '5m', target: 1000 },  // Peak load
    { duration: '10m', target: 1000 }, // Sustained peak
    { duration: '5m', target: 0 },     // Cool-down
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
  },
};

export default function() {
  // Realistic GraphQL query
  const query = `
    query GetOfferWithDetails($id: ID!) {
      offer(id: $id) {
        title
        price
        description
        images
        seller {
          name
          rating
          reviewsCount
        }
        reviews(first: 5) {
          edges {
            node {
              rating
              text
              createdAt
              user {
                name
              }
            }
          }
        }
        similarOffers(first: 3) {
          title
          price
          images
        }
      }
    }
  `;

  const response = http.post('http://gateway:4000/graphql', 
    JSON.stringify({
      query,
      variables: { id: `offer-${Math.floor(Math.random() * 10000)}` }
    }),
    {
      headers: { 'Content-Type': 'application/json' },
    }
  );

  check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 200ms': (r) => r.timings.duration < 200,
    'no GraphQL errors': (r) => {
      const body = JSON.parse(r.body);
      return !body.errors;
    },
  });
}
```

## 📈 Continuous Performance Optimization

### Automated Performance Monitoring
```typescript
// Автоматический мониторинг и оптимизация производительности
class PerformanceOptimizer {
  private metrics: MetricsCollector;
  private optimizer: QueryOptimizer;

  async runOptimizationCycle(): Promise<void> {
    // Сбор метрик производительности
    const metrics = await this.collectPerformanceMetrics();
    
    // Анализ узких мест
    const bottlenecks = this.identifyBottlenecks(metrics);
    
    // Применение оптимизаций
    for (const bottleneck of bottlenecks) {
      await this.applyOptimization(bottleneck);
    }
    
    // Валидация улучшений
    await this.validateOptimizations(metrics);
  }

  private identifyBottlenecks(metrics: PerformanceMetrics): Bottleneck[] {
    const bottlenecks = [];

    // Анализ латентности
    if (metrics.p95Latency > 200) {
      bottlenecks.push({
        type: 'latency',
        severity: 'high',
        component: this.identifySlowComponent(metrics),
        recommendation: 'Optimize query execution or add caching'
      });
    }

    // Анализ пропускной способности
    if (metrics.throughput < 5000) {
      bottlenecks.push({
        type: 'throughput',
        severity: 'medium',
        component: 'gateway',
        recommendation: 'Scale horizontally or optimize connection pooling'
      });
    }

    // Анализ использования ресурсов
    if (metrics.cpuUtilization > 80) {
      bottlenecks.push({
        type: 'cpu',
        severity: 'high',
        component: 'application',
        recommendation: 'Optimize algorithms or scale vertically'
      });
    }

    return bottlenecks;
  }

  private async applyOptimization(bottleneck: Bottleneck): Promise<void> {
    switch (bottleneck.type) {
      case 'latency':
        await this.optimizeLatency(bottleneck);
        break;
      case 'throughput':
        await this.optimizeThroughput(bottleneck);
        break;
      case 'cpu':
        await this.optimizeCPUUsage(bottleneck);
        break;
    }
  }
}
```

## 🔧 Troubleshooting Performance Issues

### Common Performance Issues

#### Slow Query Identification
```sql
-- Идентификация медленных запросов
SELECT 
  query,
  calls,
  total_time / calls as avg_time,
  rows / calls as avg_rows,
  100.0 * shared_blks_hit / nullif(shared_blks_hit + shared_blks_read, 0) AS hit_percent
FROM pg_stat_statements 
WHERE calls > 100
ORDER BY total_time / calls DESC 
LIMIT 20;
```

#### Memory Leak Detection
```typescript
// Мониторинг утечек памяти
class MemoryLeakDetector {
  private memoryUsage: number[] = [];
  
  startMonitoring(): void {
    setInterval(() => {
      const usage = process.memoryUsage();
      this.memoryUsage.push(usage.heapUsed);
      
      // Анализ тренда использования памяти
      if (this.memoryUsage.length > 100) {
        const trend = this.calculateTrend(this.memoryUsage.slice(-100));
        
        if (trend > 0.1) { // Рост более 10% за последние измерения
          this.alertMemoryLeak(usage, trend);
        }
        
        this.memoryUsage = this.memoryUsage.slice(-50); // Сохраняем последние 50 измерений
      }
    }, 30000); // Каждые 30 секунд
  }

  private calculateTrend(data: number[]): number {
    // Простой расчет тренда
    const firstHalf = data.slice(0, data.length / 2);
    const secondHalf = data.slice(data.length / 2);
    
    const firstAvg = firstHalf.reduce((a, b) => a + b) / firstHalf.length;
    const secondAvg = secondHalf.reduce((a, b) => a + b) / secondHalf.length;
    
    return (secondAvg - firstAvg) / firstAvg;
  }
}
```

### Performance Debugging Tools

#### Query Performance Analyzer
```typescript
// Анализатор производительности запросов
class QueryPerformanceAnalyzer {
  async analyzeQuery(query: string, variables: any): Promise<QueryAnalysis> {
    const startTime = Date.now();
    
    // Выполнение запроса с трейсингом
    const result = await this.executeWithTracing(query, variables);
    
    const endTime = Date.now();
    const duration = endTime - startTime;
    
    // Анализ производительности
    const analysis = {
      duration,
      complexity: this.calculateComplexity(query),
      cacheHits: this.getCacheHits(),
      databaseQueries: this.getDatabaseQueries(),
      recommendations: this.generateRecommendations(duration, query)
    };

    return analysis;
  }

  private generateRecommendations(duration: number, query: string): string[] {
    const recommendations = [];

    if (duration > 1000) {
      recommendations.push('Query is slow, consider adding indexes or optimizing joins');
    }

    if (this.hasDeepNesting(query)) {
      recommendations.push('Query has deep nesting, consider using DataLoader for related entities');
    }

    if (this.hasLargeResultSet(query)) {
      recommendations.push('Query returns large result set, consider adding pagination');
    }

    return recommendations;
  }
}
```

## 📋 Performance Optimization Checklist

### ✅ Gateway Optimization
- [ ] Query plan caching implemented и optimized
- [ ] Response caching с field-level granularity
- [ ] Rate limiting с intelligent algorithms
- [ ] Connection pooling optimized
- [ ] Circuit breakers configured
- [ ] Health checks implemented

### ✅ Subgraph Optimization  
- [ ] DataLoader optimization с batching
- [ ] Database query optimization
- [ ] Index optimization completed
- [ ] Connection pool tuning
- [ ] Background job processing optimized
- [ ] Cache strategies implemented

### ✅ Database Optimization
- [ ] PostgreSQL configuration tuned
- [ ] Indexes optimized для common queries
- [ ] Partitioning implemented где appropriate
- [ ] Connection pooling configured
- [ ] Query performance analyzed
- [ ] Slow query monitoring active

### ✅ Cache Optimization
- [ ] Redis configuration optimized
- [ ] Cache strategies implemented
- [ ] TTL policies optimized
- [ ] Memory usage optimized
- [ ] Eviction policies configured
- [ ] Cache hit rates monitored

### ✅ Search Optimization
- [ ] Elasticsearch configuration tuned
- [ ] Index mapping optimized
- [ ] Query optimization implemented
- [ ] Aggregation optimization
- [ ] Segment optimization
- [ ] Search performance monitored

### ✅ Monitoring & Alerting
- [ ] Performance metrics collected
- [ ] Alerting rules configured
- [ ] Dashboards created
- [ ] SLA monitoring active
- [ ] Performance baselines established
- [ ] Automated optimization implemented

---

**Performance Optimization Status**: ⏳ In Progress / ✅ Optimized / ❌ Needs Attention

**Last Optimization Review**: [Date]

**Next Review Scheduled**: [Date]