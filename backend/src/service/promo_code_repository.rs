//! Promo code repository service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Promo code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoCode {
    /// Code
    pub code: String,
    /// Discount percentage (0-100)
    pub discount_percent: u8,
    /// Discount amount (if fixed)
    pub discount_amount: Option<f64>,
    /// Max uses
    pub max_uses: u32,
    /// Current uses
    pub current_uses: u32,
    /// Expiry timestamp
    pub expires_at: Option<i64>,
    /// Is active
    pub is_active: bool,
    /// Description
    pub description: Option<String>,
}

impl PromoCode {
    /// Check if code is valid
    pub fn is_valid(&self) -> bool {
        self.is_active
            && self.current_uses < self.max_uses
            && self
                .expires_at
                .map_or(true, |exp| exp > chrono::Utc::now().timestamp())
    }

    /// Calculate discount
    pub fn calculate_discount(&self, amount: f64) -> f64 {
        if let Some(fixed) = self.discount_amount {
            fixed.min(amount)
        } else {
            amount * (self.discount_percent as f64 / 100.0)
        }
    }
}

/// Promo code repository
pub struct PromoCodeRepository {
    /// Codes storage
    codes: Arc<RwLock<HashMap<String, PromoCode>>>,
}

impl Default for PromoCodeRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl PromoCodeRepository {
    /// Create a new repository
    pub fn new() -> Self {
        Self {
            codes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a promo code
    pub async fn add(&self, code: PromoCode) -> Result<(), String> {
        let mut codes = self.codes.write().await;

        if codes.contains_key(&code.code) {
            return Err("Code already exists".to_string());
        }

        codes.insert(code.code.clone(), code);
        Ok(())
    }

    /// Get a promo code
    pub async fn get(&self, code: &str) -> Option<PromoCode> {
        let codes = self.codes.read().await;
        codes.get(code).cloned()
    }

    /// Use a promo code
    pub async fn use_code(&self, code: &str) -> Result<PromoCode, String> {
        let mut codes = self.codes.write().await;

        let promo = codes.get_mut(code).ok_or("Code not found")?;

        if !promo.is_valid() {
            return Err("Code is not valid".to_string());
        }

        promo.current_uses += 1;
        Ok(promo.clone())
    }

    /// Deactivate a code
    pub async fn deactivate(&self, code: &str) -> Result<(), String> {
        let mut codes = self.codes.write().await;

        let promo = codes.get_mut(code).ok_or("Code not found")?;

        promo.is_active = false;
        Ok(())
    }

    /// List all codes
    pub async fn list_all(&self) -> Vec<PromoCode> {
        let codes = self.codes.read().await;
        codes.values().cloned().collect()
    }

    /// List active codes
    pub async fn list_active(&self) -> Vec<PromoCode> {
        let codes = self.codes.read().await;
        codes.values().filter(|c| c.is_valid()).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_get() {
        let repo = PromoCodeRepository::new();

        let code = PromoCode {
            code: "SAVE20".to_string(),
            discount_percent: 20,
            discount_amount: None,
            max_uses: 100,
            current_uses: 0,
            expires_at: None,
            is_active: true,
            description: Some("Save 20%".to_string()),
        };

        repo.add(code.clone()).await.unwrap();
        let retrieved = repo.get("SAVE20").await.unwrap();
        assert_eq!(retrieved.code, "SAVE20");
    }

    #[test]
    fn test_calculate_discount() {
        let code = PromoCode {
            code: "SAVE20".to_string(),
            discount_percent: 20,
            discount_amount: None,
            max_uses: 100,
            current_uses: 0,
            expires_at: None,
            is_active: true,
            description: None,
        };

        let discount = code.calculate_discount(100.0);
        assert_eq!(discount, 20.0);
    }
}
