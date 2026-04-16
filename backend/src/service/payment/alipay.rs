//! 支付宝 Provider

use anyhow::{bail, Result};
use async_trait::async_trait;
use axum::http::HeaderMap;
use chrono::Utc;
use sha2::{Digest, Sha256};

use super::{CreatePaymentRequest, CreatePaymentResponse, PaymentProvider, WebhookEvent};

pub struct AlipayProvider {
    app_id: String,
    private_key: String,
    alipay_public_key: String,
    notify_url: String,
}

impl AlipayProvider {
    pub fn new(app_id: String, private_key: String, alipay_public_key: String, notify_url: String) -> Self {
        Self { app_id, private_key, alipay_public_key, notify_url }
    }

    fn sign(&self, params: &str) -> String {
        // RSA2 签名简化实现 — 生产环境应使用 rsa crate
        let mut hasher = Sha256::new();
        hasher.update(params.as_bytes());
        hasher.update(self.private_key.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn verify_sign(&self, params: &str, sign: &str) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(params.as_bytes());
        hasher.update(self.alipay_public_key.as_bytes());
        hex::encode(hasher.finalize()) == sign
    }
}

#[async_trait]
impl PaymentProvider for AlipayProvider {
    fn name(&self) -> &str { "Alipay" }
    fn provider_key(&self) -> &str { super::TYPE_ALIPAY }

    async fn create_payment(&self, req: CreatePaymentRequest) -> Result<CreatePaymentResponse> {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let biz_content = serde_json::json!({
            "out_trade_no": req.order_no,
            "total_amount": format!("{:.2}", req.amount_cents as f64 / 100.0),
            "subject": req.description,
            "product_code": "FAST_INSTANT_TRADE_PAY",
        });

        let params = format!(
            "app_id={}&method=alipay.trade.page.pay&charset=utf-8&sign_type=RSA2&timestamp={}&version=1.0&notify_url={}&biz_content={}",
            self.app_id, timestamp, self.notify_url, biz_content
        );
        let sign = self.sign(&params);
        let payment_url = format!(
            "https://openapi.alipay.com/gateway.do?{}&sign={}",
            params, sign
        );

        Ok(CreatePaymentResponse {
            provider_order_id: None,
            payment_url: Some(payment_url),
            client_secret: None,
            provider_data: None,
        })
    }

    async fn verify_webhook(&self, _headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent> {
        let body_str = std::str::from_utf8(body)?;
        let params: std::collections::HashMap<String, String> = body_str
            .split('&')
            .filter_map(|pair| {
                let mut kv = pair.splitn(2, '=');
                Some((kv.next()?.to_string(), kv.next().unwrap_or("").to_string()))
            })
            .collect();

        let sign = params.get("sign").cloned().unwrap_or_default();
        let mut sorted_params: Vec<_> = params.iter()
            .filter(|(k, _)| k.as_str() != "sign" && k.as_str() != "sign_type")
            .collect();
        sorted_params.sort_by_key(|(k, _)| k.clone());
        let sign_str: String = sorted_params.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        if !self.verify_sign(&sign_str, &sign) {
            bail!("Invalid Alipay signature");
        }

        let trade_status = params.get("trade_status").map(|s| s.as_str()).unwrap_or("");
        let status = match trade_status {
            "TRADE_SUCCESS" | "TRADE_FINISHED" => super::ORDER_PAID,
            "TRADE_CLOSED" => super::ORDER_CANCELLED,
            _ => "unknown",
        };

        Ok(WebhookEvent {
            provider_order_id: params.get("trade_no").cloned().unwrap_or_default(),
            order_no: params.get("out_trade_no").cloned().unwrap_or_default(),
            status: status.to_string(),
            amount_cents: params.get("total_amount")
                .and_then(|a| a.parse::<f64>().ok())
                .map(|a| (a * 100.0) as i64),
            raw_data: serde_json::to_value(&params)?,
        })
    }

    async fn query_order(&self, _provider_order_id: &str) -> Result<String> {
        // TODO: 实现支付宝订单查询 API
        Ok(super::ORDER_PENDING.to_string())
    }
}
