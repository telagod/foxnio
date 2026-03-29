//! 业务服务层

pub mod account;
pub mod account_service;
pub mod announcement;
pub mod api_key;
pub mod api_key_auth_cache;
pub mod api_key_service;
pub mod api_key_test;
pub mod audit;
pub mod auth_service;
pub mod backup;
pub mod batch_operations;
pub mod bedrock;
pub mod billing;
pub mod billing_cache_service;
pub mod billing_service;
pub mod billing_test;
pub mod claude_code_validator;
pub mod claude_token_provider;
pub mod concurrency;
pub mod credential;
pub mod domain_constants;
pub mod email;
pub mod error_passthrough_rule;
pub mod group;
pub mod header_util;
pub mod health_scorer;
pub mod identity_service;
pub mod model_registry;
pub mod model_router;
pub mod oauth;
pub mod oauth_refresh_api;
pub mod openai_account_scheduler;
pub mod openai_client_restriction_detector;
pub mod openai_client_transport;
pub mod openai_codex_transform;
pub mod openai_compat_prompt_cache_key;
pub mod openai_gateway_service;
pub mod openai_previous_response_id;
pub mod openai_privacy;
pub mod openai_privacy_service;
pub mod openai_sticky_compat;
pub mod openai_tool_continuation;
pub mod openai_tool_corrector;
pub mod openai_ws_client;
pub mod openai_ws_forwarder;
pub mod openai_ws_pool;
pub mod openai_ws_protocol_resolver;
pub mod openai_ws_state_store;
pub mod ops_port;
pub mod ops_query_mode;
pub mod ops_retry;
pub mod ops_upstream_context;
pub mod ops_window_stats;
pub mod password_reset;
pub mod password_reset_test;
pub mod permission;
pub mod pricing_service;
pub mod promo_code;
pub mod proxy;
pub mod qps_monitor;
pub mod quota;
pub mod rate_limit;
pub mod realtime_monitor;
pub mod redeem_code;
pub mod refresh_policy;
pub mod request_metadata;
pub mod response_header_filter;
pub mod rpm_cache;
pub mod scheduled_test_plan;
pub mod scheduler;
pub mod scheduler_cache;
pub mod scheduler_events;
pub mod scheduler_outbox;
pub mod scheduler_snapshot_service;
pub mod scheduler_test;
pub mod setting;
pub mod setting_service;
pub mod settings_view;
pub mod sora;
pub mod sse_scanner_buffer_pool;
pub mod sticky_session;
pub mod subscription;
pub mod subscription_expiry_service;
pub mod subscription_maintenance_queue;
pub mod subscription_test;
pub mod timing_wheel_service;
pub mod tls_fingerprint;
pub mod token_cache_invalidator;
pub mod token_cache_key;
pub mod token_refresh_service;
pub mod token_refresher;
pub mod totp;
#[cfg(test)]
mod totp_test;
pub mod turnstile;
pub mod upstream_response_limit;
pub mod usage_billing;
pub mod usage_service;
pub mod user;
pub mod user_attribute;
pub mod user_ext;
pub mod user_group;
pub mod user_subscription;
pub mod user_test;

// P0 - 新增服务文件
pub mod account_quota_reset;
pub mod account_rpm;
pub mod account_test_service;
pub mod admin_service;
pub mod announcement_service;
pub mod backup_service;
pub mod billing_cache_port;
pub mod dashboard_aggregation_service;
pub mod dashboard_service;
pub mod data_management_service;
pub mod digest_session_store;
pub mod gemini_token_cache;
pub mod group_capacity_service;
pub mod group_service;
pub mod metadata_userid;
pub mod model_rate_limit;
pub mod oauth_service;
pub mod parse_integral_number_unit;
pub mod promo_code_repository;
pub mod promo_service;
pub mod proxy_latency_cache;
pub mod proxy_service;
pub mod quota_fetcher;
pub mod ratelimit_service;
pub mod redeem_service;
pub mod registration_email_policy;
pub mod scheduled_test_runner_service;
pub mod session_limit_cache;
pub mod system_operation_lock_service;
pub mod tls_fingerprint_profile_service;
pub mod update_service;
pub mod user_attribute_service;
pub mod user_group_rate;
pub mod user_group_rate_resolver;
pub mod user_msg_queue_service;
pub mod user_subscription_port;
pub mod wire;

// P0 - 核心高级功能
// Gateway Forwarding Services
pub mod gateway_forward_as_chat_completions;
pub mod gateway_forward_as_responses;
pub mod gateway_request;
pub mod gateway_service;
pub mod http_upstream_port;

// Idempotency Services
pub mod idempotency;
pub mod idempotency_cleanup_service;
pub mod idempotency_observability;

// P1 - 运维增强功能
// Ops Services
pub mod ops_aggregation_service;
pub mod ops_alert_evaluator_service;
pub mod ops_cleanup_service;
pub mod ops_health_score;
pub mod ops_metrics_collector;
pub mod ops_realtime;
pub mod ops_realtime_traffic;
pub mod ops_scheduled_report_service;
pub mod ops_service;
pub mod ops_trends;

// Usage Management
pub mod usage_cleanup;
pub mod usage_cleanup_service;
pub mod usage_log;
pub mod usage_record_worker_pool;

// Account Enhancement
pub mod account_credentials_persistence;
pub mod account_expiry_service;
pub mod account_group;
pub mod account_usage_service;
pub mod temp_unsched;

// P2 - 平台增强功能
// Gemini Enhancement
pub mod gemini_messages_compat_service;
pub mod gemini_native_signature_cleaner;
pub mod gemini_oauth_service;
pub mod gemini_quota;
pub mod gemini_session;
pub mod gemini_token_provider;
pub mod gemini_token_refresher;
pub mod geminicli_codeassist;

// Antigravity Enhancement
pub mod antigravity_credits_overages;
pub mod antigravity_gateway_service;
pub mod antigravity_privacy_service;
pub mod antigravity_quota_fetcher;
pub mod antigravity_subscription_service;
pub mod antigravity_token_provider;
pub mod antigravity_token_refresher;

// Sora Enhancement
pub mod sora_gateway_service;
pub mod sora_generation_service;
pub mod sora_media_cleanup_service;
pub mod sora_media_storage;
pub mod sora_quota_service;
pub mod sora_s3_storage;
pub mod sora_sdk_client;
pub mod sora_upstream_forwarder;

// P3 - 其他增强功能
pub mod crs_sync_service;
pub mod deferred_service;
pub mod email_queue_service;

// Core Services
pub mod anthropic_session;

// Re-exports
pub use account::AccountService as LegacyAccountService;
pub use api_key::ApiKeyService as LegacyApiKeyService;
pub use audit::{AuditEntry, AuditFilter, AuditService};
pub use billing::BillingService as LegacyBillingService;
pub use model_registry::ModelRegistry;
pub use model_router::ModelRouter;
pub use scheduler::SchedulerService;
pub use setting::{Setting, SettingCategory, SettingError, SettingMap};
pub use setting_service::SettingService;
pub use user::UserService;

// OAuth 模块导出
