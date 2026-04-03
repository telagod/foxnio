//! Proxy migration - 代理管理

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Proxy::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Proxy::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Proxy::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Proxy::Protocol).string_len(20).not_null())
                    .col(ColumnDef::new(Proxy::Host).string_len(255).not_null())
                    .col(ColumnDef::new(Proxy::Port).integer().not_null())
                    .col(ColumnDef::new(Proxy::Username).string_len(255).null())
                    .col(ColumnDef::new(Proxy::Password).string_len(255).null())
                    .col(
                        ColumnDef::new(Proxy::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Proxy::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Proxy::Tags).json().null())
                    .col(ColumnDef::new(Proxy::HealthCheckUrl).string_len(512).null())
                    .col(
                        ColumnDef::new(Proxy::LastCheckAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Proxy::LastCheckStatus).string_len(20).null())
                    .col(ColumnDef::new(Proxy::Notes).text().null())
                    .col(
                        ColumnDef::new(Proxy::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Proxy::UpdatedAt)
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
                    .name("idx_proxies_enabled")
                    .table(Proxy::Table)
                    .col(Proxy::Enabled)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxies_priority")
                    .table(Proxy::Table)
                    .col(Proxy::Priority)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Proxy::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Proxy {
    Table,
    Id,
    Name,
    Protocol,
    Host,
    Port,
    Username,
    Password,
    Enabled,
    Priority,
    Tags,
    HealthCheckUrl,
    LastCheckAt,
    LastCheckStatus,
    Notes,
    CreatedAt,
    UpdatedAt,
}
