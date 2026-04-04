//! 路由配置 - 完整实现
//!
//! 使用角色权限系统进行路由保护

use axum::{
    http::StatusCode,
    routing::{delete, get, post, put},
    Extension, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::{
    middleware,
    websocket::{self, WSConfig},
    SharedState,
};
use crate::gateway::middleware::permission::check_permission;
use crate::handler;
use crate::health::HealthChecker;
use crate::service::permission::Permission;
use crate::service::{
    LegacyApiKeyService as ApiKeyService, LegacyBillingService as BillingService,
};
use crate::state::AppState;

pub fn build_app(state: AppState, health_checker: Arc<HealthChecker>) -> Router<()> {
    let shared_state = Arc::new(state);

    // 公开路由
    let public_routes = Router::new()
        // Health check endpoints
        .route("/health", get(handler::health_simple))
        .route("/health/live", get(handler::health_live))
        .route("/health/ready", get(handler::health_ready))
        .route("/health/detailed", get(handler::health_detailed))
        .route("/health/resources", get(handler::health_resources))
        .route("/health/database", get(handler::health_database))
        .route("/health/redis", get(handler::health_redis))
        .route("/health/info", get(handler::app_info))
        // Prometheus 指标端点
        .route("/metrics", get(handler::metrics::prometheus_metrics))
        // API 端点（OpenAI 兼容）
        .route("/v1/models", get(handler::list_models))
        // 认证
        .route("/api/v1/auth/register", post(handler::auth::register))
        .route("/api/v1/auth/login", post(handler::auth::login))
        // 认证 - Token 刷新和登出
        .route("/api/v1/auth/refresh", post(handler::auth::refresh))
        .route("/api/v1/auth/logout", post(handler::auth::logout))
        // 验证码发送
        .route(
            "/api/v1/auth/send-verify-code",
            post(handler::verify::send_verify_code),
        )
        // 优惠码验证（公开）
        .route(
            "/api/v1/auth/validate-promo-code",
            post(handler::verify::validate_promo_code),
        )
        // 邀请码验证（公开）
        .route(
            "/api/v1/auth/validate-invitation-code",
            post(handler::verify::validate_invitation_code),
        )
        // TOTP 登录（公开，使用临时 token）
        .route("/api/v1/auth/totp/login", post(handler::auth::totp_login))
        .route(
            "/api/v1/auth/totp/backup-login",
            post(handler::auth::backup_code_login),
        )
        // 密码重置
        .route(
            "/api/v1/auth/password/reset-request",
            post(handler::auth::password::request_reset),
        )
        .route(
            "/api/v1/auth/password/verify-token",
            post(handler::auth::password::verify_token),
        )
        .route(
            "/api/v1/auth/password/reset",
            post(handler::auth::password::reset_password),
        );

    // 需要认证的路由
    let auth_routes = Router::new()
        // 用户信息
        .route("/api/v1/user/me", get(handler::auth::get_me))
        .route("/api/v1/user/usage", get(get_user_usage))
        // 用户个人信息管理
        .route("/api/v1/user", put(handler::user::update_user_info))
        .route("/api/v1/user/password", put(handler::user::change_password))
        // 用户审计日志
        .route(
            "/api/v1/users/me/audit-logs",
            get(handler::list_my_audit_logs),
        )
        // TOTP 两步验证管理
        .route("/api/v1/auth/totp/enable", post(handler::auth::enable_totp))
        .route(
            "/api/v1/auth/totp/confirm",
            post(handler::auth::confirm_enable_totp),
        )
        .route(
            "/api/v1/auth/totp/disable",
            post(handler::auth::disable_totp),
        )
        .route("/api/v1/auth/totp/verify", post(handler::auth::verify_totp))
        .route(
            "/api/v1/auth/totp/status",
            get(handler::auth::get_totp_status),
        )
        .route(
            "/api/v1/auth/totp/backup-codes/regenerate",
            post(handler::auth::regenerate_backup_codes),
        )
        // Chat completions (需要 API Key)
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .route("/v1/completions", post(handle_completions))
        // API Keys
        .route("/api/v1/user/apikeys", get(list_user_apikeys))
        .route("/api/v1/user/apikeys", post(create_user_apikey))
        .route("/api/v1/user/apikeys/:id", delete(delete_user_apikey))
        .route("/api/v1/user/apikeys/:id", put(update_user_apikey))
        // 用户端分组信息
        .route(
            "/api/v1/groups/available",
            get(handler::user_groups::list_available_groups),
        )
        .route(
            "/api/v1/groups/rates",
            get(handler::user_groups::list_group_rates),
        )
        // 用户端公告
        .route(
            "/api/v1/announcements",
            get(handler::user_announcement::list_user_announcements),
        )
        // 用户端订阅
        .route(
            "/api/v1/subscriptions",
            get(handler::subscription::list_user_subscriptions),
        )
        .route(
            "/api/v1/subscriptions/:id",
            get(handler::subscription::get_subscription_detail),
        )
        // 卡密兑换（用户端）
        .route("/api/v1/redeem", post(handler::redeem::redeem_code))
        .route(
            "/api/v1/redeem/history",
            get(handler::redeem::get_redemption_history),
        )
        // Sora 图片/视频生成
        .route("/v1/images/generations", post(handle_sora_image_generation))
        .route("/v1/videos/generations", post(handle_sora_video_generation))
        .route(
            "/v1/videos/generations/:id",
            get(get_sora_generation_status),
        )
        .route("/v1/prompts/enhance", post(handle_prompt_enhance))
        // Sora 模型列表
        .route("/v1/sora/models", get(list_sora_models))
        .route("/v1/sora/families", get(list_sora_families))
        .layer(axum::middleware::from_fn(middleware::jwt_auth));

    // 管理后台路由 - 使用权限系统
    let admin_routes = Router::new()
        // 用户管理 - 需要 UserRead/Write/Delete 权限
        .route("/api/v1/admin/users", get(handler::admin::list_users))
        .route("/api/v1/admin/users", post(handler::admin::create_user))
        .route("/api/v1/admin/users/:id", get(handler::admin::get_user))
        .route("/api/v1/admin/users/:id", put(handler::admin::update_user))
        .route(
            "/api/v1/admin/users/:id",
            delete(handler::admin::delete_user),
        )
        .route(
            "/api/v1/admin/users/:id/balance",
            post(handler::admin::update_user_balance),
        )
        // 账号管理 - 需要 AccountRead/Write 权限
        .route("/api/v1/admin/accounts", get(handler::admin::list_accounts))
        .route("/api/v1/admin/accounts", post(handler::admin::add_account))
        .route(
            "/api/v1/admin/accounts/batch",
            post(handler::admin_accounts::batch_create_accounts),
        )
        .route("/api/v1/admin/accounts/:id", get(get_account_detail))
        .route("/api/v1/admin/accounts/:id", put(update_account))
        .route(
            "/api/v1/admin/accounts/:id",
            delete(handler::admin::delete_account_by_id),
        )
        .route("/api/v1/admin/accounts/test", post(test_account))
        // 账号操作端点
        .route(
            "/api/v1/admin/accounts/:id/refresh",
            post(handler::admin_accounts::refresh_account_token),
        )
        .route(
            "/api/v1/admin/accounts/:id/recover-state",
            post(handler::admin_accounts::recover_account_state),
        )
        .route(
            "/api/v1/admin/accounts/:id/set-privacy",
            post(handler::admin_accounts::set_account_privacy),
        )
        .route(
            "/api/v1/admin/accounts/:id/refresh-tier",
            post(handler::admin_accounts::refresh_account_tier),
        )
        .route(
            "/api/v1/admin/accounts/:id/clear-error",
            post(handler::admin_accounts::clear_account_error),
        )
        .route(
            "/api/v1/admin/accounts/:id/usage",
            get(handler::admin_accounts::get_account_usage),
        )
        .route(
            "/api/v1/admin/accounts/:id/today-stats",
            get(handler::admin_accounts::get_account_today_stats),
        )
        .route(
            "/api/v1/admin/accounts/today-stats/batch",
            post(handler::admin_accounts::batch_get_today_stats),
        )
        .route(
            "/api/v1/admin/accounts/:id/clear-rate-limit",
            post(handler::admin_accounts::clear_account_rate_limit),
        )
        .route(
            "/api/v1/admin/accounts/:id/reset-quota",
            post(handler::admin_accounts::reset_account_quota),
        )
        .route(
            "/api/v1/admin/accounts/data",
            get(handler::admin_accounts::export_accounts_data),
        )
        .route(
            "/api/v1/admin/accounts/data",
            post(handler::admin_accounts::import_accounts_data),
        )
        .route(
            "/api/v1/admin/accounts/batch-update-credentials",
            post(handler::admin_accounts::batch_update_credentials),
        )
        .route(
            "/api/v1/admin/accounts/batch-refresh-tier",
            post(handler::admin_accounts::batch_refresh_tier),
        )
        // 高性能批量导入 - 支持几千到几万账号
        // 暂时禁用，等待修复 Send trait 问题
        // .route(
        //     "/api/v1/admin/accounts/fast-import",
        //     post(handler::admin_accounts::fast_import_accounts),
        // )
        // API Key 管理 - 需要 ApiKeyRead 权限
        .route("/api/v1/admin/apikeys", get(handler::admin::list_apikeys))
        // 统计和监控 - 需要 BillingRead 权限
        .route("/api/v1/admin/stats", get(handler::admin::get_stats))
        .route(
            "/api/v1/admin/dashboard",
            get(handler::admin::get_dashboard),
        )
        // 指标监控端点
        .route("/api/v1/admin/metrics", get(handler::metrics::json_metrics))
        .route(
            "/api/v1/admin/metrics/detail",
            get(handler::metrics::detailed_metrics),
        )
        .route(
            "/api/v1/admin/metrics/health",
            get(handler::metrics::metrics_health),
        )
        .route(
            "/api/v1/admin/metrics/realtime",
            get(handler::metrics::realtime_metrics),
        )
        .route(
            "/api/v1/admin/metrics/cost",
            get(handler::metrics::cost_metrics),
        )
        .route(
            "/api/v1/admin/metrics/tokens",
            get(handler::metrics::token_metrics),
        )
        .route(
            "/api/v1/admin/metrics/accounts",
            get(handler::metrics::account_metrics),
        )
        // 权限管理
        .route(
            "/api/v1/admin/permissions/matrix",
            get(handler::admin::get_permission_matrix),
        )
        .route("/api/v1/admin/roles", get(handler::admin::list_roles))
        // 审计日志管理
        .route("/api/v1/admin/audit-logs", get(handler::list_audit_logs))
        .route(
            "/api/v1/admin/audit-logs/stats",
            get(handler::get_audit_stats),
        )
        .route(
            "/api/v1/admin/audit-logs/sensitive",
            get(handler::list_sensitive_audit_logs),
        )
        .route(
            "/api/v1/admin/audit-logs/users/:user_id",
            get(handler::list_user_audit_logs),
        )
        .route(
            "/api/v1/admin/audit-logs/cleanup",
            post(handler::cleanup_audit_logs),
        )
        // 告警管理
        .route(
            "/api/v1/admin/alerts/rules",
            get(handler::alerts::list_rules),
        )
        .route(
            "/api/v1/admin/alerts/rules",
            post(handler::alerts::create_rule),
        )
        .route(
            "/api/v1/admin/alerts/rules/:id",
            delete(handler::alerts::delete_rule),
        )
        .route(
            "/api/v1/admin/alerts/rules/:id",
            put(handler::alerts::update_rule),
        )
        .route(
            "/api/v1/admin/alerts/silences",
            get(handler::alerts::list_silences),
        )
        .route(
            "/api/v1/admin/alerts/silences",
            post(handler::alerts::create_silence),
        )
        .route(
            "/api/v1/admin/alerts/silences/:id",
            delete(handler::alerts::delete_silence),
        )
        .route(
            "/api/v1/admin/alerts/history",
            get(handler::alerts::list_history),
        )
        .route(
            "/api/v1/admin/alerts/stats",
            get(handler::alerts::get_stats),
        )
        .route(
            "/api/v1/admin/alerts/channels",
            get(handler::alerts::list_channels),
        )
        .route(
            "/api/v1/admin/alerts/channels",
            post(handler::alerts::register_channel),
        )
        .route(
            "/api/v1/admin/alerts/channels/:id",
            delete(handler::alerts::delete_channel),
        )
        .route(
            "/api/v1/admin/alerts/test",
            post(handler::alerts::test_alert),
        )
        // 模型管理
        .route(
            "/api/v1/admin/models",
            get(handler::models::list_models_admin),
        )
        .route("/api/v1/admin/models", post(handler::models::create_model))
        .route("/api/v1/admin/models/:id", get(handler::models::get_model))
        .route(
            "/api/v1/admin/models/:id",
            put(handler::models::update_model),
        )
        .route(
            "/api/v1/admin/models/:id",
            delete(handler::models::delete_model),
        )
        .route(
            "/api/v1/admin/models/reload",
            post(handler::models::reload_models),
        )
        .route(
            "/api/v1/admin/models/import-defaults",
            post(handler::models::import_default_models),
        )
        .route(
            "/api/v1/admin/models/:name/route",
            get(handler::models::get_model_route),
        )
        // 代理管理 API
        .route("/api/v1/admin/proxies", get(handler::proxy::list_proxies))
        .route("/api/v1/admin/proxies", post(handler::proxy::create_proxy))
        .route("/api/v1/admin/proxies/:id", get(handler::proxy::get_proxy))
        .route(
            "/api/v1/admin/proxies/:id",
            put(handler::proxy::update_proxy),
        )
        .route(
            "/api/v1/admin/proxies/:id",
            delete(handler::proxy::delete_proxy),
        )
        .route(
            "/api/v1/admin/proxies/:id/test",
            post(handler::proxy::test_proxy),
        )
        .route(
            "/api/v1/admin/proxies/test-all",
            post(handler::proxy::test_all_proxies),
        )
        .route(
            "/api/v1/admin/proxies/:id/quality",
            get(handler::proxy::get_proxy_quality),
        )
        // 卡密兑换管理 API（管理端）
        .route(
            "/api/v1/admin/redeem/generate",
            post(handler::redeem::admin_generate_codes),
        )
        .route(
            "/api/v1/admin/redeem/stats",
            get(handler::redeem::admin_get_redeem_stats),
        )
        .route(
            "/api/v1/admin/redeem/cancel",
            post(handler::redeem::admin_cancel_code),
        )
        // 配额管理 API
        .route("/api/v1/quota", get(handler::quota::get_user_quota))
        .route("/api/v1/quota", post(handler::quota::update_user_quota))
        .route(
            "/api/v1/admin/quota/:user_id/reset",
            post(handler::quota::reset_user_quota),
        )
        .route(
            "/api/v1/admin/quota/:user_id/history",
            get(handler::quota::get_quota_history),
        )
        .route(
            "/api/v1/admin/quota/stats",
            get(handler::quota::get_quota_stats),
        )
        // 公告管理 API
        .route(
            "/api/v1/admin/announcements",
            get(handler::announcement::list_announcements),
        )
        .route(
            "/api/v1/admin/announcements",
            post(handler::announcement::create_announcement),
        )
        .route(
            "/api/v1/admin/announcements/:id",
            get(handler::announcement::get_announcement),
        )
        .route(
            "/api/v1/admin/announcements/:id",
            put(handler::announcement::update_announcement),
        )
        .route(
            "/api/v1/admin/announcements/:id",
            delete(handler::announcement::delete_announcement),
        )
        .route(
            "/api/v1/announcements/:id/read",
            post(handler::announcement::mark_announcement_read),
        )
        .route(
            "/api/v1/announcements/unread",
            get(handler::announcement::get_unread_announcements),
        )
        // 优惠码管理 API
        .route(
            "/api/v1/admin/promo-codes",
            get(handler::promo_code::list_promo_codes),
        )
        .route(
            "/api/v1/admin/promo-codes",
            post(handler::promo_code::create_promo_code),
        )
        .route(
            "/api/v1/admin/promo-codes/:id",
            get(handler::promo_code::get_promo_code),
        )
        .route(
            "/api/v1/admin/promo-codes/:id",
            put(handler::promo_code::update_promo_code),
        )
        .route(
            "/api/v1/admin/promo-codes/:id",
            delete(handler::promo_code::delete_promo_code),
        )
        .route(
            "/api/v1/promo-codes/verify",
            post(handler::promo_code::verify_promo_code),
        )
        // 用户属性 API
        .route(
            "/api/v1/admin/attributes/definitions",
            post(handler::user_attribute::create_attribute_definition),
        )
        .route(
            "/api/v1/admin/attributes/definitions",
            get(handler::user_attribute::list_attribute_definitions),
        )
        .route(
            "/api/v1/admin/attributes/definitions/:id",
            put(handler::user_attribute::update_attribute_definition),
        )
        .route(
            "/api/v1/admin/attributes/definitions/:id",
            delete(handler::user_attribute::delete_attribute_definition),
        )
        .route(
            "/api/v1/user/attributes",
            post(handler::user_attribute::set_user_attribute),
        )
        .route(
            "/api/v1/user/attributes",
            get(handler::user_attribute::get_user_attributes),
        )
        // 错误透传规则 API
        .route(
            "/api/v1/admin/error-rules",
            post(handler::error_passthrough_rule::create_error_rule),
        )
        .route(
            "/api/v1/admin/error-rules",
            get(handler::error_passthrough_rule::list_error_rules),
        )
        .route(
            "/api/v1/admin/error-rules/:id",
            put(handler::error_passthrough_rule::update_error_rule),
        )
        .route(
            "/api/v1/admin/error-rules/:id",
            delete(handler::error_passthrough_rule::delete_error_rule),
        )
        .route(
            "/api/v1/error-rules/apply",
            post(handler::error_passthrough_rule::apply_error_rules),
        )
        // 定时测试计划 API
        .route(
            "/api/v1/admin/test-plans",
            post(handler::scheduled_test_plan::create_test_plan),
        )
        .route(
            "/api/v1/admin/test-plans",
            get(handler::scheduled_test_plan::list_test_plans),
        )
        .route(
            "/api/v1/admin/test-plans/:id",
            put(handler::scheduled_test_plan::update_test_plan),
        )
        .route(
            "/api/v1/admin/test-plans/:id",
            delete(handler::scheduled_test_plan::delete_test_plan),
        )
        .route(
            "/api/v1/admin/test-plans/record",
            post(handler::scheduled_test_plan::record_test_result),
        )
        .route(
            "/api/v1/admin/test-plans/:id/results",
            get(handler::scheduled_test_plan::get_test_results),
        )
        // 数据备份 API
        .route(
            "/api/v1/admin/backup/export",
            post(handler::backup::export_data),
        )
        .route(
            "/api/v1/admin/backup/import",
            post(handler::backup::import_data),
        )
        // 分组管理扩展 API
        .route(
            "/api/v1/admin/groups/usage-summary",
            get(handler::admin_groups::get_groups_usage_summary),
        )
        .route(
            "/api/v1/admin/groups/capacity-summary",
            get(handler::admin_groups::get_groups_capacity_summary),
        )
        .route(
            "/api/v1/admin/groups/sort-order",
            put(handler::admin_groups::update_groups_sort_order),
        )
        .route(
            "/api/v1/admin/groups/:id/stats",
            get(handler::admin_groups::get_group_stats),
        )
        .route(
            "/api/v1/admin/groups/:id/rate-multipliers",
            get(handler::admin_groups::get_group_rate_multipliers),
        )
        .route(
            "/api/v1/admin/groups/:id/api-keys",
            get(handler::admin_groups::get_group_api_keys),
        )
        .route(
            "/api/v1/admin/groups/all",
            get(handler::admin_groups::list_all_groups),
        )
        .route("/api/v1/admin/groups", post(handler::groups::create_group))
        .route(
            "/api/v1/admin/groups/:id",
            put(handler::groups::update_group),
        )
        .route(
            "/api/v1/admin/groups/:id",
            delete(handler::groups::delete_group),
        )
        .layer(axum::middleware::from_fn(middleware::jwt_auth));

    // WebSocket 路由 - OpenAI Realtime/Responses API
    let _ws_handler = Arc::new(websocket::create_handler(WSConfig::default()));
    let ws_routes = Router::new()
        // OpenAI Realtime API v1 - WebSocket
        .route("/v1/realtime", get(websocket::handler::ws_realtime_v1))
        // OpenAI Responses API v2 - WebSocket
        .route("/v1/responses", get(websocket::handler::ws_responses_v2))
        // WebSocket 连接池统计
        .route(
            "/api/v1/ws/pool/stats",
            get(websocket::handler::ws_pool_stats),
        );

    // Webhook 路由 - 需要用户认证
    let webhook_routes = Router::new()
        .route("/api/v1/webhooks", post(handler::webhook::create_webhook))
        .route("/api/v1/webhooks", get(handler::webhook::list_webhooks))
        .route("/api/v1/webhooks/:id", get(handler::webhook::get_webhook))
        .route(
            "/api/v1/webhooks/:id",
            put(handler::webhook::update_webhook),
        )
        .route(
            "/api/v1/webhooks/:id",
            delete(handler::webhook::delete_webhook),
        )
        .route(
            "/api/v1/webhooks/:id/test",
            post(handler::webhook::test_webhook),
        )
        .route(
            "/api/v1/webhooks/:id/deliveries",
            get(handler::webhook::list_deliveries),
        )
        .layer(axum::middleware::from_fn(middleware::jwt_auth));

    // 批量操作路由 - 需要管理员权限（权限检查在 handler 内部）
    let batch_routes = Router::new()
        .route(
            "/api/v1/admin/api-keys/batch-create",
            post(handler::batch::batch_create_api_keys),
        )
        .route(
            "/api/v1/admin/accounts/batch-update",
            post(handler::batch::batch_update_accounts),
        )
        .route(
            "/api/v1/admin/users/batch-import",
            post(handler::batch::batch_import_users),
        )
        .route(
            "/api/v1/admin/api-keys/batch-delete",
            post(handler::batch::batch_delete_api_keys),
        )
        .layer(axum::middleware::from_fn(middleware::jwt_auth));

    // Gemini Native API 路由（v1beta）
    // 注意：axum 路由不支持 {model}:action 格式，改用查询参数区分
    let gemini_routes = Router::new()
        // 模型列表和详情
        .route("/v1beta/models", get(super::gemini::list_models))
        .route("/v1beta/models/{model}", get(super::gemini::get_model))
        // 内容生成（通过查询参数 ?action=generateContent 或 ?action=streamGenerateContent 区分）
        .route(
            "/v1beta/models/{model}",
            post(super::gemini::generate_content),
        )
        // Token 计数
        .route(
            "/v1beta/models/{model}/countTokens",
            post(super::gemini::count_tokens),
        )
        // 内容嵌入
        .route(
            "/v1beta/models/{model}/embedContent",
            post(super::gemini::embed_content),
        );

    Router::new()
        .merge(public_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .merge(ws_routes)
        .merge(gemini_routes)
        .merge(webhook_routes)
        .merge(batch_routes)
        // Swagger UI - OpenAPI 文档
        // TODO: Fix Swagger UI integration
        // .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Responses API - 直接添加路由
        .route(
            "/v1/responses",
            post(super::responses_handler::handle_responses),
        )
        // Layers - 压缩中间件
        .layer(axum::middleware::from_fn(
            middleware::compression_middleware,
        ))
        // Layers
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(middleware::request_log))
        .layer(axum::middleware::from_fn(middleware::request_id))
        // 添加共享状态和健康检查器扩展
        .layer(Extension(shared_state))
        .layer(Extension(health_checker))
}

// ============ 网关端点 ============

// ============ 网关端点 ============

async fn handle_chat_completions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use crate::service::chat_completions_forwarder::{
        ChatCompletionsForwarder, ChatCompletionsRequest,
    };

    // 解析请求体
    let request: ChatCompletionsRequest = serde_json::from_slice(&body).map_err(|e| {
        handler::ApiError(StatusCode::BAD_REQUEST, format!("Invalid request: {}", e))
    })?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 创建转发器
    let account_service = crate::service::AccountService::new(state.db.clone());
    let scheduler = crate::service::SchedulerService::new(
        state.db.clone(),
        account_service.clone(),
        crate::service::scheduler::SchedulingStrategy::RoundRobin,
    );

    let forwarder =
        ChatCompletionsForwarder::new(state.db.clone(), Arc::new(account_service), scheduler);

    // TODO: 从 API Key 中获取 api_key_id
    let api_key_id = uuid::Uuid::nil();

    // 转发请求
    match forwarder.forward(request, user_id, api_key_id).await {
        Ok(result) => {
            // 返回成功响应
            Ok(axum::Json(serde_json::json!({
                "id": result.request_id,
                "object": "chat.completion",
                "created": chrono::Utc::now().timestamp() as u64,
                "model": result.model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Response from upstream"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": result.usage.prompt_tokens,
                    "completion_tokens": result.usage.completion_tokens,
                    "total_tokens": result.usage.total_tokens
                }
            })))
        }
        Err(e) => {
            tracing::error!("Chat completions forwarding failed: {}", e);
            Err(handler::ApiError(
                StatusCode::BAD_GATEWAY,
                format!("Upstream error: {}", e),
            ))
        }
    }
}

async fn handle_messages(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use crate::service::anthropic_messages_forwarder::{
        AnthropicMessagesForwarder, AnthropicMessagesRequest,
    };

    let request: AnthropicMessagesRequest = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 创建转发器
    let account_service = crate::service::AccountService::new(state.db.clone());
    let scheduler = crate::service::SchedulerService::new(
        state.db.clone(),
        account_service.clone(),
        crate::service::scheduler::SchedulingStrategy::RoundRobin,
    );

    let forwarder =
        AnthropicMessagesForwarder::new(state.db.clone(), Arc::new(account_service), scheduler);

    // TODO: 从 API Key 中获取 api_key_id
    let api_key_id = uuid::Uuid::nil();

    // 转发请求
    match forwarder.forward(request, user_id, api_key_id).await {
        Ok(result) => {
            // 返回 Anthropic 格式的响应
            Ok(axum::Json(serde_json::json!({
                "id": result.request_id,
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": result.content
                }],
                "model": result.model,
                "usage": {
                    "input_tokens": result.usage.input_tokens,
                    "output_tokens": result.usage.output_tokens
                }
            })))
        }
        Err(e) => {
            tracing::error!("Messages forwarding error: {}", e);
            Err(handler::ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ))
        }
    }
}

async fn handle_completions(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // 旧版 completions API - 转换为 chat/completions 格式
    use crate::service::chat_completions_forwarder::{
        ChatCompletionsForwarder, ChatCompletionsRequest, Message, MessageContent,
    };

    let req: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 转换 completions 格式到 chat/completions 格式
    let prompt = req
        .get("prompt")
        .and_then(|p| p.as_str())
        .ok_or(handler::ApiError(
            StatusCode::BAD_REQUEST,
            "Missing prompt".into(),
        ))?;

    let model = req
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gpt-3.5-turbo-instruct");

    // 构建 chat/completions 请求
    let chat_request = ChatCompletionsRequest {
        model: model.to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text(prompt.to_string()),
        }],
        temperature: req
            .get("temperature")
            .and_then(|t| t.as_f64())
            .map(|v| v as f32),
        max_tokens: req
            .get("max_tokens")
            .and_then(|t| t.as_u64())
            .map(|v| v as u32),
        stream: req.get("stream").and_then(|s| s.as_bool()).unwrap_or(false),
        stream_options: None,
        extra: req.clone(),
    };

    // 创建转发器
    let account_service = crate::service::AccountService::new(state.db.clone());
    let scheduler = crate::service::SchedulerService::new(
        state.db.clone(),
        account_service.clone(),
        crate::service::scheduler::SchedulingStrategy::RoundRobin,
    );

    let forwarder =
        ChatCompletionsForwarder::new(state.db.clone(), Arc::new(account_service), scheduler);

    let api_key_id = uuid::Uuid::nil();

    // 转发请求
    match forwarder.forward(chat_request, user_id, api_key_id).await {
        Ok(result) => {
            // 转换响应回 completions 格式
            Ok(axum::Json(serde_json::json!({
                "id": result.request_id,
                "object": "text_completion",
                "created": chrono::Utc::now().timestamp() as u64,
                "model": result.model,
                "choices": [{
                    "text": "",  // 需要从实际响应中提取
                    "index": 0,
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": result.usage.prompt_tokens,
                    "completion_tokens": result.usage.completion_tokens,
                    "total_tokens": result.usage.total_tokens
                }
            })))
        }
        Err(e) => {
            tracing::error!("Completions forwarding error: {}", e);
            Err(handler::ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ))
        }
    }
}

// ============ 用户端点 ============

async fn get_user_usage(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Query(query): axum::extract::Query<UserUsageQuery>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let billing_service =
        BillingService::new(state.db.clone(), state.config.gateway.rate_multiplier);

    let days = query.days.unwrap_or(30).clamp(1, 90) as i32;
    let report = billing_service
        .get_user_usage_report(user_id, days)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({
        "days": days,
        "total_requests": report.total_requests,
        "total_input_tokens": report.total_input_tokens,
        "total_output_tokens": report.total_output_tokens,
        "total_tokens": report.total_tokens,
        "total_cost": report.total_cost,
        "total_cost_yuan": report.total_cost_yuan,
        "daily_usage": report.daily_usage,
    })))
}

#[derive(Deserialize)]
struct UserUsageQuery {
    days: Option<u32>,
}

// ============ API Key 管理 ============

async fn list_user_apikeys(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let keys = api_key_service
        .list_by_user(user_id)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({
        "object": "list",
        "data": keys.iter().map(|k| json!({
            "id": k.id.to_string(),
            "key": k.key_masked,
            "name": k.name,
            "status": k.status,
            "created_at": k.created_at.to_rfc3339(),
            "last_used_at": k.last_used_at.map(|t| t.to_rfc3339()),
        })).collect::<Vec<_>>()
    })))
}

async fn create_user_apikey(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let name = body.get("name").and_then(|v| v.as_str());

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    let key = api_key_service
        .create(user_id, name.map(|s| s.to_string()))
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({
        "id": key.id.to_string(),
        "key": key.key_masked,
        "name": key.name,
        "status": key.status,
        "created_at": key.created_at.to_rfc3339(),
    })))
}

async fn delete_user_apikey(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let key_id = uuid::Uuid::parse_str(&id)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let api_key_service = ApiKeyService::new(
        state.db.clone(),
        state.config.gateway.api_key_prefix.clone(),
    );

    api_key_service
        .delete(user_id, key_id)
        .await
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(axum::Json(json!({ "success": true })))
}

async fn update_user_apikey(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(_id): axum::extract::Path<String>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现 API Key 更新
    Ok(axum::Json(json!({ "success": true })))
}

// ============ 管理端点（遗留兼容） ============

async fn get_account_detail(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountRead)
        .await
        .map_err(|e| handler::ApiError(StatusCode::FORBIDDEN, e))?;

    // TODO: 实现账号详情
    Ok(axum::Json(json!({ "id": id })))
}

async fn update_account(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(_id): axum::extract::Path<String>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| handler::ApiError(StatusCode::FORBIDDEN, e))?;

    // TODO: 实现账号更新
    Ok(axum::Json(json!({ "success": true })))
}

async fn test_account(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    axum::Json(_body): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // 权限检查
    check_permission(&claims, Permission::AccountWrite)
        .await
        .map_err(|e| handler::ApiError(StatusCode::FORBIDDEN, e))?;

    // TODO: 实现账号测试
    Ok(axum::Json(
        json!({ "success": true, "message": "Account test not yet implemented" }),
    ))
}

// ============ Sora 图片/视频生成端点 ============

/// 处理 Sora 图片生成请求
async fn handle_sora_image_generation(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use super::sora::{SoraGenerateRequest, SoraService};

    let request: SoraGenerateRequest = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let _user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 创建 Sora 服务
    let sora_service = SoraService::with_default_pricing();

    // 验证请求
    let model_config = sora_service
        .validate_request(&request)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 计算费用
    let cost = sora_service.calculate_cost(&model_config);

    // 构建上游请求
    let upstream_body = sora_service.build_upstream_request(&request, &model_config);

    // TODO: 实现实际的图片生成请求转发
    // 目前返回模拟响应
    Ok(axum::Json(json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "status": "pending",
        "model": request.model,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "cost": cost,
        "message": "Image generation request accepted. This is a placeholder response.",
        "upstream_request": upstream_body
    })))
}

/// 处理 Sora 视频生成请求
async fn handle_sora_video_generation(
    Extension(_state): Extension<SharedState>,
    Extension(claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use super::sora::{SoraGenerateRequest, SoraService};

    let request: SoraGenerateRequest = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let _user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| handler::ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 创建 Sora 服务
    let sora_service = SoraService::with_default_pricing();

    // 验证请求
    let model_config = sora_service
        .validate_request(&request)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 计算费用
    let cost = sora_service.calculate_cost(&model_config);

    // 构建上游请求
    let upstream_body = sora_service.build_upstream_request(&request, &model_config);

    // TODO: 实现实际的视频生成请求转发
    // 目前返回模拟响应
    Ok(axum::Json(json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "status": "queued",
        "model": request.model,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "cost": cost,
        "estimated_duration": model_config.duration_seconds().unwrap_or(10),
        "message": "Video generation request accepted. This is a placeholder response.",
        "upstream_request": upstream_body
    })))
}

/// 获取 Sora 生成状态
async fn get_sora_generation_status(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<crate::service::user::Claims>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    // TODO: 实现实际的状态查询
    Ok(axum::Json(json!({
        "id": id,
        "status": "processing",
        "progress": 50,
        "message": "Generation in progress. This is a placeholder response."
    })))
}

/// 处理提示词增强请求
async fn handle_prompt_enhance(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<crate::service::user::Claims>,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use super::sora::{SoraGenerateRequest, SoraService};

    let request: SoraGenerateRequest = serde_json::from_slice(&body)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 创建 Sora 服务
    let sora_service = SoraService::with_default_pricing();

    // 验证请求
    let model_config = sora_service
        .validate_request(&request)
        .map_err(|e| handler::ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    // 计算费用
    let cost = sora_service.calculate_cost(&model_config);

    // TODO: 实现实际的提示词增强请求转发
    Ok(axum::Json(json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "status": "completed",
        "model": request.model,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "cost": cost,
        "original_prompt": request.prompt,
        "enhanced_prompt": format!("Enhanced: {}", request.prompt),
        "message": "Prompt enhancement completed. This is a placeholder response."
    })))
}

/// 列出 Sora 模型
async fn list_sora_models(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use super::sora::create_sora_model_list;

    Ok(axum::Json(create_sora_model_list(false)))
}

/// 列出 Sora 模型家族
async fn list_sora_families(
    Extension(_state): Extension<SharedState>,
    Extension(_claims): Extension<crate::service::user::Claims>,
) -> Result<axum::Json<serde_json::Value>, handler::ApiError> {
    use super::sora::build_sora_model_families;

    let families = build_sora_model_families();

    Ok(axum::Json(json!({
        "object": "list",
        "data": families
    })))
}
