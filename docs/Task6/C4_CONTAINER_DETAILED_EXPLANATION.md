# Task 6: Container Diagram - Подробное объяснение архитектуры аутентификации и авторизации

## 🎯 Цель диаграммы

Container диаграмма Task 6 детализирует **внутреннюю архитектуру системы аутентификации и авторизации**, показывая как различные контейнеры взаимодействуют для обеспечения безопасности на уровне приложений. Диаграмма демонстрирует распределение ответственности между компонентами и их технологические стеки.

## 🏗️ Архитектурные слои и их реализация

### 1. Gateway Layer with Authentication - Слой шлюза с аутентификацией

#### Apollo Router with Auth - Федеративный роутер с аутентификацией
```rust
// apollo-router-auth/src/main.rs
use apollo_router::prelude::*;
use tower::ServiceBuilder;
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthenticatedRouter {
    auth_middleware: Arc<AuthMiddleware>,
    rate_limiter: Arc<RateLimiter>,
    audit_logger: Arc<AuditLogger>,
}

impl AuthenticatedRouter {
    pub fn new() -> Result<Self, RouterError> {
        Ok(Self {
            auth_middleware: Arc::new(AuthMiddleware::new()?),
            rate_limiter: Arc::new(RateLimiter::new()?),
            audit_logger: Arc::new(AuditLogger::new()?),
        })
    }

    /// Конфигурация роутера с аутентификацией
    pub fn configure_router(&self) -> Result<RouterService, RouterError> {
        let router = RouterService::builder()
            .with_yaml_config_path("router.yaml")
            .with_supergraph_path("supergraph.graphql")
            .build()?;

        // Добавление middleware для аутентификации
        let authenticated_router = ServiceBuilder::new()
            .layer(self.create_auth_layer())
            .layer(self.create_rate_limit_layer())
            .layer(self.create_audit_layer())
            .service(router);

        Ok(authenticated_router)
    }

    /// Создание слоя аутентификации
    fn create_auth_layer(&self) -> impl Layer<RouterService> {
        let auth_middleware = self.auth_middleware.clone();
        
        tower::layer::layer_fn(move |service| {
            let auth_middleware = auth_middleware.clone();
            
            tower::service_fn(move |request: RouterRequest| {
                let auth_middleware = auth_middleware.clone();
                let service = service.clone();
                
                async move {
                    // 1. Извлечение и валидация JWT токена
                    let authenticated_request = auth_middleware
                        .authenticate_request(request)
                        .await?;

                    // 2. Добавление пользовательского контекста в заголовки
                    let request_with_context = auth_middleware
                        .inject_user_context(authenticated_request)
                        .await?;

                    // 3. Передача запроса дальше по цепочке
                    service.call(request_with_context).await
                }
            })
        })
    }

    /// Создание слоя rate limiting
    fn create_rate_limit_layer(&self) -> impl Layer<RouterService> {
        let rate_limiter = self.rate_limiter.clone();
        
        tower::layer::layer_fn(move |service| {
            let rate_limiter = rate_limiter.clone();
            
            tower::service_fn(move |request: RouterRequest| {
                let rate_limiter = rate_limiter.clone();
                let service = service.clone();
                
                async move {
                    // Проверка rate limits перед обработкой запроса
                    rate_limiter.check_request_limits(&request).await?;
                    
                    service.call(request).await
                }
            })
        })
    }
}

/// Rhai скрипт для аутентификации в Apollo Router
const AUTH_RHAI_SCRIPT: &str = r#"
// Rhai скрипт для обработки аутентификации в Apollo Router
fn supergraph_service(service) {
    const request_callback = Fn("process_request");
    const response_callback = Fn("process_response");
    
    service.map_request(request_callback);
    service.map_response(response_callback);
}

fn process_request(request) {
    // Извлечение JWT токена из заголовков
    let auth_header = request.headers["authorization"];
    if auth_header == () {
        // Анонимный пользователь
        request.headers["x-user-context"] = `{"user_id": "anonymous", "roles": ["anonymous"]}`;
        return;
    }
    
    // Валидация JWT токена (вызов внешнего сервиса)
    let token = auth_header.replace("Bearer ", "");
    let validation_result = validate_jwt_token(token);
    
    if validation_result.valid {
        // Добавление пользовательского контекста
        request.headers["x-user-context"] = validation_result.user_context;
        
        // Логирование успешной аутентификации
        log_auth_event("authentication_success", validation_result.user_id);
    } else {
        // Обработка ошибки аутентификации
        log_auth_event("authentication_failure", "unknown");
        throw `Authentication failed: ${validation_result.error}`;
    }
}

fn process_response(response) {
    // Добавление заголовков безопасности
    response.headers["X-Content-Type-Options"] = "nosniff";
    response.headers["X-Frame-Options"] = "DENY";
    response.headers["X-XSS-Protection"] = "1; mode=block";
    response.headers["Strict-Transport-Security"] = "max-age=31536000; includeSubDomains";
}

// Внешние функции для интеграции с Rust
extern "rust" {
    fn validate_jwt_token(token: String) -> Map;
    fn log_auth_event(event_type: String, user_id: String);
}
"#;
```

#### Auth Middleware - Middleware аутентификации
```rust
// auth-middleware/src/lib.rs
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthMiddleware {
    jwt_validator: Arc<JwtValidator>,
    session_manager: Arc<SessionManager>,
    cache: Arc<AuthCache>,
}

impl AuthMiddleware {
    pub fn new() -> Result<Self, AuthError> {
        Ok(Self {
            jwt_validator: Arc::new(JwtValidator::new()?),
            session_manager: Arc::new(SessionManager::new()?),
            cache: Arc::new(AuthCache::new()?),
        })
    }

    /// Основной middleware для аутентификации
    pub async fn authenticate_request(
        &self,
        mut request: Request,
    ) -> Result<Request, AuthError> {
        let start_time = std::time::Instant::now();

        // 1. Извлечение JWT токена
        let token = self.extract_jwt_token(&request)?;

        let user_context = if let Some(token) = token {
            // 2. Проверка кеша
            if let Some(cached_context) = self.cache.get_user_context(&token).await? {
                cached_context
            } else {
                // 3. Валидация JWT токена
                let claims = self.jwt_validator.validate_token(&token).await?;
                
                // 4. Проверка активной сессии
                let session = self.session_manager
                    .validate_session(&claims.session_id)
                    .await?;

                // 5. Создание пользовательского контекста
                let context = UserContext::from_claims_and_session(claims, session);
                
                // 6. Кеширование контекста
                self.cache.cache_user_context(&token, &context).await?;
                
                context
            }
        } else {
            UserContext::anonymous()
        };

        // 7. Добавление контекста в заголовки запроса
        let context_json = serde_json::to_string(&user_context)?;
        request.headers_mut().insert(
            "x-user-context",
            HeaderValue::from_str(&context_json)?
        );

        // 8. Добавление метаданных для аудита
        request.headers_mut().insert(
            "x-auth-processing-time",
            HeaderValue::from_str(&format!("{}ms", start_time.elapsed().as_millis()))?
        );

        Ok(request)
    }

    /// Извлечение JWT токена из заголовков
    fn extract_jwt_token(&self, request: &Request) -> Result<Option<String>, AuthError> {
        let auth_header = request.headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok());

        if let Some(auth_header) = auth_header {
            if let Some(token) = auth_header.strip_prefix("Bearer ") {
                return Ok(Some(token.to_string()));
            }
        }

        // Проверка токена в cookies (для веб-приложений)
        if let Some(cookie_header) = request.headers().get("cookie") {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if let Some(token) = cookie.strip_prefix("auth_token=") {
                        return Ok(Some(token.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }
}

/// Axum middleware функция
pub async fn auth_middleware(
    State(auth): State<Arc<AuthMiddleware>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match auth.authenticate_request(request).await {
        Ok(authenticated_request) => {
            let response = next.run(authenticated_request).await;
            Ok(response)
        },
        Err(AuthError::InvalidToken) => Err(StatusCode::UNAUTHORIZED),
        Err(AuthError::ExpiredToken) => Err(StatusCode::UNAUTHORIZED),
        Err(AuthError::SessionExpired) => Err(StatusCode::UNAUTHORIZED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

### 2. Authentication Services - Сервисы аутентификации

#### Authentication Service - Центральный сервис аутентификации
```rust
// auth-service/src/service.rs
use tonic::{Request, Response, Status};
use sqlx::PgPool;
use redis::Client as RedisClient;
use oauth2::basic::BasicClient;

pub struct AuthenticationService {
    db_pool: PgPool,
    redis_client: RedisClient,
    jwt_config: JwtConfig,
    oauth2_clients: HashMap<String, BasicClient>,
    session_store: SessionStore,
    audit_logger: AuditLogger,
}

#[tonic::async_trait]
impl auth_service_proto::authentication_service_server::AuthenticationService for AuthenticationService {
    /// Аутентификация по логину и паролю
    async fn authenticate_credentials(
        &self,
        request: Request<AuthenticateCredentialsRequest>,
    ) -> Result<Response<AuthenticationResponse>, Status> {
        let req = request.into_inner();
        let start_time = std::time::Instant::now();

        // 1. Валидация входных данных
        self.validate_credentials_input(&req)?;

        // 2. Получение пользователя из базы данных
        let user = self.get_user_by_email(&req.email).await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            .ok_or_else(|| Status::unauthenticated("Invalid credentials"))?;

        // 3. Проверка блокировки аккаунта
        if user.is_locked() {
            self.audit_logger.log_blocked_login_attempt(&user.id, &req.ip_address).await;
            return Err(Status::permission_denied("Account is locked"));
        }

        // 4. Верификация пароля
        let password_valid = bcrypt::verify(&req.password, &user.password_hash)
            .map_err(|e| Status::internal(format!("Password verification error: {}", e)))?;

        if !password_valid {
            // Увеличение счетчика неудачных попыток
            self.increment_failed_attempts(&user.id).await?;
            self.audit_logger.log_failed_login(&user.id, &req.ip_address, "invalid_password").await;
            return Err(Status::unauthenticated("Invalid credentials"));
        }

        // 5. Сброс счетчика неудачных попыток
        self.reset_failed_attempts(&user.id).await?;

        // 6. Создание сессии
        let session = self.session_store.create_session(CreateSessionRequest {
            user_id: user.id.clone(),
            ip_address: req.ip_address.clone(),
            user_agent: req.user_agent.clone(),
        }).await?;

        // 7. Генерация JWT токенов
        let tokens = self.generate_jwt_tokens(&user, &session).await?;

        // 8. Аудит успешной аутентификации
        self.audit_logger.log_successful_login(
            &user.id,
            &req.ip_address,
            "credentials",
            start_time.elapsed(),
        ).await;

        Ok(Response::new(AuthenticationResponse {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            token_type: "Bearer".to_string(),
            user_info: Some(UserInfo {
                id: user.id,
                email: user.email,
                roles: user.roles,
                permissions: user.permissions,
            }),
        }))
    }

    /// Валидация JWT токена
    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<TokenValidationResponse>, Status> {
        let req = request.into_inner();
        let start_time = std::time::Instant::now();

        // 1. Декодирование и валидация JWT
        let token_data = decode::<Claims>(
            &req.token,
            &DecodingKey::from_rsa_pem(&self.jwt_config.public_key)
                .map_err(|e| Status::internal(format!("Key error: {}", e)))?,
            &Validation::new(Algorithm::RS256),
        ).map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))?;

        let claims = token_data.claims;

        // 2. Проверка активности сессии
        let session = self.session_store.get_session(&claims.session_id).await
            .map_err(|e| Status::internal(format!("Session error: {}", e)))?
            .ok_or_else(|| Status::unauthenticated("Session not found"))?;

        if !session.is_active {
            return Err(Status::unauthenticated("Session expired"));
        }

        // 3. Обновление времени последней активности
        self.session_store.update_last_activity(&session.id).await?;

        // 4. Получение актуальных разрешений пользователя
        let current_permissions = self.get_user_permissions(&claims.sub).await?;

        Ok(Response::new(TokenValidationResponse {
            valid: true,
            user_context: Some(UserContext {
                user_id: claims.sub,
                roles: claims.roles,
                permissions: current_permissions,
                session_id: claims.session_id,
                expires_at: claims.exp as i64,
            }),
            processing_time_ms: start_time.elapsed().as_millis() as u32,
        }))
    }

    /// Обновление токена
    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<AuthenticationResponse>, Status> {
        let req = request.into_inner();

        // 1. Валидация refresh токена
        let refresh_claims = decode::<RefreshClaims>(
            &req.refresh_token,
            &DecodingKey::from_rsa_pem(&self.jwt_config.public_key)?,
            &Validation::new(Algorithm::RS256),
        ).map_err(|e| Status::unauthenticated(format!("Invalid refresh token: {}", e)))?;

        // 2. Проверка сессии
        let session = self.session_store.get_session(&refresh_claims.claims.session_id).await?
            .ok_or_else(|| Status::unauthenticated("Session not found"))?;

        // 3. Получение актуальной информации о пользователе
        let user = self.get_user_by_id(&refresh_claims.claims.sub).await?
            .ok_or_else(|| Status::not_found("User not found"))?;

        // 4. Генерация нового access токена
        let new_tokens = self.generate_jwt_tokens(&user, &session).await?;

        Ok(Response::new(AuthenticationResponse {
            access_token: new_tokens.access_token,
            refresh_token: req.refresh_token, // Refresh токен остается тем же
            expires_in: new_tokens.expires_in,
            token_type: "Bearer".to_string(),
            user_info: Some(UserInfo {
                id: user.id,
                email: user.email,
                roles: user.roles,
                permissions: user.permissions,
            }),
        }))
    }

    /// OAuth2 аутентификация
    async fn authenticate_oauth2(
        &self,
        request: Request<OAuth2AuthenticationRequest>,
    ) -> Result<Response<AuthenticationResponse>, Status> {
        let req = request.into_inner();

        // 1. Получение OAuth2 клиента
        let oauth2_client = self.oauth2_clients.get(&req.provider)
            .ok_or_else(|| Status::invalid_argument("Unsupported OAuth2 provider"))?;

        // 2. Обмен authorization code на access token
        let token_result = oauth2_client
            .exchange_code(AuthorizationCode::new(req.authorization_code))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| Status::internal(format!("OAuth2 token exchange failed: {}", e)))?;

        // 3. Получение информации о пользователе от провайдера
        let oauth2_user_info = self.fetch_oauth2_user_info(
            &req.provider,
            token_result.access_token().secret(),
        ).await?;

        // 4. Поиск или создание пользователя в системе
        let user = self.find_or_create_oauth2_user(&oauth2_user_info, &req.provider).await?;

        // 5. Создание сессии
        let session = self.session_store.create_session(CreateSessionRequest {
            user_id: user.id.clone(),
            ip_address: req.ip_address.clone(),
            user_agent: req.user_agent.clone(),
        }).await?;

        // 6. Генерация JWT токенов
        let tokens = self.generate_jwt_tokens(&user, &session).await?;

        // 7. Аудит OAuth2 аутентификации
        self.audit_logger.log_successful_login(
            &user.id,
            &req.ip_address,
            &format!("oauth2_{}", req.provider),
            std::time::Duration::from_millis(0),
        ).await;

        Ok(Response::new(AuthenticationResponse {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            token_type: "Bearer".to_string(),
            user_info: Some(UserInfo {
                id: user.id,
                email: user.email,
                roles: user.roles,
                permissions: user.permissions,
            }),
        }))
    }
}
```

### 3. Secured Subgraphs - Защищенные подграфы

#### UGC Service (Secured) - Защищенный сервис пользовательского контента
```rust
// ugc-service-auth/src/resolvers.rs
use async_graphql::{Context, FieldResult, Object, Guard, ID};
use std::sync::Arc;

pub struct UgcMutation;

#[Object]
impl UgcMutation {
    /// Создание отзыва с защитой по ролям и GDPR
    #[graphql(
        guard = "RoleGuard::new(vec![\"user\".to_string(), \"premium_user\".to_string()])",
        guard = "RateLimitGuard::new(10, Duration::from_secs(60))", // 10 запросов в минуту
        guard = "GdprGuard::new(\"create_review\".to_string())"
    )]
    async fn create_review(
        &self,
        ctx: &Context<'_>,
        input: CreateReviewInput,
    ) -> FieldResult<Review> {
        let user_context = ctx.data::<UserContext>()?;
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let audit_logger = ctx.data::<Arc<AuditLogger>>()?;
        let gdpr_service = ctx.data::<Arc<GdprComplianceService>>()?;

        // 1. Дополнительная проверка владения ресурсом
        if let Some(offer_id) = &input.offer_id {
            let offer = ugc_service.get_offer(offer_id).await?;
            if offer.owner_id == user_context.user_id {
                return Err("Cannot review your own offer".into());
            }
        }

        // 2. Проверка GDPR согласий для обработки персональных данных
        let has_consent = gdpr_service
            .check_consent(&user_context.user_id, "create_review")
            .await?;

        if !has_consent {
            return Err("GDPR consent required for creating reviews".into());
        }

        // 3. Фильтрация и валидация контента
        let sanitized_input = ugc_service
            .sanitize_review_content(input)
            .await?;

        // 4. Создание отзыва
        let review = ugc_service
            .create_review(sanitized_input, &user_context)
            .await?;

        // 5. Аудит создания контента
        audit_logger.log_ugc_creation(
            &user_context.user_id,
            "review",
            &review.id,
            &user_context.ip_address.unwrap_or_default(),
        ).await?;

        // 6. Автоматическая модерация контента
        tokio::spawn({
            let review_id = review.id.clone();
            let moderation_service = ugc_service.moderation_service.clone();
            async move {
                if let Err(e) = moderation_service.moderate_review(&review_id).await {
                    tracing::error!("Auto-moderation failed for review {}: {}", review_id, e);
                }
            }
        });

        Ok(review)
    }

    /// Обновление отзыва с проверкой владения
    #[graphql(
        guard = "RoleGuard::new(vec![\"user\".to_string()])",
        guard = "OwnershipGuard::new(\"review\".to_string())"
    )]
    async fn update_review(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateReviewInput,
    ) -> FieldResult<Review> {
        let user_context = ctx.data::<UserContext>()?;
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let audit_logger = ctx.data::<Arc<AuditLogger>>()?;

        // 1. Проверка владения отзывом
        let existing_review = ugc_service.get_review(&id).await?;
        if existing_review.author_id != user_context.user_id && !user_context.has_role("moderator") {
            return Err("Access denied: not the owner of this review".into());
        }

        // 2. Фильтрация контента
        let sanitized_input = ugc_service
            .sanitize_review_content(input.into())
            .await?;

        // 3. Обновление отзыва
        let updated_review = ugc_service
            .update_review(&id, sanitized_input, &user_context)
            .await?;

        // 4. Аудит изменения контента
        audit_logger.log_ugc_modification(
            &user_context.user_id,
            "review",
            &id.to_string(),
            "update",
            &user_context.ip_address.unwrap_or_default(),
        ).await?;

        Ok(updated_review)
    }

    /// Удаление отзыва (мягкое удаление для GDPR)
    #[graphql(
        guard = "RoleGuard::new(vec![\"user\".to_string(), \"moderator\".to_string()])",
        guard = "OwnershipGuard::new(\"review\".to_string())"
    )]
    async fn delete_review(
        &self,
        ctx: &Context<'_>,
        id: ID,
        reason: Option<String>,
    ) -> FieldResult<bool> {
        let user_context = ctx.data::<UserContext>()?;
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let audit_logger = ctx.data::<Arc<AuditLogger>>()?;
        let gdpr_service = ctx.data::<Arc<GdprComplianceService>>()?;

        // 1. Проверка прав на удаление
        let review = ugc_service.get_review(&id).await?;
        let can_delete = review.author_id == user_context.user_id || 
                        user_context.has_role("moderator") ||
                        user_context.has_role("admin");

        if !can_delete {
            return Err("Access denied: insufficient permissions to delete this review".into());
        }

        // 2. Определение типа удаления
        let deletion_type = if user_context.has_role("moderator") {
            DeletionType::Moderation
        } else if reason.as_deref() == Some("gdpr_erasure") {
            DeletionType::GdprErasure
        } else {
            DeletionType::UserRequested
        };

        // 3. Выполнение удаления
        let success = match deletion_type {
            DeletionType::GdprErasure => {
                // Полное удаление для GDPR
                gdpr_service.erase_review_data(&id).await?;
                ugc_service.hard_delete_review(&id).await?
            },
            _ => {
                // Мягкое удаление
                ugc_service.soft_delete_review(&id, &user_context, reason.as_deref()).await?
            }
        };

        // 4. Аудит удаления
        audit_logger.log_ugc_deletion(
            &user_context.user_id,
            "review",
            &id.to_string(),
            &format!("{:?}", deletion_type),
            &user_context.ip_address.unwrap_or_default(),
        ).await?;

        Ok(success)
    }
}

pub struct UgcQuery;

#[Object]
impl UgcQuery {
    /// Получение отзывов с GDPR фильтрацией
    #[graphql(
        guard = "RateLimitGuard::new(100, Duration::from_secs(60))" // 100 запросов в минуту для чтения
    )]
    async fn reviews(
        &self,
        ctx: &Context<'_>,
        offer_id: Option<ID>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> FieldResult<Vec<Review>> {
        let user_context = ctx.data_opt::<UserContext>();
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let gdpr_service = ctx.data::<Arc<GdprComplianceService>>()?;

        // 1. Получение отзывов
        let reviews = ugc_service
            .get_reviews(GetReviewsFilter {
                offer_id: offer_id.map(|id| id.to_string()),
                limit: limit.unwrap_or(20).min(100), // Максимум 100 за раз
                offset: offset.unwrap_or(0),
                include_deleted: user_context
                    .map(|ctx| ctx.has_role("moderator"))
                    .unwrap_or(false),
            })
            .await?;

        // 2. GDPR фильтрация персональных данных
        let mut filtered_reviews = Vec::new();
        for review in reviews {
            let filtered_review = gdpr_service
                .filter_review_personal_data(ctx, &review)
                .await?;
            filtered_reviews.push(filtered_review);
        }

        Ok(filtered_reviews)
    }

    /// Получение отзыва по ID с проверкой доступа
    #[graphql(
        guard = "RateLimitGuard::new(200, Duration::from_secs(60))"
    )]
    async fn review(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> FieldResult<Option<Review>> {
        let user_context = ctx.data_opt::<UserContext>();
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let gdpr_service = ctx.data::<Arc<GdprComplianceService>>()?;
        let audit_logger = ctx.data::<Arc<AuditLogger>>()?;

        // 1. Получение отзыва
        let review = ugc_service.get_review(&id).await?;

        // 2. Проверка доступа к удаленному контенту
        if review.is_deleted && !user_context
            .map(|ctx| ctx.has_role("moderator"))
            .unwrap_or(false) {
            return Ok(None);
        }

        // 3. GDPR фильтрация
        let filtered_review = gdpr_service
            .filter_review_personal_data(ctx, &review)
            .await?;

        // 4. Аудит доступа к контенту
        if let Some(user_ctx) = user_context {
            audit_logger.log_content_access(
                &user_ctx.user_id,
                "review",
                &id.to_string(),
                &user_ctx.ip_address.unwrap_or_default(),
            ).await?;
        }

        Ok(Some(filtered_review))
    }
}

/// Guard для проверки владения ресурсом
pub struct OwnershipGuard {
    resource_type: String,
}

impl OwnershipGuard {
    pub fn new(resource_type: String) -> Self {
        Self { resource_type }
    }
}

#[async_trait::async_trait]
impl Guard for OwnershipGuard {
    async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        let user_context = ctx.data::<UserContext>()?;
        
        // Получаем ID ресурса из аргументов
        let resource_id = ctx.look_ahead()
            .field()
            .arguments()
            .get("id")
            .and_then(|v| v.string())
            .ok_or_else(|| async_graphql::Error::new("Resource ID not found"))?;

        // Проверяем владение в зависимости от типа ресурса
        let ugc_service = ctx.data::<Arc<UgcService>>()?;
        let is_owner = match self.resource_type.as_str() {
            "review" => {
                let review = ugc_service.get_review(resource_id).await?;
                review.author_id == user_context.user_id
            },
            "comment" => {
                let comment = ugc_service.get_comment(resource_id).await?;
                comment.author_id == user_context.user_id
            },
            _ => false,
        };

        // Модераторы и админы имеют доступ ко всем ресурсам
        if is_owner || user_context.has_role("moderator") || user_context.has_role("admin") {
            Ok(())
        } else {
            Err(async_graphql::Error::new("Access denied: not the owner of this resource"))
        }
    }
}
```

### 4. Security Layer - Слой безопасности

#### Rate Limiter Service - Сервис ограничения скорости
```rust
// rate-limiter-service/src/service.rs
use redis::{Client as RedisClient, Commands, Connection};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct RateLimiterService {
    redis_client: RedisClient,
    local_cache: Arc<RwLock<HashMap<String, RateLimitState>>>,
    config: RateLimitConfig,
}

impl RateLimiterService {
    pub fn new(redis_url: &str, config: RateLimitConfig) -> Result<Self, RateLimitError> {
        Ok(Self {
            redis_client: RedisClient::open(redis_url)?,
            local_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Проверка лимитов запросов
    pub async fn check_rate_limit(
        &self,
        key: &str,
        limit_type: RateLimitType,
    ) -> Result<RateLimitResult, RateLimitError> {
        let limit_config = self.get_limit_config(&limit_type);
        
        // 1. Проверка локального кеша для быстрого отклонения
        if let Some(cached_state) = self.get_cached_state(key).await {
            if cached_state.is_exceeded(&limit_config) {
                return Ok(RateLimitResult {
                    allowed: false,
                    remaining: 0,
                    reset_time: cached_state.reset_time,
                    retry_after: cached_state.retry_after(),
                });
            }
        }

        // 2. Проверка в Redis с использованием Lua скрипта
        let result = self.check_redis_rate_limit(key, &limit_config).await?;

        // 3. Обновление локального кеша
        self.update_local_cache(key, &result).await;

        // 4. Логирование превышения лимитов
        if !result.allowed {
            self.log_rate_limit_violation(key, &limit_type).await?;
        }

        Ok(result)
    }

    /// Проверка лимитов в Redis с помощью Lua скрипта
    async fn check_redis_rate_limit(
        &self,
        key: &str,
        config: &LimitConfig,
    ) -> Result<RateLimitResult, RateLimitError> {
        let mut conn = self.redis_client.get_connection()?;
        
        // Lua скрипт для атомарной проверки и обновления счетчиков
        let lua_script = r#"
            local key = KEYS[1]
            local window = tonumber(ARGV[1])
            local limit = tonumber(ARGV[2])
            local current_time = tonumber(ARGV[3])
            
            -- Получаем текущее состояние
            local current = redis.call('HMGET', key, 'count', 'window_start')
            local count = tonumber(current[1]) or 0
            local window_start = tonumber(current[2]) or current_time
            
            -- Проверяем, нужно ли сбросить окно
            if current_time - window_start >= window then
                count = 0
                window_start = current_time
            end
            
            -- Проверяем лимит
            if count >= limit then
                local reset_time = window_start + window
                return {0, count, reset_time - current_time}
            end
            
            -- Увеличиваем счетчик
            count = count + 1
            redis.call('HMSET', key, 'count', count, 'window_start', window_start)
            redis.call('EXPIRE', key, window)
            
            local remaining = limit - count
            local reset_time = window_start + window
            return {1, remaining, reset_time - current_time}
        "#;

        let result: Vec<i64> = redis::Script::new(lua_script)
            .key(format!("rate_limit:{}", key))
            .arg(config.window_seconds)
            .arg(config.max_requests)
            .arg(chrono::Utc::now().timestamp())
            .invoke(&mut conn)?;

        Ok(RateLimitResult {
            allowed: result[0] == 1,
            remaining: result[1] as u32,
            reset_time: chrono::Utc::now() + chrono::Duration::seconds(result[2]),
            retry_after: if result[0] == 0 { Some(result[2] as u32) } else { None },
        })
    }

    /// Адаптивное ограничение скорости на основе нагрузки
    pub async fn adaptive_rate_limit(
        &self,
        key: &str,
        base_limit: u32,
        current_load: f64,
    ) -> Result<RateLimitResult, RateLimitError> {
        // Адаптация лимита на основе текущей нагрузки
        let adaptive_limit = if current_load > 0.8 {
            // Высокая нагрузка - снижаем лимиты на 50%
            (base_limit as f64 * 0.5) as u32
        } else if current_load > 0.6 {
            // Средняя нагрузка - снижаем лимиты на 25%
            (base_limit as f64 * 0.75) as u32
        } else {
            // Низкая нагрузка - используем базовый лимит
            base_limit
        };

        let config = LimitConfig {
            max_requests: adaptive_limit,
            window_seconds: 60,
        };

        self.check_redis_rate_limit(key, &config).await
    }

    /// Получение статистики rate limiting
    pub async fn get_rate_limit_stats(&self, key: &str) -> Result<RateLimitStats, RateLimitError> {
        let mut conn = self.redis_client.get_connection()?;
        
        let stats: Vec<Option<String>> = conn.hmget(
            format!("rate_limit:{}", key),
            &["count", "window_start", "total_requests", "violations"]
        )?;

        Ok(RateLimitStats {
            current_count: stats[0].as_ref().and_then(|s| s.parse().ok()).unwrap_or(0),
            window_start: stats[1].as_ref().and_then(|s| s.parse().ok()).unwrap_or(0),
            total_requests: stats[2].as_ref().and_then(|s| s.parse().ok()).unwrap_or(0),
            violations: stats[3].as_ref().and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }

    /// Логирование нарушений rate limit
    async fn log_rate_limit_violation(
        &self,
        key: &str,
        limit_type: &RateLimitType,
    ) -> Result<(), RateLimitError> {
        let violation_event = RateLimitViolationEvent {
            key: key.to_string(),
            limit_type: limit_type.clone(),
            timestamp: chrono::Utc::now(),
            severity: self.calculate_violation_severity(key).await?,
        };

        // Отправка события в систему аудита
        // Это может быть Kafka, RabbitMQ, или прямой вызов audit service
        self.send_audit_event(violation_event).await?;

        Ok(())
    }

    /// Расчет серьезности нарушения
    async fn calculate_violation_severity(&self, key: &str) -> Result<ViolationSeverity, RateLimitError> {
        let stats = self.get_rate_limit_stats(key).await?;
        
        match stats.violations {
            0..=5 => Ok(ViolationSeverity::Low),
            6..=20 => Ok(ViolationSeverity::Medium),
            21..=50 => Ok(ViolationSeverity::High),
            _ => Ok(ViolationSeverity::Critical),
        }
    }
}

/// Типы ограничений скорости
#[derive(Debug, Clone)]
pub enum RateLimitType {
    UserRequests,
    IpRequests,
    ApiEndpoint(String),
    GraphqlOperation(String),
    AuthenticationAttempts,
    PasswordResetAttempts,
}

/// Конфигурация лимитов
#[derive(Debug, Clone)]
pub struct LimitConfig {
    pub max_requests: u32,
    pub window_seconds: i64,
}

/// Результат проверки rate limit
#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u32,
    pub reset_time: chrono::DateTime<chrono::Utc>,
    pub retry_after: Option<u32>,
}

/// Статистика rate limiting
#[derive(Debug)]
pub struct RateLimitStats {
    pub current_count: u32,
    pub window_start: i64,
    pub total_requests: u64,
    pub violations: u32,
}
```

## 🔍 Мониторинг и наблюдаемость

### Prometheus Integration - Интеграция с мониторингом
```rust
// monitoring/src/container_metrics.rs
use prometheus::{Counter, Histogram, Gauge, Registry};
use std::sync::Arc;

#[derive(Clone)]
pub struct ContainerMetrics {
    // Apollo Gateway метрики
    pub gateway_requests_total: Counter,
    pub gateway_auth_duration: Histogram,
    pub gateway_errors_total: Counter,
    
    // Authentication Service метрики
    pub auth_requests_total: Counter,
    pub auth_success_rate: Gauge,
    pub jwt_validation_duration: Histogram,
    pub oauth2_flow_duration: Histogram,
    
    // Rate Limiter метрики
    pub rate_limit_checks_total: Counter,
    pub rate_limit_violations_total: Counter,
    pub rate_limit_quotas: Gauge,
    
    // Security метрики
    pub security_events_total: Counter,
    pub gdpr_requests_total: Counter,
    pub audit_events_total: Counter,
}

impl ContainerMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let gateway_requests_total = Counter::with_opts(
            prometheus::Opts::new("gateway_requests_total", "Total gateway requests")
                .const_label("component", "apollo_gateway")
        )?;
        
        let auth_requests_total = Counter::with_opts(
            prometheus::Opts::new("auth_requests_total", "Total authentication requests")
                .const_label("component", "auth_service")
        )?;
        
        let rate_limit_checks_total = Counter::with_opts(
            prometheus::Opts::new("rate_limit_checks_total", "Total rate limit checks")
                .const_label("component", "rate_limiter")
        )?;

        // Регистрация метрик
        registry.register(Box::new(gateway_requests_total.clone()))?;
        registry.register(Box::new(auth_requests_total.clone()))?;
        registry.register(Box::new(rate_limit_checks_total.clone()))?;

        Ok(Self {
            gateway_requests_total,
            gateway_auth_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "gateway_auth_duration_seconds",
                    "Gateway authentication duration"
                )
            )?,
            gateway_errors_total: Counter::with_opts(
                prometheus::Opts::new("gateway_errors_total", "Gateway errors")
            )?,
            auth_requests_total,
            auth_success_rate: Gauge::with_opts(
                prometheus::Opts::new("auth_success_rate", "Authentication success rate")
            )?,
            jwt_validation_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "jwt_validation_duration_seconds",
                    "JWT validation duration"
                )
            )?,
            oauth2_flow_duration: Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "oauth2_flow_duration_seconds",
                    "OAuth2 flow duration"
                )
            )?,
            rate_limit_checks_total,
            rate_limit_violations_total: Counter::with_opts(
                prometheus::Opts::new("rate_limit_violations_total", "Rate limit violations")
            )?,
            rate_limit_quotas: Gauge::with_opts(
                prometheus::Opts::new("rate_limit_quotas", "Current rate limit quotas")
            )?,
            security_events_total: Counter::with_opts(
                prometheus::Opts::new("security_events_total", "Security events")
            )?,
            gdpr_requests_total: Counter::with_opts(
                prometheus::Opts::new("gdpr_requests_total", "GDPR requests")
            )?,
            audit_events_total: Counter::with_opts(
                prometheus::Opts::new("audit_events_total", "Audit events")
            )?,
        })
    }
}
```

Эта Container диаграмма демонстрирует детальную архитектуру системы аутентификации и авторизации на уровне контейнеров, показывая как различные сервисы взаимодействуют для обеспечения комплексной безопасности федеративной GraphQL платформы Auto.ru.