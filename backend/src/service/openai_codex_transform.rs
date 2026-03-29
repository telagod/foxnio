use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Transform service for OpenAI Codex API requests
pub struct OpenAICodexTransform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CodexChoice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexChoice {
    pub text: String,
    pub index: u32,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("Invalid request format: {0}")]
    InvalidRequest(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

impl OpenAICodexTransform {
    /// Transform chat completions to Codex format
    pub fn transform_chat_to_codex(chat_request: &Value) -> Result<CodexRequest, TransformError> {
        let messages = chat_request
            .get("messages")
            .and_then(|m| m.as_array())
            .ok_or_else(|| TransformError::MissingField("messages".to_string()))?;

        // Convert messages to prompt
        let mut prompt = String::new();
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            prompt.push_str(&format!("{}: {}\n", role.to_uppercase(), content));
        }

        Ok(CodexRequest {
            model: chat_request
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("code-davinci-002")
                .to_string(),
            prompt,
            max_tokens: chat_request
                .get("max_tokens")
                .and_then(|t| t.as_u64())
                .map(|t| t as u32),
            temperature: chat_request
                .get("temperature")
                .and_then(|t| t.as_f64())
                .map(|t| t as f32),
            top_p: chat_request
                .get("top_p")
                .and_then(|p| p.as_f64())
                .map(|p| p as f32),
            n: chat_request
                .get("n")
                .and_then(|n| n.as_u64())
                .map(|n| n as u32),
            stop: chat_request.get("stop").and_then(|s| {
                s.as_str().map(|s| vec![s.to_string()]).or_else(|| {
                    s.as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                })
            }),
            echo: chat_request.get("echo").and_then(|e| e.as_bool()),
        })
    }

    /// Transform Codex response to chat completions format
    pub fn transform_codex_to_chat(
        codex_response: &CodexResponse,
        model: &str,
    ) -> Result<Value, TransformError> {
        let choices: Vec<Value> = codex_response
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                serde_json::json!({
                    "index": idx,
                    "message": {
                        "role": "assistant",
                        "content": choice.text
                    },
                    "finish_reason": choice.finish_reason
                })
            })
            .collect();

        Ok(serde_json::json!({
            "id": codex_response.id,
            "object": "chat.completion",
            "created": codex_response.created,
            "model": model,
            "choices": choices,
            "usage": codex_response.usage
        }))
    }

    /// Extract code from response
    pub fn extract_code(response: &CodexResponse) -> Option<String> {
        response.choices.first().map(|choice| {
            // Try to extract code blocks
            if let Some(start) = choice.text.find("```") {
                if let Some(end) = choice.text[start + 3..].find("```") {
                    return choice.text[start + 3..start + 3 + end].trim().to_string();
                }
            }
            choice.text.clone()
        })
    }

    /// Validate Codex request
    pub fn validate_request(request: &CodexRequest) -> Result<(), TransformError> {
        if request.prompt.is_empty() {
            return Err(TransformError::InvalidRequest(
                "Prompt cannot be empty".to_string(),
            ));
        }

        if let Some(temp) = request.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err(TransformError::InvalidRequest(
                    "Temperature must be between 0 and 2".to_string(),
                ));
            }
        }

        if let Some(top_p) = request.top_p {
            if top_p < 0.0 || top_p > 1.0 {
                return Err(TransformError::InvalidRequest(
                    "Top P must be between 0 and 1".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Add system prompt to request
    pub fn add_system_prompt(request: &mut CodexRequest, system_prompt: &str) {
        request.prompt = format!("{}\n\n{}", system_prompt, request.prompt);
    }

    /// Estimate tokens for request
    pub fn estimate_tokens(request: &CodexRequest) -> u32 {
        // Simple estimation: ~4 characters per token
        (request.prompt.len() / 4) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_chat_to_codex() {
        let chat_request = serde_json::json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Write a hello world program"}
            ]
        });

        let codex_request = OpenAICodexTransform::transform_chat_to_codex(&chat_request).unwrap();

        assert!(codex_request.prompt.contains("USER:"));
    }

    #[test]
    fn test_extract_code() {
        let response = CodexResponse {
            id: "test".to_string(),
            object: "text_completion".to_string(),
            created: 0,
            model: "code-davinci-002".to_string(),
            choices: vec![CodexChoice {
                text: "```python\nprint('hello')\n```".to_string(),
                index: 0,
                finish_reason: Some("stop".to_string()),
            }],
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
        };

        let code = OpenAICodexTransform::extract_code(&response);
        assert!(code.is_some());
    }
}
