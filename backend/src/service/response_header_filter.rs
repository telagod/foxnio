use http::HeaderMap;
use serde::{Deserialize, Serialize};

/// Filter for response headers
pub struct ResponseHeaderFilter {
    config: FilterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub remove_headers: Vec<String>,
    pub add_headers: Vec<(String, String)>,
    pub replace_headers: Vec<(String, String)>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            remove_headers: vec!["X-Powered-By".to_string(), "Server".to_string()],
            add_headers: vec![
                ("X-Content-Type-Options".to_string(), "nosniff".to_string()),
                ("X-Frame-Options".to_string(), "DENY".to_string()),
            ],
            replace_headers: vec![],
        }
    }
}

impl ResponseHeaderFilter {
    pub fn new(config: FilterConfig) -> Self {
        Self { config }
    }

    /// Apply filter to headers
    pub fn apply(&self, headers: &mut HeaderMap) {
        // Remove headers
        for header in &self.config.remove_headers {
            headers.remove(header.as_str());
        }

        // Add headers
        for (key, value) in &self.config.add_headers {
            headers.insert(
                http::header::HeaderName::try_from(key.as_str()).unwrap(),
                http::HeaderValue::from_str(value).unwrap(),
            );
        }

        // Replace headers
        for (key, value) in &self.config.replace_headers {
            if headers.contains_key(key.as_str()) {
                headers.insert(
                    http::header::HeaderName::try_from(key.as_str()).unwrap(),
                    http::HeaderValue::from_str(value).unwrap(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_filter() {
        let filter = ResponseHeaderFilter::new(FilterConfig::default());
        let mut headers = HeaderMap::new();

        headers.insert("X-Powered-By", "PHP".parse().unwrap());

        filter.apply(&mut headers);

        assert!(!headers.contains_key("X-Powered-By"));
        assert!(headers.contains_key("X-Content-Type-Options"));
    }
}
