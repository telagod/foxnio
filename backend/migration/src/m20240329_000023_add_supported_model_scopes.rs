use sea_orm_migration::prelude::*;

use crate::m20240328_000011_create_groups::Groups;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 添加 supported_model_scopes 字段
        manager
            .alter_table(
                Table::alter()
                    .table(Groups::Table)
                    .add_column(
                        ColumnDef::new(Alias::new("supported_model_scopes"))
                            .json()
                            .comment("支持的模型系列：claude, gemini_text, gemini_image"),
                    )
                    .to_owned(),
            )
            .await?;

        // 设置默认值
        manager
            .get_connection()
            .execute_unprepared(
                r#"UPDATE groups SET supported_model_scopes = '["claude", "gemini_text", "gemini_image"]'::jsonb WHERE supported_model_scopes IS NULL"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Groups::Table)
                    .drop_column(Alias::new("supported_model_scopes"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
