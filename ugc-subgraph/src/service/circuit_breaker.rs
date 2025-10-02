use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::UgcError;

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: usize,
    /// Time to wait before transitioning from Open to HalfOpen
    pub timeout: Duration,
    /// Number of successful requests needed to close the circuit from HalfOpen
    pub success_threshold: usize,
    /// Time window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    last_failure_time: AtomicU64,
    last_success_time: AtomicU64,
    service_name: String,
}

impl CircuitBreaker {
    pub fn new(service_name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            last_failure_time: AtomicU64::new(0),
            last_success_time: AtomicU64::new(0),
            service_name,
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, UgcError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        // Check if circuit is open
        if self.is_open().await {
            return Err(UgcError::CircuitBreakerOpen {
                service: self.service_name.clone(),
            });
        }

        // Execute the function
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(err) => {
                self.on_failure().await;
                Err(err)
            }
        }
    }

    /// Check if the circuit breaker is open
    async fn is_open(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Open => {
                // Check if we should transition to half-open
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
                let last_failure = self.last_failure_time.load(Ordering::Relaxed);
                
                if now - last_failure >= self.config.timeout.as_nanos() as u64 {
                    drop(state);
                    self.transition_to_half_open().await;
                    false
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => false,
            CircuitState::Closed => false,
        }
    }

    /// Handle successful operation
    async fn on_success(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        self.last_success_time.store(now, Ordering::Relaxed);

        let state = self.state.read().await;
        match *state {
            CircuitState::HalfOpen => {
                let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if success_count >= self.config.success_threshold {
                    drop(state);
                    self.transition_to_closed().await;
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Should not happen, but reset counts just in case
                self.failure_count.store(0, Ordering::Relaxed);
                self.success_count.store(0, Ordering::Relaxed);
            }
        }
    }

    /// Handle failed operation
    async fn on_failure(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
        self.last_failure_time.store(now, Ordering::Relaxed);

        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => {
                let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failure_count >= self.config.failure_threshold {
                    drop(state);
                    self.transition_to_open().await;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state transitions back to open
                drop(state);
                self.transition_to_open().await;
            }
            CircuitState::Open => {
                // Already open, just increment failure count
                self.failure_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Transition to open state
    async fn transition_to_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Open;
        self.success_count.store(0, Ordering::Relaxed);
        
        warn!(
            service = %self.service_name,
            failure_count = self.failure_count.load(Ordering::Relaxed),
            "Circuit breaker opened"
        );
    }

    /// Transition to half-open state
    async fn transition_to_half_open(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::HalfOpen;
        self.success_count.store(0, Ordering::Relaxed);
        
        info!(
            service = %self.service_name,
            "Circuit breaker transitioned to half-open"
        );
    }

    /// Transition to closed state
    async fn transition_to_closed(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        
        info!(
            service = %self.service_name,
            "Circuit breaker closed"
        );
    }

    /// Get current state for monitoring
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.clone()
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.get_state().await,
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            service_name: self.service_name.clone(),
        }
    }
}

/// Circuit breaker metrics for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub service_name: String,
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        }
    }
}

/// Retry mechanism with exponential backoff
pub struct RetryMechanism {
    config: RetryConfig,
}

impl RetryMechanism {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Execute a function with retry logic
    pub async fn call<F, Fut, T>(&self, mut f: F) -> Result<T, UgcError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, UgcError>>,
    {
        let mut attempt = 0;
        let mut delay = self.config.initial_delay;

        loop {
            attempt += 1;
            
            match f().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    // Don't retry non-retryable errors
                    if !err.is_retryable() || attempt >= self.config.max_attempts {
                        return Err(err);
                    }

                    warn!(
                        attempt = attempt,
                        max_attempts = self.config.max_attempts,
                        delay_ms = delay.as_millis(),
                        error = %err,
                        "Retrying failed operation"
                    );

                    // Wait before retrying
                    tokio::time::sleep(delay).await;

                    // Calculate next delay with exponential backoff
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * self.config.backoff_multiplier) as u64
                        ),
                        self.config.max_delay,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 1,
            failure_window: Duration::from_secs(60),
        };
        
        let cb = CircuitBreaker::new("test".to_string(), config);
        
        // Initially closed
        assert_eq!(cb.get_state().await, CircuitState::Closed);
        
        // First failure
        let result = cb.call(|| async { Err(UgcError::InternalError("test".to_string())) }).await;
        assert!(result.is_err());
        assert_eq!(cb.get_state().await, CircuitState::Closed);
        
        // Second failure should open the circuit
        let result = cb.call(|| async { Err(UgcError::InternalError("test".to_string())) }).await;
        assert!(result.is_err());
        assert_eq!(cb.get_state().await, CircuitState::Open);
        
        // Next call should fail immediately
        let result = cb.call(|| async { Ok("success") }).await;
        assert!(matches!(result, Err(UgcError::CircuitBreakerOpen { .. })));
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            timeout: Duration::from_millis(50),
            success_threshold: 1,
            failure_window: Duration::from_secs(60),
        };
        
        let cb = CircuitBreaker::new("test".to_string(), config);
        
        // Trigger failure to open circuit
        let _ = cb.call(|| async { Err(UgcError::InternalError("test".to_string())) }).await;
        assert_eq!(cb.get_state().await, CircuitState::Open);
        
        // Wait for timeout
        sleep(Duration::from_millis(60)).await;
        
        // Next call should transition to half-open and succeed
        let result = cb.call(|| async { Ok("success") }).await;
        assert!(result.is_ok());
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_retry_mechanism() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            backoff_multiplier: 2.0,
        };
        
        let retry = RetryMechanism::new(config);
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry.call(|| {
            let count = attempt_count_clone.fetch_add(1, Ordering::Relaxed) + 1;
            async move {
                if count < 3 {
                    Err(UgcError::ExternalServiceError {
                        service: "test".to_string(),
                        message: "temporary failure".to_string(),
                    })
                } else {
                    Ok("success")
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let config = RetryConfig::default();
        let retry = RetryMechanism::new(config);
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let result = retry.call(|| {
            attempt_count_clone.fetch_add(1, Ordering::Relaxed);
            async move {
                Err(UgcError::ValidationError {
                    message: "invalid input".to_string(),
                })
            }
        }).await;
        
        assert!(result.is_err());
        // Should only attempt once for non-retryable errors
        assert_eq!(attempt_count.load(Ordering::Relaxed), 1);
    }
}