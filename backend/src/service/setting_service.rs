use crate::service::{Setting, SettingCategory, SettingError, SettingMap};
use sqlx::{query, query_as, PgPool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Service for managing system settings
pub struct SettingService {
    pool: PgPool,
    cache: Arc<RwLock<SettingMap>>,
}

impl SettingService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a setting by key
    pub async fn get(&self, key: &str) -> Result<Option<Setting>, SettingError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(setting) = cache.get(key) {
                return Ok(Some(setting.clone()));
            }
        }

        // Query database
        let result = query_as::<_, Setting>(
            r#"
            SELECT key, value, category, description, is_public, updated_at
            FROM settings
            WHERE key = $1
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        // Update cache if found
        if let Some(ref setting) = result {
            let mut cache = self.cache.write().await;
            cache.insert(key.to_string(), setting.clone());
        }

        Ok(result)
    }

    /// Set a setting value
    pub async fn set(
        &self,
        key: String,
        value: String,
        category: SettingCategory,
    ) -> Result<Setting, SettingError> {
        let category_str =
            serde_json::to_string(&category).map_err(|e| SettingError::SerializeError {
                key: key.clone(),
                source: e,
            })?;

        let setting = query_as::<_, Setting>(
            r#"
            INSERT INTO settings (key, value, category, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (key) DO UPDATE SET
                value = EXCLUDED.value,
                category = EXCLUDED.category,
                updated_at = NOW()
            RETURNING key, value, category, description, is_public, updated_at
            "#,
        )
        .bind(&key)
        .bind(&value)
        .bind(&category_str)
        .fetch_one(&self.pool)
        .await?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(setting.key.clone(), setting.clone());

        Ok(setting)
    }

    /// Delete a setting
    pub async fn delete(&self, key: &str) -> Result<(), SettingError> {
        query("DELETE FROM settings WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;

        // Remove from cache
        let mut cache = self.cache.write().await;
        cache.remove(key);

        Ok(())
    }

    /// Get all settings in a category
    pub async fn get_by_category(
        &self,
        category: &SettingCategory,
    ) -> Result<Vec<Setting>, SettingError> {
        let category_str =
            serde_json::to_string(category).map_err(|e| SettingError::SerializeError {
                key: "category".to_string(),
                source: e,
            })?;

        let settings = query_as::<_, Setting>(
            r#"
            SELECT key, value, category, description, is_public, updated_at
            FROM settings
            WHERE category = $1
            ORDER BY key
            "#,
        )
        .bind(&category_str)
        .fetch_all(&self.pool)
        .await?;

        // Update cache
        let mut cache = self.cache.write().await;
        for setting in &settings {
            cache.insert(setting.key.clone(), setting.clone());
        }

        Ok(settings)
    }

    /// Get all public settings
    pub async fn get_public(&self) -> Result<Vec<Setting>, SettingError> {
        let settings = query_as::<_, Setting>(
            r#"
            SELECT key, value, category, description, is_public, updated_at
            FROM settings
            WHERE is_public = true
            ORDER BY key
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Reload cache from database
    pub async fn reload_cache(&self) -> Result<(), SettingError> {
        let settings = query_as::<_, Setting>(
            r#"
            SELECT key, value, category, description, is_public, updated_at
            FROM settings
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut cache = self.cache.write().await;
        cache.clear();
        for setting in settings {
            cache.insert(setting.key.clone(), setting);
        }

        Ok(())
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_category() {
        assert_eq!(SettingCategory::System.as_str(), "system");
        assert_eq!(SettingCategory::Billing.as_str(), "billing");
        assert_eq!(SettingCategory::Security.as_str(), "security");
    }

    #[test]
    fn test_setting_map_operations() {
        let mut map: SettingMap = HashMap::new();

        let setting = Setting {
            id: 1,
            key: "test_key".to_string(),
            value: Some("test_value".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        map.insert("test_key".to_string(), setting.clone());

        assert!(map.contains_key("test_key"));
        assert_eq!(
            map.get("test_key").unwrap().value,
            Some("test_value".to_string())
        );
    }

    // Database-dependent tests are skipped in CI
    #[sqlx::test]
    #[ignore]
    async fn test_setting_service_basic(pool: PgPool) {
        let service = SettingService::new(pool);

        // Test set and get
        let setting = service
            .set(
                "test_key".to_string(),
                "test_value".to_string(),
                SettingCategory::System,
            )
            .await
            .unwrap();

        assert_eq!(setting.key, "test_key");
        assert_eq!(setting.value, Some("test_value".to_string()));

        // Test get from cache
        let cached = service.get("test_key").await.unwrap();
        assert!(cached.is_some());

        // Test delete
        service.delete("test_key").await.unwrap();
        let deleted = service.get("test_key").await.unwrap();
        assert!(deleted.is_none());
    }
}
