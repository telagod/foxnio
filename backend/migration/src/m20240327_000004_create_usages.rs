use sea_orm_migration::prelude::*;

use super::m20240327_000001_create_users::Users;
use super::m20240327_000002_create_accounts::Accounts;
use super::m20240327_000003_create_api_keys::ApiKeys;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Usages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Usages::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Usages::UserId).uuid().not_null())
                    .col(ColumnDef::new(Usages::ApiKeyId).uuid().not_null())
                    .col(ColumnDef::new(Usages::AccountId).uuid().not_null())
                    .col(ColumnDef::new(Usages::Model).string().not_null())
                    .col(
                        ColumnDef::new(Usages::InputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Usages::OutputTokens)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Usages::Cost)
                            .big_integer()
                            .not_null()
                            .default(0),
                    ) // 单位：分
                    .col(ColumnDef::new(Usages::RequestId).string())
                    .col(
                        ColumnDef::new(Usages::Success)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(Usages::ErrorMessage).text())
                    .col(ColumnDef::new(Usages::Metadata).json())
                    .col(
                        ColumnDef::new(Usages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_usages_user")
                            .from(Usages::Table, Usages::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_usages_apikey")
                            .from(Usages::Table, Usages::ApiKeyId)
                            .to(ApiKeys::Table, ApiKeys::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_usages_account")
                            .from(Usages::Table, Usages::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引以优化查询
        manager
            .create_index(
                Index::create()
                    .name("idx_usages_user_created")
                    .table(Usages::Table)
                    .col(Usages::UserId)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_usages_account_created")
                    .table(Usages::Table)
                    .col(Usages::AccountId)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Usages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Usages {
    Table,
    Id,
    UserId,
    ApiKeyId,
    AccountId,
    Model,
    InputTokens,
    OutputTokens,
    Cost,
    RequestId,
    Success,
    ErrorMessage,
    Metadata,
    CreatedAt,
}
