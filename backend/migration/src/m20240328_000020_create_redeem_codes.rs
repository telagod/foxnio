//! Redeem codes migration - 卡密管理

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RedeemCode::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RedeemCode::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::Code)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(RedeemCode::BatchId).string_len(64).null())
                    .col(ColumnDef::new(RedeemCode::Amount).decimal().not_null())
                    .col(
                        ColumnDef::new(RedeemCode::Type)
                            .string_len(20)
                            .not_null()
                            .default("balance"),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::MaxUses)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::UsedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::Status)
                            .string_len(20)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::ExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(RedeemCode::UsedBy).json().null())
                    .col(ColumnDef::new(RedeemCode::Notes).text().null())
                    .col(ColumnDef::new(RedeemCode::CreatedBy).big_integer().null())
                    .col(
                        ColumnDef::new(RedeemCode::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RedeemCode::UpdatedAt)
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
                    .name("idx_redeem_codes_code")
                    .table(RedeemCode::Table)
                    .col(RedeemCode::Code)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_redeem_codes_batch_id")
                    .table(RedeemCode::Table)
                    .col(RedeemCode::BatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_redeem_codes_status")
                    .table(RedeemCode::Table)
                    .col(RedeemCode::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RedeemCode::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RedeemCode {
    Table,
    Id,
    Code,
    BatchId,
    Amount,
    Type,
    MaxUses,
    UsedCount,
    Status,
    ExpiresAt,
    UsedBy,
    Notes,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}
