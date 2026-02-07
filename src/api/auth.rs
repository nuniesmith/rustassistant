//! Authentication and authorization middleware for RAG API

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// API keys (hashed with SHA256)
    pub api_keys: Vec<String>,
    /// Whether authentication is required
    pub require_auth: bool,
    /// Whether to allow anonymous read-only access
    pub allow_anonymous_read: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_keys: Vec::new(),
            require_auth: false,
            allow_anonymous_read: true,
        }
    }
}

impl AuthConfig {
    /// Create new auth config with API keys
    pub fn new(api_keys: Vec<String>) -> Self {
        Self {
            api_keys: api_keys.into_iter().map(|key| hash_api_key(&key)).collect(),
            require_auth: !api_keys.is_empty(),
            allow_anonymous_read: false,
        }
    }

    /// Add an API key
    pub fn add_key(&mut self, key: &str) {
        self.api_keys.push(hash_api_key(key));
        self.require_auth = true;
    }

    /// Validate an API key
    pub fn validate_key(&self, key: &str) -> bool {
        if !self.require_auth {
            return true;
        }
        let hashed = hash_api_key(key);
        self.api_keys.contains(&hashed)
    }

    /// Check if method is read-only
    fn is_read_only_method(method: &str) -> bool {
        matches!(method, "GET" | "HEAD" | "OPTIONS")
    }

    /// Validate request based on config
    pub fn validate_request(&self, key: Option<&str>, method: &str) -> AuthResult {
        // No auth required
        if !self.require_auth {
            return AuthResult::Allowed;
        }

        // Check if anonymous read is allowed for GET requests
        if self.allow_anonymous_read && Self::is_read_only_method(method) && key.is_none() {
            return AuthResult::Allowed;
        }

        // Validate API key
        match key {
            Some(k) if self.validate_key(k) => AuthResult::Allowed,
            Some(_) => AuthResult::InvalidKey,
            None => AuthResult::MissingKey,
        }
    }
}

/// Authentication result
#[derive(Debug, Clone, PartialEq)]
pub enum AuthResult {
    Allowed,
    MissingKey,
    InvalidKey,
}

/// Hash an API key using SHA256
fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

// ============================================================================
// Middleware
// ============================================================================

/// Authentication middleware
pub async fn auth_middleware(
    State(config): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Extract API key from headers
    let api_key = headers
        .get("X-API-Key")
        .or_else(|| headers.get("Authorization"))
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // Handle "Bearer <token>" format
            if s.starts_with("Bearer ") {
                &s[7..]
            } else {
                s
            }
        });

    // Get request method
    let method = request.method().as_str();

    // Validate
    match config.validate_request(api_key, method) {
        AuthResult::Allowed => next.run(request).await,
        AuthResult::MissingKey => (
            StatusCode::UNAUTHORIZED,
            "Missing API key. Provide via X-API-Key header or Authorization: Bearer <key>",
        )
            .into_response(),
        AuthResult::InvalidKey => (StatusCode::FORBIDDEN, "Invalid API key").into_response(),
    }
}

// ============================================================================
// API Key Management
// ============================================================================

/// Generate a new API key
pub fn generate_api_key() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LEN: usize = 32;

    let mut rng = rand::thread_rng();
    (0..KEY_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// API Key metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
}

impl ApiKeyInfo {
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: Utc::now(),
            expires_at: None,
            last_used: None,
            scopes: vec!["read".to_string(), "write".to_string()],
        }
    }

    pub fn with_expiry(mut self, days: i64) -> Self {
        self.expires_at = Some(Utc::now() + Duration::days(days));
        self
    }

    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires
        } else {
            false
        }
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_api_key() {
        let key = "test_key_123";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, key);
    }

    #[test]
    fn test_auth_config_validation() {
        let mut config = AuthConfig::default();
        assert!(!config.require_auth);
        assert!(config.validate_key("any_key"));

        config.add_key("valid_key");
        assert!(config.require_auth);
        assert!(config.validate_key("valid_key"));
        assert!(!config.validate_key("invalid_key"));
    }

    #[test]
    fn test_validate_request() {
        let mut config = AuthConfig::default();
        config.add_key("valid_key");
        config.allow_anonymous_read = true;

        // Anonymous GET should be allowed
        assert_eq!(config.validate_request(None, "GET"), AuthResult::Allowed);

        // Anonymous POST should be denied
        assert_eq!(
            config.validate_request(None, "POST"),
            AuthResult::MissingKey
        );

        // Valid key should be allowed for any method
        assert_eq!(
            config.validate_request(Some("valid_key"), "POST"),
            AuthResult::Allowed
        );

        // Invalid key should be denied
        assert_eq!(
            config.validate_request(Some("wrong_key"), "POST"),
            AuthResult::InvalidKey
        );
    }

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();

        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_api_key_info() {
        let mut info = ApiKeyInfo::new("test_key".to_string())
            .with_expiry(30)
            .with_scopes(vec!["read".to_string()]);

        assert!(!info.is_expired());
        assert!(info.has_scope("read"));
        assert!(!info.has_scope("write"));

        // Test expiry
        info.expires_at = Some(Utc::now() - Duration::days(1));
        assert!(info.is_expired());
    }
}
