//! Stripe 支付 Provider

use anyhow::{bail, Result};
use async_trait::async_trait;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{
    CreatePaymentRequest, CreatePaymentResponse, PaymentProvider, WebhookEvent,
};

pub struct StripeProvider {
    secret_key: String,
    webhook_secret: String,
    publishable_key: String,
    currency: String,
}

impl StripeProvider {
    pub fn new(secret_key: String, publishable_key: String, webhook_secret: String, currency: String) -> Self {
        Self { secret_key, publishable_key, webhook_secret, currency }
    }

    pub fn publishable_key(&self) -> &str { &self.publishable_key }
}

#[async_trait]
impl PaymentProvider for StripeProvider {
    fn name(&self) -> &str { "Stripe" }
    fn provider_key(&self) -> &str { super::TYPE_STRIPE }

    async fn create_payment(&self, req: CreatePaymentRequest) -> Result<CreatePaymentResponse> {
        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.stripe.com/v1/payment_intents")
            .basic_auth(&self.secret_key, None::<&str>)
            .form(&[
                ("amount", req.amount_cents.to_string()),
                ("currency", self.currency.clone()),
                ("description", req.description),
                ("metadata[order_no]", req.order_no),
                ("automatic_payment_methods[enabled]", "true".to_string()),
            ])
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;

        if let Some(err) = body.get("error") {
            bail!("Stripe error: {}", err.get("message").and_then(|m| m.as_str()).unwrap_or("unknown"));
        }

        Ok(CreatePaymentResponse {
            provider_order_id: body.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            payment_url: None,
            client_secret: body.get("client_secret").and_then(|v| v.as_str()).map(|s| s.to_string()),
            provider_data: Some(body),
        })
    }

    async fn verify_webhook(&self, headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent> {
        let sig_header = headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow::anyhow!("Missing stripe-signature header"))?;

        // Parse timestamp and signature from header
        let mut timestamp = "";
        let mut signature = "";
        for part in sig_header.split(',') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 {
                match kv[0] {
                    "t" => timestamp = kv[1],
                    "v1" => signature = kv[1],
                    _ => {}
                }
            }
        }

        // Verify HMAC
        let signed_payload = format!("{}.{}", timestamp, std::str::from_utf8(body)?);
        let mut mac = Hmac::<Sha256>::new_from_slice(self.webhook_secret.as_bytes())?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        if expected != signature {
            bail!("Invalid Stripe webhook signature");
        }

        let event: serde_json::Value = serde_json::from_slice(body)?;
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let data = &event["data"]["object"];

        let status = match event_type {
            "payment_intent.succeeded" => super::ORDER_PAID,
            "payment_intent.payment_failed" => super::ORDER_FAILED,
            _ => "unknown",
        };

        let order_no = data.get("metadata")
            .and_then(|m| m.get("order_no"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(WebhookEvent {
            provider_order_id: data.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            order_no,
            status: status.to_string(),
            amount_cents: data.get("amount").and_then(|v| v.as_i64()),
            raw_data: event,
        })
    }

    async fn query_order(&self, provider_order_id: &str) -> Result<String> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("https://api.stripe.com/v1/payment_intents/{}", provider_order_id))
            .basic_auth(&self.secret_key, None::<&str>)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;
        let status = body.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");

        Ok(match status {
            "succeeded" => super::ORDER_PAID.to_string(),
            "canceled" => super::ORDER_CANCELLED.to_string(),
            "requires_payment_method" | "requires_confirmation" | "requires_action" | "processing" => super::ORDER_PENDING.to_string(),
            _ => status.to_string(),
        })
    }
}
