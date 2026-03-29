use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Cache key generator for OpenAI-compatible prompts
pub struct OpenAICompatPromptCacheKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCacheKey {
    pub key: String,
    pub hash: u64,
    pub model: String,
    pub prompt_hash: String,
    pub params_hash: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptParams {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Option<Vec<String>>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
}

#[derive(Debug, thiserror::Error)]
pub enum CacheKeyError {
    #[error("Invalid prompt parameters")]
    InvalidParams,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl OpenAICompatPromptCacheKey {
    /// Generate cache key for prompt
    pub fn generate(params: &PromptParams) -> Result<PromptCacheKey, CacheKeyError> {
        let prompt_hash = Self::hash_messages(&params.messages)?;
        let params_hash = Self::hash_params(params)?;

        let mut hasher = DefaultHasher::new();
        format!("{}:{}:{}", params.model, prompt_hash, params_hash).hash(&mut hasher);
        let hash = hasher.finish();

        Ok(PromptCacheKey {
            key: format!("{}:{:016x}", params.model, hash),
            hash,
            model: params.model.clone(),
            prompt_hash,
            params_hash,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Hash messages for cache key
    fn hash_messages(messages: &[serde_json::Value]) -> Result<String, CacheKeyError> {
        let mut hasher = DefaultHasher::new();
        for msg in messages {
            let msg_str = serde_json::to_string(msg)?;
            msg_str.hash(&mut hasher);
        }
        Ok(format!("{:016x}", hasher.finish()))
    }

    /// Hash parameters for cache key
    fn hash_params(params: &PromptParams) -> Result<String, CacheKeyError> {
        let mut hasher = DefaultHasher::new();

        // Include only parameters that affect the response
        if let Some(temp) = params.temperature {
            temp.to_bits().hash(&mut hasher);
        }
        if let Some(top_p) = params.top_p {
            top_p.to_bits().hash(&mut hasher);
        }
        if let Some(max_tokens) = params.max_tokens {
            max_tokens.hash(&mut hasher);
        }
        if let Some(stop) = &params.stop {
            for s in stop {
                s.hash(&mut hasher);
            }
        }
        if let Some(presence_penalty) = params.presence_penalty {
            presence_penalty.to_bits().hash(&mut hasher);
        }
        if let Some(frequency_penalty) = params.frequency_penalty {
            frequency_penalty.to_bits().hash(&mut hasher);
        }

        Ok(format!("{:016x}", hasher.finish()))
    }

    /// Check if prompt is cacheable
    pub fn is_cacheable(params: &PromptParams) -> bool {
        // Don't cache if temperature > 0 (non-deterministic)
        if let Some(temp) = params.temperature {
            if temp > 0.0 {
                return false;
            }
        }

        // Don't cache if presence_penalty or frequency_penalty is set
        if params.presence_penalty.is_some() || params.frequency_penalty.is_some() {
            return false;
        }

        true
    }

    /// Generate short cache key
    pub fn generate_short(params: &PromptParams) -> Result<String, CacheKeyError> {
        let key = Self::generate(params)?;
        Ok(key.key)
    }

    /// Parse cache key
    pub fn parse(key: &str) -> Option<(String, u64)> {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let model = parts[0].to_string();
        let hash = u64::from_str_radix(parts[1], 16).ok()?;

        Some((model, hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_cache_key() {
        let params = PromptParams {
            model: "gpt-4".to_string(),
            messages: vec![serde_json::json!({
                "role": "user",
                "content": "Hello"
            })],
            temperature: Some(0.0),
            top_p: None,
            max_tokens: Some(100),
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
        };

        let key = OpenAICompatPromptCacheKey::generate(&params).unwrap();
        assert!(key.key.starts_with("gpt-4:"));
    }

    #[test]
    fn test_is_cacheable() {
        let params = PromptParams {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: Some(0.0),
            top_p: None,
            max_tokens: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
        };

        assert!(OpenAICompatPromptCacheKey::is_cacheable(&params));

        let params_non_cacheable = PromptParams {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            top_p: None,
            max_tokens: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
        };

        assert!(!OpenAICompatPromptCacheKey::is_cacheable(
            &params_non_cacheable
        ));
    }
}
