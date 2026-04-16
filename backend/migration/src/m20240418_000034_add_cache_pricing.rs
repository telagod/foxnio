//! 模型定价缓存字段
//!
//! model_configs 表新增 cache_read_price 和 cache_creation_price

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ModelConfigs::Table)
                    .add_column(
                        ColumnDef::new(ModelConfigs::CacheReadPrice)
                            .double()
                            .null()
                            .default(0.0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ModelConfigs::Table)
                    .add_column(
                        ColumnDef::new(ModelConfigs::CacheCreationPrice)
                            .double()
                            .null()
                            .default(0.0),
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
                    .table(ModelConfigs::Table)
                    .drop_column(ModelConfigs::CacheReadPrice)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ModelConfigs::Table)
                    .drop_column(ModelConfigs::CacheCreationPrice)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum ModelConfigs {
    Table,
    CacheReadPrice,
    CacheCreationPrice,
}
