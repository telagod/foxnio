//! Provider adapters / registry
//!
//! 第一阶段只做静态注册，不做动态加载。

use reqwest::RequestBuilder;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub key: String,
    pub display_name: String,
    pub base_url: String,
    pub auth_header: String,
    pub auth_prefix: String,
    pub requires_version_header: bool,
    pub api_version: Option<String>,
}

pub trait ProviderAdapter: Send + Sync {
    fn key(&self) -> &'static str;
    fn base_url(&self) -> &'static str;
    fn display_name(&self) -> &'static str {
        self.key()
    }
    fn auth_header(&self) -> &'static str {
        "Authorization"
    }
    fn auth_prefix(&self) -> &'static str {
        "Bearer "
    }
    fn requires_version_header(&self) -> bool {
        false
    }
    fn api_version(&self) -> Option<&'static str> {
        None
    }

    fn build_chat_completions_url(&self, mapped_model: Option<&str>, credential: &str) -> String {
        let _ = mapped_model;
        let _ = credential;
        format!("{}/v1/chat/completions", self.base_url())
    }

    fn build_responses_url(&self, _credential: &str) -> Option<String> {
        Some(format!("{}/v1/responses", self.base_url()))
    }

    fn apply_auth(&self, req: RequestBuilder, credential: &str) -> RequestBuilder;

    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            key: self.key().to_string(),
            display_name: self.display_name().to_string(),
            base_url: self.base_url().to_string(),
            auth_header: self.auth_header().to_string(),
            auth_prefix: self.auth_prefix().to_string(),
            requires_version_header: self.requires_version_header(),
            api_version: self.api_version().map(ToString::to_string),
        }
    }
}

fn normalized_env_base_url(var_name: &str, default: &'static str) -> &'static str {
    static DROID_BASE_URL: OnceLock<String> = OnceLock::new();

    match var_name {
        "DROID_BASE_URL" => DROID_BASE_URL
            .get_or_init(|| {
                std::env::var(var_name)
                    .ok()
                    .map(|value| value.trim().trim_end_matches('/').to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| default.to_string())
            })
            .as_str(),
        _ => default,
    }
}

struct BearerAdapter {
    key: &'static str,
    base_url: &'static str,
}

impl ProviderAdapter for BearerAdapter {
    fn key(&self) -> &'static str {
        self.key
    }

    fn base_url(&self) -> &'static str {
        self.base_url
    }

    fn apply_auth(&self, req: RequestBuilder, credential: &str) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {credential}"))
    }
}

struct AnthropicAdapter;

impl ProviderAdapter for AnthropicAdapter {
    fn key(&self) -> &'static str {
        "anthropic"
    }

    fn base_url(&self) -> &'static str {
        "https://api.anthropic.com"
    }

    fn display_name(&self) -> &'static str {
        "Anthropic"
    }

    fn auth_header(&self) -> &'static str {
        "x-api-key"
    }

    fn auth_prefix(&self) -> &'static str {
        ""
    }

    fn requires_version_header(&self) -> bool {
        true
    }

    fn api_version(&self) -> Option<&'static str> {
        Some("2023-06-01")
    }

    fn build_chat_completions_url(&self, _mapped_model: Option<&str>, _credential: &str) -> String {
        format!("{}/v1/messages", self.base_url())
    }

    fn apply_auth(&self, req: RequestBuilder, credential: &str) -> RequestBuilder {
        req.header("x-api-key", credential)
            .header("anthropic-version", "2023-06-01")
    }
}

struct GeminiAdapter;

impl ProviderAdapter for GeminiAdapter {
    fn key(&self) -> &'static str {
        "gemini"
    }

    fn base_url(&self) -> &'static str {
        "https://generativelanguage.googleapis.com"
    }

    fn display_name(&self) -> &'static str {
        "Google"
    }

    fn auth_header(&self) -> &'static str {
        "x-goog-api-key"
    }

    fn auth_prefix(&self) -> &'static str {
        ""
    }

    fn build_chat_completions_url(&self, mapped_model: Option<&str>, credential: &str) -> String {
        let model = mapped_model.unwrap_or("models/gemini-2.0-flash");
        format!(
            "{}{}:generateContent?key={credential}",
            self.base_url(),
            model
        )
    }

    fn apply_auth(&self, req: RequestBuilder, _credential: &str) -> RequestBuilder {
        req
    }
}

struct DroidAdapter;

impl ProviderAdapter for DroidAdapter {
    fn key(&self) -> &'static str {
        "droid"
    }

    fn base_url(&self) -> &'static str {
        normalized_env_base_url("DROID_BASE_URL", "http://127.0.0.1:3000")
    }

    fn display_name(&self) -> &'static str {
        "Droid"
    }

    fn build_chat_completions_url(&self, _mapped_model: Option<&str>, _credential: &str) -> String {
        format!("{}/v1/chat/completions", self.base_url())
    }

    fn build_responses_url(&self, _credential: &str) -> Option<String> {
        Some(format!("{}/v1/responses", self.base_url()))
    }

    fn apply_auth(&self, req: RequestBuilder, credential: &str) -> RequestBuilder {
        req.header("Authorization", format!("Bearer {credential}"))
    }
}

struct GoogleAdapter;

impl ProviderAdapter for GoogleAdapter {
    fn key(&self) -> &'static str {
        "google"
    }

    fn base_url(&self) -> &'static str {
        "https://generativelanguage.googleapis.com"
    }

    fn display_name(&self) -> &'static str {
        "Google"
    }

    fn auth_header(&self) -> &'static str {
        "x-goog-api-key"
    }

    fn auth_prefix(&self) -> &'static str {
        ""
    }

    fn build_chat_completions_url(&self, mapped_model: Option<&str>, credential: &str) -> String {
        let model = mapped_model.unwrap_or("models/gemini-2.0-flash");
        format!(
            "{}{}:generateContent?key={credential}",
            self.base_url(),
            model
        )
    }

    fn apply_auth(&self, req: RequestBuilder, _credential: &str) -> RequestBuilder {
        req
    }
}

pub struct ProviderRegistry {
    adapters: HashMap<String, Arc<dyn ProviderAdapter>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register<A>(&mut self, adapter: A)
    where
        A: ProviderAdapter + 'static,
    {
        self.adapters
            .insert(adapter.key().to_string(), Arc::new(adapter));
    }

    pub fn get(&self, provider: &str) -> Option<Arc<dyn ProviderAdapter>> {
        self.adapters.get(&provider.to_lowercase()).cloned()
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        let mut descriptors = self
            .adapters
            .values()
            .map(|adapter| adapter.descriptor())
            .collect::<Vec<_>>();
        descriptors.sort_by(|a, b| a.key.cmp(&b.key));
        descriptors
    }

    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(BearerAdapter {
            key: "openai",
            base_url: "https://api.openai.com",
        });
        registry.register(AnthropicAdapter);
        registry.register(GeminiAdapter);
        registry.register(GoogleAdapter);
        registry.register(DroidAdapter);
        registry.register(BearerAdapter {
            key: "antigravity",
            base_url: "https://antigravity.so",
        });
        registry
    }
}

static DEFAULT_PROVIDER_REGISTRY: OnceLock<ProviderRegistry> = OnceLock::new();

pub fn default_provider_registry() -> &'static ProviderRegistry {
    DEFAULT_PROVIDER_REGISTRY.get_or_init(ProviderRegistry::with_defaults)
}
