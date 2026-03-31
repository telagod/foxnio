//! Registration email policy service

use std::collections::HashSet;

/// Email policy
#[derive(Debug, Clone)]
pub struct EmailPolicy {
    /// Allowed domains
    pub allowed_domains: HashSet<String>,
    /// Blocked domains
    pub blocked_domains: HashSet<String>,
    /// Require email verification
    pub require_verification: bool,
    /// Allow disposable emails
    pub allow_disposable: bool,
}

impl Default for EmailPolicy {
    fn default() -> Self {
        Self {
            allowed_domains: HashSet::new(),
            blocked_domains: Self::default_blocked_domains(),
            require_verification: true,
            allow_disposable: false,
        }
    }
}

impl EmailPolicy {
    /// Get default blocked domains
    fn default_blocked_domains() -> HashSet<String> {
        let mut blocked = HashSet::new();
        blocked.insert("tempmail.com".to_string());
        blocked.insert("guerrillamail.com".to_string());
        blocked.insert("10minutemail.com".to_string());
        blocked.insert("mailinator.com".to_string());
        blocked.insert("throwaway.email".to_string());
        blocked
    }

    /// Check if email is allowed
    pub fn is_email_allowed(&self, email: &str) -> Result<(), String> {
        let domain = email
            .split('@')
            .nth(1)
            .ok_or("Invalid email format")?
            .to_lowercase();

        // Check blocked domains
        if self.blocked_domains.contains(&domain) {
            return Err(format!("Domain '{domain}' is blocked"));
        }

        // Check allowed domains (if configured)
        if !self.allowed_domains.is_empty() && !self.allowed_domains.contains(&domain) {
            return Err(format!("Domain '{domain}' is not in allowed list"));
        }

        Ok(())
    }

    /// Add allowed domain
    pub fn allow_domain(&mut self, domain: String) {
        self.allowed_domains.insert(domain.to_lowercase());
        self.blocked_domains.remove(&domain.to_lowercase());
    }

    /// Block a domain
    pub fn block_domain(&mut self, domain: String) {
        self.blocked_domains.insert(domain.to_lowercase());
        self.allowed_domains.remove(&domain.to_lowercase());
    }
}

/// Registration email policy service
pub struct RegistrationEmailPolicyService {
    policy: EmailPolicy,
}

impl Default for RegistrationEmailPolicyService {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistrationEmailPolicyService {
    /// Create a new service
    pub fn new() -> Self {
        Self {
            policy: EmailPolicy::default(),
        }
    }

    /// Create with custom policy
    pub fn with_policy(policy: EmailPolicy) -> Self {
        Self { policy }
    }

    /// Validate email for registration
    pub fn validate_email(&self, email: &str) -> Result<(), String> {
        // Basic email format validation
        if !email.contains('@') || !email.contains('.') {
            return Err("Invalid email format".to_string());
        }

        // Check policy
        self.policy.is_email_allowed(email)
    }

    /// Check if email is from blocked domain
    pub fn is_blocked_domain(&self, email: &str) -> bool {
        let domain = email.split('@').nth(1).unwrap_or("").to_lowercase();
        self.policy.blocked_domains.contains(&domain)
    }

    /// Get current policy
    pub fn get_policy(&self) -> &EmailPolicy {
        &self.policy
    }

    /// Update policy
    pub fn update_policy(&mut self, policy: EmailPolicy) {
        self.policy = policy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email() {
        let service = RegistrationEmailPolicyService::new();
        let result = service.validate_email("user@example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocked_domain() {
        let service = RegistrationEmailPolicyService::new();
        let result = service.validate_email("user@tempmail.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format() {
        let service = RegistrationEmailPolicyService::new();
        let result = service.validate_email("invalid-email");
        assert!(result.is_err());
    }
}
