//! Promo service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Promo campaign
#[derive(Debug, Clone)]
pub struct PromoCampaign {
    pub id: String,
    pub name: String,
    pub discount_percent: u8,
    pub start_time: i64,
    pub end_time: i64,
    pub is_active: bool,
}

/// Promo service
pub struct PromoService {
    campaigns: Arc<RwLock<HashMap<String, PromoCampaign>>>,
}

impl Default for PromoService {
    fn default() -> Self {
        Self::new()
    }
}

impl PromoService {
    pub fn new() -> Self {
        Self {
            campaigns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_campaign(&self, campaign: PromoCampaign) {
        let mut campaigns = self.campaigns.write().await;
        campaigns.insert(campaign.id.clone(), campaign);
    }

    pub async fn get_active_campaigns(&self) -> Vec<PromoCampaign> {
        let campaigns = self.campaigns.read().await;
        let now = chrono::Utc::now().timestamp();
        campaigns
            .values()
            .filter(|c| c.is_active && c.start_time <= now && c.end_time >= now)
            .cloned()
            .collect()
    }

    pub async fn get_discount(&self, campaign_id: &str) -> Option<u8> {
        let campaigns = self.campaigns.read().await;
        campaigns.get(campaign_id).map(|c| c.discount_percent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_promo() {
        let service = PromoService::new();

        let now = chrono::Utc::now().timestamp();
        service
            .create_campaign(PromoCampaign {
                id: "summer".to_string(),
                name: "Summer Sale".to_string(),
                discount_percent: 20,
                start_time: now - 100,
                end_time: now + 86400,
                is_active: true,
            })
            .await;

        let active = service.get_active_campaigns().await;
        assert_eq!(active.len(), 1);
    }
}
