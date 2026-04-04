pub use sea_orm_migration::prelude::*;

mod m20240327_000001_create_users;
mod m20240327_000002_create_accounts;
mod m20240327_000003_create_api_keys;
mod m20240327_000004_create_usages;
mod m20240327_000005_create_password_reset_tokens;
mod m20240327_000005_create_refresh_tokens;
mod m20240327_000006_create_oauth_tokens;
mod m20240327_000007_create_audit_logs;
mod m20240327_000008_create_alert_rules;
mod m20240327_000009_create_alert_history;
mod m20240327_000010_create_alert_channels;
mod m20240328_000011_create_groups;
mod m20240328_000012_create_model_configs;
mod m20240328_000013_create_tls_fingerprint_profiles;
mod m20240328_000014_create_announcements;
mod m20240328_000015_create_promo_codes;
mod m20240328_000016_create_user_attributes;
mod m20240328_000017_create_error_passthrough_rules;
mod m20240328_000018_create_scheduled_test_plans;
mod m20240328_000019_create_proxies;
mod m20240328_000020_create_redeem_codes;
mod m20240328_000021_create_quota_usage_history;
mod m20240328_000022_create_subscriptions;
mod m20240329_000023_add_supported_model_scopes;
mod m20240330_000024_add_api_key_permissions;
mod m20240330_000026_create_webhook_endpoints;
mod m20240330_000027_create_webhook_deliveries;
mod m20240401_000028_add_performance_indexes;
mod m20240402_000029_add_scheduler_indexes;
mod m20240403_000030_create_redeem_code_ledger;
mod m20240405_000031_create_balance_ledger;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240327_000001_create_users::Migration),
            Box::new(m20240327_000002_create_accounts::Migration),
            Box::new(m20240327_000003_create_api_keys::Migration),
            Box::new(m20240327_000004_create_usages::Migration),
            Box::new(m20240327_000005_create_refresh_tokens::Migration),
            Box::new(m20240327_000005_create_password_reset_tokens::Migration),
            Box::new(m20240327_000006_create_oauth_tokens::Migration),
            Box::new(m20240327_000007_create_audit_logs::Migration),
            Box::new(m20240327_000008_create_alert_rules::Migration),
            Box::new(m20240327_000009_create_alert_history::Migration),
            Box::new(m20240327_000010_create_alert_channels::Migration),
            Box::new(m20240328_000011_create_groups::Migration),
            Box::new(m20240328_000012_create_model_configs::Migration),
            Box::new(m20240328_000013_create_tls_fingerprint_profiles::Migration),
            Box::new(m20240328_000014_create_announcements::Migration),
            Box::new(m20240328_000015_create_promo_codes::Migration),
            Box::new(m20240328_000016_create_user_attributes::Migration),
            Box::new(m20240328_000017_create_error_passthrough_rules::Migration),
            Box::new(m20240328_000018_create_scheduled_test_plans::Migration),
            Box::new(m20240328_000019_create_proxies::Migration),
            Box::new(m20240328_000020_create_redeem_codes::Migration),
            Box::new(m20240328_000021_create_quota_usage_history::Migration),
            Box::new(m20240328_000022_create_subscriptions::Migration),
            Box::new(m20240329_000023_add_supported_model_scopes::Migration),
            Box::new(m20240330_000024_add_api_key_permissions::Migration),
            Box::new(m20240330_000026_create_webhook_endpoints::Migration),
            Box::new(m20240330_000027_create_webhook_deliveries::Migration),
            Box::new(m20240401_000028_add_performance_indexes::Migration),
            Box::new(m20240402_000029_add_scheduler_indexes::Migration),
            Box::new(m20240403_000030_create_redeem_code_ledger::Migration),
            Box::new(m20240405_000031_create_balance_ledger::Migration),
        ]
    }
}
