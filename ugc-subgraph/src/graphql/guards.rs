use crate::auth::{AuthError, UserContext};
use async_graphql::{Context, Guard, Result};
use uuid::Uuid;

/// Guard that requires user to be authenticated
pub struct RequireAuth;

#[async_trait::async_trait]
impl Guard for RequireAuth {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if user_context.is_authenticated {
            Ok(())
        } else {
            Err("Authentication required".into())
        }
    }
}

/// Guard that requires user to have a specific role
pub struct RequireRole {
    role: String,
}

impl RequireRole {
    pub fn new(role: impl Into<String>) -> Self {
        Self { role: role.into() }
    }
}

#[async_trait::async_trait]
impl Guard for RequireRole {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if !user_context.is_authenticated {
            return Err("Authentication required".into());
        }
        
        if user_context.has_role(&self.role) {
            Ok(())
        } else {
            Err(format!("Role '{}' required", self.role).into())
        }
    }
}

/// Guard that requires user to have any of the specified roles
pub struct RequireAnyRole {
    roles: Vec<String>,
}

impl RequireAnyRole {
    pub fn new(roles: Vec<impl Into<String>>) -> Self {
        Self {
            roles: roles.into_iter().map(|r| r.into()).collect(),
        }
    }
}

#[async_trait::async_trait]
impl Guard for RequireAnyRole {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if !user_context.is_authenticated {
            return Err("Authentication required".into());
        }
        
        let roles_str: Vec<&str> = self.roles.iter().map(|s| s.as_str()).collect();
        if user_context.has_any_role(&roles_str) {
            Ok(())
        } else {
            Err(format!("One of roles {:?} required", self.roles).into())
        }
    }
}

/// Guard that requires user to be admin
pub struct RequireAdmin;

#[async_trait::async_trait]
impl Guard for RequireAdmin {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if !user_context.is_authenticated {
            return Err("Authentication required".into());
        }
        
        if user_context.is_admin() {
            Ok(())
        } else {
            Err("Admin role required".into())
        }
    }
}

/// Guard that requires user to be moderator or admin
pub struct RequireModerator;

#[async_trait::async_trait]
impl Guard for RequireModerator {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if !user_context.is_authenticated {
            return Err("Authentication required".into());
        }
        
        if user_context.is_moderator() {
            Ok(())
        } else {
            Err("Moderator or admin role required".into())
        }
    }
}

/// Guard that requires user to own the resource or be admin
pub struct RequireOwnershipOrAdmin {
    resource_owner_id: Uuid,
}

impl RequireOwnershipOrAdmin {
    pub fn new(resource_owner_id: Uuid) -> Self {
        Self { resource_owner_id }
    }
}

#[async_trait::async_trait]
impl Guard for RequireOwnershipOrAdmin {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        if !user_context.is_authenticated {
            return Err("Authentication required".into());
        }
        
        if user_context.can_access_user_resource(self.resource_owner_id) {
            Ok(())
        } else {
            Err("Access denied: insufficient permissions".into())
        }
    }
}

/// Rate limiting guard
pub struct RateLimit {
    max_requests: u32,
    window_seconds: u64,
}

impl RateLimit {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
        }
    }
    
    pub fn per_minute(max_requests: u32) -> Self {
        Self::new(max_requests, 60)
    }
    
    pub fn per_hour(max_requests: u32) -> Self {
        Self::new(max_requests, 3600)
    }
}

#[async_trait::async_trait]
impl Guard for RateLimit {
    async fn check(&self, ctx: &Context<'_>) -> Result<()> {
        let user_context = ctx.data::<UserContext>()
            .map_err(|_| "User context not found")?;
        
        // For now, just log the rate limit check
        // In a real implementation, you would use Redis or similar to track requests
        tracing::debug!(
            "Rate limit check for user {}: {} requests per {} seconds",
            user_context.user_id,
            self.max_requests,
            self.window_seconds
        );
        
        // TODO: Implement actual rate limiting logic
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::UserContext;
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};
    use uuid::Uuid;
    
    struct TestQuery;
    
    #[Object]
    impl TestQuery {
        #[graphql(guard = "RequireAuth")]
        async fn protected_field(&self) -> &str {
            "protected data"
        }
        
        #[graphql(guard = "RequireRole::new(\"admin\")")]
        async fn admin_field(&self) -> &str {
            "admin data"
        }
        
        #[graphql(guard = "RequireModerator")]
        async fn moderator_field(&self) -> &str {
            "moderator data"
        }
    }
    
    #[tokio::test]
    async fn test_require_auth_guard_success() {
        let schema = Schema::new(TestQuery, EmptyMutation, EmptySubscription);
        
        let user_context = UserContext::new(
            Uuid::new_v4(),
            "Test User".to_string(),
            "test@example.com".to_string(),
            vec!["user".to_string()],
        );
        
        let result = schema
            .execute(
                async_graphql::Request::new("{ protectedField }")
                    .data(user_context)
            )
            .await;
        
        assert!(result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_require_auth_guard_failure() {
        let schema = Schema::new(TestQuery, EmptyMutation, EmptySubscription);
        
        let user_context = UserContext::anonymous();
        
        let result = schema
            .execute(
                async_graphql::Request::new("{ protectedField }")
                    .data(user_context)
            )
            .await;
        
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("Authentication required"));
    }
    
    #[tokio::test]
    async fn test_require_role_guard_success() {
        let schema = Schema::new(TestQuery, EmptyMutation, EmptySubscription);
        
        let user_context = UserContext::new(
            Uuid::new_v4(),
            "Admin User".to_string(),
            "admin@example.com".to_string(),
            vec!["admin".to_string()],
        );
        
        let result = schema
            .execute(
                async_graphql::Request::new("{ adminField }")
                    .data(user_context)
            )
            .await;
        
        assert!(result.errors.is_empty());
    }
    
    #[tokio::test]
    async fn test_require_role_guard_failure() {
        let schema = Schema::new(TestQuery, EmptyMutation, EmptySubscription);
        
        let user_context = UserContext::new(
            Uuid::new_v4(),
            "Regular User".to_string(),
            "user@example.com".to_string(),
            vec!["user".to_string()],
        );
        
        let result = schema
            .execute(
                async_graphql::Request::new("{ adminField }")
                    .data(user_context)
            )
            .await;
        
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].message.contains("Role 'admin' required"));
    }
}