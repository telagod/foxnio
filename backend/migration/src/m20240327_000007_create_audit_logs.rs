//! 审计日志表迁移

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditLogs::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(AuditLogs::UserId).uuid())
                    .col(
                        ColumnDef::new(AuditLogs::Action)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditLogs::ResourceType).string_len(50))
                    .col(ColumnDef::new(AuditLogs::ResourceId).string_len(100))
                    .col(ColumnDef::new(AuditLogs::IpAddress).string_len(45)) // IPv6 max length
                    .col(ColumnDef::new(AuditLogs::UserAgent).text())
                    .col(ColumnDef::new(AuditLogs::RequestData).json())
                    .col(ColumnDef::new(AuditLogs::ResponseStatus).integer())
                    .col(
                        ColumnDef::new(AuditLogs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Indexes for common queries
                    .index(
                        Index::create()
                            .name("idx_audit_logs_user_id")
                            .col(AuditLogs::UserId),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_logs_action")
                            .col(AuditLogs::Action),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_logs_created_at")
                            .col(AuditLogs::CreatedAt),
                    )
                    .index(
                        Index::create()
                            .name("idx_audit_logs_resource")
                            .col(AuditLogs::ResourceType)
                            .col(AuditLogs::ResourceId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum AuditLogs {
    Table,
    Id,
    UserId,
    Action,
    ResourceType,
    ResourceId,
    IpAddress,
    UserAgent,
    RequestData,
    ResponseStatus,
    CreatedAt,
}
