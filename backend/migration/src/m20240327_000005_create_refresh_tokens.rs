//! Refresh Tokens 表迁移
//! 
//! 用于存储 JWT refresh token 的哈希值，支持安全的 token 轮换

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RefreshTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RefreshTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::UserId)
                            .uuid()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::TokenHash)
                            .string()
                            .not_null()
                            .unique_key()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp())
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::Revoked)
                            .boolean()
                            .not_null()
                            .default(false)
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::RevokedAt)
                            .timestamp_with_time_zone()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::RevokedReason)
                            .string()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::UserAgent)
                            .string()
                    )
                    .col(
                        ColumnDef::new(RefreshTokens::IpAddress)
                            .string()
                    )
                    // 外键约束
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_refresh_tokens_user_id")
                            .from(RefreshTokens::Table, RefreshTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    // 索引
                    .index(
                        Index::create()
                            .name("idx_refresh_tokens_user_id")
                            .col(RefreshTokens::UserId)
                    )
                    .index(
                        Index::create()
                            .name("idx_refresh_tokens_expires_at")
                            .col(RefreshTokens::ExpiresAt)
                    )
                    .index(
                        Index::create()
                            .name("idx_refresh_tokens_token_hash")
                            .col(RefreshTokens::TokenHash)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RefreshTokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum RefreshTokens {
    Table,
    Id,
    UserId,
    TokenHash,
    ExpiresAt,
    CreatedAt,
    Revoked,
    RevokedAt,
    RevokedReason,
    UserAgent,
    IpAddress,
}

#[derive(DeriveIden)]
pub enum Users {
    Table,
    Id,
}
