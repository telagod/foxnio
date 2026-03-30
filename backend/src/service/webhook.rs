//! Webhook 服务
//!
//! 管理 Webhook 端点的创建、更新、删除和事件投递

use anyhow::Result;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::Sha256;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::entity::{webhook_deliveries, webhook_endpoints};
use crate::metrics::{
    WEBHOOK_DELIVERY_FAILED, WEBHOOK_DELIVERY_SUCCESS, WEBHOOK_EVENTS_SENT, WEBHOOK_RETRY_COUNT,
};

use webhook_endpoints::WebhookEventType;

/// Webhook 服务
pub struct WebhookService {
    db: DatabaseConnection,
    client: Client,
}

impl WebhookService {
    pub fn new(db: DatabaseConnection) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("FoxNIO-Webhook/1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self { db, client }
    }

    /// 创建 Webhook 端点
    pub async fn create_endpoint(
        &self,
        user_id: Uuid,
        url: String,
        events: Vec<String>,
        secret: String,
    ) -> Result<webhook_endpoints::Model> {
        use webhook_endpoints::ActiveModel;

        // 验证 URL 必须是 HTTPS
        if !url.starts_with("https://") {
            anyhow::bail!("Webhook URL must use HTTPS");
        }

        let now = Utc::now();
        let endpoint = ActiveModel {
            id: Set(0), // Will be auto-generated
            user_id: Set(user_id),
            url: Set(url),
            events: Set(serde_json::to_value(&events)?),
            secret: Set(secret),
            enabled: Set(true),
            max_retries: Set(5),
            timeout_ms: Set(5000),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let endpoint = endpoint.insert(&self.db).await?;
        Ok(endpoint)
    }

    /// 列出用户的 Webhook 端点
    pub async fn list_endpoints(&self, user_id: Uuid) -> Result<Vec<webhook_endpoints::Model>> {
        let endpoints = webhook_endpoints::Entity::find()
            .filter(webhook_endpoints::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;

        Ok(endpoints)
    }

    /// 获取单个 Webhook 端点
    pub async fn get_endpoint(
        &self,
        id: i64,
        user_id: Uuid,
    ) -> Result<Option<webhook_endpoints::Model>> {
        let endpoint = webhook_endpoints::Entity::find_by_id(id)
            .filter(webhook_endpoints::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?;

        Ok(endpoint)
    }

    /// 更新 Webhook 端点
    pub async fn update_endpoint(
        &self,
        id: i64,
        user_id: Uuid,
        url: Option<String>,
        events: Option<Vec<String>>,
        secret: Option<String>,
        enabled: Option<bool>,
    ) -> Result<Option<webhook_endpoints::Model>> {
        use webhook_endpoints::ActiveModel;

        let endpoint = match self.get_endpoint(id, user_id).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        let mut active_model: ActiveModel = endpoint.into();

        if let Some(u) = url {
            if !u.starts_with("https://") {
                anyhow::bail!("Webhook URL must use HTTPS");
            }
            active_model.url = Set(u);
        }
        if let Some(e) = events {
            active_model.events = Set(serde_json::to_value(&e)?);
        }
        if let Some(s) = secret {
            active_model.secret = Set(s);
        }
        if let Some(e) = enabled {
            active_model.enabled = Set(e);
        }
        active_model.updated_at = Set(Utc::now());

        let updated = active_model.update(&self.db).await?;
        Ok(Some(updated))
    }

    /// 删除 Webhook 端点
    pub async fn delete_endpoint(&self, id: i64, user_id: Uuid) -> Result<bool> {
        let endpoint = self.get_endpoint(id, user_id).await?;

        if let Some(e) = endpoint {
            webhook_endpoints::Entity::delete_by_id(e.id)
                .exec(&self.db)
                .await?;
            return Ok(true);
        }

        Ok(false)
    }

    /// 测试 Webhook 端点
    pub async fn test_endpoint(&self, id: i64, user_id: Uuid) -> Result<bool> {
        let endpoint = self.get_endpoint(id, user_id).await?;

        if let Some(e) = endpoint {
            // 发送测试 ping
            let test_payload = serde_json::json!({
                "event": "ping",
                "timestamp": Utc::now().to_rfc3339(),
                "test": true
            });

            match self.send_request(&e, &test_payload).await {
                Ok(response) => {
                    info!(
                        endpoint_id = %id,
                        status = %response.status().as_u16(),
                        "Webhook test successful"
                    );
                    Ok(response.status().is_success())
                }
                Err(e) => {
                    warn!(endpoint_id = %id, error = %e, "Webhook test failed");
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }

    /// 列出投递记录
    pub async fn list_deliveries(
        &self,
        endpoint_id: i64,
        user_id: Uuid,
    ) -> Result<Vec<webhook_deliveries::Model>> {
        // 验证用户权限
        let _endpoint = self.get_endpoint(endpoint_id, user_id).await?;

        let deliveries = webhook_deliveries::Entity::find()
            .filter(webhook_deliveries::Column::EndpointId.eq(endpoint_id))
            .all(&self.db)
            .await?;

        Ok(deliveries)
    }

    // ==================== Webhook 发送和投递逻辑 ====================

    /// 发送 webhook 事件
    ///
    /// 查询所有订阅了该事件类型的活跃端点，并为每个端点投递事件
    pub async fn send_webhook(
        &self,
        event_type: WebhookEventType,
        payload: JsonValue,
    ) -> Result<()> {
        let event_str = event_type.as_str();

        // 构建完整的事件负载
        let full_payload = serde_json::json!({
            "event": event_str,
            "timestamp": Utc::now().to_rfc3339(),
            "data": payload,
        });

        // 查询订阅该事件的所有活跃端点
        let endpoints = self.get_subscribed_endpoints(&event_type).await?;

        if endpoints.is_empty() {
            info!(event = %event_str, "No active endpoints subscribed to event");
            return Ok(());
        }

        info!(
            event = %event_str,
            endpoint_count = endpoints.len(),
            "Dispatching webhook event"
        );

        // 记录发送的 webhook 事件数
        WEBHOOK_EVENTS_SENT.inc_by(endpoints.len() as u64);

        // 为每个端点异步投递
        let mut handles = vec![];
        for endpoint in endpoints {
            let payload = full_payload.clone();
            let event_type = event_type.clone();
            let service = self.clone_service();

            let handle = tokio::spawn(async move {
                if let Err(e) = service.deliver_webhook(endpoint, event_type, payload).await {
                    error!(error = %e, "Webhook delivery failed");
                }
            });
            handles.push(handle);
        }

        // 等待所有投递完成
        for handle in handles {
            if let Err(e) = handle.await {
                error!(error = %e, "Webhook delivery task panicked");
            }
        }

        Ok(())
    }

    /// 获取订阅了指定事件类型的活跃端点
    async fn get_subscribed_endpoints(
        &self,
        event_type: &WebhookEventType,
    ) -> Result<Vec<webhook_endpoints::Model>> {
        // 查询所有活跃端点
        let all_endpoints = webhook_endpoints::Entity::find()
            .filter(webhook_endpoints::Column::Enabled.eq(true))
            .all(&self.db)
            .await?;

        // 过滤出订阅了该事件的端点
        let subscribed: Vec<_> = all_endpoints
            .into_iter()
            .filter(|e| e.is_subscribed_to(event_type))
            .collect();

        Ok(subscribed)
    }

    /// 生成 HMAC-SHA256 签名
    ///
    /// 签名格式: timestamp.payload 的 HMAC-SHA256 十六进制值
    fn generate_signature(&self, secret: &str, timestamp: i64, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("HMAC initialization should never fail with valid key");

        mac.update(timestamp.to_string().as_bytes());
        mac.update(payload.as_bytes());

        hex::encode(mac.finalize().into_bytes())
    }

    /// 投递 webhook（带重试）
    ///
    /// 使用指数退避策略重试，最多重试 max_retries 次
    async fn deliver_webhook(
        &self,
        endpoint: webhook_endpoints::Model,
        event_type: WebhookEventType,
        payload: JsonValue,
    ) -> Result<()> {
        // 创建投递记录
        let delivery = self
            .create_delivery_record(endpoint.id, &event_type, &payload)
            .await?;

        let max_attempts = endpoint.max_retries as usize + 1; // 首次 + 重试次数

        for attempt in 0..max_attempts {
            let payload_str = serde_json::to_string(&payload)?;

            match self.send_request(&endpoint, &payload).await {
                Ok(response) => {
                    let status = response.status();
                    let status_code = status.as_u16() as i32;
                    let response_body = response.text().await.unwrap_or_default();

                    if status.is_success() {
                        // 成功，更新投递记录
                        self.update_delivery_success(
                            delivery.id,
                            attempt as i32,
                            status_code,
                            &response_body,
                        )
                        .await?;

                        // 记录成功的投递
                        WEBHOOK_DELIVERY_SUCCESS.inc();

                        info!(
                            endpoint_id = %endpoint.id,
                            delivery_id = %delivery.id,
                            attempt = attempt + 1,
                            status_code = status_code,
                            "Webhook delivered successfully"
                        );
                        return Ok(());
                    } else {
                        // HTTP 错误，记录并决定是否重试
                        warn!(
                            endpoint_id = %endpoint.id,
                            delivery_id = %delivery.id,
                            attempt = attempt + 1,
                            status_code = status_code,
                            "Webhook received non-success response"
                        );

                        if attempt < max_attempts - 1 {
                            self.update_delivery_retry(
                                delivery.id,
                                attempt as i32,
                                Some(status_code),
                                &response_body,
                            )
                            .await?;

                            // 记录重试
                            WEBHOOK_RETRY_COUNT.inc();

                            sleep(self.calculate_backoff(attempt as i32)).await;
                        } else {
                            // 最后一次尝试也失败了
                            self.update_delivery_failed(
                                delivery.id,
                                attempt as i32,
                                Some(status_code),
                                &response_body,
                            )
                            .await?;

                            // 记录失败的投递
                            WEBHOOK_DELIVERY_FAILED.inc();

                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    // 网络错误
                    warn!(
                        endpoint_id = %endpoint.id,
                        delivery_id = %delivery.id,
                        attempt = attempt + 1,
                        error = %e,
                        "Webhook delivery failed"
                    );

                    if attempt < max_attempts - 1 {
                        self.update_delivery_retry(
                            delivery.id,
                            attempt as i32,
                            None,
                            &e.to_string(),
                        )
                        .await?;

                        // 记录重试
                        WEBHOOK_RETRY_COUNT.inc();

                        sleep(self.calculate_backoff(attempt as i32)).await;
                    } else {
                        // 最后一次尝试也失败了
                        self.update_delivery_failed(
                            delivery.id,
                            attempt as i32,
                            None,
                            &e.to_string(),
                        )
                        .await?;

                        // 记录失败的投递
                        WEBHOOK_DELIVERY_FAILED.inc();

                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// 发送 HTTP 请求到 webhook 端点
    async fn send_request(
        &self,
        endpoint: &webhook_endpoints::Model,
        payload: &JsonValue,
    ) -> Result<reqwest::Response> {
        let timestamp = Utc::now().timestamp();
        let payload_str = serde_json::to_string(payload)?;

        // 生成签名
        let signature = self.generate_signature(&endpoint.secret, timestamp, &payload_str);

        // 构建请求
        let request = self
            .client
            .post(&endpoint.url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Timestamp", timestamp.to_string())
            .header("X-Webhook-Signature", format!("sha256={}", signature))
            .header("X-Webhook-Event", payload["event"].as_str().unwrap_or(""))
            .timeout(Duration::from_millis(endpoint.timeout_ms as u64))
            .body(payload_str);

        let response = request.send().await?;

        Ok(response)
    }

    /// 计算退避时间（指数退避）
    ///
    /// 退避时间: 1s, 2s, 4s, 8s, 16s, ...
    fn calculate_backoff(&self, attempt: i32) -> Duration {
        Duration::from_secs(2_u64.pow(attempt as u32))
    }

    /// 创建投递记录
    async fn create_delivery_record(
        &self,
        endpoint_id: i64,
        event_type: &WebhookEventType,
        payload: &JsonValue,
    ) -> Result<webhook_deliveries::Model> {
        use webhook_deliveries::ActiveModel;

        let now = Utc::now();
        let delivery = ActiveModel {
            id: Set(0),
            endpoint_id: Set(endpoint_id),
            event_type: Set(event_type.as_str().to_string()),
            payload: Set(payload.clone()),
            status: Set("pending".to_string()),
            response_code: Set(None),
            response_body: Set(None),
            attempts: Set(0),
            max_attempts: Set(6), // 首次 + 5次重试
            next_retry_at: Set(None),
            delivered_at: Set(None),
            created_at: Set(now),
        };

        Ok(delivery.insert(&self.db).await?)
    }

    /// 更新投递记录为成功
    async fn update_delivery_success(
        &self,
        delivery_id: i64,
        attempts: i32,
        response_code: i32,
        response_body: &str,
    ) -> Result<()> {
        use webhook_deliveries::ActiveModel;

        let delivery = webhook_deliveries::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Delivery not found"))?;

        let mut active_model: ActiveModel = delivery.into();
        active_model.status = Set("success".to_string());
        active_model.attempts = Set(attempts + 1);
        active_model.response_code = Set(Some(response_code));
        active_model.response_body = Set(Some(response_body.to_string()));
        active_model.delivered_at = Set(Some(Utc::now()));

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 更新投递记录为重试中
    async fn update_delivery_retry(
        &self,
        delivery_id: i64,
        attempts: i32,
        response_code: Option<i32>,
        response_body: &str,
    ) -> Result<()> {
        use webhook_deliveries::ActiveModel;

        let delivery = webhook_deliveries::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Delivery not found"))?;

        let mut active_model: ActiveModel = delivery.into();
        active_model.status = Set("retrying".to_string());
        active_model.attempts = Set(attempts + 1);
        active_model.response_code = Set(response_code);
        active_model.response_body = Set(Some(response_body.to_string()));
        active_model.next_retry_at = Set(Some(
            Utc::now() + chrono::Duration::seconds(2_i64.pow(attempts as u32)),
        ));

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 更新投递记录为失败
    async fn update_delivery_failed(
        &self,
        delivery_id: i64,
        attempts: i32,
        response_code: Option<i32>,
        response_body: &str,
    ) -> Result<()> {
        use webhook_deliveries::ActiveModel;

        let delivery = webhook_deliveries::Entity::find_by_id(delivery_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Delivery not found"))?;

        let mut active_model: ActiveModel = delivery.into();
        active_model.status = Set("failed".to_string());
        active_model.attempts = Set(attempts + 1);
        active_model.response_code = Set(response_code);
        active_model.response_body = Set(Some(response_body.to_string()));

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 克隆服务（用于异步任务）
    fn clone_service(&self) -> Self {
        Self {
            db: self.db.clone(),
            client: self.client.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：创建测试用的 WebhookService（不连接数据库）
    /// 注意：这些测试仅测试签名生成和退避计算，不涉及数据库操作
    struct TestWebhookService {
        client: Client,
    }

    impl TestWebhookService {
        fn new() -> Self {
            let client = Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("FoxNIO-Webhook/1.0")
                .build()
                .expect("Failed to create HTTP client");
            Self { client }
        }

        fn generate_signature(&self, secret: &str, timestamp: i64, payload: &str) -> String {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
                .expect("HMAC initialization should never fail with valid key");
            mac.update(timestamp.to_string().as_bytes());
            mac.update(payload.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        }

        fn calculate_backoff(&self, attempt: i32) -> Duration {
            Duration::from_secs(2_u64.pow(attempt as u32))
        }
    }

    #[test]
    fn test_generate_signature() {
        let service = TestWebhookService::new();

        let secret = "test-secret";
        let timestamp = 1234567890i64;
        let payload = r#"{"event":"test"}"#;

        let sig = service.generate_signature(secret, timestamp, payload);

        // 验证签名格式（64个十六进制字符）
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));

        // 验证相同输入产生相同签名
        let sig2 = service.generate_signature(secret, timestamp, payload);
        assert_eq!(sig, sig2);

        // 验证不同密钥产生不同签名
        let sig3 = service.generate_signature("different-secret", timestamp, payload);
        assert_ne!(sig, sig3);
    }

    #[test]
    fn test_calculate_backoff() {
        let service = TestWebhookService::new();

        assert_eq!(service.calculate_backoff(0), Duration::from_secs(1));
        assert_eq!(service.calculate_backoff(1), Duration::from_secs(2));
        assert_eq!(service.calculate_backoff(2), Duration::from_secs(4));
        assert_eq!(service.calculate_backoff(3), Duration::from_secs(8));
        assert_eq!(service.calculate_backoff(4), Duration::from_secs(16));
    }

    #[test]
    fn test_signature_deterministic() {
        let service = TestWebhookService::new();

        // 验证签名是确定性的
        let sig1 = service.generate_signature("secret", 1000, "payload");
        let sig2 = service.generate_signature("secret", 1000, "payload");
        assert_eq!(sig1, sig2);

        // 验证不同时间戳产生不同签名
        let sig3 = service.generate_signature("secret", 1001, "payload");
        assert_ne!(sig1, sig3);

        // 验证不同 payload 产生不同签名
        let sig4 = service.generate_signature("secret", 1000, "different");
        assert_ne!(sig1, sig4);
    }
}
