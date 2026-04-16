//! 支付业务服务
//!
//! 创建订单、处理回调、充值余额（原子事务）

use anyhow::{bail, Result};
use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::entity::{payment_orders, users};
use crate::service::balance_ledger::BalanceLedgerService;

use super::{PaymentRegistry, ORDER_COMPLETED, ORDER_EXPIRED, ORDER_PAID, ORDER_PENDING};

/// 支付业务服务
pub struct PaymentService {
    db: DatabaseConnection,
    registry: Arc<PaymentRegistry>,
    order_expire_minutes: i64,
}

impl PaymentService {
    pub fn new(db: DatabaseConnection, registry: Arc<PaymentRegistry>, order_expire_minutes: i64) -> Self {
        Self { db, registry, order_expire_minutes }
    }

    /// 创建支付订单
    pub async fn create_order(
        &self,
        user_id: Uuid,
        amount_cents: i64,
        provider_key: &str,
        payment_type: &str,
        currency: &str,
        notify_url: &str,
        return_url: Option<&str>,
    ) -> Result<payment_orders::Model> {
        if amount_cents <= 0 {
            bail!("Amount must be positive");
        }

        let provider = self.registry.get(provider_key)
            .ok_or_else(|| anyhow::anyhow!("Payment provider not found: {provider_key}"))?;

        let order_no = generate_order_no();
        let now = Utc::now();
        let expires_at = now + Duration::minutes(self.order_expire_minutes);

        // 调第三方创建支付
        let resp = provider.create_payment(super::CreatePaymentRequest {
            order_no: order_no.clone(),
            amount_cents,
            currency: currency.to_string(),
            description: format!("FoxNIO Recharge - {}", amount_cents as f64 / 100.0),
            return_url: return_url.map(|s| s.to_string()),
            notify_url: notify_url.to_string(),
        }).await?;

        // 入库
        let order = payment_orders::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            order_no: Set(order_no),
            provider: Set(provider_key.to_string()),
            payment_type: Set(payment_type.to_string()),
            amount_cents: Set(amount_cents),
            currency: Set(currency.to_string()),
            status: Set(ORDER_PENDING.to_string()),
            provider_order_id: Set(resp.provider_order_id),
            provider_data: Set(resp.provider_data),
            payment_url: Set(resp.payment_url),
            client_secret: Set(resp.client_secret),
            expires_at: Set(Some(expires_at)),
            paid_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let order = order.insert(&self.db).await?;
        tracing::info!("Payment order created: {} provider={} amount={}", order.order_no, provider_key, amount_cents);
        Ok(order)
    }

    /// 处理支付回调（验签 + 更新状态 + 充值余额，原子事务）
    pub async fn handle_webhook(
        &self,
        provider_key: &str,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<()> {
        let provider = self.registry.get(provider_key)
            .ok_or_else(|| anyhow::anyhow!("Payment provider not found: {provider_key}"))?;

        // 验签
        let event = provider.verify_webhook(headers, body).await?;

        // 查找订单
        let order = payment_orders::Entity::find()
            .filter(payment_orders::Column::OrderNo.eq(&event.order_no))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Order not found: {}", event.order_no))?;

        // 幂等：已完成的订单不重复处理
        if order.status == ORDER_COMPLETED || order.status == ORDER_PAID {
            tracing::info!("Order {} already processed, skipping", order.order_no);
            return Ok(());
        }

        if order.status != ORDER_PENDING {
            bail!("Order {} in unexpected status: {}", order.order_no, order.status);
        }

        if event.status != ORDER_PAID {
            // 非成功状态，只更新订单状态
            let mut model: payment_orders::ActiveModel = order.into();
            model.status = Set(event.status.clone());
            model.provider_order_id = Set(Some(event.provider_order_id));
            model.provider_data = Set(Some(event.raw_data));
            model.updated_at = Set(Utc::now());
            model.update(&self.db).await?;
            return Ok(());
        }

        // 支付成功 — 原子事务：更新订单 + 充值余额 + 审计记录
        let txn = self.db.begin().await?;

        let now = Utc::now();
        let mut model: payment_orders::ActiveModel = order.clone().into();
        model.status = Set(ORDER_COMPLETED.to_string());
        model.provider_order_id = Set(Some(event.provider_order_id));
        model.provider_data = Set(Some(event.raw_data));
        model.paid_at = Set(Some(now));
        model.completed_at = Set(Some(now));
        model.updated_at = Set(now);
        model.update(&txn).await?;

        // 充值余额
        let user = users::Entity::find_by_id(order.user_id)
            .one(&txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let new_balance = user.balance + order.amount_cents;
        let mut user_model: users::ActiveModel = user.clone().into();
        user_model.balance = Set(new_balance);
        user_model.updated_at = Set(now);
        user_model.update(&txn).await?;

        // 审计记录
        use crate::entity::balance_ledger;
        let ledger = balance_ledger::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(order.user_id),
            delta_cents: Set(order.amount_cents),
            balance_before: Set(user.balance),
            balance_after: Set(new_balance),
            source_type: Set("payment".to_string()),
            source_id: Set(Some(order.id.to_string())),
            description: Set(Some(format!(
                "Payment: {} {} via {}",
                order.amount_cents as f64 / 100.0,
                order.currency,
                order.provider
            ))),
            metadata: Set(None),
            created_at: Set(now),
        };
        ledger.insert(&txn).await?;

        txn.commit().await?;

        tracing::info!(
            "Payment completed: order={} user={} amount={} balance={}→{}",
            order.order_no, order.user_id, order.amount_cents, user.balance, new_balance
        );

        Ok(())
    }

    /// 查询订单
    pub async fn get_order(&self, order_id: Uuid, user_id: Uuid) -> Result<Option<payment_orders::Model>> {
        Ok(payment_orders::Entity::find_by_id(order_id)
            .filter(payment_orders::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?)
    }

    /// 用户订单列表
    pub async fn list_user_orders(&self, user_id: Uuid, page: u64, per_page: u64) -> Result<Vec<payment_orders::Model>> {
        Ok(payment_orders::Entity::find()
            .filter(payment_orders::Column::UserId.eq(user_id))
            .order_by_desc(payment_orders::Column::CreatedAt)
            .paginate(&self.db, per_page)
            .fetch_page(page.saturating_sub(1))
            .await?)
    }

    /// 过期待支付订单
    pub async fn expire_pending_orders(&self) -> Result<u64> {
        let now = Utc::now();
        let expired = payment_orders::Entity::find()
            .filter(payment_orders::Column::Status.eq(ORDER_PENDING))
            .filter(payment_orders::Column::ExpiresAt.lte(now))
            .all(&self.db)
            .await?;

        let count = expired.len() as u64;
        for order in expired {
            let mut model: payment_orders::ActiveModel = order.into();
            model.status = Set(ORDER_EXPIRED.to_string());
            model.updated_at = Set(now);
            model.update(&self.db).await?;
        }

        if count > 0 {
            tracing::info!("Expired {} pending payment orders", count);
        }
        Ok(count)
    }

    /// 可用支付方式
    pub fn available_providers(&self) -> Vec<String> {
        self.registry.available_providers()
    }
}

/// 生成业务订单号: FN + 时间戳 + 随机数
fn generate_order_no() -> String {
    let ts = Utc::now().format("%Y%m%d%H%M%S");
    let rand: u32 = rand::random::<u32>() % 999999;
    format!("FN{}{:06}", ts, rand)
}
