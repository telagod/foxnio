//! User attribute service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// User attribute value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

/// User attribute service
pub struct UserAttributeService {
    /// Attributes per user
    attributes: Arc<RwLock<HashMap<i64, HashMap<String, AttributeValue>>>>,
}

impl Default for UserAttributeService {
    fn default() -> Self {
        Self::new()
    }
}

impl UserAttributeService {
    /// Create a new service
    pub fn new() -> Self {
        Self {
            attributes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set an attribute
    pub async fn set(&self, user_id: i64, key: String, value: AttributeValue) {
        let mut attrs = self.attributes.write().await;
        let user_attrs = attrs.entry(user_id).or_insert_with(HashMap::new);
        user_attrs.insert(key, value);
    }

    /// Get an attribute
    pub async fn get(&self, user_id: i64, key: &str) -> Option<AttributeValue> {
        let attrs = self.attributes.read().await;
        attrs.get(&user_id).and_then(|ua| ua.get(key).cloned())
    }

    /// Get all attributes for a user
    pub async fn get_all(&self, user_id: i64) -> HashMap<String, AttributeValue> {
        let attrs = self.attributes.read().await;
        attrs.get(&user_id).cloned().unwrap_or_default()
    }

    /// Remove an attribute
    pub async fn remove(&self, user_id: i64, key: &str) -> bool {
        let mut attrs = self.attributes.write().await;
        if let Some(user_attrs) = attrs.get_mut(&user_id) {
            user_attrs.remove(key).is_some()
        } else {
            false
        }
    }

    /// Clear all attributes for a user
    pub async fn clear(&self, user_id: i64) {
        let mut attrs = self.attributes.write().await;
        attrs.remove(&user_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let service = UserAttributeService::new();

        service
            .set(
                123,
                "theme".to_string(),
                AttributeValue::String("dark".to_string()),
            )
            .await;
        let value = service.get(123, "theme").await.unwrap();

        match value {
            AttributeValue::String(s) => assert_eq!(s, "dark"),
            _ => panic!("Wrong type"),
        }
    }
}
