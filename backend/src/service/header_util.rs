use http::HeaderMap;

/// Utility functions for HTTP headers
pub struct HeaderUtil;

impl HeaderUtil {
    /// Get header value as string
    pub fn get(headers: &HeaderMap, key: &str) -> Option<String> {
        headers
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Parse authorization header
    pub fn parse_auth_header(headers: &HeaderMap) -> Option<(String, String)> {
        let auth = Self::get(headers, "Authorization")?;

        let parts: Vec<&str> = auth.splitn(2, ' ').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    /// Extract bearer token
    pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
        let (scheme, token) = Self::parse_auth_header(headers)?;

        if scheme.to_lowercase() == "bearer" {
            Some(token)
        } else {
            None
        }
    }

    /// Check if content type is JSON
    pub fn is_json(headers: &HeaderMap) -> bool {
        Self::get(headers, "Content-Type")
            .map(|ct| ct.starts_with("application/json"))
            .unwrap_or(false)
    }

    /// Check if content type is multipart
    pub fn is_multipart(headers: &HeaderMap) -> bool {
        Self::get(headers, "Content-Type")
            .map(|ct| ct.starts_with("multipart/"))
            .unwrap_or(false)
    }

    /// Get content length
    pub fn get_content_length(headers: &HeaderMap) -> Option<u64> {
        Self::get(headers, "Content-Length").and_then(|s| s.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer test_token".parse().unwrap());

        let token = HeaderUtil::extract_bearer_token(&headers);
        assert_eq!(token, Some("test_token".to_string()));
    }

    #[test]
    fn test_is_json() {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());

        assert!(HeaderUtil::is_json(&headers));
    }
}
