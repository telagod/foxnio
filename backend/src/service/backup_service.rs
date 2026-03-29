//! Backup service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Backup record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub created_at: i64,
    pub backup_type: String,
    pub is_complete: bool,
}

/// Backup service
pub struct BackupService {
    backups: Arc<RwLock<HashMap<String, BackupRecord>>>,
}

impl Default for BackupService {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupService {
    pub fn new() -> Self {
        Self {
            backups: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_backup(&self, name: &str, backup_type: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let record = BackupRecord {
            id: id.clone(),
            name: name.to_string(),
            size_bytes: 0,
            created_at: chrono::Utc::now().timestamp(),
            backup_type: backup_type.to_string(),
            is_complete: false,
        };

        let mut backups = self.backups.write().await;
        backups.insert(id.clone(), record);

        id
    }

    pub async fn complete_backup(&self, id: &str, size: u64) -> Result<(), String> {
        let mut backups = self.backups.write().await;
        let backup = backups.get_mut(id).ok_or("Backup not found")?;

        backup.size_bytes = size;
        backup.is_complete = true;
        Ok(())
    }

    pub async fn list_backups(&self) -> Vec<BackupRecord> {
        let backups = self.backups.read().await;
        let mut list: Vec<_> = backups.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub async fn delete_backup(&self, id: &str) -> bool {
        let mut backups = self.backups.write().await;
        backups.remove(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backup() {
        let service = BackupService::new();

        let id = service.create_backup("daily", "full").await;
        service.complete_backup(&id, 1024).await.unwrap();

        let backups = service.list_backups().await;
        assert_eq!(backups.len(), 1);
        assert!(backups[0].is_complete);
    }
}
