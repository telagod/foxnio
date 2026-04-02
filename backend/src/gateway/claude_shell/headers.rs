// HTTP 头配置 - 基于 Claude Code 2.1.87 提取

use reqwest::header::{HeaderMap, HeaderValue};

/// 构建标准请求头
pub fn build_headers(api_key: &str, api_version: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();

    // 核心认证头
    headers.insert("x-api-key", HeaderValue::from_str(api_key).unwrap());
    headers.insert(
        "anthropic-version",
        HeaderValue::from_str(api_version).unwrap(),
    );

    // 内容类型
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers.insert("accept", HeaderValue::from_static("application/json"));

    // 客户端标识
    headers.insert("x-client-app", HeaderValue::from_static("claude-code"));

    headers
}

/// 构建带遥测的请求头
pub fn build_headers_with_telemetry(
    api_key: &str,
    api_version: &str,
    request_id: &str,
) -> HeaderMap {
    let mut headers = build_headers(api_key, api_version);

    // 请求 ID
    headers.insert(
        "x-client-request-id",
        HeaderValue::from_str(request_id).unwrap(),
    );

    // 遥测数据（可选，可根据需要实现）
    // headers.insert("x-client-current-telemetry", ...);
    // headers.insert("x-client-last-telemetry", ...);

    headers
}

/// 构建 Beta 功能请求头
pub fn build_headers_with_beta(
    api_key: &str,
    api_version: &str,
    beta_features: &[&str],
) -> HeaderMap {
    let mut headers = build_headers(api_key, api_version);

    if !beta_features.is_empty() {
        let beta = beta_features.join(",");
        headers.insert("anthropic-beta", HeaderValue::from_str(&beta).unwrap());
    }

    headers
}

/// User-Agent 配置
pub fn get_user_agent() -> String {
    "claude-code-shell/0.1.0 (FoxNIO)".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_headers() {
        let headers = build_headers("test-key", "2023-06-01");

        assert!(headers.contains_key("x-api-key"));
        assert!(headers.contains_key("anthropic-version"));
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("accept"));
        assert!(headers.contains_key("x-client-app"));
    }

    #[test]
    fn test_build_headers_with_telemetry() {
        let headers = build_headers_with_telemetry("test-key", "2023-06-01", "req-123");

        assert!(headers.contains_key("x-client-request-id"));
    }

    #[test]
    fn test_build_headers_with_beta() {
        let headers =
            build_headers_with_beta("test-key", "2023-06-01", &["feature-1", "feature-2"]);

        let beta = headers.get("anthropic-beta").unwrap();
        assert_eq!(beta, "feature-1,feature-2");
    }
}
