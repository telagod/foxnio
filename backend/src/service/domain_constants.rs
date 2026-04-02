use once_cell::sync::Lazy;

/// Domain constants for the application
pub struct DomainConstants;

/// Default page size for list queries
pub static DEFAULT_PAGE_SIZE: Lazy<u32> = Lazy::new(|| 20);

/// Max page size for list queries
pub static MAX_PAGE_SIZE: Lazy<u32> = Lazy::new(|| 100);

/// Default token expiry in seconds
pub static DEFAULT_TOKEN_EXPIRY_SECONDS: Lazy<u64> = Lazy::new(|| 3600);

/// Max token expiry in seconds
pub static MAX_TOKEN_EXPIRY_SECONDS: Lazy<u64> = Lazy::new(|| 86400);

/// Default cache TTL in seconds
pub static DEFAULT_CACHE_TTL_SECONDS: Lazy<u64> = Lazy::new(|| 300);

/// Max retry attempts
pub static MAX_RETRY_ATTEMPTS: Lazy<u32> = Lazy::new(|| 5);

/// Request timeout in seconds
pub static REQUEST_TIMEOUT_SECONDS: Lazy<u64> = Lazy::new(|| 30);

impl DomainConstants {
    /// Get default page size
    pub fn default_page_size() -> u32 {
        *DEFAULT_PAGE_SIZE
    }

    /// Get max page size
    pub fn max_page_size() -> u32 {
        *MAX_PAGE_SIZE
    }

    /// Get default token expiry
    pub fn default_token_expiry() -> u64 {
        *DEFAULT_TOKEN_EXPIRY_SECONDS
    }

    /// Validate page size
    pub fn validate_page_size(size: u32) -> u32 {
        size.min(*MAX_PAGE_SIZE).max(1)
    }

    /// Get request timeout
    pub fn request_timeout() -> std::time::Duration {
        std::time::Duration::from_secs(*REQUEST_TIMEOUT_SECONDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_constants() {
        assert_eq!(DomainConstants::default_page_size(), 20);
        assert_eq!(DomainConstants::max_page_size(), 100);
    }

    #[test]
    fn test_validate_page_size() {
        assert_eq!(DomainConstants::validate_page_size(50), 50);
        assert_eq!(DomainConstants::validate_page_size(200), 100); // Max
        assert_eq!(DomainConstants::validate_page_size(0), 1); // Min
    }
}
