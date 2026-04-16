//! API Key 配额重置服务
//!
//! 定时重置 api_keys.daily_used_quota，尊重 quota_reset_at 时间戳

use anyhow::Result;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entity::api_keys;

/// API Key 配额重置服务
pub struct ApiKeyQuotaResetService {
    db: DatabaseConnection,
}

impl ApiKeyQuotaResetService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 重置所有需要重置的 API Key 日配额
    /// 条件：daily_quota 已设置 且 (quota_reset_at 已过期 或 quota_reset_at 为空)
    pub async fn reset_expired_quotas(&self) -> Result<u64> {
        let now = Utc::now();
        let tomorrow = now + chrono::Duration::days(1);
        let tomorrow_start = tomorrow
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|t| chrono::TimeZone::from_utc_datetime(&Utc, &t))
            .unwrap_or(now);

        // 找到所有需要重置的 API Key
        let keys = api_keys::Entity::find()
            .filter(api_keys::Column::DailyQuota.is_not_null())
            .filter(api_keys::Column::Status.eq("active"))
            .filter(
                sea_orm::Condition::any()
                    .add(api_keys::Column::QuotaResetAt.is_null())
                    .add(api_keys::Column::QuotaResetAt.lte(now)),
            )
            .all(&self.db)
            .await?;

        let mut reset_count = 0u64;

        for key in keys {
            let used = key.daily_used_quota.unwrap_or(0);
            if used == 0 && key.quota_reset_at.is_some() {
                // 已经是 0 且有 reset_at，只更新下次重置时间
                let mut model: api_keys::ActiveModel = key.into();
                model.quota_reset_at = Set(Some(tomorrow_start));
                model.update(&self.db).await?;
                continue;
            }

            let mut model: api_keys::ActiveModel = key.into();
            model.daily_used_quota = Set(Some(0));
            model.quota_reset_at = Set(Some(tomorrow_start));
            model.update(&self.db).await?;
            reset_count += 1;
        }

        if reset_count > 0 {
            tracing::info!("Reset daily quota for {} API keys", reset_count);
        }

        Ok(reset_count)
    }
}
