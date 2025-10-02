# Task 9: Component Diagram - Подробное объяснение компонентов оптимизации производительности

## 🎯 Цель диаграммы

Component диаграмма Task 9 детализирует **внутреннюю структуру компонентов системы оптимизации производительности**, показывая конкретные компоненты внутри каждого слоя кеширования, DataLoader оптимизации и rate limiting, их взаимодействие и специализированные функции для обеспечения максимальной производительности GraphQL федерации.

## 🏗️ Детальная структура компонентов оптимизации

### 1. Caching Components - Компоненты кеширования

#### Redis Integration - Интеграция Redis
```rust
// ugc-subgraph/src/cache/redis_integration/redis_client.rs
use redis::{Client, Commands, Connection, RedisResult, ConnectionLike};
use redis::cluster::ClusterClient;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Redis клиент с поддержкой кластера и connection pooling
#[derive(Clone)]
pub struct RedisClient {
    client_type: RedisClientType,
    connection_pool: Arc<ConnectionPool>,
    config: RedisClientConfig,
    metrics: Arc<RedisMetrics>,
}

#[derive(Clone)]
enum RedisClientType {
    Single(Client),
    Cluster(ClusterClient),
}

#[derive(Debug, Clone)]
pub struct RedisClientConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub command_timeout: Duration,
    pub retry_attempts: usize,
    pub retry_delay: Duration,
    pub pipeline_enabled: bool,
    pub compression_enabled: bool,
}

impl RedisClient {
    pub async fn new(redis_urls: Vec<String>, config: RedisClientConfig) -> Result<Self, RedisError> {
        let client_type = if redis_urls.len() > 1 {
            // Redis Cluster mode
            let cluster_client = ClusterClient::new(redis_urls)
                .map_err(RedisError::ConnectionError)?;
            RedisClientType::Cluster(cluster_client)
        } else {
            // Single Redis instance
            let client = Client::open(redis_urls[0].as_str())
                .map_err(RedisError::ConnectionError)?;
            RedisClientType::Single(client)
        };

        let connection_pool = Arc::new(
            ConnectionPool::new(config.max_connections).await?
        );

        let metrics = Arc::new(RedisMetrics::new()?);

        Ok(Self {
            client_type,
            connection_pool,
            config,
            metrics,
        })
    }

    /// Execute Redis command with retry logic
    pub async fn execute_command<T, F>(&self, command: F) -> Result<T, RedisError>
    where
        F: Fn(&mut dyn ConnectionLike) -> RedisResult<T> + Send + Sync,
        T: Send + 'static,
    {
        let start_time = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..self.config.retry_attempts {
            match self.try_execute_command(&command).await {
                Ok(result) => {
                    // Record success metrics
                    let execution_time = start_time.elapsed();
                    self.metrics.command_duration
                        .observe(execution_time.as_secs_f64());
                    self.metrics.commands_total
                        .with_label_values(&["success"])
                        .inc();

                    if attempt > 0 {
                        self.metrics.retry_success_total.inc();
                    }

                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < self.config.retry_attempts - 1 {
                        tracing::warn!(
                            attempt = attempt + 1,
                            max_attempts = self.config.retry_attempts,
                            error = %last_error.as_ref().unwrap(),
                            "Redis command failed, retrying"
                        );

                        tokio::time::sleep(self.config.retry_delay).await;
                    }
                }
            }
        }

        // Record failure metrics
        let execution_time = start_time.elapsed();
        self.metrics.command_duration
            .observe(execution_time.as_secs_f64());
        self.metrics.commands_total
            .with_label_values(&["error"])
            .inc();
        self.metrics.retry_exhausted_total.inc();

        Err(last_error.unwrap())
    }

    /// Try to execute command once
    async fn try_execute_command<T, F>(&self, command: &F) -> Result<T, RedisError>
    where
        F: Fn(&mut dyn ConnectionLike) -> RedisResult<T>,
    {
        let mut connection = self.connection_pool.get_connection().await?;
        
        let result = tokio::time::timeout(
            self.config.command_timeout,
            tokio::task::spawn_blocking({
                let command = command.clone();
                move || command(&mut *connection)
            })
        ).await
        .map_err(|_| RedisError::Timeout)?
        .map_err(|e| RedisError::ExecutionError(e.to_string()))?
        .map_err(RedisError::CommandError)?;

        self.connection_pool.return_connection(connection).await;
        Ok(result)
    }

    /// Pipeline multiple commands for better performance
    pub async fn pipeline<T>(&self, commands: Vec<RedisCommand>) -> Result<Vec<T>, RedisError>
    where
        T: redis::FromRedisValue + Send + 'static,
    {
        if !self.config.pipeline_enabled || commands.is_empty() {
            return Err(RedisError::PipelineDisabled);
        }

        let start_time = std::time::Instant::now();
        let mut connection = self.connection_pool.get_connection().await?;

        let results = tokio::task::spawn_blocking({
            let commands = commands.clone();
            move || {
                let mut pipe = redis::pipe();
                
                for command in commands {
                    match command {
                        RedisCommand::Get(key) => { pipe.get(&key); }
                        RedisCommand::Set(key, value, ttl) => {
                            if let Some(ttl_secs) = ttl {
                                pipe.setex(&key, ttl_secs, &value);
                            } else {
                                pipe.set(&key, &value);
                            }
                        }
                        RedisCommand::Del(key) => { pipe.del(&key); }
                    }
                }
                
                pipe.query::<Vec<T>>(&mut *connection)
            }
        }).await
        .map_err(|e| RedisError::ExecutionError(e.to_string()))?
        .map_err(RedisError::CommandError)?;

        let execution_time = start_time.elapsed();
        self.metrics.pipeline_duration
            .observe(execution_time.as_secs_f64());
        self.metrics.pipeline_commands_total
            .observe(commands.len() as f64);

        self.connection_pool.return_connection(connection).await;

        tracing::debug!(
            commands_count = commands.len(),
            execution_time_ms = execution_time.as_millis(),
            "Redis pipeline executed successfully"
        );

        Ok(results)
    }
}

#[derive(Debug, Clone)]
pub enum RedisCommand {
    Get(String),
    Set(String, Vec<u8>, Option<u64>),
    Del(String),
}

/// Connection pool for Redis connections
struct ConnectionPool {
    connections: Arc<Mutex<Vec<Box<dyn ConnectionLike + Send>>>>,
    max_size: usize,
    current_size: Arc<AtomicUsize>,
}

impl ConnectionPool {
    async fn new(max_size: usize) -> Result<Self, RedisError> {
        Ok(Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            max_size,
            current_size: Arc::new(AtomicUsize::new(0)),
        })
    }

    async fn get_connection(&self) -> Result<Box<dyn ConnectionLike + Send>, RedisError> {
        // Try to get from pool first
        {
            let mut connections = self.connections.lock().await;
            if let Some(conn) = connections.pop() {
                return Ok(conn);
            }
        }

        // Create new connection if pool is empty and under limit
        let current = self.current_size.load(Ordering::Relaxed);
        if current < self.max_size {
            self.current_size.fetch_add(1, Ordering::Relaxed);
            // Create new connection logic here
            todo!("Create new connection")
        } else {
            Err(RedisError::PoolExhausted)
        }
    }

    async fn return_connection(&self, connection: Box<dyn ConnectionLike + Send>) {
        let mut connections = self.connections.lock().await;
        if connections.len() < self.max_size {
            connections.push(connection);
        } else {
            self.current_size.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
```