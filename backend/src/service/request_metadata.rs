use http::HeaderMap;
use serde::{Deserialize, Serialize};

/// Request metadata extractor
pub struct RequestMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub request_id: String,
    pub user_id: Option<i64>,
    pub api_key_id: Option<i64>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub timestamp: i64,
}

impl RequestMetadata {
    /// Extract metadata from headers
    pub fn extract(headers: &HeaderMap) -> Metadata {
        Metadata {
            request_id: headers
                .get("X-Request-ID")
                .and_then(|v| v.to_str().ok())
                .unwrap_or(&uuid::Uuid::new_v4().to_string())
                .to_string(),
            user_id: headers
                .get("X-User-ID")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            api_key_id: headers
                .get("X-API-Key-ID")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            ip_address: headers
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            user_agent: headers
                .get(http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            content_type: headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            content_length: headers
                .get(http::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// Inject metadata into headers
    pub fn inject(metadata: &Metadata, headers: &mut HeaderMap) {
        if let Some(user_id) = metadata.user_id {
            headers.insert("X-User-ID", user_id.to_string().parse().unwrap());
        }

        if let Some(api_key_id) = metadata.api_key_id {
            headers.insert("X-API-Key-ID", api_key_id.to_string().parse().unwrap());
        }

        headers.insert("X-Request-ID", metadata.request_id.parse().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert("X-User-ID", "123".parse().unwrap());

        let metadata = RequestMetadata::extract(&headers);

        assert_eq!(metadata.user_id, Some(123));
    }
}
