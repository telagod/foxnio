//! Error passthrough rule migration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ErrorPassthroughRule::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::ErrorCodes)
                            .json()
                            .null(),
                    )
                    .col(ColumnDef::new(ErrorPassthroughRule::Keywords).json().null())
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::MatchMode)
                            .string_len(10)
                            .not_null()
                            .default("any"),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Platforms)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::PassthroughCode)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::ResponseCode)
                            .integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::PassthroughBody)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::CustomMessage)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::SkipMonitoring)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::Description)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ErrorPassthroughRule::UpdatedAt)
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
                    .name("idx_error_passthrough_rules_enabled")
                    .table(ErrorPassthroughRule::Table)
                    .col(ErrorPassthroughRule::Enabled)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_error_passthrough_rules_priority")
                    .table(ErrorPassthroughRule::Table)
                    .col(ErrorPassthroughRule::Priority)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ErrorPassthroughRule::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ErrorPassthroughRule {
    Table,
    Id,
    Name,
    Enabled,
    Priority,
    ErrorCodes,
    Keywords,
    MatchMode,
    Platforms,
    PassthroughCode,
    ResponseCode,
    PassthroughBody,
    CustomMessage,
    SkipMonitoring,
    Description,
    CreatedAt,
    UpdatedAt,
}
