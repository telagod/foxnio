use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PasswordResetTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PasswordResetTokens::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::UserId)
                            .uuid()
                            .not_null()
                    )
                    .col(ColumnDef::new(PasswordResetTokens::TokenHash).string().not_null().unique_key())
                    .col(
                        ColumnDef::new(PasswordResetTokens::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null()
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::UsedAt)
                            .timestamp_with_time_zone()
                    )
                    .col(
                        ColumnDef::new(PasswordResetTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_password_reset_tokens_user_id")
                            .from(PasswordResetTokens::Table, PasswordResetTokens::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                    )
                    .index(
                        Index::create()
                            .name("idx_password_reset_tokens_user_id")
                            .col(PasswordResetTokens::UserId)
                    )
                    .index(
                        Index::create()
                            .name("idx_password_reset_tokens_expires_at")
                            .col(PasswordResetTokens::ExpiresAt)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PasswordResetTokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum PasswordResetTokens {
    Table,
    Id,
    UserId,
    TokenHash,
    ExpiresAt,
    UsedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum Users {
    Table,
    Id,
}
