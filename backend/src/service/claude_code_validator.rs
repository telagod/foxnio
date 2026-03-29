use serde::{Deserialize, Serialize};

/// Code validator for Claude API requests
pub struct ClaudeCodeValidator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<CodeIssue>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIssue {
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid code: {0}")]
    InvalidCode(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl ClaudeCodeValidator {
    /// Validate code
    pub fn validate(code: &str, language: &str) -> ValidationResult {
        let mut issues = Vec::new();
        let mut suggestions = Vec::new();

        // Basic validation
        let lines: Vec<&str> = code.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            // Check line length
            if line.len() > 120 {
                issues.push(CodeIssue {
                    line: idx + 1,
                    column: 120,
                    severity: Severity::Warning,
                    message: "Line exceeds 120 characters".to_string(),
                });
                suggestions.push("Consider breaking this line for better readability".to_string());
            }
        }

        // Language-specific validation
        match language {
            "python" => {
                // Check for common Python issues
                if code.contains("print ") && !code.contains("print(") {
                    issues.push(CodeIssue {
                        line: 0,
                        column: 0,
                        severity: Severity::Error,
                        message: "Python 3 requires print() function".to_string(),
                    });
                }
            }
            "javascript" | "typescript" => {
                // Check for console.log
                if code.contains("console.log") {
                    issues.push(CodeIssue {
                        line: 0,
                        column: 0,
                        severity: Severity::Info,
                        message: "Remove console.log statements in production".to_string(),
                    });
                }
            }
            _ => {}
        }

        ValidationResult {
            is_valid: issues.iter().all(|i| i.severity != Severity::Error),
            issues,
            suggestions,
        }
    }

    /// Detect code language
    pub fn detect_language(code: &str) -> Option<String> {
        if code.contains("def ") && code.contains(":") {
            return Some("python".to_string());
        }
        if code.contains("function") && code.contains("{") {
            return Some("javascript".to_string());
        }
        if code.contains("fn ") && code.contains("->") {
            return Some("rust".to_string());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_code() {
        let code = "def hello():\n    print('hello')";
        let result = ClaudeCodeValidator::validate(code, "python");

        assert!(result.is_valid);
    }

    #[test]
    fn test_detect_language() {
        let python_code = "def hello():\n    pass";
        assert_eq!(
            ClaudeCodeValidator::detect_language(python_code),
            Some("python".to_string())
        );

        let js_code = "function hello() { return 1; }";
        assert_eq!(
            ClaudeCodeValidator::detect_language(js_code),
            Some("javascript".to_string())
        );
    }
}
