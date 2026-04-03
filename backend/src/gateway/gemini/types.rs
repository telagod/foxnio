//! Gemini Native API 类型定义
//!
//! 提供 Gemini SDK 兼容的请求/响应格式

use serde::{Deserialize, Serialize};

// ============ 模型相关 ============

/// Gemini 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModel {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_generation_methods: Option<Vec<String>>,
}

/// 模型列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModelsListResponse {
    pub models: Vec<GeminiModel>,
}

impl GeminiModelsListResponse {
    /// 默认模型列表
    pub fn default_models() -> Self {
        let methods = vec![
            "generateContent".to_string(),
            "streamGenerateContent".to_string(),
        ];

        let models = vec![
            GeminiModel {
                name: "models/gemini-2.0-flash".to_string(),
                display_name: Some("Gemini 2.0 Flash".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-2.5-flash".to_string(),
                display_name: Some("Gemini 2.5 Flash".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-2.5-flash-image".to_string(),
                display_name: Some("Gemini 2.5 Flash Image".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-2.5-pro".to_string(),
                display_name: Some("Gemini 2.5 Pro".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-3-flash-preview".to_string(),
                display_name: Some("Gemini 3 Flash Preview".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-3-pro-preview".to_string(),
                display_name: Some("Gemini 3 Pro Preview".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-3.1-pro-preview".to_string(),
                display_name: Some("Gemini 3.1 Pro Preview".to_string()),
                description: None,
                supported_generation_methods: Some(methods.clone()),
            },
            GeminiModel {
                name: "models/gemini-3.1-flash-image".to_string(),
                display_name: Some("Gemini 3.1 Flash Image".to_string()),
                description: None,
                supported_generation_methods: Some(methods),
            },
        ];

        Self { models }
    }
}

// ============ 内容生成请求 ============

/// Gemini 生成内容请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateContentRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content: Option<String>,
}

/// 内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: Option<String>,
    pub parts: Vec<Part>,
}

/// 部分（文本、图片等）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_data: Option<FileData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_code: Option<ExecutableCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution_result: Option<CodeExecutionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

/// 内联数据（图片等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

/// 文件数据引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub mime_type: String,
    pub file_uri: String,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

/// 函数响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// 可执行代码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableCode {
    pub language: String,
    pub code: String,
}

/// 代码执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutionResult {
    pub outcome: String,
    pub output: String,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_declarations: Option<Vec<FunctionDeclaration>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search_retrieval: Option<serde_json::Value>,
}

/// 函数声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

/// 工具配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub function_calling_config: Option<FunctionCallingConfig>,
}

/// 函数调用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallingConfig {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

/// 安全设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

/// 生成配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modalities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

/// 思考配置（用于 Gemini 2.5+ 扩展思考）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

// ============ 内容生成响应 ============

/// Gemini 生成内容响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateContentResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<Candidate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<PromptFeedback>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "usageMetadata")]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

/// 候选响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_metadata: Option<GroundingMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_metadata: Option<CitationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

/// 安全评级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
}

/// 提示反馈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFeedback {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// 使用量元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: i32,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: i32,
    #[serde(rename = "totalTokenCount")]
    pub total_token_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thoughts_token_count: Option<i32>,
}

/// Grounding 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunks: Option<Vec<GroundingChunk>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_supports: Option<Vec<GroundingSupport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_queries: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_entry_point: Option<SearchEntryPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_metadata: Option<RetrievalMetadata>,
}

/// Grounding 块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingChunk {
    pub web: Option<WebChunk>,
}

/// Web 块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebChunk {
    pub uri: String,
    pub title: String,
}

/// Grounding 支持
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingSupport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grounding_chunk_indices: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_scores: Option<Vec<f32>>,
    pub segment: Option<Segment>,
}

/// 文本段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub part_index: i32,
    pub start_index: i32,
    pub end_index: i32,
    pub text: String,
}

/// 搜索入口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEntryPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sblob: Option<String>,
}

/// 检索元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetadata {
    pub google_search_dynamic_retrieval_score: Option<f32>,
}

/// 引用元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationMetadata {
    pub citation_sources: Vec<CitationSource>,
}

/// 引用来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

// ============ 错误响应 ============

/// Gemini 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiErrorResponse {
    pub error: GeminiError,
}

/// Gemini 错误详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiError {
    pub code: i32,
    pub message: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<serde_json::Value>>,
}

// ============ 流式响应 ============

/// 流式响应块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<Candidate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

// ============ 辅助函数 ============

/// 创建简单的文本内容
pub fn text_content(role: &str, text: &str) -> Content {
    Content {
        role: Some(role.to_string()),
        parts: vec![Part {
            text: Some(text.to_string()),
            ..Default::default()
        }],
    }
}

/// 创建用户消息
pub fn user_message(text: &str) -> Content {
    text_content("user", text)
}

/// 创建模型消息
pub fn model_message(text: &str) -> Content {
    text_content("model", text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_models() {
        let response = GeminiModelsListResponse::default_models();
        assert!(!response.models.is_empty());
        assert!(response
            .models
            .iter()
            .any(|m| m.name.contains("gemini-2.0-flash")));
    }

    #[test]
    fn test_text_content() {
        let content = user_message("Hello");
        assert_eq!(content.role, Some("user".to_string()));
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts[0].text, Some("Hello".to_string()));
    }

    #[test]
    fn test_serialize_request() {
        let request = GenerateContentRequest {
            contents: vec![user_message("Hello")],
            tools: None,
            tool_config: None,
            safety_settings: None,
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                max_output_tokens: Some(1024),
                ..Default::default()
            }),
            system_instruction: None,
            cached_content: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("temperature"));
    }

    #[test]
    fn test_deserialize_response() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello, how can I help you?"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 5,
                "candidatesTokenCount": 10,
                "totalTokenCount": 15
            }
        }"#;

        let response: GenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(response.candidates.is_some());
        let candidates = response.candidates.unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(response.usage_metadata.is_some());
    }
}
