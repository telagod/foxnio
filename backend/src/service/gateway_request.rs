//! Gateway 请求处理服务
//!
//! 处理网关层的请求解析、验证和预处理

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// 解析后的请求
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub method: String,
    pub path: String,
    pub model: String,
    pub stream: bool,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub content_type: String,
    pub user_id: Option<String>,
    pub api_key: Option<String>,
    pub organization: Option<String>,
}

/// 请求元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

/// 请求验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

/// 模型路由信息
#[derive(Debug, Clone)]
pub struct ModelRoute {
    pub original_model: String,
    pub mapped_model: String,
    pub provider: String,
    pub account_type: String,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
}

/// Gateway 请求服务
pub struct GatewayRequestService {
    max_body_size: usize,
    allowed_origins: Vec<String>,
}

impl GatewayRequestService {
    pub fn new(max_body_size: usize, allowed_origins: Vec<String>) -> Self {
        Self {
            max_body_size,
            allowed_origins,
        }
    }

    /// 解析请求
    pub fn parse_request(
        &self,
        method: &str,
        path: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<ParsedRequest> {
        // 验证请求体大小
        if body.len() > self.max_body_size {
            return Err(anyhow!("Request body too large: {} bytes", body.len()));
        }

        // 解析 Content-Type
        let content_type = headers
            .get("content-type")
            .cloned()
            .unwrap_or_else(|| "application/json".to_string());

        // 解析模型
        let model = self.extract_model(&body, path)?;

        // 解析是否流式
        let stream = self.extract_stream_flag(&body);

        // 提取认证信息
        let api_key = self.extract_api_key(&headers);
        let organization = headers.get("openai-organization").cloned();
        let user_id = headers.get("x-user-id").cloned();

        Ok(ParsedRequest {
            method: method.to_string(),
            path: path.to_string(),
            model,
            stream,
            headers,
            body,
            content_type,
            user_id,
            api_key,
            organization,
        })
    }

    /// 验证请求
    pub fn validate_request(&self, parsed: &ParsedRequest) -> ValidationResult {
        let mut warnings = Vec::new();

        // 验证方法
        if parsed.method != "POST" {
            return ValidationResult {
                is_valid: false,
                error: Some(format!("Method {} not allowed", parsed.method)),
                warnings,
            };
        }

        // 验证路径
        if !parsed.path.starts_with("/v1/") {
            warnings.push(format!("Non-standard API path: {}", parsed.path));
        }

        // 验证模型
        if parsed.model.is_empty() {
            return ValidationResult {
                is_valid: false,
                error: Some("Model not specified".to_string()),
                warnings,
            };
        }

        // 验证 API Key
        if parsed.api_key.is_none() {
            warnings.push("No API key provided".to_string());
        }

        // 验证请求体
        if parsed.body.is_empty() {
            return ValidationResult {
                is_valid: false,
                error: Some("Empty request body".to_string()),
                warnings,
            };
        }

        // 验证 JSON 格式
        if parsed.content_type.contains("application/json") {
            if let Err(e) = serde_json::from_slice::<JsonValue>(&parsed.body) {
                return ValidationResult {
                    is_valid: false,
                    error: Some(format!("Invalid JSON: {e}")),
                    warnings,
                };
            }
        }

        ValidationResult {
            is_valid: true,
            error: None,
            warnings,
        }
    }

    /// 路由模型
    pub fn route_model(&self, parsed: &ParsedRequest) -> ModelRoute {
        let original_model = parsed.model.clone();
        let mapped_model = self.map_model(&original_model);
        let provider = self.detect_provider(&mapped_model);

        ModelRoute {
            original_model,
            mapped_model: mapped_model.clone(),
            provider,
            account_type: "api_key".to_string(),
            supports_streaming: true,
            supports_tools: true,
            supports_vision: self.supports_vision(&mapped_model),
        }
    }

    /// 从请求体或路径提取模型名
    fn extract_model(&self, body: &[u8], path: &str) -> Result<String> {
        // 尝试从请求体解析
        if let Ok(json) = serde_json::from_slice::<JsonValue>(body) {
            if let Some(model) = json.get("model").and_then(|m| m.as_str()) {
                return Ok(model.to_string());
            }
        }

        // 从路径提取
        let segments: Vec<&str> = path.split('/').collect();
        if let Some(idx) = segments.iter().position(|s| *s == "models") {
            if let Some(model) = segments.get(idx + 1) {
                return Ok(model.to_string());
            }
        }

        Ok(String::new())
    }

    /// 提取流式标志
    fn extract_stream_flag(&self, body: &[u8]) -> bool {
        if let Ok(json) = serde_json::from_slice::<JsonValue>(body) {
            return json
                .get("stream")
                .and_then(|s| s.as_bool())
                .unwrap_or(false);
        }
        false
    }

    /// 提取 API Key
    fn extract_api_key(&self, headers: &HashMap<String, String>) -> Option<String> {
        // 尝试 Authorization header
        if let Some(auth) = headers.get("authorization") {
            if let Some(stripped) = auth.strip_prefix("Bearer ") {
                return Some(stripped.to_string());
            }
            return Some(auth.clone());
        }

        // 尝试 X-API-Key header
        headers.get("x-api-key").cloned()
    }

    /// 映射模型名
    fn map_model(&self, model: &str) -> String {
        // 模型别名映射
        let mappings = [
            ("gpt-4-turbo", "gpt-4-turbo-preview"),
            ("gpt-4-vision", "gpt-4-vision-preview"),
            ("claude-instant", "claude-3-haiku"),
        ];

        for (alias, target) in mappings {
            if model == alias || model.starts_with(&format!("{alias}-")) {
                return target.to_string();
            }
        }

        model.to_string()
    }

    /// 检测 Provider
    fn detect_provider(&self, model: &str) -> String {
        if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
            "openai".to_string()
        } else if model.starts_with("claude") {
            "anthropic".to_string()
        } else if model.starts_with("gemini") {
            "gemini".to_string()
        } else if model.starts_with("bedrock") {
            "bedrock".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// 检查是否支持视觉
    fn supports_vision(&self, model: &str) -> bool {
        let vision_models = [
            "gpt-4-vision",
            "gpt-4-turbo",
            "gpt-4o",
            "claude-3",
            "gemini-pro-vision",
        ];

        vision_models.iter().any(|m| model.contains(m))
    }

    /// 生成请求元数据
    pub fn generate_metadata(&self, headers: &HashMap<String, String>) -> RequestMetadata {
        RequestMetadata {
            request_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            client_ip: headers
                .get("x-forwarded-for")
                .or_else(|| headers.get("x-real-ip"))
                .cloned(),
            user_agent: headers.get("user-agent").cloned(),
            referer: headers.get("referer").cloned(),
        }
    }

    /// 检查 CORS
    pub fn check_cors(&self, origin: Option<&str>) -> bool {
        if self.allowed_origins.is_empty() {
            return true; // 允许所有来源
        }

        if let Some(origin) = origin {
            return self
                .allowed_origins
                .iter()
                .any(|o| o == "*" || o == origin || origin.ends_with(&format!(".{o}")));
        }

        false
    }
}

impl Default for GatewayRequestService {
    fn default() -> Self {
        Self::new(10 * 1024 * 1024, vec![]) // 10MB 默认限制
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let service = GatewayRequestService::default();
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("authorization".to_string(), "Bearer test-key".to_string());

        let body = br#"{"model": "gpt-4", "messages": [], "stream": true}"#;

        let parsed = service
            .parse_request("POST", "/v1/chat/completions", headers, body.to_vec())
            .unwrap();
        assert_eq!(parsed.model, "gpt-4");
        assert!(parsed.stream);
        assert_eq!(parsed.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_validate_request() {
        let service = GatewayRequestService::default();
        let parsed = ParsedRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: "gpt-4".to_string(),
            stream: false,
            headers: HashMap::new(),
            body: br#"{"model": "gpt-4"}"#.to_vec(),
            content_type: "application/json".to_string(),
            user_id: None,
            api_key: Some("key".to_string()),
            organization: None,
        };

        let result = service.validate_request(&parsed);
        assert!(result.is_valid);
    }

    #[test]
    fn test_route_model() {
        let service = GatewayRequestService::default();
        let parsed = ParsedRequest {
            method: "POST".to_string(),
            path: "/v1/chat/completions".to_string(),
            model: "gpt-4".to_string(),
            stream: false,
            headers: HashMap::new(),
            body: vec![],
            content_type: "application/json".to_string(),
            user_id: None,
            api_key: None,
            organization: None,
        };

        let route = service.route_model(&parsed);
        assert_eq!(route.provider, "openai");
    }

    #[test]
    fn test_detect_provider() {
        let service = GatewayRequestService::default();
        assert_eq!(service.detect_provider("gpt-4"), "openai");
        assert_eq!(service.detect_provider("claude-3-opus"), "anthropic");
        assert_eq!(service.detect_provider("gemini-pro"), "gemini");
    }
}
