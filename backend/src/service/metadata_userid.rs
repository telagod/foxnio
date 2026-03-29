//! Metadata user ID service

use serde::{Deserialize, Serialize};

/// Metadata user ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetadataUserId {
    /// User ID
    pub user_id: i64,
    /// Organization ID (optional)
    pub org_id: Option<i64>,
    /// Team ID (optional)
    pub team_id: Option<i64>,
}

impl MetadataUserId {
    /// Create a new metadata user ID
    pub fn new(user_id: i64) -> Self {
        Self {
            user_id,
            org_id: None,
            team_id: None,
        }
    }

    /// Set organization ID
    pub fn with_org(mut self, org_id: i64) -> Self {
        self.org_id = Some(org_id);
        self
    }

    /// Set team ID
    pub fn with_team(mut self, team_id: i64) -> Self {
        self.team_id = Some(team_id);
        self
    }

    /// Get unique key for this user
    pub fn to_key(&self) -> String {
        match (self.org_id, self.team_id) {
            (Some(org), Some(team)) => format!("{}:{}:{}", self.user_id, org, team),
            (Some(org), None) => format!("{}:{}", self.user_id, org),
            (None, Some(team)) => format!("{}::{}", self.user_id, team),
            (None, None) => self.user_id.to_string(),
        }
    }
}

/// Metadata user ID service
pub struct MetadataUserIdService;

impl MetadataUserIdService {
    /// Parse user ID from string
    pub fn parse(s: &str) -> Result<MetadataUserId, String> {
        let parts: Vec<&str> = s.split(':').collect();

        let user_id = parts
            .get(0)
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or("Invalid user ID")?;

        let mut meta = MetadataUserId::new(user_id);

        if parts.len() >= 2 {
            if !parts[1].is_empty() {
                meta.org_id = parts[1].parse::<i64>().ok();
            }
        }

        if parts.len() >= 3 {
            meta.team_id = parts[2].parse::<i64>().ok();
        }

        Ok(meta)
    }

    /// Create metadata from user ID
    pub fn from_user_id(user_id: i64) -> MetadataUserId {
        MetadataUserId::new(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_userid() {
        let meta = MetadataUserId::new(123);
        assert_eq!(meta.user_id, 123);
        assert_eq!(meta.to_key(), "123");
    }

    #[test]
    fn test_metadata_with_org() {
        let meta = MetadataUserId::new(123).with_org(456);
        assert_eq!(meta.user_id, 123);
        assert_eq!(meta.org_id, Some(456));
        assert_eq!(meta.to_key(), "123:456");
    }

    #[test]
    fn test_parse() {
        let meta = MetadataUserIdService::parse("123:456:789").unwrap();
        assert_eq!(meta.user_id, 123);
        assert_eq!(meta.org_id, Some(456));
        assert_eq!(meta.team_id, Some(789));
    }
}
