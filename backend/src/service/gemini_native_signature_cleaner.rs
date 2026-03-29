use regex::Regex;
use serde::{Deserialize, Serialize};

/// Clean signatures from Gemini native responses
pub struct GeminiNativeSignatureCleaner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleaningResult {
    pub original: String,
    pub cleaned: String,
    pub signatures_found: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CleaningError {
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

impl GeminiNativeSignatureCleaner {
    /// Remove Gemini-specific signatures from response
    pub fn clean(content: &str) -> Result<CleaningResult, CleaningError> {
        let mut cleaned = content.to_string();
        let mut signatures_found = Vec::new();

        // Common Gemini signature patterns
        let patterns = vec![
            r"\[MODEL_SIGNATURE:.*?\]",
            r"\[GEMINI_ID:.*?\]",
            r"<signature>.*?</signature>",
            r"<!-- GEMINI:.*? -->",
        ];

        for pattern in patterns {
            let re = Regex::new(pattern)?;
            if re.is_match(&cleaned) {
                for cap in re.captures_iter(&cleaned) {
                    signatures_found.push(cap[0].to_string());
                }
                cleaned = re.replace_all(&cleaned, "").to_string();
            }
        }

        // Trim whitespace
        cleaned = cleaned.trim().to_string();

        Ok(CleaningResult {
            original: content.to_string(),
            cleaned,
            signatures_found,
        })
    }

    /// Check if content contains signatures
    pub fn has_signatures(content: &str) -> bool {
        let patterns = vec![
            r"\[MODEL_SIGNATURE:.*?\]",
            r"\[GEMINI_ID:.*?\]",
            r"<signature>.*?</signature>",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(content) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_signatures() {
        let content = "Hello [MODEL_SIGNATURE:abc123] world";
        let result = GeminiNativeSignatureCleaner::clean(content).unwrap();

        assert_eq!(result.cleaned, "Hello  world");
        assert_eq!(result.signatures_found.len(), 1);
    }

    #[test]
    fn test_has_signatures() {
        let content_with = "Hello [MODEL_SIGNATURE:abc]";
        assert!(GeminiNativeSignatureCleaner::has_signatures(content_with));

        let content_without = "Hello world";
        assert!(!GeminiNativeSignatureCleaner::has_signatures(
            content_without
        ));
    }
}
