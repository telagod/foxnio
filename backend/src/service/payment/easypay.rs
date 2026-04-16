//! EasyPay 聚合支付 Provider

use anyhow::{bail, Result};
use async_trait::async_trait;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{CreatePaymentRequest, CreatePaymentResponse, PaymentProvider, WebhookEvent};

pub struct EasyPayProvider {
    merchant_id: String,
    api_key: String,
    notify_url: String,
}

impl EasyPayProvider {
    pub fn new(merchant_id: String, api_key: String, notify_url: String) -> Self {
        Self { merchant_id, api_key, notify_url }
    }

    fn sign(&self, params: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_key.as_bytes())
            .expect("HMAC key length");
        mac.update(params.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

#[async_trait]
impl PaymentProvider for EasyPayProvider {
    fn name(&self) -> &str { "EasyPay" }
    fn provider_key(&self) -> &str { super::TYPE_EASYPAY }

    async fn create_payment(&self, req: CreatePaymentRequest) -> Result<CreatePaymentResponse> {
        let params = serde_json::json!({
            "merchant_id": self.merchant_id,
            "out_trade_no": req.order_no,
            "amount": req.amount_cents,
            "currency": req.currency,
            "description": req.description,
            "notify_url": self.notify_url,
            "return_url": req.return_url,
        });

        let sign_str = format!("amount={}&merchant_id={}&out_trade_no={}",
            req.amount_cents, self.merchant_id, req.order_no);
        let sign = self.sign(&sign_str);

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.easypay.com/v1/orders")
            .header("X-Sign", &sign)
            .json(&params)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;

        if body.get("code").and_then(|c| c.as_i64()) != Some(0) {
            bail!("EasyPay error: {}", body.get("message").and_then(|m| m.as_str()).unwrap_or("unknown"));
        }

        Ok(CreatePaymentResponse {
            provider_order_id: body["data"].get("trade_no").and_then(|v| v.as_str()).map(|s| s.to_string()),
            payment_url: body["data"].get("payment_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
            client_secret: None,
            provider_data: Some(body),
        })
    }

    async fn verify_webhook(&self, headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent> {
        let sig = headers.get("x-sign")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing x-sign header"))?;

        let event: serde_json::Value = serde_json::from_slice(body)?;

        let sign_str = format!("amount={}&merchant_id={}&out_trade_no={}",
            event.get("amount").and_then(|v| v.as_i64()).unwrap_or(0),
            event.get("merchant_id").and_then(|v| v.as_str()).unwrap_or(""),
            event.get("out_trade_no").and_then(|v| v.as_str()).unwrap_or(""),
        );
        let expected = self.sign(&sign_str);

        if expected != sig {
            bail!("Invalid EasyPay signature");
        }

        let trade_status = event.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let status = match trade_status {
            "SUCCESS" | "PAID" => super::ORDER_PAID,
            "CLOSED" | "CANCELLED" => super::ORDER_CANCELLED,
            "FAILED" => super::ORDER_FAILED,
            _ => "unknown",
        };

        Ok(WebhookEvent {
            provider_order_id: event.get("trade_no").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            order_no: event.get("out_trade_no").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            status: status.to_string(),
            amount_cents: event.get("amount").and_then(|v| v.as_i64()),
            raw_data: event,
        })
    }

    async fn query_order(&self, _provider_order_id: &str) -> Result<String> {
        Ok(super::ORDER_PENDING.to_string())
    }
}
