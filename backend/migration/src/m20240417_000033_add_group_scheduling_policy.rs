//! 分组调度策略字段
//!
//! groups 表新增 scheduling_policy 列

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Groups::Table)
                    .add_column(
                        ColumnDef::new(Groups::SchedulingPolicy)
                            .string()
                            .not_null()
                            .default("load_balance"),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Groups::Table)
                    .drop_column(Groups::SchedulingPolicy)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Groups {
    Table,
    SchedulingPolicy,
}
