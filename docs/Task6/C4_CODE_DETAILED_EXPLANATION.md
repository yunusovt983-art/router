# Task 6: Code Diagram - Подробное объяснение реализации безопасности на уровне кода

## 🎯 Цель диаграммы

Code диаграмма Task 6 демонстрирует **конкретную реализацию системы аутентификации и авторизации на уровне классов, структур и методов**. Диаграмма показывает как высокоуровневые принципы безопасности воплощаются в исполняемый Rust код, обеспечивая полную трассируемость от архитектурных решений до конкретных строк кода.

## 🔐 Authentication Implementation: Детальная реализация аутентификации

### AuthService - Центральный сервис аутентификации

#### Полная реализация с JWT и кешированием
```rust
// crates/shared/src/auth/service.rs
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use redis::{Client as RedisClient, Commands};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AuthService {
    // JWT конфигурация
    decoding_key: DecodingKey,
    encoding_key: EncodingKey,
    validation: Validation,
    
    // Кеширование
    cache: Arc<AuthCache>,
    
    // Базы данных
    db_pool: Pool<Postgres>,
    
    // Метрики и аудит
    metrics: Arc<AuthMetrics>,
    audit_logger: Arc<AuditLogger>,
}

impl AuthService {
    pub fn new(
        jwt_secret: &str,
        db_pool: Pool<Postgres>,
        redis_client: RedisClient,
    ) -> Result<Self, AuthError> {
        let encoding_key = EncodingKey::from_secret(jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());
        
        let mut validation = Validation::default();
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.set_audience(&["auto.ru-api"]);
        validation.set_issuer(&["https://auth.auto.ru"]);
        
        Ok(Self {
            decoding_key,
            encoding_key,
            validation,
            cache: Arc::new(AuthCache::new(redis_client, Duration::from_secs(300))?),
            db_pool,
            metrics: Arc::new(AuthMetrics::new()),
            audit_logger: Arc::new(AuditLogger::new()),
        })
    }
    
    /// Валидация JWT токена с комплексными проверками
    pub async fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let validation_start = std::time::Instant::now();
        
        // 1. Предварительные проверки
        if token.is_empty() {
            return Err(AuthError::EmptyToken);
        }
        
        let token_hash = self.calculate_token_hash(token);
        
        // 2. Проверка в blacklist
        if self.is_token_blacklisted(&token_hash).await? {
            self.metrics.blacklisted_tokens.inc();
            return Err(AuthError::TokenBlacklisted);
        }
        
        // 3. Проверка в кеше валидации
        if let Some(cached_claims) = self.cache.get_validated_token(&token_hash).await? {
            self.metrics.cache_hits.inc();
            return Ok(cached_claims);
        }
        
        // 4. JWT декодирование и валидация
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| {
                self.metrics.validation_failures.inc();
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => AuthError::InvalidSignature,
                    jsonwebtoken::errors::ErrorKind::InvalidToken => AuthError::InvalidToken,
                    _ => AuthError::ValidationError(e.to_string()),
                }
            })?;
        
        let claims = token_data.claims;
        
        // 5. Дополнительные проверки безопасности
        self.perform_security_validations(&claims).await?;
        
        // 6. Кеширование результата
        self.cache.cache_validated_token(&token_hash, &claims).await?;
        
        // 7. Метрики и аудит
        let validation_duration = validation_start.elapsed();
        self.metrics.validation_duration.observe(validation_duration.as_secs_f64());
        self.metrics.successful_validations.inc();
        
        self.audit_logger.log_token_validation(&claims.sub, true).await?;
        
        Ok(claims)
    }
    
    /// Создание JWT токена с полным контекстом
    pub fn create_token(&self, claims: &Claims) -> Result<String, AuthError> {
        let header = Header::default();
        
        let token = encode(&header, claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenCreationError(e.to_string()))?;
        
        // Метрики
        self.metrics.tokens_created.inc();
        
        Ok(token)
    }
    
    /// Обновление токена через refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        // 1. Валидация refresh токена
        let refresh_claims = self.validate_refresh_token(refresh_token).await?;
        
        // 2. Проверка активности сессии
        if !self.is_session_active(&refresh_claims.session_id).await? {
            return Err(AuthError::SessionInactive);
        }
        
        // 3. Получение актуальной информации о пользователе
        let user = self.get_user_from_db(&refresh_claims.sub).await?;
        let roles = self.get_user_roles(&user.id).await?;
        let permissions = self.get_user_permissions(&user.id).await?;
        
        // 4. Создание новых claims
        let now = chrono::Utc::now();
        let new_claims = Claims {
            sub: user.id.clone(),
            exp: (now + chrono::Duration::minutes(15)).timestamp() as usize,
            iat: now.timestamp() as usize,
            roles,
            permissions,
            session_id: refresh_claims.session_id.clone(),
        };
        
        // 5. Создание новых токенов
        let access_token = self.create_token(&new_claims)?;
        let new_refresh_token = self.create_refresh_token(&refresh_claims.sub, &refresh_claims.session_id)?;
        
        // 6. Инвалидация старого refresh токена
        self.revoke_refresh_token(refresh_token).await?;
        
        // 7. Аудит
        self.audit_logger.log_token_refresh(&user.id).await?;
        
        Ok(TokenPair {
            access_token,
            refresh_token: new_refresh_token,
            expires_in: 900, // 15 минут
            token_type: "Bearer".to_string(),
        })
    }
    
    /// Комплексные проверки безопасности токена
    async fn perform_security_validations(&self, claims: &Claims) -> Result<(), AuthError> {
        // Проверка временных рамок
        let now = chrono::Utc::now().timestamp() as usize;
        if claims.exp <= now {
            return Err(AuthError::TokenExpired);
        }
        
        if claims.iat > now + 300 { // Токен из будущего (с учетом clock skew)
            return Err(AuthError::InvalidIssuedAt);
        }
        
        // Проверка активности пользователя
        if !self.is_user_active(&claims.sub).await? {
            return Err(AuthError::UserInactive);
        }
        
        // Проверка активности сессии
        if !self.is_session_active(&claims.session_id).await? {
            return Err(AuthError::SessionInactive);
        }
        
        // Проверка на подозрительную активность
        if self.detect_anomalous_behavior(&claims).await? {
            self.audit_logger.log_suspicious_activity(&claims.sub).await?;
            return Err(AuthError::SuspiciousActivity);
        }
        
        Ok(())
    }
}
```

### Claims - Структура JWT claims

#### Расширенные claims с метаданными безопасности
```rust
// crates/shared/src/auth/claims.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    // Стандартные JWT claims
    pub sub: String,    // Subject (user ID)
    pub exp: usize,     // Expiration time
    pub iat: usize,     // Issued at
    pub nbf: Option<usize>, // Not before
    pub jti: Option<String>, // JWT ID для отзыва
    
    // Кастомные claims для авторизации
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    
    // Сессионные данные
    pub session_id: String,
    pub device_id: Option<String>,
    pub ip_address: Option<String>,
    
    // Метаданные безопасности
    pub auth_method: AuthMethod,
    pub mfa_verified: bool,
    pub risk_score: Option<f32>,
    pub last_activity: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    OAuth2(String), // Провайдер
    SAML,
    LDAP,
    MFA,
}

impl Claims {
    pub fn new(user_id: String, roles: Vec<String>) -> Self {
        let now = chrono::Utc::now();
        
        Self {
            sub: user_id,
            exp: (now + chrono::Duration::minutes(15)).timestamp() as usize,
            iat: now.timestamp() as usize,
            nbf: Some(now.timestamp() as usize),
            jti: Some(uuid::Uuid::new_v4().to_string()),
            roles,
            permissions: Vec::new(), // Будет заполнено позже
            session_id: uuid::Uuid::new_v4().to_string(),
            device_id: None,
            ip_address: None,
            auth_method: AuthMethod::Password,
            mfa_verified: false,
            risk_score: None,
            last_activity: Some(now.timestamp() as usize),
        }
    }
    
    /// Проверка истечения токена
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp() as usize;
        self.exp <= now
    }
    
    /// Проверка наличия роли
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }
    
    /// Проверка наличия разрешения
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }
    
    /// Время до истечения токена
    pub fn time_to_expiry(&self) -> Duration {
        let now = chrono::Utc::now().timestamp() as usize;
        if self.exp > now {
            Duration::from_secs((self.exp - now) as u64)
        } else {
            Duration::from_secs(0)
        }
    }
    
    /// Проверка необходимости обновления токена
    pub fn needs_refresh(&self) -> bool {
        self.time_to_expiry() < Duration::from_secs(300) // Обновляем за 5 минут до истечения
    }
    
    /// Создание UserContext из claims
    pub fn to_user_context(&self) -> UserContext {
        UserContext {
            user_id: self.sub.clone(),
            roles: self.roles.clone(),
            permissions: self.permissions.clone(),
            session_id: self.session_id.clone(),
            ip_address: self.ip_address.clone(),
            device_id: self.device_id.clone(),
            auth_method: self.auth_method.clone(),
            mfa_verified: self.mfa_verified,
            risk_score: self.risk_score,
        }
    }
}
```

### AuthMiddleware - Middleware функция с полной интеграцией

#### Axum middleware с комплексной обработкой
```rust
// crates/shared/src/auth/middleware.rs
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Основная функция middleware для аутентификации
pub async fn auth_middleware(
    State(auth_state): State<Arc<AuthState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let request_start = std::time::Instant::now();
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Добавляем request ID для трассировки
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap()
    );
    
    // 1. Извлечение метаданных запроса
    let client_ip = addr.ip().to_string();
    let user_agent = extract_user_agent(&request);
    let request_path = request.uri().path().to_string();
    let request_method = request.method().to_string();
    
    // 2. Обработка аутентификации
    let auth_result = match extract_and_validate_jwt(&request, &auth_state.auth_service).await {
        Ok(claims) => {
            // Успешная аутентификация
            let user_context = UserContext::from_claims(claims);
            
            // Проверка rate limiting
            if let Err(rate_limit_error) = auth_state.rate_limiter
                .check_rate_limit(&user_context.user_id, &request_path)
                .await 
            {
                // Логирование превышения лимита
                auth_state.audit_service.log_rate_limit_violation(
                    &user_context.user_id,
                    &client_ip,
                    &request_path,
                    &rate_limit_error.to_string()
                ).await.ok();
                
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            
            // Добавление контекста в request
            request.extensions_mut().insert(user_context.clone());
            
            // Логирование успешной аутентификации
            info!(
                request_id = %request_id,
                user_id = %user_context.user_id,
                path = %request_path,
                "Successful authentication"
            );
            
            AuthenticationResult::Authenticated(user_context)
        }
        Err(auth_error) => {
            // Неудачная аутентификация
            warn!(
                request_id = %request_id,
                client_ip = %client_ip,
                path = %request_path,
                error = %auth_error,
                "Authentication failed"
            );
            
            // Аудит неудачной аутентификации
            auth_state.audit_service.log_authentication_failure(
                &client_ip,
                &user_agent,
                &request_path,
                &auth_error.to_string()
            ).await.ok();
            
            // Для публичных эндпоинтов продолжаем без аутентификации
            if is_public_endpoint(&request_path) {
                AuthenticationResult::Anonymous
            } else {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    };
    
    // 3. Выполнение запроса
    let response = next.run(request).await;
    
    // 4. Постобработка и аудит
    let request_duration = request_start.elapsed();
    
    // Логирование завершения запроса
    match &auth_result {
        AuthenticationResult::Authenticated(user_context) => {
            auth_state.audit_service.log_authenticated_request(
                &user_context.user_id,
                &request_method,
                &request_path,
                response.status(),
                request_duration
            ).await.ok();
        }
        AuthenticationResult::Anonymous => {
            auth_state.audit_service.log_anonymous_request(
                &client_ip,
                &request_method,
                &request_path,
                response.status(),
                request_duration
            ).await.ok();
        }
    }
    
    // Метрики производительности
    auth_state.metrics.request_duration.observe(request_duration.as_secs_f64());
    auth_state.metrics.requests_total.inc();
    
    Ok(response)
}

/// Извлечение и валидация JWT токена
async fn extract_and_validate_jwt(
    request: &Request,
    auth_service: &AuthService,
) -> Result<Claims, AuthError> {
    // Извлечение токена из заголовка Authorization
    let auth_header = request.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(AuthError::MissingAuthorizationHeader)?;
    
    // Проверка формата Bearer токена
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidAuthorizationFormat)?;
    
    // Валидация токена
    auth_service.validate_token(token).await
}

/// Проверка публичных эндпоинтов
fn is_public_endpoint(path: &str) -> bool {
    const PUBLIC_PATHS: &[&str] = &[
        "/health",
        "/ready",
        "/metrics",
        "/graphql", // Некоторые GraphQL операции могут быть публичными
    ];
    
    PUBLIC_PATHS.iter().any(|&public_path| path.starts_with(public_path))
}

/// Извлечение User-Agent
fn extract_user_agent(request: &Request) -> Option<String> {
    request.headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(String::from)
}

/// Состояние middleware
pub struct AuthState {
    pub auth_service: Arc<AuthService>,
    pub rate_limiter: Arc<RateLimitingService>,
    pub audit_service: Arc<AuditService>,
    pub metrics: Arc<AuthMetrics>,
}

#[derive(Debug)]
enum AuthenticationResult {
    Authenticated(UserContext),
    Anonymous,
}
```

## 🛡️ Authorization Classes: Классы авторизации

### RoleGuard - Guard для проверки ролей

#### Реализация с расширенной функциональностью
```rust
// crates/shared/src/auth/guards/role_guard.rs
use async_graphql::{Context, Guard, Result};
use crate::auth::{UserContext, AuditService, PermissionEngine};
use std::collections::HashSet;

pub struct RoleGuard {
    required_roles: Vec<String>,
    operation_mode: RoleCheckMode,
    audit_enabled: bool,
    cache_result: bool,
}

#[derive(Debug, Clone)]
pub enum RoleCheckMode {
    Any,        // OR - любая из ролей
    All,        // AND - все роли
    Hierarchical, // Учет иерархии ролей
}

impl RoleGuard {
    pub fn new(roles: Vec<&str>) -> Self {
        Self {
            required_roles: roles.into_iter().map(String::from).collect(),
            operation_mode: RoleCheckMode::Any,
            audit_enabled: true,
            cache_result: true,
        }
    }
    
    pub fn require_all_roles(mut self) -> Self {
        self.operation_mode = RoleCheckMode::All;
        self
    }
    
    pub fn with_hierarchy(mut self) -> Self {
        self.operation_mode = RoleCheckMode::Hierarchical;
        self
    }
    
    pub fn without_audit(mut self) -> Self {
        self.audit_enabled = false;
        self
    }
}

#[async_trait::async_trait]
impl Guard for RoleGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let check_start = std::time::Instant::now();
        
        // 1. Получение пользовательского контекста
        let user_context = ctx.data_opt::<UserContext>()
            .ok_or_else(|| "User authentication required for role-based access")?;
        
        // 2. Проверка кеша результатов (если включено)
        if self.cache_result {
            let cache_key = self.build_cache_key(&user_context.user_id);
            if let Ok(permission_cache) = ctx.data::<PermissionCache>() {
                if let Some(cached_result) = permission_cache.get(&cache_key).await {
                    return if cached_result { Ok(()) } else { 
                        Err("Cached permission denied".into()) 
                    };
                }
            }
        }
        
        // 3. Выполнение проверки ролей
        let has_required_roles = match self.operation_mode {
            RoleCheckMode::Any => {
                self.required_roles.iter()
                    .any(|role| user_context.roles.contains(role))
            }
            RoleCheckMode::All => {
                self.required_roles.iter()
                    .all(|role| user_context.roles.contains(role))
            }
            RoleCheckMode::Hierarchical => {
                self.check_hierarchical_roles(&user_context, ctx).await?
            }
        };
        
        // 4. Кеширование результата
        if self.cache_result {
            if let Ok(permission_cache) = ctx.data::<PermissionCache>() {
                let cache_key = self.build_cache_key(&user_context.user_id);
                permission_cache.set(&cache_key, has_required_roles, Duration::from_secs(300)).await;
            }
        }
        
        // 5. Аудит логирование
        if self.audit_enabled {
            if let Ok(audit_service) = ctx.data::<AuditService>() {
                let check_duration = check_start.elapsed();
                
                if has_required_roles {
                    audit_service.log_role_check_success(
                        &user_context.user_id,
                        &self.required_roles,
                        &self.operation_mode,
                        check_duration
                    ).await.ok();
                } else {
                    audit_service.log_role_check_failure(
                        &user_context.user_id,
                        &self.required_roles,
                        &user_context.roles,
                        &self.operation_mode,
                        check_duration
                    ).await.ok();
                }
            }
        }
        
        // 6. Возврат результата
        if has_required_roles {
            Ok(())
        } else {
            Err(format!(
                "Access denied. Required roles: {:?} (mode: {:?}), User roles: {:?}",
                self.required_roles, self.operation_mode, user_context.roles
            ).into())
        }
    }
    
    /// Проверка иерархических ролей
    async fn check_hierarchical_roles(
        &self,
        user_context: &UserContext,
        ctx: &Context<'_>,
    ) -> Result<bool, async_graphql::Error> {
        let permission_engine = ctx.data::<PermissionEngine>()
            .map_err(|_| "Permission engine not available")?;
        
        // Получение всех ролей пользователя с учетом иерархии
        let all_user_roles = permission_engine
            .get_user_roles_with_hierarchy(&user_context.user_id)
            .await
            .map_err(|e| format!("Failed to get user roles: {}", e))?;
        
        // Проверка пересечения с требуемыми ролями
        let user_roles_set: HashSet<String> = all_user_roles.into_iter().collect();
        let required_roles_set: HashSet<String> = self.required_roles.iter().cloned().collect();
        
        Ok(!user_roles_set.is_disjoint(&required_roles_set))
    }
    
    fn build_cache_key(&self, user_id: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        user_id.hash(&mut hasher);
        self.required_roles.hash(&mut hasher);
        format!("role_guard:{:x}", hasher.finish())
    }
}
```

## 🎯 Ключевые принципы реализации на уровне кода

### 1. Type Safety и Error Handling
Строгая типизация обеспечивает безопасность на уровне компиляции:
```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("JWT token is expired")]
    TokenExpired,
    
    #[error("Invalid JWT signature")]
    InvalidSignature,
    
    #[error("Missing authorization header")]
    MissingAuthorizationHeader,
    
    #[error("User is not active")]
    UserInactive,
    
    #[error("Session is inactive")]
    SessionInactive,
    
    #[error("Suspicious activity detected")]
    SuspiciousActivity,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("GDPR compliance violation: {0}")]
    GDPRViolation(String),
}
```

### 2. Performance Optimization
Оптимизация производительности через кеширование:
```rust
impl AuthCache {
    pub async fn get_user_context(&self, token_hash: &str) -> Result<Option<CachedUserContext>, AuthCacheError> {
        let cache_key = format!("auth:token:{}", token_hash);
        
        let cached_data: Option<String> = self.redis_client
            .get(&cache_key)
            .await
            .map_err(AuthCacheError::RedisError)?;
        
        if let Some(data) = cached_data {
            let user_context: CachedUserContext = serde_json::from_str(&data)
                .map_err(AuthCacheError::DeserializationError)?;
            
            // Проверка актуальности кеша
            if user_context.is_valid() {
                return Ok(Some(user_context));
            }
        }
        
        Ok(None)
    }
}
```

### 3. Observability Integration
Встроенные метрики и трассировка:
```rust
use prometheus::{Counter, Histogram, IntGauge};

pub struct AuthMetrics {
    pub successful_validations: Counter,
    pub validation_failures: Counter,
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub validation_duration: Histogram,
    pub active_sessions: IntGauge,
    pub blacklisted_tokens: Counter,
}

impl AuthMetrics {
    pub fn record_validation(&self, duration: Duration, success: bool) {
        self.validation_duration.observe(duration.as_secs_f64());
        
        if success {
            self.successful_validations.inc();
        } else {
            self.validation_failures.inc();
        }
    }
}
```

### 4. Security Best Practices
Реализация лучших практик безопасности:
```rust
impl AuthService {
    /// Безопасное сравнение токенов (защита от timing attacks)
    fn secure_compare(&self, a: &str, b: &str) -> bool {
        use subtle::ConstantTimeEq;
        
        if a.len() != b.len() {
            return false;
        }
        
        a.as_bytes().ct_eq(b.as_bytes()).into()
    }
    
    /// Генерация криптографически стойкого хеша токена
    fn calculate_token_hash(&self, token: &str) -> String {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hasher.update(self.salt.as_bytes()); // Добавляем соль
        
        format!("{:x}", hasher.finalize())
    }
}
```

Эта Code диаграмма демонстрирует, как высокоуровневые принципы безопасности воплощаются в конкретный, production-ready Rust код с полным соблюдением лучших практик безопасности, производительности и наблюдаемости.