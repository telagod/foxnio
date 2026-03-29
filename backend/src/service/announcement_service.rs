//! Announcement service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: i64,
    pub is_active: bool,
    pub priority: u8,
}

/// Announcement service
pub struct AnnouncementService {
    announcements: Arc<RwLock<HashMap<String, Announcement>>>,
}

impl Default for AnnouncementService {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnouncementService {
    pub fn new() -> Self {
        Self {
            announcements: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, announcement: Announcement) {
        let mut announcements = self.announcements.write().await;
        announcements.insert(announcement.id.clone(), announcement);
    }

    pub async fn get(&self, id: &str) -> Option<Announcement> {
        let announcements = self.announcements.read().await;
        announcements.get(id).cloned()
    }

    pub async fn list_active(&self) -> Vec<Announcement> {
        let announcements = self.announcements.read().await;
        let mut active: Vec<_> = announcements
            .values()
            .filter(|a| a.is_active)
            .cloned()
            .collect();
        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active
    }

    pub async fn deactivate(&self, id: &str) -> bool {
        let mut announcements = self.announcements.write().await;
        if let Some(a) = announcements.get_mut(id) {
            a.is_active = false;
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
    async fn test_announcement() {
        let service = AnnouncementService::new();

        service
            .create(Announcement {
                id: "1".to_string(),
                title: "Test".to_string(),
                content: "Content".to_string(),
                created_at: chrono::Utc::now().timestamp(),
                is_active: true,
                priority: 1,
            })
            .await;

        let active = service.list_active().await;
        assert_eq!(active.len(), 1);
    }
}
