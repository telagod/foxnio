//! TLS fingerprint profile service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TLS fingerprint profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsFingerprintProfile {
    /// Profile ID
    pub id: String,
    /// Profile name
    pub name: String,
    /// TLS version
    pub tls_version: String,
    /// Cipher suites
    pub cipher_suites: Vec<String>,
    /// Extensions
    pub extensions: Vec<u16>,
    /// Curves
    pub curves: Vec<String>,
    /// User agent
    pub user_agent: Option<String>,
}

/// TLS fingerprint profile service
pub struct TlsFingerprintProfileService {
    /// Profiles
    profiles: HashMap<String, TlsFingerprintProfile>,
}

impl Default for TlsFingerprintProfileService {
    fn default() -> Self {
        Self::new()
    }
}

impl TlsFingerprintProfileService {
    /// Create a new service
    pub fn new() -> Self {
        let mut service = Self {
            profiles: HashMap::new(),
        };
        service.load_default_profiles();
        service
    }

    /// Load default profiles
    fn load_default_profiles(&mut self) {
        // Chrome profile
        self.profiles.insert(
            "chrome".to_string(),
            TlsFingerprintProfile {
                id: "chrome".to_string(),
                name: "Chrome 120".to_string(),
                tls_version: "TLS 1.3".to_string(),
                cipher_suites: vec![
                    "TLS_AES_128_GCM_SHA256".to_string(),
                    "TLS_AES_256_GCM_SHA384".to_string(),
                    "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                ],
                extensions: vec![0, 5, 10, 11, 13, 43, 45, 51, 65281],
                curves: vec!["X25519".to_string(), "prime256v1".to_string()],
                user_agent: Some(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
                ),
            },
        );

        // Firefox profile
        self.profiles.insert("firefox".to_string(), TlsFingerprintProfile {
            id: "firefox".to_string(),
            name: "Firefox 121".to_string(),
            tls_version: "TLS 1.3".to_string(),
            cipher_suites: vec![
                "TLS_AES_128_GCM_SHA256".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
            ],
            extensions: vec![0, 5, 10, 11, 13, 43, 45, 51],
            curves: vec!["X25519".to_string(), "prime256v1".to_string()],
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0".to_string()),
        });

        // Safari profile
        self.profiles.insert(
            "safari".to_string(),
            TlsFingerprintProfile {
                id: "safari".to_string(),
                name: "Safari 17".to_string(),
                tls_version: "TLS 1.3".to_string(),
                cipher_suites: vec![
                    "TLS_AES_128_GCM_SHA256".to_string(),
                    "TLS_AES_256_GCM_SHA384".to_string(),
                ],
                extensions: vec![0, 5, 10, 11, 13, 43, 45, 51, 65281],
                curves: vec!["X25519".to_string(), "prime256v1".to_string()],
                user_agent: Some(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_2) AppleWebKit/605.1.15".to_string(),
                ),
            },
        );
    }

    /// Get profile by ID
    pub fn get_profile(&self, id: &str) -> Option<&TlsFingerprintProfile> {
        self.profiles.get(id)
    }

    /// List all profiles
    pub fn list_profiles(&self) -> Vec<&TlsFingerprintProfile> {
        self.profiles.values().collect()
    }

    /// Add custom profile
    pub fn add_profile(&mut self, profile: TlsFingerprintProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    /// Remove profile
    pub fn remove_profile(&mut self, id: &str) -> bool {
        self.profiles.remove(id).is_some()
    }

    /// Generate TLS client hello fingerprint
    pub fn generate_fingerprint(&self, profile_id: &str) -> Option<String> {
        self.profiles
            .get(profile_id)
            .map(|p| format!("{}:{}", p.tls_version, p.cipher_suites.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profiles() {
        let service = TlsFingerprintProfileService::new();
        assert!(service.get_profile("chrome").is_some());
        assert!(service.get_profile("firefox").is_some());
        assert!(service.get_profile("safari").is_some());
    }

    #[test]
    fn test_list_profiles() {
        let service = TlsFingerprintProfileService::new();
        let profiles = service.list_profiles();
        assert!(profiles.len() >= 3);
    }

    #[test]
    fn test_generate_fingerprint() {
        let service = TlsFingerprintProfileService::new();
        let fingerprint = service.generate_fingerprint("chrome");
        assert!(fingerprint.is_some());
    }
}
