//! Subscriptions migration - 订阅管理

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Subscription::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Subscription::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Subscription::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::PlanId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::PlanName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::Status)
                            .string_len(20)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Subscription::QuotaLimit)
                            .decimal()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Subscription::QuotaUsed)
                            .decimal()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Subscription::RateLimit5h).decimal().null())
                    .col(ColumnDef::new(Subscription::RateLimit1d).decimal().null())
                    .col(ColumnDef::new(Subscription::RateLimit7d).decimal().null())
                    .col(ColumnDef::new(Subscription::Features).json().null())
                    .col(
                        ColumnDef::new(Subscription::StripeSubscriptionId)
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::StripeCustomerId)
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::CurrentPeriodStart)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::CurrentPeriodEnd)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::CancelAtPeriodEnd)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Subscription::CanceledAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Subscription::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Subscription::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_subscriptions_user_id")
                    .table(Subscription::Table)
                    .col(Subscription::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_subscriptions_status")
                    .table(Subscription::Table)
                    .col(Subscription::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_subscriptions_stripe_subscription_id")
                    .table(Subscription::Table)
                    .col(Subscription::StripeSubscriptionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Subscription::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Subscription {
    Table,
    Id,
    UserId,
    PlanId,
    PlanName,
    Status,
    QuotaLimit,
    QuotaUsed,
    RateLimit5h,
    RateLimit1d,
    RateLimit7d,
    Features,
    StripeSubscriptionId,
    StripeCustomerId,
    CurrentPeriodStart,
    CurrentPeriodEnd,
    CancelAtPeriodEnd,
    CanceledAt,
    CreatedAt,
    UpdatedAt,
}
