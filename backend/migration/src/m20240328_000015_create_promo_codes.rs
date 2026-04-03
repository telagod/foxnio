//! Promo code migration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PromoCode::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PromoCode::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PromoCode::Code)
                            .string_len(32)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(PromoCode::BonusAmount)
                            .decimal_len(20, 8)
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(PromoCode::MaxUses)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PromoCode::UsedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PromoCode::Status)
                            .string_len(20)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(PromoCode::ExpiresAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(PromoCode::Notes).text().null())
                    .col(
                        ColumnDef::new(PromoCode::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PromoCode::UpdatedAt)
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
                    .name("idx_promo_codes_status")
                    .table(PromoCode::Table)
                    .col(PromoCode::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_promo_codes_expires_at")
                    .table(PromoCode::Table)
                    .col(PromoCode::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        // Create promo_code_usages table
        manager
            .create_table(
                Table::create()
                    .table(PromoCodeUsage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PromoCodeUsage::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PromoCodeUsage::PromoCodeId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PromoCodeUsage::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PromoCodeUsage::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for promo_code_usages
        manager
            .create_index(
                Index::create()
                    .name("idx_promo_code_usages_promo_code_id")
                    .table(PromoCodeUsage::Table)
                    .col(PromoCodeUsage::PromoCodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_promo_code_usages_user_id")
                    .table(PromoCodeUsage::Table)
                    .col(PromoCodeUsage::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PromoCodeUsage::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PromoCode::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PromoCode {
    Table,
    Id,
    Code,
    BonusAmount,
    MaxUses,
    UsedCount,
    Status,
    ExpiresAt,
    Notes,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum PromoCodeUsage {
    Table,
    Id,
    PromoCodeId,
    UserId,
    CreatedAt,
}
