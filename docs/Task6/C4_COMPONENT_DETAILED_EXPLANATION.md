# Task 6: Component Diagram - Подробное объяснение внутренней архитектуры компонентов безопасности

## 🎯 Цель диаграммы

Component диаграмма Task 6 раскрывает **внутреннюю архитектуру сервисов аутентификации и авторизации**, показывая как принципы безопасности реализуются на уровне отдельных компонентов и их взаимодействий. Диаграмма демонстрирует конкретную реализацию security patterns внутри каждого сервиса.

## 🔐 Authentication Service - Внутренние компоненты

### JWT Management Components - Управление JWT токенами

#### JWT Validator - Валидация токенов с оптимизацией
```rust
// crates/auth-service/src/jwt/validator.rs
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct JWTValidator {
    // Ключи для валидации
    decoding_keys: Arc<RwLock<HashMap<String, DecodingKey>>>,
    validation_config: Validation,
    
    // Кеширование результатов валидации
    validation_cache: Arc<ValidationCache>,
    
    // JWKS клиент для получения ключей
    jwks_client: Arc<JWKSClient>,
    
    // Метрики
    metrics: Arc<JWTMetrics>,
}

impl JWTValidator {
    pub fn new(config: &JWTConfig) -> Result<Self, JWTError> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&config.audience);
        validation.set_issuer(&config.issuer);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        
        Ok(Self {
            decoding_keys: Arc::new(RwLock::new(HashMap::new())),
            validation_config: validation,
            validation_cache: Arc::new(ValidationCache::new(config.cache_ttl)),
            jwks_client: Arc::new(JWKSClient::new(&config.jwks_url)?),
            metrics: Arc::new(JWTMetrics::new()),
        })
    }
    
    /// Валидация JWT токена с кешированием и метриками
    pub async fn validate_token(&self, token: &str) -> Result<Claims, JWTError> {
        let start_time = std::time::Instant::now();
        
        // 1. Проверка в кеше валидации
        let token_hash = self.calculate_token_hash(token);
        if let Some(cached_claims) = self.validation_cache.get(&token_hash).await {
            self.metrics.cache_hits.inc();
            return Ok(cached_claims);
        }
        
        // 2. Извлечение заголовка для определения kid (key ID)
        let header = jsonwebtoken::decode_header(token)?;
        let key_id = header.kid.ok_or(JWTError::MissingKeyId)?;
        
        // 3. Получение ключа для валидации
        let decoding_key = self.get_decoding_key(&key_id).await?;
        
        // 4. Валидация токена
        let token_data = decode::<Claims>(token, &decoding_key, &self.validation_config)
            .map_err(|e| {
                self.metrics.validation_failures.inc();
                JWTError::ValidationFailed(e.to_string())
            })?;
        
        // 5. Дополнительные проверки безопасности
        self.perform_security_checks(&token_data.claims).await?;
        
        // 6. Кеширование результата
        self.validation_cache.set(&token_hash, &token_data.claims).await;
        
        // 7. Метрики
        let validation_duration = start_time.elapsed();
        self.metrics.validation_duration.observe(validation_duration.as_secs_f64());
        self.metrics.successful_validations.inc();
        
        Ok(token_data.claims)
    }
    
    /// Получение ключа для валидации с автоматическим обновлением
    async fn get_decoding_key(&self, key_id: &str) -> Result<DecodingKey, JWTError> {
        // Проверка в локальном кеше ключей
        {
            let keys = self.decoding_keys.read().await;
            if let Some(key) = keys.get(key_id) {
                return Ok(key.clone());
            }
        }
        
        // Получение ключа через JWKS
        let jwk = self.jwks_client.get_key(key_id).await?;
        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?;
        
        // Сохранение в кеше
        {
            let mut keys = self.decoding_keys.write().await;
            keys.insert(key_id.to_string(), decoding_key.clone());
        }
        
        Ok(decoding_key)
    }
    
    /// Дополнительные проверки безопасности
    async fn perform_security_checks(&self, claims: &Claims) -> Result<(), JWTError> {
        // Проверка на blacklist
        if self.is_token_blacklisted(&claims.jti).await? {
            return Err(JWTError::TokenBlacklisted);
        }
        
        // Проверка активности сессии
        if !self.is_session_active(&claims.session_id).await? {
            return Err(JWTError::SessionInactive);
        }
        
        // Проверка на подозрительную активность
        if self.detect_suspicious_activity(claims).await? {
            return Err(JWTError::SuspiciousActivity);
        }
        
        Ok(())
    }
}
```

#### JWKS Client - Управление ключами
```rust
// crates/auth-service/src/jwt/jwks_client.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{interval, Duration};

#[derive(Debug, Deserialize)]
pub struct JWKSet {
    pub keys: Vec<JWK>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JWK {
    pub kty: String,
    pub kid: String,
    pub n: String,
    pub e: String,
    pub alg: Option<String>,
    #[serde(rename = "use")]
    pub key_use: Option<String>,
}

pub struct JWKSClient {
    http_client: Client,
    jwks_url: String,
    key_cache: Arc<RwLock<HashMap<String, JWK>>>,
    cache_ttl: Duration,
    last_update: Arc<RwLock<std::time::Instant>>,
}

impl JWKSClient {
    pub fn new(jwks_url: &str) -> Result<Self, JWKSError> {
        let client = Self {
            http_client: Client::new(),
            jwks_url: jwks_url.to_string(),
            key_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(3600), // 1 час
            last_update: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        
        // Запуск фонового обновления ключей
        client.start_background_refresh();
        
        Ok(client)
    }
    
    /// Получение ключа по ID с автоматическим обновлением
    pub async fn get_key(&self, key_id: &str) -> Result<JWK, JWKSError> {
        // Проверка в кеше
        {
            let cache = self.key_cache.read().await;
            if let Some(key) = cache.get(key_id) {
                return Ok(key.clone());
            }
        }
        
        // Обновление кеша если необходимо
        self.refresh_keys_if_needed().await?;
        
        // Повторная проверка в кеше
        {
            let cache = self.key_cache.read().await;
            if let Some(key) = cache.get(key_id) {
                return Ok(key.clone());
            }
        }
        
        Err(JWKSError::KeyNotFound(key_id.to_string()))
    }
    
    /// Обновление ключей из JWKS endpoint
    async fn refresh_keys(&self) -> Result<(), JWKSError> {
        let response = self.http_client
            .get(&self.jwks_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(JWKSError::HttpError(response.status()));
        }
        
        let jwk_set: JWKSet = response.json().await?;
        
        // Обновление кеша
        {
            let mut cache = self.key_cache.write().await;
            cache.clear();
            for key in jwk_set.keys {
                cache.insert(key.kid.clone(), key);
            }
        }
        
        // Обновление времени последнего обновления
        {
            let mut last_update = self.last_update.write().await;
            *last_update = std::time::Instant::now();
        }
        
        Ok(())
    }
    
    /// Фоновое обновление ключей
    fn start_background_refresh(&self) {
        let client = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1800)); // Каждые 30 минут
            
            loop {
                interval.tick().await;
                if let Err(e) = client.refresh_keys().await {
                    eprintln!("Failed to refresh JWKS keys: {}", e);
                }
            }
        });
    }
}
```

### OAuth2 Integration Components - Интеграция OAuth2

#### OAuth2 Client - Мультипровайдерная аутентификация
```rust
// crates/auth-service/src/oauth2/client.rs
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, RedirectUrl, Scope, TokenResponse, TokenUrl
};
use reqwest::Client as HttpClient;

pub struct OAuth2Client {
    providers: HashMap<String, ProviderClient>,
    http_client: HttpClient,
    state_store: Arc<StateStore>,
}

struct ProviderClient {
    oauth_client: BasicClient,
    user_info_endpoint: String,
    scopes: Vec<String>,
    config: ProviderConfig,
}

impl OAuth2Client {
    /// Инициация OAuth2 flow
    pub async fn initiate_auth_flow(
        &self,
        provider: &str,
        redirect_uri: &str,
    ) -> Result<AuthFlowInitiation, OAuth2Error> {
        let provider_client = self.providers.get(provider)
            .ok_or(OAuth2Error::UnsupportedProvider)?;
        
        // Генерация state для защиты от CSRF
        let csrf_state = CsrfToken::new_random();
        
        // Сохранение state в хранилище
        self.state_store.store_state(&csrf_state, provider, redirect_uri).await?;
        
        // Построение URL авторизации
        let (auth_url, _) = provider_client.oauth_client
            .authorize_url(|| csrf_state.clone())
            .add_scopes(provider_client.scopes.iter().map(|s| Scope::new(s.clone())))
            .url();
        
        Ok(AuthFlowInitiation {
            authorization_url: auth_url.to_string(),
            state: csrf_state.secret().clone(),
            provider: provider.to_string(),
        })
    }
    
    /// Обработка OAuth2 callback
    pub async fn handle_callback(
        &self,
        provider: &str,
        authorization_code: &str,
        state: &str,
    ) -> Result<OAuth2TokenResponse, OAuth2Error> {
        // Валидация state
        let stored_state = self.state_store.get_and_remove_state(state).await?;
        if stored_state.provider != provider {
            return Err(OAuth2Error::InvalidState);
        }
        
        let provider_client = self.providers.get(provider)
            .ok_or(OAuth2Error::UnsupportedProvider)?;
        
        // Обмен authorization code на access token
        let token_response = provider_client.oauth_client
            .exchange_code(AuthorizationCode::new(authorization_code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await?;
        
        // Получение информации о пользователе
        let user_info = self.fetch_user_info(
            provider,
            token_response.access_token().secret()
        ).await?;
        
        Ok(OAuth2TokenResponse {
            access_token: token_response.access_token().secret().clone(),
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
            expires_in: token_response.expires_in().map(|d| d.as_secs()),
            user_info,
        })
    }
    
    /// Получение информации о пользователе от провайдера
    async fn fetch_user_info(
        &self,
        provider: &str,
        access_token: &str,
    ) -> Result<UserInfo, OAuth2Error> {
        let provider_client = self.providers.get(provider)
            .ok_or(OAuth2Error::UnsupportedProvider)?;
        
        let response = self.http_client
            .get(&provider_client.user_info_endpoint)
            .bearer_auth(access_token)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(OAuth2Error::UserInfoFetchFailed(response.status()));
        }
        
        let user_data: serde_json::Value = response.json().await?;
        
        // Нормализация данных в зависимости от провайдера
        let user_info = match provider {
            "google" => self.parse_google_user_info(&user_data)?,
            "github" => self.parse_github_user_info(&user_data).await?,
            "vk" => self.parse_vk_user_info(&user_data)?,
            _ => return Err(OAuth2Error::UnsupportedProvider),
        };
        
        Ok(user_info)
    }
}
```

## 🛡️ Authorization Service - Компоненты авторизации

### RBAC Components - Ролевая модель доступа

#### Permission Engine - Движок разрешений
```rust
// crates/auth-service/src/rbac/permission_engine.rs
use std::collections::{HashMap, HashSet};
use async_trait::async_trait;

pub struct PermissionEngine {
    policy_store: Arc<PolicyStore>,
    permission_cache: Arc<PermissionCache>,
    abac_evaluator: Arc<ABACEvaluator>,
    audit_logger: Arc<AuditLogger>,
}

#[async_trait]
pub trait PolicyEvaluator {
    async fn evaluate(
        &self,
        subject: &Subject,
        resource: &Resource,
        action: &Action,
        context: &EvaluationContext,
    ) -> Result<PolicyDecision, PolicyError>;
}

impl PermissionEngine {
    /// Проверка разрешений с учетом RBAC и ABAC
    pub async fn check_permissions(
        &self,
        user_context: &UserContext,
        required_permissions: &[String],
        resource: Option<&String>,
    ) -> Result<bool, PermissionError> {
        let evaluation_start = std::time::Instant::now();
        
        // 1. Построение контекста оценки
        let eval_context = EvaluationContext {
            user_id: user_context.user_id.clone(),
            roles: user_context.roles.clone(),
            permissions: user_context.permissions.clone(),
            session_id: user_context.session_id.clone(),
            ip_address: user_context.ip_address.clone(),
            timestamp: chrono::Utc::now(),
            resource: resource.cloned(),
        };
        
        // 2. Проверка кеша разрешений
        let cache_key = self.build_permission_cache_key(&eval_context, required_permissions);
        if let Some(cached_result) = self.permission_cache.get(&cache_key).await {
            return Ok(cached_result);
        }
        
        // 3. RBAC проверка - базовые разрешения
        let rbac_result = self.evaluate_rbac_permissions(&eval_context, required_permissions).await?;
        
        if !rbac_result {
            // Логирование отказа в доступе
            self.audit_logger.log_permission_denied(
                &user_context.user_id,
                required_permissions,
                "RBAC_DENIED"
            ).await?;
            
            return Ok(false);
        }
        
        // 4. ABAC проверка - контекстные условия
        let abac_result = if let Some(resource_name) = resource {
            self.evaluate_abac_conditions(&eval_context, resource_name).await?
        } else {
            true // Нет ресурса - пропускаем ABAC
        };
        
        let final_result = rbac_result && abac_result;
        
        // 5. Кеширование результата
        self.permission_cache.set(&cache_key, final_result, Duration::from_secs(300)).await;
        
        // 6. Аудит логирование
        if final_result {
            self.audit_logger.log_permission_granted(
                &user_context.user_id,
                required_permissions,
                resource
            ).await?;
        } else {
            self.audit_logger.log_permission_denied(
                &user_context.user_id,
                required_permissions,
                "ABAC_DENIED"
            ).await?;
        }
        
        // 7. Метрики
        let evaluation_duration = evaluation_start.elapsed();
        self.record_permission_evaluation_metrics(evaluation_duration, final_result);
        
        Ok(final_result)
    }
    
    /// RBAC оценка базовых разрешений
    async fn evaluate_rbac_permissions(
        &self,
        context: &EvaluationContext,
        required_permissions: &[String],
    ) -> Result<bool, PermissionError> {
        // Проверка прямых разрешений пользователя
        for required_permission in required_permissions {
            if context.permissions.contains(required_permission) {
                return Ok(true);
            }
        }
        
        // Проверка разрешений через роли
        for role in &context.roles {
            let role_permissions = self.policy_store.get_role_permissions(role).await?;
            for required_permission in required_permissions {
                if role_permissions.contains(required_permission) {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// ABAC оценка контекстных условий
    async fn evaluate_abac_conditions(
        &self,
        context: &EvaluationContext,
        resource: &str,
    ) -> Result<bool, PermissionError> {
        // Получение политик для ресурса
        let policies = self.policy_store.get_resource_policies(resource).await?;
        
        for policy in policies {
            let decision = self.abac_evaluator.evaluate_policy(&policy, context).await?;
            
            match decision {
                PolicyDecision::Permit => return Ok(true),
                PolicyDecision::Deny => return Ok(false),
                PolicyDecision::NotApplicable => continue,
            }
        }
        
        // По умолчанию разрешаем если нет явного запрета
        Ok(true)
    }
}
```

### GraphQL Guard Components - Защита GraphQL операций

#### Role Guard - Guard для проверки ролей
```rust
// crates/shared/src/auth/guards/role_guard.rs
use async_graphql::{Context, Guard, Result};
use crate::auth::{UserContext, AuditService};

pub struct RoleGuard {
    required_roles: Vec<String>,
    require_all: bool, // true = AND, false = OR
    audit_access: bool,
}

impl RoleGuard {
    pub fn new(roles: Vec<&str>) -> Self {
        Self {
            required_roles: roles.into_iter().map(String::from).collect(),
            require_all: false,
            audit_access: true,
        }
    }
    
    pub fn require_all_roles(mut self) -> Self {
        self.require_all = true;
        self
    }
    
    pub fn without_audit(mut self) -> Self {
        self.audit_access = false;
        self
    }
}

#[async_trait::async_trait]
impl Guard for RoleGuard {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        // 1. Получение пользовательского контекста
        let user_context = ctx.data_opt::<UserContext>()
            .ok_or_else(|| "Authentication required")?;
        
        // 2. Проверка ролей
        let has_required_roles = if self.require_all {
            // Все роли должны присутствовать (AND)
            self.required_roles.iter()
                .all(|role| user_context.roles.contains(role))
        } else {
            // Достаточно одной роли (OR)
            self.required_roles.iter()
                .any(|role| user_context.roles.contains(role))
        };
        
        if !has_required_roles {
            // Аудит неудачной попытки доступа
            if self.audit_access {
                if let Ok(audit_service) = ctx.data::<AuditService>() {
                    audit_service.log_access_denied(
                        &user_context.user_id,
                        &format!("Required roles: {:?}, User roles: {:?}", 
                                self.required_roles, user_context.roles),
                        "ROLE_GUARD_DENIED"
                    ).await.ok();
                }
            }
            
            return Err(format!(
                "Insufficient permissions. Required roles: {:?}",
                self.required_roles
            ).into());
        }
        
        // 3. Аудит успешного доступа
        if self.audit_access {
            if let Ok(audit_service) = ctx.data::<AuditService>() {
                audit_service.log_access_granted(
                    &user_context.user_id,
                    &format!("Role guard passed with roles: {:?}", self.required_roles),
                    "ROLE_GUARD_GRANTED"
                ).await.ok();
            }
        }
        
        Ok(())
    }
}

// Макрос для удобного создания Role Guards
#[macro_export]
macro_rules! require_roles {
    ($($role:expr),+) => {
        RoleGuard::new(vec![$($role),+])
    };
    (all: $($role:expr),+) => {
        RoleGuard::new(vec![$($role),+]).require_all_roles()
    };
}

// Использование в GraphQL резолверах
#[Object]
impl Query {
    #[graphql(guard = "require_roles!(\"admin\", \"moderator\")")]
    async fn admin_users(&self, ctx: &Context<'_>) -> Result<Vec<User>> {
        // Только админы и модераторы могут получить список пользователей
        self.user_service.get_all_users().await
    }
    
    #[graphql(guard = "require_roles!(all: \"admin\", \"super_admin\")")]
    async fn system_settings(&self, ctx: &Context<'_>) -> Result<SystemSettings> {
        // Требуются ОБЕ роли: admin И super_admin
        self.system_service.get_settings().await
    }
}
```

## 🎯 Ключевые архитектурные принципы Component уровня

### 1. Separation of Concerns (Разделение ответственности)
Каждый компонент имеет четко определенную ответственность:
- **JWT Validator**: Только валидация токенов
- **Permission Engine**: Только проверка разрешений
- **Rate Limiter**: Только ограничение скорости
- **Audit Logger**: Только логирование событий

### 2. Caching Strategy (Стратегия кеширования)
Агрессивное кеширование для производительности:
- **Token Validation Cache**: Кеширование результатов валидации JWT
- **Permission Cache**: Кеширование результатов проверки разрешений
- **JWKS Cache**: Кеширование публичных ключей
- **Role Cache**: Кеширование ролей и их разрешений

### 3. Extensibility (Расширяемость)
Компоненты спроектированы для легкого расширения:
- **Policy Evaluator Trait**: Возможность добавления новых типов политик
- **Guard Trait**: Создание кастомных Guards для GraphQL
- **OAuth2 Provider Interface**: Добавление новых OAuth2 провайдеров
- **Audit Event Types**: Расширение типов событий аудита

### 4. Fault Tolerance (Отказоустойчивость)
Компоненты устойчивы к сбоям:
- **Circuit Breaker**: Защита от каскадных сбоев
- **Retry Logic**: Автоматические повторы при временных сбоях
- **Graceful Degradation**: Работа в ограниченном режиме при сбоях
- **Fallback Mechanisms**: Резервные механизмы при недоступности сервисов

Эта Component диаграмма показывает, как принципы безопасности реализуются на самом детальном уровне архитектуры, обеспечивая надежную, производительную и расширяемую систему аутентификации и авторизации.