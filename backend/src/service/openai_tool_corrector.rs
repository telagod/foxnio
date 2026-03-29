use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tool call corrector for fixing malformed or invalid tool calls
pub struct OpenAIToolCorrector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub original: Value,
    pub corrected: Value,
    pub correction_type: CorrectionType,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CorrectionType {
    JsonSyntax,
    MissingField,
    InvalidType,
    ParameterRename,
    ValueNormalization,
}

#[derive(Debug, thiserror::Error)]
pub enum CorrectionError {
    #[error("Invalid tool call format")]
    InvalidFormat,
    #[error("Failed to parse JSON: {0}")]
    ParseError(String),
    #[error("Correction failed: {0}")]
    CorrectionFailed(String),
}

impl OpenAIToolCorrector {
    /// Correct tool call arguments
    pub fn correct_arguments(
        tool_call: &Value,
        schema: Option<&Value>,
    ) -> Result<Option<Correction>, CorrectionError> {
        let function = tool_call
            .get("function")
            .ok_or(CorrectionError::InvalidFormat)?;

        let args_str = function
            .get("arguments")
            .and_then(|a| a.as_str())
            .ok_or(CorrectionError::InvalidFormat)?;

        // Try to parse JSON
        let args: Value = match serde_json::from_str(args_str) {
            Ok(v) => v,
            Err(_) => {
                // Try to fix common JSON errors
                return Self::fix_json_and_correct(args_str, schema);
            }
        };

        // Validate against schema if provided
        if let Some(schema) = schema {
            if let Some(correction) = Self::validate_and_correct(&args, schema)? {
                return Ok(Some(correction));
            }
        }

        Ok(None)
    }

    /// Fix JSON syntax errors and correct
    fn fix_json_and_correct(
        args_str: &str,
        schema: Option<&Value>,
    ) -> Result<Option<Correction>, CorrectionError> {
        let mut corrected = args_str.to_string();

        // Fix missing quotes around keys
        corrected = corrected.replace(":", "\":");
        corrected = corrected.replace(", ", ", \"");

        // Fix trailing commas
        corrected = corrected.replace(", }", "}");
        corrected = corrected.replace(", ]", "]");

        // Try to parse again
        let args: Value = serde_json::from_str(&corrected)
            .map_err(|e| CorrectionError::ParseError(e.to_string()))?;

        if let Some(schema) = schema {
            return Self::validate_and_correct(&args, schema);
        }

        Ok(None)
    }

    /// Validate and correct arguments against schema
    fn validate_and_correct(
        args: &Value,
        schema: &Value,
    ) -> Result<Option<Correction>, CorrectionError> {
        let mut corrected = args.clone();
        let mut corrections = Vec::new();

        // Check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            let args_obj = corrected
                .as_object_mut()
                .ok_or(CorrectionError::InvalidFormat)?;

            for field in required {
                if let Some(field_name) = field.as_str() {
                    if !args_obj.contains_key(field_name) {
                        // Add default value if available
                        if let Some(default) = schema
                            .get("properties")
                            .and_then(|p| p.get(field_name))
                            .and_then(|f| f.get("default"))
                        {
                            args_obj.insert(field_name.to_string(), default.clone());
                            corrections.push(CorrectionType::MissingField);
                        }
                    }
                }
            }
        }

        if corrections.is_empty() {
            return Ok(None);
        }

        Ok(Some(Correction {
            original: args.clone(),
            corrected,
            correction_type: corrections.into_iter().next().unwrap(),
            message: "Arguments corrected".to_string(),
        }))
    }

    /// Normalize parameter values
    pub fn normalize_parameter(value: &Value, expected_type: &str) -> Value {
        match expected_type {
            "string" => {
                if value.is_string() {
                    value.clone()
                } else {
                    Value::String(value.to_string())
                }
            }
            "number" | "integer" => {
                if value.is_number() {
                    value.clone()
                } else if let Some(s) = value.as_str() {
                    s.parse::<f64>()
                        .ok()
                        .and_then(|n| serde_json::Number::from_f64(n).map(Value::Number))
                        .unwrap_or_else(|| value.clone())
                } else {
                    value.clone()
                }
            }
            "boolean" => {
                if value.is_boolean() {
                    value.clone()
                } else if let Some(s) = value.as_str() {
                    Value::Bool(s == "true" || s == "1")
                } else {
                    value.clone()
                }
            }
            _ => value.clone(),
        }
    }

    /// Rename deprecated parameters
    pub fn rename_deprecated_parameter(args: &mut Value, old_name: &str, new_name: &str) -> bool {
        if let Some(obj) = args.as_object_mut() {
            if let Some(value) = obj.remove(old_name) {
                obj.insert(new_name.to_string(), value);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_arguments() {
        let tool_call = serde_json::json!({
            "function": {
                "name": "test",
                "arguments": "{\"param\": \"value\"}"
            }
        });

        let result = OpenAIToolCorrector::correct_arguments(&tool_call, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_normalize_parameter() {
        let value = Value::String("123".to_string());
        let normalized = OpenAIToolCorrector::normalize_parameter(&value, "integer");
        assert!(normalized.is_number());
    }
}
