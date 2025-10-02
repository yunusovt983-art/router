# Task 6: Context Diagram - Подробное объяснение системы аутентификации и авторизации

## 🎯 Цель диаграммы

Context диаграмма Task 6 демонстрирует **высокоуровневую архитектуру системы аутентификации и авторизации** для федеративной GraphQL платформы Auto.ru. Диаграмма показывает как система безопасности интегрируется с внешними провайдерами, обеспечивает GDPR compliance и создает комплексную защиту на всех уровнях.

## 🔐 Эволюция от базовой к enterprise-grade безопасности

### От простой аутентификации к Zero Trust Architecture

#### Архитектурная трансформация
```typescript
// Было: Простая проверка токенов
app.use((req, res, next) => {
  const token = req.headers.authorization?.split(' ')[1];
  if (token && validateToken(token)) {
    next();
  } else {
    res.status(401).send('Unauthorized');
  }
});

// Стало: Комплексная система безопасности
app.use(authenticationMiddleware({
  jwtValidation: {
    issuer: 'https://auth.auto.ru',
    audience: 'auto.ru-api',
    algorithms: ['RS256'],
    jwksUri: 'https://auth.auto.ru/.well-known/jwks.json'
  },
  sessionManagement: {
    store: redisSessionStore,
    maxAge: 3600000, // 1 hour
    rolling: true,
    secure: true
  },
  rateLimit: {
    windowMs: 60000, // 1 minute
    max: 100, // requests per window
    keyGenerator: (req) => req.user?.id || req.ip
  },
  audit: {
    logAllRequests: true,
    sensitiveFields: ['email', 'phone', 'address'],
    gdprCompliance: true
  }
}));
```

**Объяснение**: Система эволюционировала от простой проверки токенов к комплексной enterprise-grade платформе безопасности с JWT валидацией, управлением сессиями, rate limiting и GDPR compliance.

## 🏗️ Архитектурные компоненты и их реализация

### 1. Apollo Gateway with Auth - Интеллектуальный шлюз безопасности

#### Rust реализация с комплексной аутентификацией
```rust
// apollo-gateway-auth/src/main.rs
use apollo_router::prelude::*;
use jsonwebtoken::{decode, DecodingKey, Validation};
use redis::Client as RedisClient;
use std::collections::HashMap;
use tower::ServiceBuilder;

#[derive(Clone)]
pub struct AuthenticatedGateway {
    jwt_validator: JwtValidator,
    session_manager: SessionManager,
    rate_limiter: RateLimiter,
    audit_logger: AuditLogger,
    gdpr_compliance: GdprCompliance,
}

impl AuthenticatedGateway {
    pub fn new() -> Self {
        Self {
            jwt_validator: JwtValidator::new(JwtConfig {
                issuer: "https://auth.auto.ru".to_string(),
                audience: "auto.ru-api".to_string(),
                jwks_uri: "https://auth.auto.ru/.well-known/jwks.json".to_string(),
                algorithms: vec!["RS256".to_string()],
                cache_ttl: Duration::from_secs(300),
            }),
            session_manager: SessionManager::new(RedisClient::open("redis://redis:6379")?),
            rate_limiter: RateLimiter::new(RateLimitConfig {
                default_limit: 100,
                window_size: Duration::from_secs(60),
                burst_limit: 20,
            }),
            audit_logger: AuditLogger::new("https://elasticsearch:9200"),
            gdpr_compliance: GdprCompliance::new(),
        }
    }

    /// Основной middleware для обработки аутентификации
    pub fn authentication_middleware(&self) -> impl Fn(Request) -> Result<Request, BoxError> {
        let validator = self.jwt_validator.clone();
        let session_mgr = self.session_manager.clone();
        let rate_limiter = self.rate_limiter.clone();
        let audit_logger = self.audit_logger.clone();

        move |mut request: Request| -> Result<Request, BoxError> {
            let start_time = Instant::now();
            
            // 1. Извлечение JWT токена
            let auth_header = request.headers()
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "));

            let user_context = if let Some(token) = auth_header {
                // 2. Валидация JWT токена
                let claims = validator.validate_token(token)
                    .map_err(|e| {
                        audit_logger.log_security_event(SecurityEvent {
                            event_type: SecurityEventType::InvalidToken,
                            ip_address: request.headers().get("x-forwarded-for")
                                .and_then(|h| h.to_str().ok())
                                .unwrap_or("unknown").to_string(),
                            user_agent: request.headers().get("user-agent")
                                .and_then(|h| h.to_str().ok())
                                .unwrap_or("unknown").to_string(),
                            details: format!("JWT validation failed: {}", e),
                            timestamp: Utc::now(),
                        });
                        BoxError::from(format!("Invalid token: {}", e))
                    })?;

                // 3. Проверка активной сессии
                let session = session_mgr.validate_session(&claims.session_id)
                    .map_err(|e| BoxError::from(format!("Session validation failed: {}", e)))?;

                // 4. Rate limiting проверка
                rate_limiter.check_limit(&claims.sub)
                    .map_err(|e| {
                        audit_logger.log_security_event(SecurityEvent {
                            event_type: SecurityEventType::RateLimitExceeded,
                            user_id: Some(claims.sub.clone()),
                            ip_address: request.headers().get("x-forwarded-for")
                                .and_then(|h| h.to_str().ok())
                                .unwrap_or("unknown").to_string(),
                            details: "Rate limit exceeded".to_string(),
                            timestamp: Utc::now(),
                        });
                        BoxError::from("Rate limit exceeded")
                    })?;

                // 5. Создание пользовательского контекста
                UserContext {
                    user_id: claims.sub,
                    roles: claims.roles,
                    permissions: claims.permissions,
                    session_id: claims.session_id,
                    ip_address: request.headers().get("x-forwarded-for")
                        .and_then(|h| h.to_str().ok())
                        .map(String::from),
                    authenticated_at: session.created_at,
                    expires_at: claims.exp,
                }
            } else {
                // Анонимный пользователь
                UserContext::anonymous()
            };

            // 6. Добавление контекста в заголовки для подграфов
            let context_json = serde_json::to_string(&user_context)
                .map_err(|e| BoxError::from(format!("Failed to serialize user context: {}", e)))?;
            
            request.headers_mut().insert(
                "x-user-context",
                HeaderValue::from_str(&context_json)?
            );

            // 7. Аудит успешной аутентификации
            audit_logger.log_security_event(SecurityEvent {
                event_type: SecurityEventType::AuthenticationSuccess,
                user_id: Some(user_context.user_id.clone()),
                ip_address: user_context.ip_address.clone().unwrap_or("unknown".to_string()),
                details: format!("Authentication successful, roles: {:?}", user_context.roles),
                timestamp: Utc::now(),
                processing_time: start_time.elapsed(),
            });

            Ok(request)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserContext {
    pub user_id: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub session_id: String,
    pub ip_address: Option<String>,
    pub authenticated_at: DateTime<Utc>,
    pub expires_at: i64,
}

impl UserContext {
    pub fn anonymous() -> Self {
        Self {
            user_id: "anonymous".to_string(),
            roles: vec!["anonymous".to_string()],
            permissions: vec!["read:public".to_string()],
            session_id: "anonymous".to_string(),
            ip_address: None,
            authenticated_at: Utc::now(),
            expires_at: 0,
        }
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(&role.to_string())
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string()) || 
        self.permissions.contains(&"*".to_string())
    }

    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }

    pub fn is_authenticated(&self) -> bool {
        self.user_id != "anonymous"
    }
}
```

### 2. Authentication Service - Центральный сервис аутентификации

#### Комплексная реализация с OAuth2 и JWT
```rust
// auth-service/src/main.rs
use oauth2::{AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use redis::Commands;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct AuthenticationService {
    db_pool: PgPool,
    redis_client: redis::Client,
    jwt_config: JwtConfig,
    oauth2_providers: HashMap<String, OAuth2Provider>,
    session_store: SessionStore,
    audit_logger: AuditLogger,
}

impl AuthenticationService {
    pub fn new(config: AuthConfig) -> Result<Self, AuthError> {
        let oauth2_providers = Self::initialize_oauth2_providers(&config)?;
        
        Ok(Self {
            db_pool: PgPool::connect(&config.database_url).await?,
            redis_client: redis::Client::open(config.redis_url)?,
            jwt_config: config.jwt,
            oauth2_providers,
            session_store: SessionStore::new(redis::Client::open(config.redis_url)?),
            audit_logger: AuditLogger::new(&config.elasticsearch_url),
        })
    }

    /// Аутентификация через логин/пароль
    pub async fn authenticate_credentials(
        &self,
        email: &str,
        password: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<AuthenticationResult, AuthError> {
        let start_time = Instant::now();
        
        // 1. Получение пользователя из БД
        let user_row = sqlx::query(
            "SELECT id, email, password_hash, roles, is_active, failed_login_attempts, locked_until 
             FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

        let user_id: String = user_row.get("id");
        let password_hash: String = user_row.get("password_hash");
        let roles: Vec<String> = user_row.get("roles");
        let is_active: bool = user_row.get("is_active");
        let failed_attempts: i32 = user_row.get("failed_login_attempts");
        let locked_until: Option<DateTime<Utc>> = user_row.get("locked_until");

        // 2. Проверка блокировки аккаунта
        if let Some(locked_until) = locked_until {
            if Utc::now() < locked_until {
                self.audit_logger.log_security_event(SecurityEvent {
                    event_type: SecurityEventType::LoginAttemptBlocked,
                    user_id: Some(user_id.clone()),
                    ip_address: ip_address.to_string(),
                    user_agent: user_agent.to_string(),
                    details: "Account is locked".to_string(),
                    timestamp: Utc::now(),
                }).await;
                
                return Err(AuthError::AccountLocked);
            }
        }

        // 3. Проверка активности аккаунта
        if !is_active {
            return Err(AuthError::AccountDisabled);
        }

        // 4. Верификация пароля
        let password_valid = bcrypt::verify(password, &password_hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        if !password_valid {
            // Увеличение счетчика неудачных попыток
            let new_failed_attempts = failed_attempts + 1;
            let lock_until = if new_failed_attempts >= 5 {
                Some(Utc::now() + Duration::minutes(30))
            } else {
                None
            };

            sqlx::query(
                "UPDATE users SET failed_login_attempts = $1, locked_until = $2 WHERE id = $3"
            )
            .bind(new_failed_attempts)
            .bind(lock_until)
            .bind(&user_id)
            .execute(&self.db_pool)
            .await?;

            self.audit_logger.log_security_event(SecurityEvent {
                event_type: SecurityEventType::LoginFailure,
                user_id: Some(user_id),
                ip_address: ip_address.to_string(),
                user_agent: user_agent.to_string(),
                details: format!("Invalid password, attempt {}", new_failed_attempts),
                timestamp: Utc::now(),
            }).await;

            return Err(AuthError::InvalidCredentials);
        }

        // 5. Сброс счетчика неудачных попыток при успешном входе
        sqlx::query("UPDATE users SET failed_login_attempts = 0, locked_until = NULL WHERE id = $1")
            .bind(&user_id)
            .execute(&self.db_pool)
            .await?;

        // 6. Получение разрешений пользователя
        let permissions = self.get_user_permissions(&user_id).await?;

        // 7. Создание сессии
        let session = self.session_store.create_session(SessionData {
            user_id: user_id.clone(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        }).await?;

        // 8. Генерация JWT токенов
        let claims = Claims {
            sub: user_id.clone(),
            iss: self.jwt_config.issuer.clone(),
            aud: self.jwt_config.audience.clone(),
            exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
            roles: roles.clone(),
            permissions: permissions.clone(),
            session_id: session.id.clone(),
        };

        let access_token = encode(
            &Header::new(Algorithm::RS256),
            &claims,
            &EncodingKey::from_rsa_pem(&self.jwt_config.private_key)?
        )?;

        let refresh_claims = RefreshClaims {
            sub: user_id.clone(),
            session_id: session.id.clone(),
            exp: (Utc::now() + Duration::days(30)).timestamp() as usize,
        };

        let refresh_token = encode(
            &Header::new(Algorithm::RS256),
            &refresh_claims,
            &EncodingKey::from_rsa_pem(&self.jwt_config.private_key)?
        )?;

        // 9. Аудит успешной аутентификации
        self.audit_logger.log_security_event(SecurityEvent {
            event_type: SecurityEventType::LoginSuccess,
            user_id: Some(user_id.clone()),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            details: format!("Successful login, roles: {:?}", roles),
            timestamp: Utc::now(),
            processing_time: start_time.elapsed(),
        }).await;

        Ok(AuthenticationResult {
            access_token,
            refresh_token,
            expires_in: 3600,
            token_type: "Bearer".to_string(),
            user: UserInfo {
                id: user_id,
                email: email.to_string(),
                roles,
                permissions,
            },
            session_id: session.id,
        })
    }
}
```

## 🛡️ Security Infrastructure - Комплексная защита

### 1. Security Audit Service - Система аудита безопасности

#### Elasticsearch интеграция для аудита
```rust
// security-audit/src/main.rs
use elasticsearch::{Elasticsearch, IndexParts};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct SecurityAuditService {
    elasticsearch: Elasticsearch,
    event_buffer: Arc<Mutex<Vec<SecurityEvent>>>,
    threat_detector: ThreatDetector,
    compliance_reporter: ComplianceReporter,
}

impl SecurityAuditService {
    pub fn new(elasticsearch_url: &str) -> Result<Self, AuditError> {
        let transport = Transport::single_node(elasticsearch_url)?;
        let elasticsearch = Elasticsearch::new(transport);
        
        Ok(Self {
            elasticsearch,
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            threat_detector: ThreatDetector::new(),
            compliance_reporter: ComplianceReporter::new(),
        })
    }

    /// Логирование события безопасности
    pub async fn log_security_event(&self, event: SecurityEvent) -> Result<(), AuditError> {
        // 1. Немедленная обработка критических событий
        if event.is_critical() {
            self.handle_critical_event(&event).await?;
        }

        // 2. Анализ угроз в реальном времени
        if let Some(threat) = self.threat_detector.analyze_event(&event).await? {
            self.handle_threat_detection(threat).await?;
        }

        // 3. Буферизация для batch индексирования
        {
            let mut buffer = self.event_buffer.lock().await;
            buffer.push(event.clone());
            
            // Flush buffer если достигнут лимит
            if buffer.len() >= 100 {
                let events_to_flush = buffer.drain(..).collect::<Vec<_>>();
                drop(buffer); // Освобождаем lock
                self.flush_events_to_elasticsearch(events_to_flush).await?;
            }
        }

        // 4. Структурированное логирование
        match event.event_type {
            SecurityEventType::LoginFailure | 
            SecurityEventType::UnauthorizedAccess |
            SecurityEventType::PermissionDenied => {
                warn!(
                    user_id = ?event.user_id,
                    ip_address = %event.ip_address,
                    event_type = ?event.event_type,
                    "Security violation detected"
                );
            },
            SecurityEventType::RateLimitExceeded => {
                error!(
                    user_id = ?event.user_id,
                    ip_address = %event.ip_address,
                    "Rate limit exceeded - potential attack"
                );
            },
            _ => {
                info!(
                    user_id = ?event.user_id,
                    event_type = ?event.event_type,
                    "Security event logged"
                );
            }
        }

        Ok(())
    }
}
```

## 🌐 External Integrations - Внешние интеграции

### 1. OAuth2 Providers - Социальная аутентификация

#### Мульти-провайдерная OAuth2 интеграция
```rust
// oauth2-integration/src/providers.rs
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, TokenUrl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct OAuth2ProviderManager {
    providers: HashMap<String, OAuth2Provider>,
}

impl OAuth2ProviderManager {
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        
        // Google OAuth2
        providers.insert("google".to_string(), OAuth2Provider {
            name: "google".to_string(),
            client: BasicClient::new(
                ClientId::new(env::var("GOOGLE_CLIENT_ID").unwrap()),
                Some(ClientSecret::new(env::var("GOOGLE_CLIENT_SECRET").unwrap())),
                AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
                Some(TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string()).unwrap())
            ),
            user_info_url: "https://www.googleapis.com/oauth2/v2/userinfo".to_string(),
            scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
        });

        // GitHub OAuth2
        providers.insert("github".to_string(), OAuth2Provider {
            name: "github".to_string(),
            client: BasicClient::new(
                ClientId::new(env::var("GITHUB_CLIENT_ID").unwrap()),
                Some(ClientSecret::new(env::var("GITHUB_CLIENT_SECRET").unwrap())),
                AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
                Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap())
            ),
            user_info_url: "https://api.github.com/user".to_string(),
            scopes: vec!["user:email".to_string()],
        });

        // VK OAuth2
        providers.insert("vk".to_string(), OAuth2Provider {
            name: "vk".to_string(),
            client: BasicClient::new(
                ClientId::new(env::var("VK_CLIENT_ID").unwrap()),
                Some(ClientSecret::new(env::var("VK_CLIENT_SECRET").unwrap())),
                AuthUrl::new("https://oauth.vk.com/authorize".to_string()).unwrap(),
                Some(TokenUrl::new("https://oauth.vk.com/access_token".to_string()).unwrap())
            ),
            user_info_url: "https://api.vk.com/method/users.get".to_string(),
            scopes: vec!["email".to_string()],
        });

        Self { providers }
    }

    pub async fn authenticate_with_provider(
        &self,
        provider_name: &str,
        authorization_code: &str,
        redirect_uri: &str,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        let provider = self.providers.get(provider_name)
            .ok_or(OAuth2Error::UnsupportedProvider)?;

        // 1. Обмен кода на токен
        let token_result = provider.client
            .exchange_code(AuthorizationCode::new(authorization_code.to_string()))
            .set_redirect_uri(Cow::Owned(RedirectUrl::new(redirect_uri.to_string())?))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;

        // 2. Получение информации о пользователе
        let user_info = self.fetch_user_info(provider, token_result.access_token().secret()).await?;

        Ok(user_info)
    }

    async fn fetch_user_info(
        &self,
        provider: &OAuth2Provider,
        access_token: &str,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        let client = reqwest::Client::new();
        let response = client
            .get(&provider.user_info_url)
            .bearer_auth(access_token)
            .send()
            .await?;

        let user_data: serde_json::Value = response.json().await?;

        // Нормализация данных пользователя в зависимости от провайдера
        let user_info = match provider.name.as_str() {
            "google" => OAuth2UserInfo {
                id: user_data["id"].as_str().unwrap().to_string(),
                email: user_data["email"].as_str().map(String::from),
                name: user_data["name"].as_str().map(String::from),
                avatar_url: user_data["picture"].as_str().map(String::from),
                provider: "google".to_string(),
            },
            "github" => OAuth2UserInfo {
                id: user_data["id"].as_u64().unwrap().to_string(),
                email: user_data["email"].as_str().map(String::from),
                name: user_data["name"].as_str().map(String::from),
                avatar_url: user_data["avatar_url"].as_str().map(String::from),
                provider: "github".to_string(),
            },
            "vk" => {
                let user = &user_data["response"][0];
                OAuth2UserInfo {
                    id: user["id"].as_u64().unwrap().to_string(),
                    email: None, // VK может не предоставлять email
                    name: format!("{} {}", 
                        user["first_name"].as_str().unwrap_or(""),
                        user["last_name"].as_str().unwrap_or("")
                    ).trim().to_string().into(),
                    avatar_url: user["photo_200"].as_str().map(String::from),
                    provider: "vk".to_string(),
                }
            },
            _ => return Err(OAuth2Error::UnsupportedProvider),
        };

        Ok(user_info)
    }
}
```

## 📊 Мониторинг и наблюдаемость

### 1. Prometheus Metrics - Метрики безопасности

#### Комплексный мониторинг аутентификации
```rust
// monitoring/src/auth_metrics.rs
use prometheus::{Counter, Histogram, Gauge, Registry, Opts};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthMetrics {
    // Счетчики аутентификации
    pub login_attempts_total: Counter,
    pub login_successes_total: Counter,
    pub login_failures_total: Counter,
    pub oauth2_authentications_total: Counter,
    
    // Счетчики авторизации
    pub authorization_checks_total: Counter,
    pub authorization_denials_total: Counter,
    pub permission_checks_total: Counter,
    
    // Счетчики безопасности
    pub rate_limit_violations_total: Counter,
    pub security_events_total: Counter,
    pub jwt_validation_failures_total: Counter,
    
    // Гистограммы производительности
    pub jwt_validation_duration: Histogram,
    pub oauth2_flow_duration: Histogram,
    pub database_query_duration: Histogram,
    
    // Gauge метрики
    pub active_sessions: Gauge,
    pub concurrent_users: Gauge,
    pub rate_limit_quotas_remaining: Gauge,
}

impl AuthMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let login_attempts_total = Counter::with_opts(
            Opts::new("auth_login_attempts_total", "Total number of login attempts")
                .const_label("service", "auth")
        )?;
        
        let login_successes_total = Counter::with_opts(
            Opts::new("auth_login_successes_total", "Total number of successful logins")
                .const_label("service", "auth")
        )?;
        
        let login_failures_total = Counter::with_opts(
            Opts::new("auth_login_failures_total", "Total number of failed logins")
                .const_label("service", "auth")
        )?;
        
        let jwt_validation_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "auth_jwt_validation_duration_seconds",
                "Time spent validating JWT tokens"
            ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0])
        )?;

        // Регистрация всех метрик
        registry.register(Box::new(login_attempts_total.clone()))?;
        registry.register(Box::new(login_successes_total.clone()))?;
        registry.register(Box::new(login_failures_total.clone()))?;
        registry.register(Box::new(jwt_validation_duration.clone()))?;

        Ok(Self {
            login_attempts_total,
            login_successes_total,
            login_failures_total,
            oauth2_authentications_total,
            authorization_checks_total,
            authorization_denials_total,
            permission_checks_total,
            rate_limit_violations_total,
            security_events_total,
            jwt_validation_failures_total,
            jwt_validation_duration,
            oauth2_flow_duration,
            database_query_duration,
            active_sessions,
            concurrent_users,
            rate_limit_quotas_remaining,
        })
    }

    /// Запись метрик успешной аутентификации
    pub fn record_successful_login(&self, provider: &str, duration: Duration) {
        self.login_attempts_total.inc();
        self.login_successes_total.inc();
        
        // Добавляем лейбл провайдера
        self.login_successes_total
            .with_label_values(&[provider])
            .inc();
    }

    /// Запись метрик неудачной аутентификации
    pub fn record_failed_login(&self, reason: &str) {
        self.login_attempts_total.inc();
        self.login_failures_total.inc();
        
        // Добавляем лейбл причины неудачи
        self.login_failures_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Запись метрик JWT валидации
    pub fn record_jwt_validation(&self, success: bool, duration: Duration) {
        let timer = self.jwt_validation_duration.start_timer();
        
        if !success {
            self.jwt_validation_failures_total.inc();
        }
        
        timer.observe_duration();
    }

    /// Обновление активных сессий
    pub fn update_active_sessions(&self, count: f64) {
        self.active_sessions.set(count);
    }

    /// Запись нарушения rate limit
    pub fn record_rate_limit_violation(&self, user_id: &str, endpoint: &str) {
        self.rate_limit_violations_total
            .with_label_values(&[user_id, endpoint])
            .inc();
    }
}
```

## 🔒 GDPR Compliance - Соответствие требованиям

### 1. GDPR Service - Управление персональными данными

#### Комплексная система защиты персональных данных
```rust
// gdpr-compliance/src/main.rs
use async_graphql::{Context, FieldResult, Guard};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone)]
pub struct GdprComplianceService {
    sensitive_fields: HashSet<String>,
    consent_manager: Arc<ConsentManager>,
    data_classifier: Arc<DataClassifier>,
    audit_logger: Arc<AuditLogger>,
    encryption_service: Arc<EncryptionService>,
}

impl GdprComplianceService {
    pub fn new() -> Self {
        let mut sensitive_fields = HashSet::new();
        sensitive_fields.insert("email".to_string());
        sensitive_fields.insert("phone".to_string());
        sensitive_fields.insert("address".to_string());
        sensitive_fields.insert("passport".to_string());
        sensitive_fields.insert("driver_license".to_string());
        sensitive_fields.insert("credit_card".to_string());
        
        Self {
            sensitive_fields,
            consent_manager: Arc::new(ConsentManager::new()),
            data_classifier: Arc::new(DataClassifier::new()),
            audit_logger: Arc::new(AuditLogger::new()),
            encryption_service: Arc::new(EncryptionService::new()),
        }
    }

    /// Проверка доступа к персональным данным
    pub async fn check_data_access_permission(
        &self,
        user_context: &UserContext,
        field_name: &str,
        resource_owner_id: &str,
    ) -> Result<bool, GdprError> {
        // 1. Проверка является ли поле чувствительным
        if !self.sensitive_fields.contains(field_name) {
            return Ok(true); // Не чувствительные данные доступны всем
        }

        // 2. Владелец данных всегда имеет доступ
        if user_context.user_id == resource_owner_id {
            return Ok(true);
        }

        // 3. Администраторы имеют доступ с аудитом
        if user_context.has_role("admin") {
            self.audit_logger.log_admin_data_access(
                &user_context.user_id,
                resource_owner_id,
                field_name,
            ).await?;
            return Ok(true);
        }

        // 4. Проверка согласия на обработку данных
        let has_consent = self.consent_manager
            .check_consent(resource_owner_id, &format!("access:{}", field_name))
            .await?;

        if !has_consent {
            self.audit_logger.log_gdpr_violation(
                &user_context.user_id,
                resource_owner_id,
                field_name,
                "No consent for data access",
            ).await?;
            return Ok(false);
        }

        // 5. Проверка бизнес-правил доступа
        let has_business_permission = self.check_business_access_rules(
            user_context,
            field_name,
            resource_owner_id,
        ).await?;

        if has_business_permission {
            self.audit_logger.log_data_access(
                &user_context.user_id,
                resource_owner_id,
                field_name,
                "Business rule access granted",
            ).await?;
        }

        Ok(has_business_permission)
    }

    /// Фильтрация чувствительных данных в GraphQL ответах
    pub async fn filter_sensitive_data(
        &self,
        ctx: &Context<'_>,
        field_name: &str,
        data: serde_json::Value,
        resource_owner_id: &str,
    ) -> FieldResult<serde_json::Value> {
        let user_context = ctx.data::<UserContext>()?;

        // Проверка разрешения на доступ к данным
        let has_permission = self.check_data_access_permission(
            user_context,
            field_name,
            resource_owner_id,
        ).await?;

        if !has_permission {
            // Возвращаем замаскированные данные
            return Ok(self.mask_sensitive_data(field_name, &data));
        }

        // Логируем доступ к чувствительным данным
        if self.sensitive_fields.contains(field_name) {
            self.audit_logger.log_data_access(
                &user_context.user_id,
                resource_owner_id,
                field_name,
                "Data access granted",
            ).await?;
        }

        Ok(data)
    }

    /// Маскирование чувствительных данных
    fn mask_sensitive_data(&self, field_name: &str, data: &serde_json::Value) -> serde_json::Value {
        match field_name {
            "email" => {
                if let Some(email) = data.as_str() {
                    let parts: Vec<&str> = email.split('@').collect();
                    if parts.len() == 2 {
                        let masked_local = format!("{}***", &parts[0][..1.min(parts[0].len())]);
                        return serde_json::Value::String(format!("{}@{}", masked_local, parts[1]));
                    }
                }
                serde_json::Value::String("***@***.***".to_string())
            },
            "phone" => {
                if let Some(phone) = data.as_str() {
                    let masked = format!("+7***-***-{}", &phone[phone.len()-2..]);
                    return serde_json::Value::String(masked);
                }
                serde_json::Value::String("+7***-***-**".to_string())
            },
            "address" => serde_json::Value::String("[PROTECTED ADDRESS]".to_string()),
            "passport" | "driver_license" => serde_json::Value::String("[PROTECTED DOCUMENT]".to_string()),
            "credit_card" => serde_json::Value::String("****-****-****-****".to_string()),
            _ => serde_json::Value::String("[PROTECTED DATA]".to_string()),
        }
    }

    /// Обработка запроса на удаление данных (Right to be Forgotten)
    pub async fn handle_erasure_request(
        &self,
        user_id: &str,
        requester_id: &str,
    ) -> Result<ErasureResult, GdprError> {
        // 1. Проверка прав на запрос удаления
        if user_id != requester_id {
            return Err(GdprError::UnauthorizedErasureRequest);
        }

        // 2. Проверка возможности удаления (legal holds, etc.)
        let can_erase = self.check_erasure_eligibility(user_id).await?;
        if !can_erase.eligible {
            return Ok(ErasureResult {
                success: false,
                reason: can_erase.reason,
                retained_data: can_erase.retained_categories,
            });
        }

        // 3. Начало процесса удаления
        let erasure_job = self.initiate_data_erasure(user_id).await?;

        // 4. Аудит запроса на удаление
        self.audit_logger.log_erasure_request(
            user_id,
            requester_id,
            &erasure_job.job_id,
        ).await?;

        Ok(ErasureResult {
            success: true,
            reason: "Erasure initiated".to_string(),
            retained_data: vec![],
        })
    }

    /// Экспорт персональных данных (Data Portability)
    pub async fn export_user_data(
        &self,
        user_id: &str,
        requester_id: &str,
        format: ExportFormat,
    ) -> Result<DataExport, GdprError> {
        // 1. Проверка прав на экспорт
        if user_id != requester_id {
            return Err(GdprError::UnauthorizedDataExport);
        }

        // 2. Сбор всех персональных данных пользователя
        let user_data = self.collect_user_data(user_id).await?;

        // 3. Шифрование экспортируемых данных
        let encrypted_data = self.encryption_service
            .encrypt_export_data(&user_data)
            .await?;

        // 4. Создание экспорта в запрошенном формате
        let export = match format {
            ExportFormat::Json => self.create_json_export(encrypted_data).await?,
            ExportFormat::Csv => self.create_csv_export(encrypted_data).await?,
            ExportFormat::Xml => self.create_xml_export(encrypted_data).await?,
        };

        // 5. Аудит экспорта данных
        self.audit_logger.log_data_export(
            user_id,
            requester_id,
            &format,
            export.size_bytes,
        ).await?;

        Ok(export)
    }
}

/// Guard для проверки GDPR соответствия в GraphQL
pub struct GdprGuard {
    field_name: String,
}

impl GdprGuard {
    pub fn new(field_name: String) -> Self {
        Self { field_name }
    }
}

#[async_trait::async_trait]
impl Guard for GdprGuard {
    async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        let user_context = ctx.data::<UserContext>()?;
        let gdpr_service = ctx.data::<GdprComplianceService>()?;
        
        // Получаем ID владельца ресурса из контекста или аргументов
        let resource_owner_id = ctx.look_ahead()
            .field()
            .arguments()
            .get("userId")
            .and_then(|v| v.string())
            .unwrap_or(&user_context.user_id);

        let has_permission = gdpr_service
            .check_data_access_permission(user_context, &self.field_name, resource_owner_id)
            .await
            .map_err(|e| async_graphql::Error::new(format!("GDPR check failed: {}", e)))?;

        if has_permission {
            Ok(())
        } else {
            Err(async_graphql::Error::new("Access to personal data denied due to GDPR compliance"))
        }
    }
}
```

## 🚀 Практическое применение

### Интеграция в GraphQL схему
```rust
// Пример использования в GraphQL резолверах
impl Query {
    #[graphql(guard = "RoleGuard::new(vec![\"user\".to_string()])")]
    #[graphql(guard = "GdprGuard::new(\"email\".to_string())")]
    async fn user_profile(
        &self,
        ctx: &Context<'_>,
        user_id: ID,
    ) -> FieldResult<UserProfile> {
        let user_context = ctx.data::<UserContext>()?;
        let gdpr_service = ctx.data::<GdprComplianceService>()?;
        
        // Получение данных пользователя
        let mut profile = self.user_service.get_profile(&user_id).await?;
        
        // GDPR фильтрация чувствительных полей
        profile.email = gdpr_service
            .filter_sensitive_data(ctx, "email", profile.email.into(), &user_id)
            .await?
            .as_str()
            .map(String::from);
            
        profile.phone = gdpr_service
            .filter_sensitive_data(ctx, "phone", profile.phone.into(), &user_id)
            .await?
            .as_str()
            .map(String::from);

        Ok(profile)
    }
}
```

Эта Context диаграмма демонстрирует комплексную архитектуру безопасности, которая обеспечивает enterprise-grade защиту для федеративной GraphQL платформы Auto.ru с полным соответствием GDPR требованиям и современными стандартами безопасности.