//! Quota usage history migration - 配额使用历史

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(QuotaUsageHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(QuotaUsageHistory::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(QuotaUsageHistory::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(QuotaUsageHistory::ApiKeyId).uuid().null())
                    .col(
                        ColumnDef::new(QuotaUsageHistory::AccountId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(QuotaUsageHistory::Model)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(QuotaUsageHistory::Amount)
                            .decimal()
                            .not_null(),
                    )
                    .col(ColumnDef::new(QuotaUsageHistory::TokensIn).integer().null())
                    .col(
                        ColumnDef::new(QuotaUsageHistory::TokensOut)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(QuotaUsageHistory::RequestType)
                            .string_len(32)
                            .null(),
                    )
                    .col(ColumnDef::new(QuotaUsageHistory::Metadata).json().null())
                    .col(
                        ColumnDef::new(QuotaUsageHistory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for efficient querying
        manager
            .create_index(
                Index::create()
                    .name("idx_quota_usage_history_user_id")
                    .table(QuotaUsageHistory::Table)
                    .col(QuotaUsageHistory::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_quota_usage_history_api_key_id")
                    .table(QuotaUsageHistory::Table)
                    .col(QuotaUsageHistory::ApiKeyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_quota_usage_history_account_id")
                    .table(QuotaUsageHistory::Table)
                    .col(QuotaUsageHistory::AccountId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_quota_usage_history_model")
                    .table(QuotaUsageHistory::Table)
                    .col(QuotaUsageHistory::Model)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_quota_usage_history_created_at")
                    .table(QuotaUsageHistory::Table)
                    .col(QuotaUsageHistory::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(QuotaUsageHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum QuotaUsageHistory {
    Table,
    Id,
    UserId,
    ApiKeyId,
    AccountId,
    Model,
    Amount,
    TokensIn,
    TokensOut,
    RequestType,
    Metadata,
    CreatedAt,
}
