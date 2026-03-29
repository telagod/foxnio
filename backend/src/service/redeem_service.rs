//! Redeem service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Redeem code
#[derive(Debug, Clone)]
pub struct RedeemCode {
    pub code: String,
    pub value: f64,
    pub is_used: bool,
    pub used_by: Option<i64>,
    pub expires_at: Option<i64>,
}

/// Redeem service
pub struct RedeemService {
    codes: Arc<RwLock<HashMap<String, RedeemCode>>>,
}

impl Default for RedeemService {
    fn default() -> Self {
        Self::new()
    }
}

impl RedeemService {
    pub fn new() -> Self {
        Self {
            codes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_code(&self, code: RedeemCode) {
        let mut codes = self.codes.write().await;
        codes.insert(code.code.clone(), code);
    }

    pub async fn redeem(&self, user_id: i64, code: &str) -> Result<f64, String> {
        let mut codes = self.codes.write().await;
        let redeem_code = codes.get_mut(code).ok_or("Code not found")?;

        if redeem_code.is_used {
            return Err("Code already used".to_string());
        }

        if let Some(exp) = redeem_code.expires_at {
            if exp < chrono::Utc::now().timestamp() {
                return Err("Code expired".to_string());
            }
        }

        let value = redeem_code.value;
        redeem_code.is_used = true;
        redeem_code.used_by = Some(user_id);

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redeem() {
        let service = RedeemService::new();

        service
            .add_code(RedeemCode {
                code: "CODE123".to_string(),
                value: 10.0,
                is_used: false,
                used_by: None,
                expires_at: None,
            })
            .await;

        let value = service.redeem(123, "CODE123").await.unwrap();
        assert_eq!(value, 10.0);
    }
}
