use async_graphql::{
    extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute, NextParseQuery},
    parser::types::{ExecutableDocument, Field, Selection, SelectionSet},
    PathSegment, ServerError, ServerResult, Variables,
};
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

/// Configuration for query complexity and depth limits
#[derive(Debug, Clone)]
pub struct QueryLimitsConfig {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub default_field_complexity: usize,
    pub enable_introspection_limits: bool,
    pub per_user_limits: HashMap<String, UserLimits>,
}

#[derive(Debug, Clone)]
pub struct UserLimits {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub rate_limit_per_minute: usize,
}

impl Default for QueryLimitsConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_complexity: 1000,
            default_field_complexity: 1,
            enable_introspection_limits: false,
            per_user_limits: HashMap::new(),
        }
    }
}

/// Query complexity analyzer
pub struct QueryComplexityAnalyzer {
    config: QueryLimitsConfig,
    field_complexities: HashMap<String, usize>,
}

impl QueryComplexityAnalyzer {
    pub fn new(config: QueryLimitsConfig) -> Self {
        let mut field_complexities = HashMap::new();
        
        // Define complexity scores for different field types
        field_complexities.insert("reviews".to_string(), 5);
        field_complexities.insert("reviewsConnection".to_string(), 10);
        field_complexities.insert("offer".to_string(), 3);
        field_complexities.insert("user".to_string(), 2);
        field_complexities.insert("averageRating".to_string(), 3);
        field_complexities.insert("reviewsCount".to_string(), 2);
        field_complexities.insert("createReview".to_string(), 10);
        field_complexities.insert("updateReview".to_string(), 8);
        field_complexities.insert("deleteReview".to_string(), 5);
        field_complexities.insert("moderateReview".to_string(), 7);
        
        Self {
            config,
            field_complexities,
        }
    }

    /// Calculate the complexity of a GraphQL query
    pub fn calculate_complexity(&self, document: &ExecutableDocument, variables: &Variables) -> usize {
        let mut total_complexity = 0;
        
        for definition in &document.operations {
            if let Some(operation) = definition.node.as_query() {
                total_complexity += self.calculate_selection_set_complexity(
                    &operation.selection_set.node,
                    variables,
                    1, // multiplier
                );
            } else if let Some(operation) = definition.node.as_mutation() {
                total_complexity += self.calculate_selection_set_complexity(
                    &operation.selection_set.node,
                    variables,
                    2, // mutations are more expensive
                );
            }
        }
        
        total_complexity
    }

    /// Calculate the maximum depth of a GraphQL query
    pub fn calculate_depth(&self, document: &ExecutableDocument) -> usize {
        let mut max_depth = 0;
        
        for definition in &document.operations {
            if let Some(operation) = definition.node.as_query() {
                let depth = self.calculate_selection_set_depth(&operation.selection_set.node, 1);
                max_depth = max_depth.max(depth);
            } else if let Some(operation) = definition.node.as_mutation() {
                let depth = self.calculate_selection_set_depth(&operation.selection_set.node, 1);
                max_depth = max_depth.max(depth);
            }
        }
        
        max_depth
    }

    fn calculate_selection_set_complexity(
        &self,
        selection_set: &SelectionSet,
        variables: &Variables,
        multiplier: usize,
    ) -> usize {
        let mut complexity = 0;
        
        for selection in &selection_set.items {
            match &selection.node {
                Selection::Field(field) => {
                    complexity += self.calculate_field_complexity(&field.node, variables, multiplier);
                }
                Selection::InlineFragment(fragment) => {
                    complexity += self.calculate_selection_set_complexity(
                        &fragment.node.selection_set.node,
                        variables,
                        multiplier,
                    );
                }
                Selection::FragmentSpread(_) => {
                    // Fragment spreads would need to be resolved from the document
                    // For simplicity, we'll add a base complexity
                    complexity += self.config.default_field_complexity * multiplier;
                }
            }
        }
        
        complexity
    }

    fn calculate_field_complexity(
        &self,
        field: &Field,
        variables: &Variables,
        multiplier: usize,
    ) -> usize {
        let field_name = field.name.node.as_str();
        
        // Get base complexity for this field
        let base_complexity = self.field_complexities
            .get(field_name)
            .copied()
            .unwrap_or(self.config.default_field_complexity);

        // Calculate multiplier based on arguments (e.g., first, limit)
        let arg_multiplier = self.calculate_argument_multiplier(field, variables);
        
        // Calculate nested complexity
        let nested_complexity = self.calculate_selection_set_complexity(
            &field.selection_set.node,
            variables,
            multiplier,
        );
        
        (base_complexity + nested_complexity) * multiplier * arg_multiplier
    }

    fn calculate_argument_multiplier(&self, field: &Field, variables: &Variables) -> usize {
        let mut multiplier = 1;
        
        for (arg_name, arg_value) in &field.arguments {
            match arg_name.node.as_str() {
                "first" | "limit" => {
                    if let Some(value) = self.resolve_argument_value(arg_value, variables) {
                        if let Ok(limit) = value.parse::<usize>() {
                            multiplier = multiplier.max(limit.min(100)); // Cap at 100
                        }
                    }
                }
                _ => {}
            }
        }
        
        multiplier
    }

    fn resolve_argument_value(
        &self,
        value: &async_graphql::parser::types::Value,
        variables: &Variables,
    ) -> Option<String> {
        match value {
            async_graphql::parser::types::Value::Variable(var_name) => {
                variables.get(&var_name.node).map(|v| v.to_string())
            }
            async_graphql::parser::types::Value::Number(n) => Some(n.to_string()),
            async_graphql::parser::types::Value::String(s) => Some(s.to_string()),
            _ => None,
        }
    }

    fn calculate_selection_set_depth(&self, selection_set: &SelectionSet, current_depth: usize) -> usize {
        let mut max_depth = current_depth;
        
        for selection in &selection_set.items {
            match &selection.node {
                Selection::Field(field) => {
                    let field_depth = self.calculate_selection_set_depth(
                        &field.node.selection_set.node,
                        current_depth + 1,
                    );
                    max_depth = max_depth.max(field_depth);
                }
                Selection::InlineFragment(fragment) => {
                    let fragment_depth = self.calculate_selection_set_depth(
                        &fragment.node.selection_set.node,
                        current_depth,
                    );
                    max_depth = max_depth.max(fragment_depth);
                }
                Selection::FragmentSpread(_) => {
                    // For fragment spreads, assume they add one level of depth
                    max_depth = max_depth.max(current_depth + 1);
                }
            }
        }
        
        max_depth
    }
}

/// Rate limiter for per-user query limits
pub struct QueryRateLimiter {
    user_requests: std::sync::Arc<tokio::sync::RwLock<HashMap<String, UserRequestTracker>>>,
}

#[derive(Debug, Clone)]
struct UserRequestTracker {
    requests: Vec<std::time::Instant>,
    last_cleanup: std::time::Instant,
}

impl QueryRateLimiter {
    pub fn new() -> Self {
        Self {
            user_requests: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_rate_limit(&self, user_id: &str, limit_per_minute: usize) -> bool {
        let mut trackers = self.user_requests.write().await;
        let now = std::time::Instant::now();
        
        let tracker = trackers.entry(user_id.to_string()).or_insert_with(|| UserRequestTracker {
            requests: Vec::new(),
            last_cleanup: now,
        });

        // Clean up old requests (older than 1 minute)
        if now.duration_since(tracker.last_cleanup).as_secs() > 10 {
            let cutoff = now - std::time::Duration::from_secs(60);
            tracker.requests.retain(|&request_time| request_time > cutoff);
            tracker.last_cleanup = now;
        }

        // Check if user has exceeded rate limit
        if tracker.requests.len() >= limit_per_minute {
            warn!("Rate limit exceeded for user: {}", user_id);
            return false;
        }

        // Add current request
        tracker.requests.push(now);
        true
    }

    pub async fn cleanup_expired(&self) {
        let mut trackers = self.user_requests.write().await;
        let now = std::time::Instant::now();
        let cutoff = now - std::time::Duration::from_secs(300); // 5 minutes

        trackers.retain(|_, tracker| {
            tracker.requests.retain(|&request_time| request_time > cutoff);
            !tracker.requests.is_empty()
        });
    }
}

impl Default for QueryRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// GraphQL extension for query complexity and depth limiting
pub struct QueryLimitsExtension {
    analyzer: QueryComplexityAnalyzer,
    rate_limiter: QueryRateLimiter,
}

impl QueryLimitsExtension {
    pub fn new(config: QueryLimitsConfig) -> Self {
        Self {
            analyzer: QueryComplexityAnalyzer::new(config),
            rate_limiter: QueryRateLimiter::new(),
        }
    }
}

#[async_trait::async_trait]
impl Extension for QueryLimitsExtension {
    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let document = next.run(ctx, query, variables).await?;

        // Skip limits for introspection queries if configured
        if !self.analyzer.config.enable_introspection_limits && self.is_introspection_query(&document) {
            return Ok(document);
        }

        // Check depth limit
        let depth = self.analyzer.calculate_depth(&document);
        if depth > self.analyzer.config.max_depth {
            warn!("Query depth {} exceeds limit {}", depth, self.analyzer.config.max_depth);
            return Err(ServerError::new(
                format!("Query depth {} exceeds maximum allowed depth of {}", depth, self.analyzer.config.max_depth),
                Some("QUERY_TOO_DEEP".into()),
            ));
        }

        // Check complexity limit
        let complexity = self.analyzer.calculate_complexity(&document, variables);
        if complexity > self.analyzer.config.max_complexity {
            warn!("Query complexity {} exceeds limit {}", complexity, self.analyzer.config.max_complexity);
            return Err(ServerError::new(
                format!("Query complexity {} exceeds maximum allowed complexity of {}", complexity, self.analyzer.config.max_complexity),
                Some("QUERY_TOO_COMPLEX".into()),
            ));
        }

        debug!("Query analysis: depth={}, complexity={}", depth, complexity);

        Ok(document)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> async_graphql::Response {
        // Check rate limiting if user context is available
        if let Some(user_context) = ctx.data_opt::<crate::auth::UserContext>() {
            let user_id = user_context.user_id.to_string();
            
            // Get user-specific limits or use defaults
            let limits = self.analyzer.config.per_user_limits
                .get(&user_id)
                .cloned()
                .unwrap_or(UserLimits {
                    max_depth: self.analyzer.config.max_depth,
                    max_complexity: self.analyzer.config.max_complexity,
                    rate_limit_per_minute: 60, // Default rate limit
                });

            if !self.rate_limiter.check_rate_limit(&user_id, limits.rate_limit_per_minute).await {
                return async_graphql::Response::from_errors(vec![ServerError::new(
                    "Rate limit exceeded. Please slow down your requests.",
                    Some("RATE_LIMIT_EXCEEDED".into()),
                )]);
            }
        }

        next.run(ctx, operation_name).await
    }
}

impl QueryLimitsExtension {
    fn is_introspection_query(&self, document: &ExecutableDocument) -> bool {
        for definition in &document.operations {
            if let Some(operation) = definition.node.as_query() {
                for selection in &operation.selection_set.node.items {
                    if let Selection::Field(field) = &selection.node {
                        let field_name = field.node.name.node.as_str();
                        if field_name == "__schema" || field_name == "__type" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Extension factory for query limits
pub struct QueryLimitsExtensionFactory {
    config: QueryLimitsConfig,
}

impl QueryLimitsExtensionFactory {
    pub fn new(config: QueryLimitsConfig) -> Self {
        Self { config }
    }
}

impl ExtensionFactory for QueryLimitsExtensionFactory {
    fn create(&self) -> std::sync::Arc<dyn Extension> {
        std::sync::Arc::new(QueryLimitsExtension::new(self.config.clone()))
    }
}

/// Background task to cleanup expired rate limit data
pub async fn start_rate_limit_cleanup_task(rate_limiter: std::sync::Arc<QueryRateLimiter>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
    
    loop {
        interval.tick().await;
        rate_limiter.cleanup_expired().await;
        debug!("Rate limit cleanup completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{parser::parse_query, Variables};

    #[test]
    fn test_query_depth_calculation() {
        let config = QueryLimitsConfig::default();
        let analyzer = QueryComplexityAnalyzer::new(config);

        let query = r#"
            query {
                offer(id: "123") {
                    id
                    title
                    reviews {
                        id
                        text
                        author {
                            id
                            name
                        }
                    }
                }
            }
        "#;

        let document = parse_query(query).unwrap();
        let depth = analyzer.calculate_depth(&document);
        
        // offer -> reviews -> author = depth of 3
        assert_eq!(depth, 3);
    }

    #[test]
    fn test_query_complexity_calculation() {
        let config = QueryLimitsConfig::default();
        let analyzer = QueryComplexityAnalyzer::new(config);

        let query = r#"
            query {
                offer(id: "123") {
                    id
                    reviews(first: 10) {
                        id
                        text
                    }
                }
            }
        "#;

        let document = parse_query(query).unwrap();
        let variables = Variables::default();
        let complexity = analyzer.calculate_complexity(&document, &variables);
        
        // Should be higher than base complexity due to the reviews field and first argument
        assert!(complexity > 10);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let rate_limiter = QueryRateLimiter::new();
        let user_id = "test_user";
        let limit = 5;

        // Should allow requests up to the limit
        for _ in 0..limit {
            assert!(rate_limiter.check_rate_limit(user_id, limit).await);
        }

        // Should reject the next request
        assert!(!rate_limiter.check_rate_limit(user_id, limit).await);
    }

    #[test]
    fn test_introspection_query_detection() {
        let config = QueryLimitsConfig::default();
        let extension = QueryLimitsExtension::new(config);

        let introspection_query = r#"
            query {
                __schema {
                    types {
                        name
                    }
                }
            }
        "#;

        let document = parse_query(introspection_query).unwrap();
        assert!(extension.is_introspection_query(&document));

        let regular_query = r#"
            query {
                offer(id: "123") {
                    id
                    title
                }
            }
        "#;

        let document = parse_query(regular_query).unwrap();
        assert!(!extension.is_introspection_query(&document));
    }
}