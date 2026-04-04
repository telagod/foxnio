//! Redeem code ledger migration

use sea_orm_migration::prelude::*;

use super::m20240327_000001_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RedeemCodeLedger::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RedeemCodeLedger::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::RedeemCodeId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RedeemCodeLedger::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(RedeemCodeLedger::IdempotencyKey)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::RequestFingerprint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::CodeType)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::Amount)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::BalanceDeltaCents)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::SubscriptionDays)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::QuotaDelta)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::SubscriptionId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(RedeemCodeLedger::ResultMessage)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(RedeemCodeLedger::Metadata).json().null())
                    .col(
                        ColumnDef::new(RedeemCodeLedger::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_redeem_code_ledger_code")
                            .from(RedeemCodeLedger::Table, RedeemCodeLedger::RedeemCodeId)
                            .to(RedeemCodeTable::Table, RedeemCodeTable::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_redeem_code_ledger_user")
                            .from(RedeemCodeLedger::Table, RedeemCodeLedger::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_redeem_code_ledger_user_created")
                    .table(RedeemCodeLedger::Table)
                    .col(RedeemCodeLedger::UserId)
                    .col(RedeemCodeLedger::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_redeem_code_ledger_user_idempotency")
                    .table(RedeemCodeLedger::Table)
                    .col(RedeemCodeLedger::UserId)
                    .col(RedeemCodeLedger::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_redeem_code_ledger_code_user")
                    .table(RedeemCodeLedger::Table)
                    .col(RedeemCodeLedger::RedeemCodeId)
                    .col(RedeemCodeLedger::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RedeemCodeLedger::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RedeemCodeLedger {
    Table,
    Id,
    RedeemCodeId,
    UserId,
    IdempotencyKey,
    RequestFingerprint,
    CodeType,
    Amount,
    BalanceDeltaCents,
    SubscriptionDays,
    QuotaDelta,
    SubscriptionId,
    ResultMessage,
    Metadata,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RedeemCodeTable {
    #[sea_orm(iden = "redeem_code")]
    Table,
    Id,
}
