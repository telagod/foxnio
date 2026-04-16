//! 查询性能优化索引
//!
//! 补充 dashboard 聚合查询和账号分组查询所需的复合索引

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // accounts: (group_id, status) — 分组筛选 + 状态过滤
        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_group_id_status")
                    .table(Accounts::Table)
                    .col(Accounts::GroupId)
                    .col(Accounts::Status)
                    .to_owned(),
            )
            .await?;

        // usages: created_at — dashboard 时间范围查询
        manager
            .create_index(
                Index::create()
                    .name("idx_usages_created_at")
                    .table(Usages::Table)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // usages: (user_id, created_at) — 用户用量查询
        manager
            .create_index(
                Index::create()
                    .name("idx_usages_user_id_created_at")
                    .table(Usages::Table)
                    .col(Usages::UserId)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_accounts_group_id_status")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(Index::drop().name("idx_usages_created_at").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_usages_user_id_created_at")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Accounts {
    Table,
    GroupId,
    Status,
}

#[derive(DeriveIden)]
pub enum Usages {
    Table,
    CreatedAt,
    UserId,
}
