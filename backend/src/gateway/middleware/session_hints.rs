//! Session hints 中间件
//!
//! 从 HTTP headers 提取 IP / UA / x-session-id，注入 Extension

use axum::{body::Body, http::Request, middleware::Next, response::Response};

use crate::service::session_key::RequestSessionHints;

use super::audit::{extract_ip, extract_user_agent};

/// 提取 session hints 并注入 Extension
pub async fn session_hints_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let client_ip = extract_ip(&req);
    let user_agent = extract_user_agent(&req);
    let x_session_id = req
        .headers()
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let hints = RequestSessionHints {
        metadata_session_id: None, // 由 forwarder 从 body 解析后补充
        x_session_id,
        client_ip,
        user_agent,
    };

    req.extensions_mut().insert(hints);
    next.run(req).await
}
