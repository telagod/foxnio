//! 支付网关模块
//!
//! Provider Registry 模式，统一抽象 Stripe/Alipay/WxPay/EasyPay

pub mod alipay;
pub mod easypay;
pub mod service;
pub mod stripe;
pub mod wxpay;

use anyhow::Result;
use async_trait::async_trait;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 支付类型
pub const TYPE_STRIPE: &str = "stripe";
pub const TYPE_ALIPAY: &str = "alipay";
pub const TYPE_WXPAY: &str = "wxpay";
pub const TYPE_EASYPAY: &str = "easypay";

/// 订单状态
pub const ORDER_PENDING: &str = "pending";
pub const ORDER_PAID: &str = "paid";
pub const ORDER_COMPLETED: &str = "completed";
pub const ORDER_EXPIRED: &str = "expired";
pub const ORDER_CANCELLED: &str = "cancelled";
pub const ORDER_REFUNDED: &str = "refunded";
pub const ORDER_FAILED: &str = "failed";

/// 创建支付请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentRequest {
    pub order_no: String,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
    pub return_url: Option<String>,
    pub notify_url: String,
}

/// 创建支付响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePaymentResponse {
    pub provider_order_id: Option<String>,
    pub payment_url: Option<String>,
    pub client_secret: Option<String>,
    pub provider_data: Option<serde_json::Value>,
}

/// Webhook 事件
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub provider_order_id: String,
    pub order_no: String,
    pub status: String,
    pub amount_cents: Option<i64>,
    pub raw_data: serde_json::Value,
}

/// 支付 Provider trait
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_key(&self) -> &str;
    async fn create_payment(&self, req: CreatePaymentRequest) -> Result<CreatePaymentResponse>;
    async fn verify_webhook(&self, headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent>;
    async fn query_order(&self, provider_order_id: &str) -> Result<String>;
}

/// 支付 Provider 注册表
pub struct PaymentRegistry {
    providers: HashMap<String, Arc<dyn PaymentProvider>>,
}

impl PaymentRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn PaymentProvider>) {
        self.providers
            .insert(provider.provider_key().to_string(), provider);
    }

    pub fn get(&self, key: &str) -> Option<Arc<dyn PaymentProvider>> {
        self.providers.get(key).cloned()
    }

    pub fn available_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}
