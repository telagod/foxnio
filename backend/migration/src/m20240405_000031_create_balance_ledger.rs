//! Balance ledger migration

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
                    .table(BalanceLedger::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BalanceLedger::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(BalanceLedger::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(BalanceLedger::SourceType)
                            .string_len(30)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BalanceLedger::SourceId)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BalanceLedger::DeltaCents)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BalanceLedger::BalanceBefore)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BalanceLedger::BalanceAfter)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BalanceLedger::Description).text().null())
                    .col(ColumnDef::new(BalanceLedger::Metadata).json().null())
                    .col(
                        ColumnDef::new(BalanceLedger::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_balance_ledger_user")
                            .from(BalanceLedger::Table, BalanceLedger::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_balance_ledger_user_created")
                    .table(BalanceLedger::Table)
                    .col(BalanceLedger::UserId)
                    .col(BalanceLedger::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_balance_ledger_source")
                    .table(BalanceLedger::Table)
                    .col(BalanceLedger::SourceType)
                    .col(BalanceLedger::SourceId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BalanceLedger::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub(crate) enum BalanceLedger {
    Table,
    Id,
    UserId,
    SourceType,
    SourceId,
    DeltaCents,
    BalanceBefore,
    BalanceAfter,
    Description,
    Metadata,
    CreatedAt,
}
