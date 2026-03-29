//! Admin service

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Admin service
pub struct AdminService {
    admins: Arc<RwLock<HashSet<i64>>>,
}

impl Default for AdminService {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminService {
    pub fn new() -> Self {
        Self {
            admins: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn add_admin(&self, user_id: i64) {
        let mut admins = self.admins.write().await;
        admins.insert(user_id);
    }

    pub async fn remove_admin(&self, user_id: i64) {
        let mut admins = self.admins.write().await;
        admins.remove(&user_id);
    }

    pub async fn is_admin(&self, user_id: i64) -> bool {
        let admins = self.admins.read().await;
        admins.contains(&user_id)
    }

    pub async fn list_admins(&self) -> Vec<i64> {
        let admins = self.admins.read().await;
        admins.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_admin_service() {
        let service = AdminService::new();

        service.add_admin(123).await;
        assert!(service.is_admin(123).await);

        service.remove_admin(123).await;
        assert!(!service.is_admin(123).await);
    }
}
