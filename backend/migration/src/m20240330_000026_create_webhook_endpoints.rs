//! Webhook Endpoints 表迁移

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
                    .table(WebhookEndpoints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebhookEndpoints::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(WebhookEndpoints::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(WebhookEndpoints::Url)
                            .string_len(2048)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::Events)
                            .json()
                            .not_null()
                            .default(Expr::cust("'[]'::jsonb")),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::Secret)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::MaxRetries)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::TimeoutMs)
                            .integer()
                            .not_null()
                            .default(5000),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(WebhookEndpoints::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_webhook_endpoints_user")
                            .from(WebhookEndpoints::Table, WebhookEndpoints::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Indexes
                    .index(
                        Index::create()
                            .name("idx_webhook_endpoints_user")
                            .col(WebhookEndpoints::UserId),
                    )
                    .index(
                        Index::create()
                            .name("idx_webhook_endpoints_enabled")
                            .col(WebhookEndpoints::Enabled),
                    )
                    .to_owned(),
            )
            .await?;

        // Add check constraint for HTTPS URL
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE webhook_endpoints 
                ADD CONSTRAINT valid_webhook_url 
                CHECK (url ~ '^https://')
                "#,
            )
            .await?;

        // Add check constraint for non-empty events array
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE webhook_endpoints 
                ADD CONSTRAINT valid_webhook_events 
                CHECK (jsonb_array_length(events) > 0)
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebhookEndpoints::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WebhookEndpoints {
    Table,
    Id,
    UserId,
    Url,
    Events,
    Secret,
    Enabled,
    MaxRetries,
    TimeoutMs,
    CreatedAt,
    UpdatedAt,
}
