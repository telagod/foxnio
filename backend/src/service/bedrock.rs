//! AWS Bedrock 平台支持
//!
//! 实现 AWS Signature V4 签名和 Bedrock API 调用

use anyhow::{bail, Context, Result};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// AWS Bedrock 配置
#[derive(Debug, Clone)]
pub struct BedrockConfig {
    /// AWS Access Key ID
    pub access_key_id: String,
    /// AWS Secret Access Key
    pub secret_access_key: String,
    /// AWS Session Token (可选，用于临时凭证)
    pub session_token: Option<String>,
    /// AWS Region
    pub region: String,
    /// 是否强制使用全局端点
    pub force_global: bool,
}

impl BedrockConfig {
    /// 创建新的配置
    pub fn new(access_key_id: String, secret_access_key: String, region: String) -> Self {
        Self {
            access_key_id,
            secret_access_key,
            session_token: None,
            region,
            force_global: false,
        }
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID")
            .context("AWS_ACCESS_KEY_ID environment variable not set")?;
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .context("AWS_SECRET_ACCESS_KEY environment variable not set")?;

        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let force_global = std::env::var("AWS_FORCE_GLOBAL")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Ok(Self {
            access_key_id,
            secret_access_key,
            session_token,
            region,
            force_global,
        })
    }

    /// 设置会话令牌
    pub fn with_session_token(mut self, token: String) -> Self {
        self.session_token = Some(token);
        self
    }

    /// 设置强制全局端点
    pub fn with_force_global(mut self, force: bool) -> Self {
        self.force_global = force;
        self
    }

    /// 获取 Bedrock Runtime 端点
    pub fn runtime_endpoint(&self) -> String {
        if self.force_global {
            "https://bedrock-runtime.global.amazonaws.com".to_string()
        } else {
            format!("https://bedrock-runtime.{}.amazonaws.com", self.region)
        }
    }
}

/// AWS Signature V4 签名器
pub struct AwsSignatureV4 {
    config: BedrockConfig,
}

impl AwsSignatureV4 {
    /// 创建新的签名器
    pub fn new(config: BedrockConfig) -> Self {
        Self { config }
    }

    /// 生成签名
    pub fn sign(
        &self,
        method: &str,
        uri: &str,
        query: &str,
        headers: &mut Vec<(String, String)>,
        payload_hash: &str,
    ) -> Result<String> {
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        // 添加必需的 headers
        headers.push(("X-Amz-Date".to_string(), amz_date.clone()));
        if let Some(ref token) = self.config.session_token {
            headers.push(("X-Amz-Security-Token".to_string(), token.clone()));
        }

        // 创建规范请求
        let canonical_headers = Self::create_canonical_headers(headers);
        let signed_headers = Self::create_signed_headers(headers);

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, uri, query, canonical_headers, signed_headers, payload_hash
        );

        // 创建待签字符串
        let credential_scope = format!("{date_stamp}/bedrock/aws4_request");

        let canonical_request_hash = Self::hash(&canonical_request);

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date, credential_scope, canonical_request_hash
        );

        // 计算签名
        let signing_key = Self::get_signature_key(
            &self.config.secret_access_key,
            &date_stamp,
            &self.config.region,
            "bedrock",
        )?;

        let signature = Self::hmac_sha256_hex(&signing_key, &string_to_sign)?;

        // 构建授权头
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.config.access_key_id, credential_scope, signed_headers, signature
        );

        Ok(authorization)
    }

    /// 创建规范 headers
    fn create_canonical_headers(headers: &[(String, String)]) -> String {
        let mut sorted_headers: Vec<_> = headers.to_vec();
        sorted_headers.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        sorted_headers
            .iter()
            .map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v.trim()))
            .collect()
    }

    /// 创建已签名的 headers 列表
    fn create_signed_headers(headers: &[(String, String)]) -> String {
        let mut sorted_names: Vec<_> = headers.iter().map(|(k, _)| k.to_lowercase()).collect();
        sorted_names.sort();
        sorted_names.join(";")
    }

    /// SHA256 哈希
    fn hash(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// HMAC-SHA256
    fn hmac_sha256(key: &[u8], data: &str) -> Result<Vec<u8>> {
        let mut mac = HmacSha256::new_from_slice(key).context("Failed to create HMAC")?;
        mac.update(data.as_bytes());
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// HMAC-SHA256 十六进制输出
    fn hmac_sha256_hex(key: &[u8], data: &str) -> Result<String> {
        let result = Self::hmac_sha256(key, data)?;
        Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
    }

    /// 获取签名密钥
    fn get_signature_key(
        key: &str,
        date_stamp: &str,
        region: &str,
        service: &str,
    ) -> Result<Vec<u8>> {
        let k_date = Self::hmac_sha256(format!("AWS4{key}").as_bytes(), date_stamp)?;
        let k_region = Self::hmac_sha256(&k_date, region)?;
        let k_service = Self::hmac_sha256(&k_region, service)?;
        let k_signing = Self::hmac_sha256(&k_service, "aws4_request")?;
        Ok(k_signing)
    }
}

/// Bedrock API 客户端
pub struct BedrockClient {
    config: BedrockConfig,
    http_client: Client,
    signer: AwsSignatureV4,
}

impl BedrockClient {
    /// 创建新的客户端
    pub fn new(config: BedrockConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        let signer = AwsSignatureV4::new(config.clone());

        Ok(Self {
            config,
            http_client,
            signer,
        })
    }

    /// 调用 Bedrock InvokeModel API
    pub async fn invoke_model(
        &self,
        model_id: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let endpoint = self.config.runtime_endpoint();
        let uri = format!("/model/{model_id}/invoke");
        let url = format!("{endpoint}{uri}");

        let body_str = serde_json::to_string(body)?;
        let payload_hash = AwsSignatureV4::hash(&body_str);

        let mut headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            (
                "Host".to_string(),
                url::Url::parse(&endpoint)
                    .context("Invalid endpoint URL")?
                    .host_str()
                    .context("No host in endpoint URL")?
                    .to_string(),
            ),
        ];

        let authorization = self
            .signer
            .sign("POST", &uri, "", &mut headers, &payload_hash)?;
        headers.push(("Authorization".to_string(), authorization));

        let mut request = self.http_client.post(&url);

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = request
            .body(body_str)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Bedrock API call failed: {} - {}", status, body);
        }

        let result: serde_json::Value = response.json().await?;
        Ok(result)
    }

    /// 调用 Bedrock InvokeModelWithResponseStream API（流式响应）
    pub async fn invoke_model_stream(
        &self,
        model_id: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let endpoint = self.config.runtime_endpoint();
        let uri = format!("/model/{model_id}/invoke-with-response-stream");
        let url = format!("{endpoint}{uri}");

        let body_str = serde_json::to_string(body)?;
        let payload_hash = AwsSignatureV4::hash(&body_str);

        let mut headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            (
                "Accept".to_string(),
                "application/vnd.amazon.eventstream".to_string(),
            ),
            (
                "Host".to_string(),
                url::Url::parse(&endpoint)
                    .context("Invalid endpoint URL")?
                    .host_str()
                    .context("No host in endpoint URL")?
                    .to_string(),
            ),
        ];

        let authorization = self
            .signer
            .sign("POST", &uri, "", &mut headers, &payload_hash)?;
        headers.push(("Authorization".to_string(), authorization));

        let mut request = self.http_client.post(&url);

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = request
            .body(body_str)
            .send()
            .await
            .context("Failed to send streaming request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Bedrock streaming API call failed: {} - {}", status, body);
        }

        Ok(response)
    }

    /// 模型 ID 区域前缀调整
    pub fn adjust_model_region_prefix(&self, model_id: &str) -> String {
        let prefix = Self::get_cross_region_prefix(&self.config.region);

        for p in &["us.", "eu.", "apac.", "jp.", "au.", "us-gov.", "global."] {
            if model_id.starts_with(p) {
                if *p == format!("{prefix}.") {
                    return model_id.to_string();
                }
                return format!("{}.{}", prefix, model_id.strip_prefix(p).unwrap());
            }
        }

        model_id.to_string()
    }

    /// 获取跨区域前缀
    fn get_cross_region_prefix(region: &str) -> &str {
        if region.starts_with("us-gov") {
            "us-gov"
        } else if region.starts_with("us-") {
            "us"
        } else if region.starts_with("eu-") {
            "eu"
        } else if region == "ap-northeast-1" {
            "jp"
        } else if region == "ap-southeast-2" {
            "au"
        } else if region.starts_with("ap-") {
            "apac"
        } else {
            "us"
        }
    }
}

/// Bedrock 模型映射
pub mod model_mapping {
    use std::collections::HashMap;

    /// 默认 Bedrock 模型映射
    pub fn default_mapping() -> HashMap<&'static str, &'static str> {
        let mut map = HashMap::new();

        // Claude 模型
        map.insert("claude-3-opus", "anthropic.claude-3-opus-20240229-v1:0");
        map.insert("claude-3-sonnet", "anthropic.claude-3-sonnet-20240229-v1:0");
        map.insert("claude-3-haiku", "anthropic.claude-3-haiku-20240307-v1:0");
        map.insert(
            "claude-3-5-sonnet",
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
        );
        map.insert(
            "claude-3-5-haiku",
            "anthropic.claude-3-5-haiku-20241022-v1:0",
        );

        // Amazon 模型
        map.insert("titan-text", "amazon.titan-text-premier-v1:0");
        map.insert("titan-embed", "amazon.titan-embed-text-v2:0");

        // Meta 模型
        map.insert("llama3-70b", "meta.llama3-70b-instruct-v1:0");
        map.insert("llama3-8b", "meta.llama3-8b-instruct-v1:0");

        // Mistral 模型
        map.insert("mistral-large", "mistral.mistral-large-2402-v1:0");
        map.insert("mistral-small", "mistral.mistral-small-2402-v1:0");

        // Cohere 模型
        map.insert("command-r", "cohere.command-r-v1:0");
        map.insert("command-r-plus", "cohere.command-r-plus-v1:0");

        // AI21 模型
        map.insert("jamba", "ai21.jamba-1-5-large-v1:0");

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = BedrockConfig::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            "us-east-1".to_string(),
        );

        assert_eq!(config.access_key_id, "test_key");
        assert_eq!(config.region, "us-east-1");
        assert!(!config.force_global);
    }

    #[test]
    fn test_runtime_endpoint() {
        let config = BedrockConfig::new(
            "test_key".to_string(),
            "test_secret".to_string(),
            "us-west-2".to_string(),
        );

        assert_eq!(
            config.runtime_endpoint(),
            "https://bedrock-runtime.us-west-2.amazonaws.com"
        );

        let config_global = config.with_force_global(true);
        assert_eq!(
            config_global.runtime_endpoint(),
            "https://bedrock-runtime.global.amazonaws.com"
        );
    }

    #[test]
    fn test_hash() {
        let data = "test data";
        let hash = AwsSignatureV4::hash(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars
    }

    #[test]
    fn test_cross_region_prefix() {
        assert_eq!(BedrockClient::get_cross_region_prefix("us-east-1"), "us");
        assert_eq!(BedrockClient::get_cross_region_prefix("eu-west-1"), "eu");
        assert_eq!(
            BedrockClient::get_cross_region_prefix("ap-northeast-1"),
            "jp"
        );
        assert_eq!(
            BedrockClient::get_cross_region_prefix("ap-southeast-2"),
            "au"
        );
        assert_eq!(BedrockClient::get_cross_region_prefix("ap-south-1"), "apac");
    }

    #[test]
    fn test_model_mapping() {
        let mapping = model_mapping::default_mapping();

        assert_eq!(
            mapping.get("claude-3-opus"),
            Some(&"anthropic.claude-3-opus-20240229-v1:0")
        );
        assert_eq!(
            mapping.get("claude-3-5-sonnet"),
            Some(&"anthropic.claude-3-5-sonnet-20241022-v2:0")
        );
    }
}
