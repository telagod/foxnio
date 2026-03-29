//! Update service

use serde::{Deserialize, Serialize};

/// Update info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub release_date: i64,
    pub changelog: String,
    pub is_critical: bool,
    pub download_url: Option<String>,
}

/// Update service
pub struct UpdateService {
    current_version: String,
    latest_version: Option<UpdateInfo>,
}

impl UpdateService {
    pub fn new(current_version: &str) -> Self {
        Self {
            current_version: current_version.to_string(),
            latest_version: None,
        }
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub async fn check_for_updates(&mut self) -> Option<&UpdateInfo> {
        // Placeholder for update check
        self.latest_version.as_ref()
    }

    pub fn set_latest(&mut self, info: UpdateInfo) {
        self.latest_version = Some(info);
    }

    pub fn is_update_available(&self) -> bool {
        self.latest_version
            .as_ref()
            .map(|v| v.version != self.current_version)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_check() {
        let mut service = UpdateService::new("1.0.0");

        assert!(!service.is_update_available());

        service.set_latest(UpdateInfo {
            version: "1.1.0".to_string(),
            release_date: chrono::Utc::now().timestamp(),
            changelog: "Bug fixes".to_string(),
            is_critical: false,
            download_url: None,
        });

        assert!(service.is_update_available());
    }
}
