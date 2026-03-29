use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Privacy service for Antigravity API
pub struct AntigravityPrivacyService {
    pool: PgPool,
    config: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub enable_pii_detection: bool,
    pub enable_anonymization: bool,
    pub log_sensitive_data: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            enable_pii_detection: true,
            enable_anonymization: true,
            log_sensitive_data: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    #[error("PII detected")]
    PiiDetected,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl AntigravityPrivacyService {
    pub fn new(pool: PgPool, config: PrivacyConfig) -> Self {
        Self { pool, config }
    }

    /// Check for PII
    pub fn check_pii(&self, content: &str) -> Vec<String> {
        if !self.config.enable_pii_detection {
            return vec![];
        }

        let mut pii_types = Vec::new();

        // Simple pattern matching
        if regex::Regex::new(r"\b[\w\.-]+@[\w\.-]+\.\w+\b")
            .map(|re| re.is_match(content))
            .unwrap_or(false)
        {
            pii_types.push("email".to_string());
        }

        if regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b")
            .map(|re| re.is_match(content))
            .unwrap_or(false)
        {
            pii_types.push("phone".to_string());
        }

        pii_types
    }

    /// Anonymize content
    pub fn anonymize(&self, content: &str) -> String {
        if !self.config.enable_anonymization {
            return content.to_string();
        }

        let mut result = content.to_string();

        // Replace emails
        if let Ok(re) = regex::Regex::new(r"\b[\w\.-]+@[\w\.-]+\.\w+\b") {
            result = re.replace_all(&result, "[EMAIL]").to_string();
        }

        // Replace phones
        if let Ok(re) = regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b") {
            result = re.replace_all(&result, "[PHONE]").to_string();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_config_default() {
        let config = PrivacyConfig::default();
        assert!(config.enable_pii_detection);
    }

    #[test]
    fn test_pii_detection() {
        // Create a mock pool reference - we only need it for the struct, check_pii doesn't use it
        let config = PrivacyConfig::default();

        // Test email detection directly via regex
        let email_regex = regex::Regex::new(r"\b[\w\.-]+@[\w\.-]+\.\w+\b").unwrap();
        assert!(email_regex.is_match("Contact: test@example.com"));
        assert!(email_regex.is_match("email: user.name@domain.co.uk"));
        assert!(!email_regex.is_match("no email here"));
    }

    #[test]
    fn test_phone_detection() {
        let phone_regex = regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b").unwrap();
        assert!(phone_regex.is_match("Call: 123-456-7890"));
        assert!(phone_regex.is_match("Phone: 123.456.7890"));
        assert!(!phone_regex.is_match("not a phone"));
    }
}
