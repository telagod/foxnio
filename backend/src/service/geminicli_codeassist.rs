use serde::{Deserialize, Serialize};

/// Code assistance service for Gemini CLI
pub struct GeminicliCodeassist;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAssistRequest {
    pub prompt: String,
    pub language: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAssistResponse {
    pub code: String,
    pub explanation: Option<String>,
    pub language: String,
    pub tokens_used: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeContext {
    pub file_path: Option<String>,
    pub selected_text: Option<String>,
    pub surrounding_code: Option<String>,
    pub cursor_position: Option<(usize, usize)>,
}

#[derive(Debug, thiserror::Error)]
pub enum CodeAssistError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Code generation failed: {0}")]
    GenerationFailed(String),
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

impl GeminicliCodeassist {
    /// Generate code completion
    pub fn generate_completion(
        request: &CodeAssistRequest,
        context: Option<&CodeContext>,
    ) -> Result<CodeAssistResponse, CodeAssistError> {
        let mut prompt = request.prompt.clone();

        // Add context if available
        if let Some(ctx) = context {
            if let Some(code) = &ctx.surrounding_code {
                prompt = format!("Context:\n{code}\n\nRequest: {prompt}");
            }
        }

        // Add language hint
        if let Some(lang) = &request.language {
            let _prompt = format!("Language: {lang}\n\n{prompt}");
        }

        // In real implementation, this would call Gemini API
        Ok(CodeAssistResponse {
            code: "// Generated code placeholder".to_string(),
            explanation: Some("This is a placeholder response".to_string()),
            language: request
                .language
                .clone()
                .unwrap_or_else(|| "text".to_string()),
            tokens_used: 0,
        })
    }

    /// Extract code from response
    pub fn extract_code(response: &str) -> Option<String> {
        // Try to extract code blocks
        if let Some(start) = response.find("```") {
            let rest = &response[start + 3..];
            if let Some(end) = rest.find("```") {
                // Skip language identifier if present
                let code_start = rest.find('\n').unwrap_or(0);
                return Some(rest[code_start..end].trim().to_string());
            }
        }
        None
    }

    /// Build context from file
    pub fn build_context(
        file_path: &str,
        content: &str,
        cursor_line: usize,
        cursor_col: usize,
    ) -> CodeContext {
        let lines: Vec<&str> = content.lines().collect();
        let start_line = cursor_line.saturating_sub(10);
        let end_line = (cursor_line + 10).min(lines.len());

        let surrounding_code = lines[start_line..end_line].join("\n");

        let selected_text = None; // Would be filled from editor selection

        CodeContext {
            file_path: Some(file_path.to_string()),
            selected_text,
            surrounding_code: Some(surrounding_code),
            cursor_position: Some((cursor_line, cursor_col)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code() {
        let response = "Here's the code:\n```python\nprint('hello')\n```\nDone";
        let code = GeminicliCodeassist::extract_code(response);
        assert!(code.is_some());
        assert!(code.unwrap().contains("print"));
    }
}
