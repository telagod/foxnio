//! 微信支付 Provider

use anyhow::Result;
use async_trait::async_trait;
use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{CreatePaymentRequest, CreatePaymentResponse, PaymentProvider, WebhookEvent};

pub struct WxPayProvider {
    app_id: String,
    mch_id: String,
    api_key: String,
    notify_url: String,
}

impl WxPayProvider {
    pub fn new(app_id: String, mch_id: String, api_key: String, notify_url: String) -> Self {
        Self { app_id, mch_id, api_key, notify_url }
    }

    fn sign(&self, params: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_key.as_bytes())
            .expect("HMAC key length");
        mac.update(params.as_bytes());
        hex::encode(mac.finalize().into_bytes()).to_uppercase()
    }
}

#[async_trait]
impl PaymentProvider for WxPayProvider {
    fn name(&self) -> &str { "WxPay" }
    fn provider_key(&self) -> &str { super::TYPE_WXPAY }

    async fn create_payment(&self, req: CreatePaymentRequest) -> Result<CreatePaymentResponse> {
        let nonce = uuid::Uuid::new_v4().to_string().replace("-", "");
        let params = format!(
            "appid={}&body={}&mch_id={}&nonce_str={}&notify_url={}&out_trade_no={}&total_fee={}&trade_type=NATIVE",
            self.app_id, req.description, self.mch_id, nonce, self.notify_url, req.order_no, req.amount_cents
        );
        let sign = self.sign(&format!("{}&key={}", params, self.api_key));

        let xml_body = format!(
            "<xml><appid>{}</appid><body>{}</body><mch_id>{}</mch_id><nonce_str>{}</nonce_str><notify_url>{}</notify_url><out_trade_no>{}</out_trade_no><total_fee>{}</total_fee><trade_type>NATIVE</trade_type><sign>{}</sign></xml>",
            self.app_id, req.description, self.mch_id, nonce, self.notify_url, req.order_no, req.amount_cents, sign
        );

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
            .header("Content-Type", "application/xml")
            .body(xml_body)
            .send()
            .await?;

        let resp_text = resp.text().await?;
        // 简化 XML 解析 — 提取 code_url
        let code_url = resp_text
            .split("<code_url><![CDATA[")
            .nth(1)
            .and_then(|s| s.split("]]></code_url>").next())
            .map(|s| s.to_string());

        Ok(CreatePaymentResponse {
            provider_order_id: None,
            payment_url: code_url,
            client_secret: None,
            provider_data: Some(serde_json::json!({"raw_response": resp_text})),
        })
    }

    async fn verify_webhook(&self, _headers: &HeaderMap, body: &[u8]) -> Result<WebhookEvent> {
        let body_str = std::str::from_utf8(body)?;

        // 简化 XML 解析
        let extract = |tag: &str| -> String {
            body_str
                .split(&format!("<{0}><![CDATA[", tag))
                .nth(1)
                .and_then(|s| s.split(&format!("]]></{0}>", tag)).next())
                .or_else(|| {
                    body_str.split(&format!("<{0}>", tag)).nth(1)
                        .and_then(|s| s.split(&format!("</{0}>", tag)).next())
                })
                .unwrap_or("")
                .to_string()
        };

        let result_code = extract("result_code");
        let status = if result_code == "SUCCESS" {
            super::ORDER_PAID
        } else {
            super::ORDER_FAILED
        };

        let total_fee = extract("total_fee").parse::<i64>().ok();

        Ok(WebhookEvent {
            provider_order_id: extract("transaction_id"),
            order_no: extract("out_trade_no"),
            status: status.to_string(),
            amount_cents: total_fee,
            raw_data: serde_json::json!({"xml": body_str}),
        })
    }

    async fn query_order(&self, _provider_order_id: &str) -> Result<String> {
        Ok(super::ORDER_PENDING.to_string())
    }
}
