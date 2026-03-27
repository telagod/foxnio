use sea_orm_migration::prelude::*;

use super::m20240327_000002_create_accounts::Accounts;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OauthTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OauthTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(OauthTokens::AccountId).uuid().not_null())
                    .col(ColumnDef::new(OauthTokens::Provider).string().not_null())
                    // 加密存储的 access_token
                    .col(ColumnDef::new(OauthTokens::AccessToken).string().not_null())
                    // 加密存储的 refresh_token
                    .col(ColumnDef::new(OauthTokens::RefreshToken).string())
                    .col(ColumnDef::new(OauthTokens::ExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(OauthTokens::TokenType).string())
                    .col(ColumnDef::new(OauthTokens::Scope).string())
                    .col(ColumnDef::new(OauthTokens::Metadata).json())
                    .col(
                        ColumnDef::new(OauthTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OauthTokens::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_tokens_account")
                            .from(OauthTokens::Table, OauthTokens::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_oauth_tokens_account_provider")
                            .table(OauthTokens::Table)
                            .col(OauthTokens::AccountId)
                            .col(OauthTokens::Provider)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(OauthTokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum OauthTokens {
    Table,
    Id,
    AccountId,
    Provider,
    AccessToken,
    RefreshToken,
    ExpiresAt,
    TokenType,
    Scope,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
