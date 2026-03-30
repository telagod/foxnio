//! Webhook Deliveries 表迁移

use sea_orm_migration::prelude::*;

use super::m20240330_000026_create_webhook_endpoints::WebhookEndpoints;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WebhookDeliveries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebhookDeliveries::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::EndpointId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::EventType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::Payload)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(WebhookDeliveries::ResponseCode).integer())
                    .col(ColumnDef::new(WebhookDeliveries::ResponseBody).text())
                    .col(
                        ColumnDef::new(WebhookDeliveries::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(WebhookDeliveries::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .col(ColumnDef::new(WebhookDeliveries::NextRetryAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(WebhookDeliveries::DeliveredAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(WebhookDeliveries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_webhook_deliveries_endpoint")
                            .from(WebhookDeliveries::Table, WebhookDeliveries::EndpointId)
                            .to(WebhookEndpoints::Table, WebhookEndpoints::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    // Indexes
                    .index(
                        Index::create()
                            .name("idx_webhook_deliveries_status")
                            .col(WebhookDeliveries::Status),
                    )
                    .index(
                        Index::create()
                            .name("idx_webhook_deliveries_next_retry")
                            .col(WebhookDeliveries::NextRetryAt),
                    )
                    .index(
                        Index::create()
                            .name("idx_webhook_deliveries_endpoint")
                            .col(WebhookDeliveries::EndpointId),
                    )
                    .to_owned(),
            )
            .await?;

        // Add check constraint for valid status
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE webhook_deliveries 
                ADD CONSTRAINT valid_webhook_status 
                CHECK (status IN ('pending', 'success', 'failed', 'retrying'))
                "#,
            )
            .await?;

        // Create partial index for retrying status
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_webhook_deliveries_retry 
                ON webhook_deliveries (next_retry_at) 
                WHERE status = 'retrying'
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebhookDeliveries::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum WebhookDeliveries {
    Table,
    Id,
    EndpointId,
    EventType,
    Payload,
    Status,
    ResponseCode,
    ResponseBody,
    Attempts,
    MaxAttempts,
    NextRetryAt,
    DeliveredAt,
    CreatedAt,
}
