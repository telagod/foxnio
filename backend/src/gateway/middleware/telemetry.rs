//! 遥测拦截中间件

use axum::{
    body::Body,
    http::{Request, Response, StatusCode, Uri},
    middleware::Next,
};
use rand::Rng;
use uuid::Uuid;

/// 遥测域名黑名单
pub const TELEMETRY_DOMAINS: &[&str] = &[
    // 错误上报
    "sentry.io",
    "sentry.io/api",
    
    // 分析平台
    "amplitude.com",
    "api.amplitude.com",
    "segment.io",
    "api.segment.io",
    "posthog.com",
    "app.posthog.com",
    
    // 广告追踪
    "google-analytics.com",
    "www.google-analytics.com",
    "googletagmanager.com",
    
    // 功能开关
    "statsig.com",
    "api.statsig.com",
    "launchdarkly.com",
    "optimizely.com",
    
    // 其他遥测
    "mixpanel.com",
    "heap.io",
    "hotjar.com",
    "fullstory.com",
    "logrocket.com",
    "datadoghq.com",
    "newrelic.com",
];

/// 检查是否为遥测端点
pub fn is_telemetry_endpoint(uri: &Uri) -> bool {
    let host = uri.host().unwrap_or("");
    
    TELEMETRY_DOMAINS.iter().any(|domain| {
        host == *domain || host.ends_with(&format!(".{}", domain))
    })
}

/// 拦截遥测请求中间件
pub async fn block_telemetry(
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // 检查是否为遥测端点
    if is_telemetry_endpoint(req.uri()) {
        // 返回 204 No Content
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap();
    }
    
    next.run(req).await
}

/// 生成设备 ID（64 字符 hex）
pub fn generate_device_id() -> String {
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| format!("{:x}", rng.gen_range(0..16)))
        .collect()
}

/// 生成会话 ID（UUID）
pub fn generate_session_id() -> String {
    Uuid::new_v4().to_string()
}

/// 生成 metadata.user_id
/// 
/// 根据 Claude Code 版本选择格式：
/// - >= 2.1.78: JSON 格式
/// - < 2.1.78: 字符串格式
pub fn generate_metadata_user_id(version: &str) -> String {
    let device_id = generate_device_id();
    let session_id = generate_session_id();
    
    // 比较版本号
    if compare_versions(version, "2.1.78") >= 0 {
        // 新格式（JSON）
        serde_json::json!({
            "device_id": device_id,
            "account_uuid": "",
            "session_id": session_id
        })
        .to_string()
    } else {
        // 旧格式（字符串）
        format!("user_{}_account__session_{}", device_id, session_id)
    }
}

/// 比较版本号
/// 返回: -1 (a < b), 0 (a == b), 1 (a > b)
fn compare_versions(a: &str, b: &str) -> i32 {
    let parse_version = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };
    
    let a_parts = parse_version(a);
    let b_parts = parse_version(b);
    
    for i in 0..3 {
        let a_val = a_parts.get(i).unwrap_or(&0);
        let b_val = b_parts.get(i).unwrap_or(&0);
        
        if a_val < b_val {
            return -1;
        }
        if a_val > b_val {
            return 1;
        }
    }
    
    0
}

/// 解析 metadata.user_id
#[derive(Debug, Clone)]
pub struct ParsedUserID {
    pub device_id: String,
    pub account_uuid: String,
    pub session_id: String,
    pub is_new_format: bool,
}

impl ParsedUserID {
    /// 从字符串解析
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        
        // 尝试 JSON 格式
        if raw.starts_with('{') {
            let json: serde_json::Value = serde_json::from_str(raw).ok()?;
            
            let device_id = json.get("device_id")?.as_str()?.to_string();
            let session_id = json.get("session_id")?.as_str()?.to_string();
            let account_uuid = json.get("account_uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            
            if device_id.is_empty() || session_id.is_empty() {
                return None;
            }
            
            return Some(Self {
                device_id,
                account_uuid,
                session_id,
                is_new_format: true,
            });
        }
        
        // 尝试旧格式
        // user_{64hex}_account_{optional_uuid}_session_{uuid}
        let parts: Vec<&str> = raw.split('_').collect();
        if parts.len() < 6 {
            return None;
        }
        
        if parts[0] != "user" || parts[2] != "account" || parts[4] != "session" {
            return None;
        }
        
        let device_id = parts[1].to_string();
        let account_uuid = parts[3].to_string();
        let session_id = parts[5..].join("_");
        
        if device_id.len() != 64 || session_id.len() != 36 {
            return None;
        }
        
        Some(Self {
            device_id,
            account_uuid,
            session_id,
            is_new_format: false,
        })
    }
    
    /// 格式化为字符串
    pub fn format(&self, version: &str) -> String {
        if compare_versions(version, "2.1.78") >= 0 {
            serde_json::json!({
                "device_id": self.device_id,
                "account_uuid": self.account_uuid,
                "session_id": self.session_id
            })
            .to_string()
        } else {
            format!(
                "user_{}_account_{}_session_{}",
                self.device_id, self.account_uuid, self.session_id
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_telemetry_endpoint() {
        assert!(is_telemetry_endpoint(&"https://sentry.io/api/envelope".parse().unwrap()));
        assert!(is_telemetry_endpoint(&"https://api.amplitude.com/track".parse().unwrap()));
        assert!(!is_telemetry_endpoint(&"https://api.anthropic.com/v1/messages".parse().unwrap()));
    }
    
    #[test]
    fn test_generate_device_id() {
        let id = generate_device_id();
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
    
    #[test]
    fn test_generate_session_id() {
        let id = generate_session_id();
        assert_eq!(id.len(), 36);
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }
    
    #[test]
    fn test_generate_metadata_user_id() {
        // 新格式
        let new_id = generate_metadata_user_id("2.1.78");
        assert!(new_id.starts_with('{'));
        
        let parsed = ParsedUserID::parse(&new_id);
        assert!(parsed.is_some());
        assert!(parsed.unwrap().is_new_format);
        
        // 旧格式
        let old_id = generate_metadata_user_id("2.1.77");
        assert!(old_id.starts_with("user_"));
        
        let parsed = ParsedUserID::parse(&old_id);
        assert!(parsed.is_some());
        assert!(!parsed.unwrap().is_new_format);
    }
    
    #[test]
    fn test_parse_user_id_json() {
        let json = r#"{"device_id":"abc123","account_uuid":"","session_id":"uuid-456"}"#;
        let parsed = ParsedUserID::parse(json).unwrap();
        
        assert_eq!(parsed.device_id, "abc123");
        assert_eq!(parsed.account_uuid, "");
        assert_eq!(parsed.session_id, "uuid-456");
        assert!(parsed.is_new_format);
    }
    
    #[test]
    fn test_parse_user_id_legacy() {
        let legacy = "user_1234567890123456789012345678901234567890123456789012345678901234_account__session_12345678-1234-1234-1234-123456789abc";
        let parsed = ParsedUserID::parse(legacy).unwrap();
        
        assert_eq!(parsed.device_id.len(), 64);
        assert_eq!(parsed.account_uuid, "");
        assert_eq!(parsed.session_id.len(), 36);
        assert!(!parsed.is_new_format);
    }
    
    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("2.1.78", "2.1.78"), 0);
        assert_eq!(compare_versions("2.1.79", "2.1.78"), 1);
        assert_eq!(compare_versions("2.1.77", "2.1.78"), -1);
        assert_eq!(compare_versions("3.0.0", "2.99.99"), 1);
    }
}
