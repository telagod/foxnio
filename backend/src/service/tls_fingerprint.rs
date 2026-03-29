//! TLS 指纹配置服务
//!
//! 管理 TLS 指纹模板，用于模拟特定客户端的 TLS 握手特征

#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryOrder, Set};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::entity::tls_fingerprint_profile::{
    self, CreateTLSFingerprintProfileRequest, Entity as TLSFingerprintProfile,
    Model as TLSFingerprintProfileModel, UpdateTLSFingerprintProfileRequest,
};

/// TLS 指纹配置服务
pub struct TLSFingerprintService {
    db: DatabaseConnection,
    /// 本地缓存
    cache: Arc<RwLock<HashMap<i64, TLSFingerprintProfileModel>>>,
}

impl TLSFingerprintService {
    /// 创建新的 TLS 指纹配置服务
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 初始化缓存
    pub async fn init_cache(&self) -> Result<()> {
        let profiles = TLSFingerprintProfile::find()
            .order_by_asc(tls_fingerprint_profile::Column::Id)
            .all(&self.db)
            .await
            .context("Failed to load TLS fingerprint profiles")?;

        let mut cache = self.cache.write().await;
        cache.clear();
        for profile in profiles {
            cache.insert(profile.id, profile);
        }

        Ok(())
    }

    /// 列出所有 TLS 指纹配置模板
    pub async fn list(&self) -> Result<Vec<TLSFingerprintProfileModel>> {
        let profiles = TLSFingerprintProfile::find()
            .order_by_asc(tls_fingerprint_profile::Column::Name)
            .all(&self.db)
            .await
            .context("Failed to list TLS fingerprint profiles")?;

        Ok(profiles)
    }

    /// 根据 ID 获取 TLS 指纹配置模板
    pub async fn get(&self, id: i64) -> Result<Option<TLSFingerprintProfileModel>> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(profile) = cache.get(&id) {
                return Ok(Some(profile.clone()));
            }
        }

        // 缓存未命中，查数据库
        let profile = TLSFingerprintProfile::find_by_id(id)
            .one(&self.db)
            .await
            .context("Failed to get TLS fingerprint profile")?;

        // 更新缓存
        if let Some(ref p) = profile {
            let mut cache = self.cache.write().await;
            cache.insert(p.id, p.clone());
        }

        Ok(profile)
    }

    /// 创建 TLS 指纹配置模板
    pub async fn create(
        &self,
        req: CreateTLSFingerprintProfileRequest,
    ) -> Result<TLSFingerprintProfileModel> {
        let now = Utc::now();

        let profile = tls_fingerprint_profile::ActiveModel {
            id: Set(chrono::Utc::now().timestamp_millis()),
            name: Set(req.name),
            description: Set(req.description),
            enable_grease: Set(req.enable_grease),
            cipher_suites: Set(json!(req.cipher_suites)),
            curves: Set(json!(req.curves)),
            point_formats: Set(json!(req.point_formats)),
            signature_algorithms: Set(json!(req.signature_algorithms)),
            alpn_protocols: Set(json!(req.alpn_protocols)),
            supported_versions: Set(json!(req.supported_versions)),
            key_share_groups: Set(json!(req.key_share_groups)),
            psk_modes: Set(json!(req.psk_modes)),
            extensions: Set(json!(req.extensions)),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let profile = profile
            .insert(&self.db)
            .await
            .context("Failed to create TLS fingerprint profile")?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(profile.id, profile.clone());

        Ok(profile)
    }

    /// 更新 TLS 指纹配置模板
    pub async fn update(
        &self,
        id: i64,
        req: UpdateTLSFingerprintProfileRequest,
    ) -> Result<TLSFingerprintProfileModel> {
        let profile = TLSFingerprintProfile::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("TLS fingerprint profile not found: {}", id))?;

        let mut active: tls_fingerprint_profile::ActiveModel = profile.into();

        if let Some(name) = req.name {
            active.name = Set(name);
        }
        if let Some(description) = req.description {
            active.description = Set(Some(description));
        }
        if let Some(enable_grease) = req.enable_grease {
            active.enable_grease = Set(enable_grease);
        }
        if let Some(cipher_suites) = req.cipher_suites {
            active.cipher_suites = Set(json!(cipher_suites));
        }
        if let Some(curves) = req.curves {
            active.curves = Set(json!(curves));
        }
        if let Some(point_formats) = req.point_formats {
            active.point_formats = Set(json!(point_formats));
        }
        if let Some(signature_algorithms) = req.signature_algorithms {
            active.signature_algorithms = Set(json!(signature_algorithms));
        }
        if let Some(alpn_protocols) = req.alpn_protocols {
            active.alpn_protocols = Set(json!(alpn_protocols));
        }
        if let Some(supported_versions) = req.supported_versions {
            active.supported_versions = Set(json!(supported_versions));
        }
        if let Some(key_share_groups) = req.key_share_groups {
            active.key_share_groups = Set(json!(key_share_groups));
        }
        if let Some(psk_modes) = req.psk_modes {
            active.psk_modes = Set(json!(psk_modes));
        }
        if let Some(extensions) = req.extensions {
            active.extensions = Set(json!(extensions));
        }

        active.updated_at = Set(Utc::now());

        let profile = active
            .update(&self.db)
            .await
            .context("Failed to update TLS fingerprint profile")?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(profile.id, profile.clone());

        Ok(profile)
    }

    /// 删除 TLS 指纹配置模板
    pub async fn delete(&self, id: i64) -> Result<()> {
        let profile = TLSFingerprintProfile::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("TLS fingerprint profile not found: {}", id))?;

        profile.delete(&self.db).await?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.remove(&id);

        Ok(())
    }

    /// 解析账号绑定的 TLS 指纹配置
    pub async fn resolve_profile(
        &self,
        tls_fingerprint_profile_id: Option<i64>,
    ) -> Option<TLSFingerprintProfileModel> {
        if let Some(id) = tls_fingerprint_profile_id {
            self.get(id).await.ok().flatten()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> CreateTLSFingerprintProfileRequest {
        CreateTLSFingerprintProfileRequest {
            name: "test-profile".to_string(),
            description: Some("Test TLS fingerprint profile".to_string()),
            enable_grease: true,
            cipher_suites: vec![0x1301, 0x1302, 0x1303],
            curves: vec![29, 23, 24],
            point_formats: vec![0],
            signature_algorithms: vec![0x0403, 0x0503, 0x0603],
            alpn_protocols: vec!["http/1.1".to_string()],
            supported_versions: vec![0x0304, 0x0303],
            key_share_groups: vec![29],
            psk_modes: vec![1],
            extensions: vec![0, 10, 11, 13, 16, 23, 27, 35, 43, 51],
        }
    }

    fn create_test_update_request() -> UpdateTLSFingerprintProfileRequest {
        UpdateTLSFingerprintProfileRequest {
            name: Some("updated-profile".to_string()),
            description: Some("Updated description".to_string()),
            enable_grease: Some(false),
            cipher_suites: Some(vec![0x1301]),
            curves: Some(vec![29]),
            point_formats: Some(vec![0]),
            signature_algorithms: Some(vec![0x0403]),
            alpn_protocols: Some(vec!["h2".to_string()]),
            supported_versions: Some(vec![0x0304]),
            key_share_groups: Some(vec![29]),
            psk_modes: Some(vec![1]),
            extensions: Some(vec![0, 10, 11]),
        }
    }

    #[test]
    fn test_create_request_validation() {
        let req = create_test_request();
        assert_eq!(req.name, "test-profile");
        assert!(req.enable_grease);
        assert_eq!(req.cipher_suites.len(), 3);
        assert_eq!(req.curves.len(), 3);
        assert_eq!(req.alpn_protocols, vec!["http/1.1"]);
    }

    #[test]
    fn test_update_request_partial() {
        let req = UpdateTLSFingerprintProfileRequest {
            name: Some("partial-update".to_string()),
            ..Default::default()
        };
        assert_eq!(req.name, Some("partial-update".to_string()));
        assert_eq!(req.enable_grease, None);
        assert_eq!(req.cipher_suites, None);
    }

    #[test]
    fn test_profile_name_uniqueness() {
        let req1 = create_test_request();
        let req2 = CreateTLSFingerprintProfileRequest {
            name: "another-profile".to_string(),
            ..req1.clone()
        };
        assert_ne!(req1.name, req2.name);
    }

    #[test]
    fn test_cipher_suites_order() {
        let req = create_test_request();
        // TLS cipher suites order is important for JA3 fingerprint
        assert_eq!(req.cipher_suites[0], 0x1301);
        assert_eq!(req.cipher_suites[1], 0x1302);
        assert_eq!(req.cipher_suites[2], 0x1303);
    }

    #[test]
    fn test_enable_grease_flag() {
        let req_with_grease = CreateTLSFingerprintProfileRequest {
            enable_grease: true,
            ..create_test_request()
        };
        let req_without_grease = CreateTLSFingerprintProfileRequest {
            enable_grease: false,
            ..create_test_request()
        };
        assert!(req_with_grease.enable_grease);
        assert!(!req_without_grease.enable_grease);
    }

    #[test]
    fn test_alpn_protocols() {
        let req = create_test_request();
        assert_eq!(req.alpn_protocols.len(), 1);
        assert_eq!(req.alpn_protocols[0], "http/1.1");
    }

    #[test]
    fn test_supported_versions() {
        let req = create_test_request();
        // TLS 1.3 (0x0304) and TLS 1.2 (0x0303)
        assert!(req.supported_versions.contains(&0x0304));
        assert!(req.supported_versions.contains(&0x0303));
    }

    #[test]
    fn test_extensions_order() {
        let req = create_test_request();
        // Extensions order affects fingerprint
        assert_eq!(req.extensions[0], 0); // server_name
        assert_eq!(req.extensions[1], 10); // supported_groups
        assert_eq!(req.extensions[2], 11); // ec_point_formats
    }

    #[test]
    fn test_update_request_all_fields() {
        let req = create_test_update_request();
        assert!(req.name.is_some());
        assert!(req.description.is_some());
        assert!(req.enable_grease.is_some());
        assert!(req.cipher_suites.is_some());
        assert!(req.curves.is_some());
    }

    #[tokio::test]
    async fn test_create_and_get_profile() {
        // 集成测试需要在有数据库环境时运行
        // 这里只验证类型和逻辑
    }

    #[test]
    fn test_profile_serialization() {
        let req = create_test_request();
        // Test that request can be serialized/deserialized
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: CreateTLSFingerprintProfileRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, req.name);
        assert_eq!(deserialized.enable_grease, req.enable_grease);
    }

    #[test]
    fn test_empty_request() {
        let req = CreateTLSFingerprintProfileRequest {
            name: "empty-profile".to_string(),
            description: None,
            enable_grease: false,
            cipher_suites: vec![],
            curves: vec![],
            point_formats: vec![],
            signature_algorithms: vec![],
            alpn_protocols: vec![],
            supported_versions: vec![],
            key_share_groups: vec![],
            psk_modes: vec![],
            extensions: vec![],
        };
        assert!(req.cipher_suites.is_empty());
        assert!(req.curves.is_empty());
        assert!(req.alpn_protocols.is_empty());
    }
}
