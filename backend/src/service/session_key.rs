//! Session key 构建器
//!
//! 从多种来源提取 session 标识，按优先级 fallback

use sha2::{Digest, Sha256};

/// 请求中可用的 session 线索
#[derive(Debug, Clone, Default)]
pub struct RequestSessionHints {
    /// 从 metadata.user_id 解析出的 session_id
    pub metadata_session_id: Option<String>,
    /// x-session-id header
    pub x_session_id: Option<String>,
    /// 客户端 IP
    pub client_ip: Option<String>,
    /// User-Agent
    pub user_agent: Option<String>,
}

impl RequestSessionHints {
    /// 按优先级解析出最终 session key
    /// 优先级: metadata_session_id > x_session_id > sha256(ip|ua)[..16]
    pub fn resolve(&self) -> Option<String> {
        // 1. 从 body metadata 解析的 session_id（最精确）
        if let Some(ref sid) = self.metadata_session_id {
            if !sid.is_empty() {
                return Some(sid.clone());
            }
        }

        // 2. x-session-id header
        if let Some(ref xsid) = self.x_session_id {
            if !xsid.is_empty() {
                return Some(format!("xsid:{xsid}"));
            }
        }

        // 3. IP + UA 组合 hash fallback
        let ip = self.client_ip.as_deref().unwrap_or("");
        let ua = self.user_agent.as_deref().unwrap_or("");
        if ip.is_empty() && ua.is_empty() {
            return None;
        }
        let composite = format!("{ip}|{ua}");
        let hash = Sha256::digest(composite.as_bytes());
        let hex = hex::encode(hash);
        Some(format!("fb:{}", &hex[..16]))
    }

    /// 带分组前缀的 session key
    pub fn resolve_for_group(&self, group_id: i64) -> Option<String> {
        self.resolve().map(|key| format!("g:{group_id}:{key}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_session_id_priority() {
        let hints = RequestSessionHints {
            metadata_session_id: Some("sess-abc".into()),
            x_session_id: Some("header-xyz".into()),
            client_ip: Some("1.2.3.4".into()),
            user_agent: Some("test-agent".into()),
        };
        assert_eq!(hints.resolve(), Some("sess-abc".into()));
    }

    #[test]
    fn test_x_session_id_fallback() {
        let hints = RequestSessionHints {
            metadata_session_id: None,
            x_session_id: Some("header-xyz".into()),
            client_ip: Some("1.2.3.4".into()),
            user_agent: Some("test-agent".into()),
        };
        assert_eq!(hints.resolve(), Some("xsid:header-xyz".into()));
    }

    #[test]
    fn test_ip_ua_fallback() {
        let hints = RequestSessionHints {
            metadata_session_id: None,
            x_session_id: None,
            client_ip: Some("1.2.3.4".into()),
            user_agent: Some("test-agent".into()),
        };
        let resolved = hints.resolve().unwrap();
        assert!(resolved.starts_with("fb:"));
        assert_eq!(resolved.len(), 3 + 16); // "fb:" + 16 hex chars
    }

    #[test]
    fn test_ip_ua_deterministic() {
        let hints1 = RequestSessionHints {
            client_ip: Some("1.2.3.4".into()),
            user_agent: Some("agent".into()),
            ..Default::default()
        };
        let hints2 = hints1.clone();
        assert_eq!(hints1.resolve(), hints2.resolve());
    }

    #[test]
    fn test_empty_returns_none() {
        let hints = RequestSessionHints::default();
        assert_eq!(hints.resolve(), None);
    }

    #[test]
    fn test_group_prefix() {
        let hints = RequestSessionHints {
            metadata_session_id: Some("sess-abc".into()),
            ..Default::default()
        };
        assert_eq!(
            hints.resolve_for_group(42),
            Some("g:42:sess-abc".into())
        );
    }
}
