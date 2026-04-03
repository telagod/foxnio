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
                    .table(ApiKeys::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApiKeys::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(ApiKeys::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(ApiKeys::Key)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(ApiKeys::Name).string())
                    .col(
                        ColumnDef::new(ApiKeys::Prefix)
                            .string()
                            .not_null()
                            .default("sk-"),
                    )
                    .col(
                        ColumnDef::new(ApiKeys::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(ApiKeys::ConcurrentLimit)
                            .integer()
                            .default(5),
                    )
                    .col(ColumnDef::new(ApiKeys::RateLimitRpm).integer().default(60))
                    .col(ColumnDef::new(ApiKeys::AllowedModels).json()) // 允许的模型列表
                    .col(ColumnDef::new(ApiKeys::ExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(ApiKeys::LastUsedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(ApiKeys::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_apikeys_user")
                            .from(ApiKeys::Table, ApiKeys::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiKeys::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ApiKeys {
    Table,
    Id,
    UserId,
    Key,
    Name,
    Prefix,
    Status,
    ConcurrentLimit,
    RateLimitRpm,
    AllowedModels,
    ExpiresAt,
    LastUsedAt,
    CreatedAt,
}
