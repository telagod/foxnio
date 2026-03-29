use serde::{Deserialize, Serialize};
use sqlx::{query, PgPool};

/// Privacy service for OpenAI API requests
pub struct OpenAIPrivacyService {
    pool: PgPool,
    config: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub enable_pii_detection: bool,
    pub enable_content_filtering: bool,
    pub log_sensitive_data: bool,
    pub anonymize_user_data: bool,
    pub retention_days: u32,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            enable_pii_detection: true,
            enable_content_filtering: true,
            log_sensitive_data: false,
            anonymize_user_data: true,
            retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyAudit {
    pub request_id: String,
    pub user_id: i64,
    pub pii_detected: bool,
    pub pii_types: Vec<String>,
    pub filtered: bool,
    pub anonymized: bool,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum PrivacyError {
    #[error("PII detected in request")]
    PiiDetected,
    #[error("Content filtering failed")]
    ContentFilteringFailed,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl OpenAIPrivacyService {
    pub fn new(pool: PgPool, config: PrivacyConfig) -> Self {
        Self { pool, config }
    }

    /// Check request for PII
    pub async fn check_pii(&self, content: &str) -> Result<Vec<String>, PrivacyError> {
        if !self.config.enable_pii_detection {
            return Ok(vec![]);
        }

        let mut pii_types = Vec::new();

        // Simple PII detection patterns
        let patterns = vec![
            (r"\b\d{3}-\d{2}-\d{4}\b", "SSN"),
            (r"\b\d{16}\b", "CreditCard"),
            (r"\b[A-Z]{2}\d{6,9}\b", "Passport"),
            (r"\b[\w\.-]+@[\w\.-]+\.\w+\b", "Email"),
            (r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b", "Phone"),
        ];

        for (pattern, pii_type) in patterns {
            if regex::Regex::new(pattern)
                .map_err(|_| PrivacyError::ContentFilteringFailed)?
                .is_match(content)
            {
                pii_types.push(pii_type.to_string());
            }
        }

        Ok(pii_types)
    }

    /// Anonymize content
    pub async fn anonymize(&self, content: &str) -> Result<String, PrivacyError> {
        if !self.config.anonymize_user_data {
            return Ok(content.to_string());
        }

        let mut anonymized = content.to_string();

        // Replace email addresses
        let email_regex = regex::Regex::new(r"\b[\w\.-]+@[\w\.-]+\.\w+\b")
            .map_err(|_| PrivacyError::ContentFilteringFailed)?;
        anonymized = email_regex.replace_all(&anonymized, "[EMAIL]").to_string();

        // Replace phone numbers
        let phone_regex = regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b")
            .map_err(|_| PrivacyError::ContentFilteringFailed)?;
        anonymized = phone_regex.replace_all(&anonymized, "[PHONE]").to_string();

        // Replace SSN
        let ssn_regex = regex::Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")
            .map_err(|_| PrivacyError::ContentFilteringFailed)?;
        anonymized = ssn_regex.replace_all(&anonymized, "[SSN]").to_string();

        Ok(anonymized)
    }

    /// Log privacy audit
    pub async fn log_audit(&self, audit: &PrivacyAudit) -> Result<(), PrivacyError> {
        if !self.config.log_sensitive_data && audit.pii_detected {
            // Don't log if sensitive data detected and logging is disabled
            return Ok(());
        }

        query(
            r#"
            INSERT INTO privacy_audits (
                request_id, user_id, pii_detected, pii_types, filtered, anonymized, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&audit.request_id)
        .bind(audit.user_id)
        .bind(audit.pii_detected)
        .bind(&audit.pii_types)
        .bind(audit.filtered)
        .bind(audit.anonymized)
        .bind(audit.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_config_default() {
        let config = PrivacyConfig::default();
        assert!(config.enable_pii_detection);
        assert!(!config.log_sensitive_data);
    }
}
