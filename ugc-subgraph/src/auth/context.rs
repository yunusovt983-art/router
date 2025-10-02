use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User context extracted from JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub is_authenticated: bool,
}

impl UserContext {
    /// Create a new authenticated user context
    pub fn new(user_id: Uuid, name: String, email: String, roles: Vec<String>) -> Self {
        Self {
            user_id,
            name,
            email,
            roles,
            is_authenticated: true,
        }
    }
    
    /// Create an anonymous (unauthenticated) user context
    pub fn anonymous() -> Self {
        Self {
            user_id: Uuid::nil(),
            name: "Anonymous".to_string(),
            email: String::new(),
            roles: vec![],
            is_authenticated: false,
        }
    }
    
    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
    
    /// Check if user has any of the specified roles
    pub fn has_any_role(&self, roles: &[&str]) -> bool {
        roles.iter().any(|role| self.has_role(role))
    }
    
    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }
    
    /// Check if user is moderator
    pub fn is_moderator(&self) -> bool {
        self.has_role("moderator") || self.is_admin()
    }
    
    /// Check if user can access resource owned by another user
    pub fn can_access_user_resource(&self, resource_owner_id: Uuid) -> bool {
        self.is_authenticated && (self.user_id == resource_owner_id || self.is_admin())
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// User name
    pub name: String,
    /// User email
    pub email: String,
    /// User roles
    pub roles: Vec<String>,
    /// Issued at timestamp
    pub iat: usize,
    /// Expiration timestamp
    pub exp: usize,
    /// Issuer
    pub iss: Option<String>,
    /// Audience
    pub aud: Option<String>,
}

impl Claims {
    /// Convert claims to user context
    pub fn to_user_context(&self) -> Result<UserContext, crate::auth::AuthError> {
        let user_id = Uuid::parse_str(&self.sub).map_err(|_| {
            crate::auth::AuthError::InvalidToken {
                reason: "Invalid user ID format".to_string(),
            }
        })?;
        
        Ok(UserContext::new(
            user_id,
            self.name.clone(),
            self.email.clone(),
            self.roles.clone(),
        ))
    }
    
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp() as usize;
        now > self.exp
    }
}