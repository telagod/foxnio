use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Accounts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Accounts::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Accounts::Name).string().not_null())
                    .col(ColumnDef::new(Accounts::Provider).string().not_null())
                    .col(ColumnDef::new(Accounts::CredentialType).string().not_null())
                    .col(ColumnDef::new(Accounts::Credential).string().not_null()) // 加密存储
                    .col(ColumnDef::new(Accounts::Metadata).json()) // OAuth token 等额外信息
                    .col(
                        ColumnDef::new(Accounts::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    .col(ColumnDef::new(Accounts::LastError).string())
                    .col(
                        ColumnDef::new(Accounts::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Accounts::ConcurrentLimit)
                            .integer()
                            .default(5),
                    )
                    .col(ColumnDef::new(Accounts::RateLimitRpm).integer().default(60))
                    .col(
                        ColumnDef::new(Accounts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Accounts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Accounts::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Accounts {
    Table,
    Id,
    Name,
    Provider,
    CredentialType,
    Credential,
    Metadata,
    Status,
    LastError,
    Priority,
    ConcurrentLimit,
    RateLimitRpm,
    CreatedAt,
    UpdatedAt,
}
