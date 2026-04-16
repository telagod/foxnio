//! 支付 HTTP Handler

use axum::{
    extract::{Extension, Path},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::gateway::SharedState;
use crate::service::payment::service::PaymentService;
use crate::service::payment::PaymentRegistry;
use crate::service::user::Claims;

use super::ApiError;

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub amount_cents: i64,
    pub provider: String,
    #[serde(default = "default_payment_type")]
    pub payment_type: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub return_url: Option<String>,
}

fn default_payment_type() -> String { "balance".to_string() }
fn default_currency() -> String { "CNY".to_string() }

/// 创建支付订单
pub async fn create_order(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let registry = build_registry(&state);
    let service = PaymentService::new(state.db.clone(), Arc::new(registry), 30);

    let notify_base = std::env::var("FOXNIO_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    let notify_url = format!("{}/api/v1/payment/webhook/{}", notify_base, req.provider);

    let order = service
        .create_order(
            user_id,
            req.amount_cents,
            &req.provider,
            &req.payment_type,
            &req.currency,
            &notify_url,
            req.return_url.as_deref(),
        )
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "id": order.id,
        "order_no": order.order_no,
        "status": order.status,
        "amount_cents": order.amount_cents,
        "currency": order.currency,
        "provider": order.provider,
        "payment_url": order.payment_url,
        "client_secret": order.client_secret,
        "expires_at": order.expires_at.map(|t| t.to_rfc3339()),
    })))
}

/// 支付回调（公开，验签）
pub async fn webhook(
    Extension(state): Extension<SharedState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<Value>, ApiError> {
    let registry = build_registry(&state);
    let service = PaymentService::new(state.db.clone(), Arc::new(registry), 30);

    service
        .handle_webhook(&provider, &headers, &body)
        .await
        .map_err(|e| {
            tracing::error!("Payment webhook error: {e}");
            ApiError(StatusCode::BAD_REQUEST, e.to_string())
        })?;

    Ok(Json(json!({"success": true})))
}

/// 用户订单列表
pub async fn list_orders(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let registry = build_registry(&state);
    let service = PaymentService::new(state.db.clone(), Arc::new(registry), 30);

    let orders = service
        .list_user_orders(user_id, 1, 50)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let data: Vec<Value> = orders
        .iter()
        .map(|o| {
            json!({
                "id": o.id,
                "order_no": o.order_no,
                "provider": o.provider,
                "amount_cents": o.amount_cents,
                "currency": o.currency,
                "status": o.status,
                "created_at": o.created_at.to_rfc3339(),
                "paid_at": o.paid_at.map(|t| t.to_rfc3339()),
            })
        })
        .collect();

    Ok(Json(json!({"data": data, "total": data.len()})))
}

/// 订单详情
pub async fn get_order(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let order_id = Uuid::parse_str(&id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let registry = build_registry(&state);
    let service = PaymentService::new(state.db.clone(), Arc::new(registry), 30);

    let order = service
        .get_order(order_id, user_id)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Order not found".into()))?;

    Ok(Json(json!({
        "id": order.id,
        "order_no": order.order_no,
        "provider": order.provider,
        "payment_type": order.payment_type,
        "amount_cents": order.amount_cents,
        "currency": order.currency,
        "status": order.status,
        "payment_url": order.payment_url,
        "client_secret": order.client_secret,
        "created_at": order.created_at.to_rfc3339(),
        "paid_at": order.paid_at.map(|t| t.to_rfc3339()),
        "completed_at": order.completed_at.map(|t| t.to_rfc3339()),
    })))
}

/// 可用支付方式
pub async fn get_config(
    Extension(state): Extension<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let registry = build_registry(&state);
    let providers = registry.available_providers();

    Ok(Json(json!({
        "providers": providers,
        "currency": "CNY",
    })))
}

/// 从配置构建 PaymentRegistry
fn build_registry(state: &SharedState) -> PaymentRegistry {
    use crate::service::payment::{stripe::StripeProvider, alipay::AlipayProvider, wxpay::WxPayProvider, easypay::EasyPayProvider};

    let mut registry = PaymentRegistry::new();

    // Stripe
    if let (Ok(sk), Ok(pk), Ok(ws)) = (
        std::env::var("STRIPE_SECRET_KEY"),
        std::env::var("STRIPE_PUBLISHABLE_KEY"),
        std::env::var("STRIPE_WEBHOOK_SECRET"),
    ) {
        if !sk.is_empty() {
            let currency = std::env::var("STRIPE_CURRENCY").unwrap_or_else(|_| "cny".to_string());
            registry.register(Arc::new(StripeProvider::new(sk, pk, ws, currency)));
        }
    }

    // Alipay
    if let (Ok(app_id), Ok(priv_key), Ok(pub_key)) = (
        std::env::var("ALIPAY_APP_ID"),
        std::env::var("ALIPAY_PRIVATE_KEY"),
        std::env::var("ALIPAY_PUBLIC_KEY"),
    ) {
        if !app_id.is_empty() {
            let notify = std::env::var("ALIPAY_NOTIFY_URL").unwrap_or_default();
            registry.register(Arc::new(AlipayProvider::new(app_id, priv_key, pub_key, notify)));
        }
    }

    // WxPay
    if let (Ok(app_id), Ok(mch_id), Ok(api_key)) = (
        std::env::var("WXPAY_APP_ID"),
        std::env::var("WXPAY_MCH_ID"),
        std::env::var("WXPAY_API_KEY"),
    ) {
        if !app_id.is_empty() {
            let notify = std::env::var("WXPAY_NOTIFY_URL").unwrap_or_default();
            registry.register(Arc::new(WxPayProvider::new(app_id, mch_id, api_key, notify)));
        }
    }

    // EasyPay
    if let (Ok(merchant_id), Ok(api_key)) = (
        std::env::var("EASYPAY_MERCHANT_ID"),
        std::env::var("EASYPAY_API_KEY"),
    ) {
        if !merchant_id.is_empty() {
            let notify = std::env::var("EASYPAY_NOTIFY_URL").unwrap_or_default();
            registry.register(Arc::new(EasyPayProvider::new(merchant_id, api_key, notify)));
        }
    }

    registry
}
