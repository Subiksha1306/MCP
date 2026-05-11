use serde::{Deserialize, Serialize};
use std::env;

// ============================
// Permission Levels (existing)
// ============================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    ReadOnly,
    ReadWrite,
}

impl PermissionLevel {
    pub fn from_env() -> Self {
        match env::var("PERMISSIONS_MODE").unwrap_or_default().to_lowercase().as_str() {
            "readwrite" | "rw" => PermissionLevel::ReadWrite,
            _ => PermissionLevel::ReadOnly, // Default to ReadOnly for safety
        }
    }

    pub fn can_execute(&self, required: PermissionLevel) -> bool {
        match (self, required) {
            (PermissionLevel::ReadWrite, _) => true,
            (PermissionLevel::ReadOnly, PermissionLevel::ReadOnly) => true,
            (PermissionLevel::ReadOnly, PermissionLevel::ReadWrite) => false,
        }
    }

    /// Returns a human-readable label for the current permission level.
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionLevel::ReadOnly => "ReadOnly",
            PermissionLevel::ReadWrite => "ReadWrite",
        }
    }
}

// ============================
// RBAC: Role-Based Access Control
// ============================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    Admin,    // Full access to all sources, categories, and admin functions
    Analyst,  // Query access to allowed sources/categories, no admin
    Viewer,   // Read-only access to allowed sources, no AI queries
}

impl UserRole {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "admin" => UserRole::Admin,
            "analyst" => UserRole::Analyst,
            _ => UserRole::Viewer,
        }
    }

    pub fn can_query(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Analyst)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }

    pub fn can_view(&self) -> bool {
        true // All roles can view
    }
}

/// Security context for a user session, loaded from environment or config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub user_id: String,
    pub role: UserRole,
    pub allowed_sources: Vec<String>,     // ["SharePoint", "Dataverse"]
    pub allowed_categories: Vec<String>,  // ["Policy", "Finance", "Engineering"]
    pub max_queries_per_minute: u32,
}

impl SecurityContext {
    /// Load security context from environment variables
    pub fn from_env() -> Self {
        let role_str = env::var("USER_ROLE").unwrap_or_else(|_| "admin".to_string());
        let user_id = env::var("USER_ID").unwrap_or_else(|_| "default_user".to_string());

        let allowed_sources = env::var("ALLOWED_SOURCES")
            .unwrap_or_else(|_| "SharePoint,Dataverse".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let allowed_categories = env::var("ALLOWED_CATEGORIES")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let max_qpm = env::var("MAX_QUERIES_PER_MINUTE")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30);

        Self {
            user_id,
            role: UserRole::from_str(&role_str),
            allowed_sources,
            allowed_categories,
            max_queries_per_minute: max_qpm,
        }
    }

    /// Check if user can access a specific data source
    pub fn can_access_source(&self, source: &str) -> bool {
        if self.role == UserRole::Admin {
            return true;
        }
        self.allowed_sources.iter().any(|s| s.eq_ignore_ascii_case(source))
    }

    /// Check if user can access a specific category
    pub fn can_access_category(&self, category: &str) -> bool {
        if self.role == UserRole::Admin {
            return true;
        }
        // Wildcard = all categories
        if self.allowed_categories.iter().any(|c| c == "*") {
            return true;
        }
        self.allowed_categories.iter().any(|c| c.eq_ignore_ascii_case(category))
    }

    /// Validate a query for authorization
    pub fn authorize_query(&self, prompt: &str) -> Result<(), String> {
        // Check role
        if !self.role.can_query() {
            return Err(format!(
                "Forbidden: User '{}' with role '{:?}' does not have query permissions.",
                self.user_id, self.role
            ));
        }

        // Check prompt safety
        sanitize_prompt(prompt)?;

        Ok(())
    }
}

// ============================
// Input Sanitization
// ============================

/// Sanitize user prompts to prevent injection attacks
pub fn sanitize_prompt(prompt: &str) -> Result<(), String> {
    let trimmed = prompt.trim();

    if trimmed.is_empty() {
        return Err("Prompt cannot be empty.".to_string());
    }

    if trimmed.len() > 5000 {
        return Err("Prompt too long. Maximum 5000 characters allowed.".to_string());
    }

    let lower = trimmed.to_lowercase();

    // Block common prompt injection patterns
    let injection_patterns = [
        "ignore previous instructions",
        "ignore all instructions",
        "forget your instructions",
        "system prompt",
        "you are now",
        "act as if",
        "pretend you are",
        "override your",
        "disregard all",
        "ignore the above",
    ];

    for pattern in &injection_patterns {
        if lower.contains(pattern) {
            return Err(format!(
                "Blocked: Prompt contains a restricted pattern ('{}').",
                pattern
            ));
        }
    }

    // Block script/code injection
    if lower.contains("<script") || lower.contains("javascript:") || lower.contains("onerror=") {
        return Err("Blocked: Prompt contains potentially unsafe content.".to_string());
    }

    Ok(())
}

// ============================
// Existing utility functions
// ============================

/// Validates and logs the security posture at application startup.
/// Prints a warning to stderr if the environment is set to ReadWrite.
pub fn validate_startup_permissions() {
    let level = PermissionLevel::from_env();
    let ctx = SecurityContext::from_env();

    match level {
        PermissionLevel::ReadOnly => {
            println!("🔒 Security Posture: READ-ONLY mode enforced. Write operations are blocked.");
        }
        PermissionLevel::ReadWrite => {
            eprintln!("⚠️  Security Warning: READ-WRITE mode is active. Write operations are permitted.");
            eprintln!("   Set PERMISSIONS_MODE=ReadOnly in .env for production environments.");
        }
    }

    println!("👤 RBAC: User '{}' | Role: {:?} | Sources: {:?} | Categories: {:?}",
        ctx.user_id, ctx.role, ctx.allowed_sources, ctx.allowed_categories
    );
}

/// Returns a consistent, user-facing error string when a write operation is denied.
pub fn deny_write_operation(operation: &str) -> String {
    format!(
        "Forbidden: '{}' requires write permissions, but the server is in Read-Only mode. \
         Contact your administrator to change PERMISSIONS_MODE.",
        operation
    )
}

/// Validates a URL for safe usage. Rejects dangerous schemes and non-HTTPS URLs.
/// Returns Ok(()) if the URL is valid, or Err(String) with a user-facing message.
pub fn validate_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();

    if trimmed.is_empty() {
        return Err("URL cannot be empty.".to_string());
    }

    let lower = trimmed.to_lowercase();

    // Block dangerous URI schemes
    let blocked_schemes = ["javascript:", "data:", "file:", "vbscript:", "blob:"];
    for scheme in &blocked_schemes {
        if lower.starts_with(scheme) {
            return Err(format!("Blocked: '{}' scheme is not allowed.", scheme.trim_end_matches(':')));
        }
    }

    // Require HTTPS
    if !lower.starts_with("https://") {
        return Err("Only HTTPS URLs are permitted.".to_string());
    }

    // Basic structural check — must have a host after the scheme
    let after_scheme = &trimmed[8..]; // skip "https://"
    if after_scheme.is_empty() || after_scheme.starts_with('/') || after_scheme.starts_with('?') {
        return Err("Invalid URL: missing hostname.".to_string());
    }

    // Reject URLs containing script-like patterns
    if lower.contains("<script") || lower.contains("javascript:") || lower.contains("onerror=") {
        return Err("URL contains potentially unsafe content.".to_string());
    }

    Ok(())
}
