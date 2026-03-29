//! Group service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
}

/// Group service
pub struct GroupService {
    groups: Arc<RwLock<HashMap<i64, Group>>>,
}

impl Default for GroupService {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupService {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, id: i64, name: String, description: Option<String>) -> Group {
        let group = Group {
            id,
            name,
            description,
            is_active: true,
            created_at: chrono::Utc::now().timestamp(),
        };

        let mut groups = self.groups.write().await;
        groups.insert(id, group.clone());

        group
    }

    pub async fn get(&self, id: i64) -> Option<Group> {
        let groups = self.groups.read().await;
        groups.get(&id).cloned()
    }

    pub async fn list_active(&self) -> Vec<Group> {
        let groups = self.groups.read().await;
        groups.values().filter(|g| g.is_active).cloned().collect()
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<Group, String> {
        let mut groups = self.groups.write().await;
        let group = groups.get_mut(&id).ok_or("Group not found")?;

        if let Some(n) = name {
            group.name = n;
        }
        if let Some(d) = description {
            group.description = Some(d);
        }

        Ok(group.clone())
    }

    pub async fn deactivate(&self, id: i64) -> bool {
        let mut groups = self.groups.write().await;
        if let Some(group) = groups.get_mut(&id) {
            group.is_active = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_group() {
        let service = GroupService::new();

        let group = service.create(1, "Default".to_string(), None).await;
        assert_eq!(group.name, "Default");

        let retrieved = service.get(1).await.unwrap();
        assert_eq!(retrieved.id, 1);
    }
}
