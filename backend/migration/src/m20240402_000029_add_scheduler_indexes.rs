//! 调度器性能优化索引

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 账号表：优先级索引（调度排序）
        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_priority")
                    .table(Accounts::Table)
                    .col(Accounts::Priority)
                    .to_owned(),
            )
            .await?;

        // 账号表：复合索引（状态 + 优先级）
        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_status_priority")
                    .table(Accounts::Table)
                    .col(Accounts::Status)
                    .col(Accounts::Priority)
                    .to_owned(),
            )
            .await?;

        // 账号表：Provider 索引（分组查询）
        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_provider")
                    .table(Accounts::Table)
                    .col(Accounts::Provider)
                    .to_owned(),
            )
            .await?;

        // 分组表：名称索引
        manager
            .create_index(
                Index::create()
                    .name("idx_groups_name")
                    .table(Groups::Table)
                    .col(Groups::Name)
                    .to_owned(),
            )
            .await?;

        // 模型配置表：Provider + 名称复合索引
        manager
            .create_index(
                Index::create()
                    .name("idx_model_configs_provider_name")
                    .table(ModelConfigs::Table)
                    .col(ModelConfigs::Provider)
                    .col(ModelConfigs::Name)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx_accounts_priority").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_accounts_status_priority")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(Index::drop().name("idx_accounts_provider").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_groups_name").to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_model_configs_provider_name")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Accounts {
    Table,
    Provider,
    Status,
    Priority,
}

#[derive(DeriveIden)]
pub enum Groups {
    Table,
    Name,
}

#[derive(DeriveIden)]
pub enum ModelConfigs {
    Table,
    Provider,
    Name,
}
