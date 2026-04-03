//! 告警规则表迁移

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AlertRules::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AlertRules::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(AlertRules::Name).string_len(100).not_null())
                    .col(ColumnDef::new(AlertRules::Description).text())
                    .col(
                        ColumnDef::new(AlertRules::ConditionType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlertRules::ConditionConfig)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AlertRules::DurationSecs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(AlertRules::Level)
                            .string_len(20)
                            .not_null()
                            .default("warning"),
                    )
                    .col(ColumnDef::new(AlertRules::Channels).json().not_null())
                    .col(
                        ColumnDef::new(AlertRules::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(AlertRules::Labels).json())
                    .col(
                        ColumnDef::new(AlertRules::TriggerCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(AlertRules::LastTriggeredAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(AlertRules::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(AlertRules::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .index(
                        Index::create()
                            .name("idx_alert_rules_name")
                            .col(AlertRules::Name),
                    )
                    .index(
                        Index::create()
                            .name("idx_alert_rules_enabled")
                            .col(AlertRules::Enabled),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AlertRules::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum AlertRules {
    Table,
    Id,
    Name,
    Description,
    ConditionType,
    ConditionConfig,
    DurationSecs,
    Level,
    Channels,
    Enabled,
    Labels,
    TriggerCount,
    LastTriggeredAt,
    CreatedAt,
    UpdatedAt,
}
