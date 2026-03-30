//! OpenAPI 文档配置
//!
//! 提供 OpenAPI 3.0 规范和 Swagger UI 集成

use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// API 信息
pub const API_TITLE: &str = "FoxNIO API Gateway";
pub const API_VERSION: &str = "0.2.0";
pub const API_DESCRIPTION: &str = r#"
AI API Gateway - Subscription quota distribution management

## 认证方式

所有需要认证的端点都需要在请求头中携带 Bearer Token：
```
Authorization: Bearer <your_access_token>
```

## 主要功能

- **用户认证**: 注册、登录、Token 刷新、TOTP 两步验证
- **用户管理**: 用户 CRUD、余额管理
- **账号管理**: 上游账号管理、健康检查
- **API Key 管理**: 用户 API Key 的创建、查看、删除
- **订阅管理**: 订阅计划、配额管理
- **代理管理**: 代理服务器配置和测试
- **告警管理**: 告警规则、静默期、通知渠道
- **模型管理**: 模型配置、路由规则
- **审计日志**: 操作日志查询和清理

## 权限系统

系统使用基于角色的权限控制（RBAC）：
- `admin`: 管理员，拥有所有权限
- `operator`: 运维人员，拥有账号和监控权限
- `user`: 普通用户，仅能访问自己的资源
"#;

/// OpenAPI 文档结构
#[derive(OpenApi)]
#[openapi(
    info(
        title = "FoxNIO API Gateway",
        version = "0.2.0",
        description = "AI API Gateway - Subscription quota distribution management",
        contact(
            name = "FoxNIO Team",
            email = "support@foxnio.com"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    tags(
        (name = "认证", description = "用户认证相关 API"),
        (name = "用户", description = "用户信息管理"),
        (name = "用户管理", description = "管理员用户管理"),
        (name = "账号管理", description = "上游账号管理"),
        (name = "API Key", description = "API Key 管理"),
        (name = "订阅", description = "订阅和配额管理"),
        (name = "代理管理", description = "代理服务器配置"),
        (name = "告警管理", description = "告警规则和通知"),
        (name = "模型管理", description = "模型配置管理"),
        (name = "公告管理", description = "公告发布管理"),
        (name = "优惠码", description = "优惠码管理"),
        (name = "审计日志", description = "操作审计日志"),
        (name = "健康检查", description = "系统健康状态"),
        (name = "指标监控", description = "系统指标和监控"),
        (name = "分组管理", description = "用户分组管理"),
        (name = "用户属性", description = "用户自定义属性"),
        (name = "卡密兑换", description = "卡密兑换管理"),
        (name = "备份", description = "数据备份和恢复"),
        (name = "OpenAI兼容", description = "OpenAI 兼容 API"),
        (name = "Gemini", description = "Gemini Native API"),
        (name = "Sora", description = "Sora 图片/视频生成"),
    ),
    paths(
        // 认证
        crate::handler::auth::register,
        crate::handler::auth::login,
        crate::handler::auth::get_me,
        crate::handler::auth::refresh,
        crate::handler::auth::logout,
        crate::handler::auth::totp::enable_totp,
        crate::handler::auth::totp::confirm_enable_totp,
        crate::handler::auth::totp::disable_totp,
        crate::handler::auth::totp::totp_login,
        crate::handler::auth::password::request_reset,
        crate::handler::auth::password::reset_password,

        // 用户管理
        crate::handler::admin::list_users,
        crate::handler::admin::create_user,
        crate::handler::admin::get_user,
        crate::handler::admin::update_user,
        crate::handler::admin::delete_user,
        crate::handler::admin::update_user_balance,

        // 账号管理
        crate::handler::admin::list_accounts,
        crate::handler::admin::add_account,
        crate::handler::admin::delete_account_by_id,
        crate::handler::admin_accounts::batch_create_accounts,
        crate::handler::admin_accounts::refresh_account_token,
        crate::handler::admin_accounts::get_account_usage,
        crate::handler::admin_accounts::get_account_today_stats,

        // 健康检查
        crate::handler::health_simple,
        crate::handler::health_live,
        crate::handler::health_ready,
        crate::handler::health_detailed,

        // 公告管理
        crate::handler::announcement::list_announcements,
        crate::handler::announcement::create_announcement,
        crate::handler::announcement::get_announcement,
        crate::handler::announcement::update_announcement,
        crate::handler::announcement::delete_announcement,

        // 代理管理
        crate::handler::proxy::list_proxies,
        crate::handler::proxy::create_proxy,
        crate::handler::proxy::get_proxy,
        crate::handler::proxy::update_proxy,
        crate::handler::proxy::delete_proxy,
        crate::handler::proxy::test_proxy,

        // 告警管理
        crate::handler::alerts::list_rules,
        crate::handler::alerts::create_rule,
        crate::handler::alerts::update_rule,
        crate::handler::alerts::delete_rule,
        crate::handler::alerts::list_silences,
        crate::handler::alerts::create_silence,
        crate::handler::alerts::test_alert,

        // 模型管理
        crate::handler::models::list_models_admin,
        crate::handler::models::create_model,
        crate::handler::models::get_model,
        crate::handler::models::update_model,
        crate::handler::models::delete_model,

        // 优惠码管理
        crate::handler::promo_code::list_promo_codes,
        crate::handler::promo_code::create_promo_code,
        crate::handler::promo_code::get_promo_code,
        crate::handler::promo_code::update_promo_code,
        crate::handler::promo_code::delete_promo_code,

        // 审计日志
        crate::handler::list_audit_logs,
        crate::handler::get_audit_stats,
        crate::handler::cleanup_audit_logs,

        // 卡密兑换
        crate::handler::redeem::redeem_code,
        crate::handler::redeem::get_redemption_history,
        crate::handler::redeem::admin_generate_codes,

        // 用户属性
        crate::handler::user_attribute::list_attribute_definitions,
        crate::handler::user_attribute::create_attribute_definition,
        crate::handler::user_attribute::get_user_attributes,

        // 分组管理
        crate::handler::groups::create_group,
        crate::handler::groups::update_group,
        crate::handler::groups::delete_group,
        crate::handler::admin_groups::list_all_groups,
        crate::handler::admin_groups::get_group_stats,

        // 订阅
        crate::handler::subscription::list_user_subscriptions,
        crate::handler::subscription::get_subscription_detail,

        // 配额
        crate::handler::quota::get_user_quota,
        crate::handler::quota::update_user_quota,

        // 备份
        crate::handler::backup::export_data,
        crate::handler::backup::import_data,

        // 指标
        crate::handler::metrics::json_metrics,
        crate::handler::metrics::realtime_metrics,
    ),
    components(
        schemas(
            // 认证相关
            crate::handler::auth::RegisterRequest,
            crate::handler::auth::LoginRequest,
            crate::handler::auth::UserInfo,
            crate::handler::auth::AuthResponse,
            crate::handler::auth::RegisterResponse,

            // 用户管理
            crate::handler::admin::CreateUserRequest,
            crate::handler::admin::UpdateUserRequest,
            crate::handler::admin::UpdateBalanceRequest,
            crate::handler::admin::UserResponse,
        )
    )
)]
pub struct ApiDoc;

/// 创建 Swagger UI 路由
pub fn create_swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi())
}

/// 获取 OpenAPI JSON
pub fn get_openapi_json() -> String {
    ApiDoc::openapi()
        .to_json()
        .expect("Failed to serialize OpenAPI spec")
}
