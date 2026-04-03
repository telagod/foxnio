use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 groups 表
        manager
            .create_table(
                Table::create()
                    .table(Groups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Groups::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Groups::Name).string().not_null())
                    .col(ColumnDef::new(Groups::Description).text())
                    .col(
                        ColumnDef::new(Groups::Platform)
                            .string()
                            .not_null()
                            .default("openai"),
                    )
                    .col(
                        ColumnDef::new(Groups::Status)
                            .string()
                            .not_null()
                            .default("active"),
                    )
                    // 配额管理
                    .col(ColumnDef::new(Groups::DailyLimitUsd).double())
                    .col(ColumnDef::new(Groups::WeeklyLimitUsd).double())
                    .col(ColumnDef::new(Groups::MonthlyLimitUsd).double())
                    // 速率限制
                    .col(
                        ColumnDef::new(Groups::RateMultiplier)
                            .double()
                            .not_null()
                            .default(1.0),
                    )
                    // 模型路由
                    .col(ColumnDef::new(Groups::ModelRouting).json())
                    .col(
                        ColumnDef::new(Groups::ModelRoutingEnabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // 降级配置
                    .col(ColumnDef::new(Groups::FallbackGroupId).big_integer())
                    // Claude Code 限制
                    .col(
                        ColumnDef::new(Groups::ClaudeCodeOnly)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Groups::FallbackGroupIdOnInvalidRequest).big_integer())
                    // 排序和显示
                    .col(
                        ColumnDef::new(Groups::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Groups::IsExclusive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // 时间戳
                    .col(
                        ColumnDef::new(Groups::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Groups::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Groups::DeletedAt).timestamp_with_time_zone())
                    // 索引
                    .index(
                        Index::create()
                            .name("idx_groups_status")
                            .col(Groups::Status),
                    )
                    .index(
                        Index::create()
                            .name("idx_groups_platform")
                            .col(Groups::Platform),
                    )
                    .index(
                        Index::create()
                            .name("idx_groups_sort_order")
                            .col(Groups::SortOrder),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建 account_groups 关联表
        manager
            .create_table(
                Table::create()
                    .table(AccountGroups::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(AccountGroups::AccountId).uuid().not_null())
                    .col(
                        ColumnDef::new(AccountGroups::GroupId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AccountGroups::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(AccountGroups::AccountId)
                            .col(AccountGroups::GroupId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_account_groups_account")
                            .from(AccountGroups::Table, AccountGroups::AccountId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_account_groups_group")
                            .from(AccountGroups::Table, AccountGroups::GroupId)
                            .to(Groups::Table, Groups::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 为 accounts 表添加 group_id 字段（可选，用于默认分组）
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .add_column(ColumnDef::new(Accounts::GroupId).big_integer())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_accounts_group")
                            .from_tbl(Accounts::Table)
                            .from_col(Accounts::GroupId)
                            .to_tbl(Groups::Table)
                            .to_col(Groups::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // 为 api_keys 表添加 group_id 字段
        manager
            .alter_table(
                Table::alter()
                    .table(ApiKeys::Table)
                    .add_column(ColumnDef::new(ApiKeys::GroupId).big_integer())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_api_keys_group")
                            .from_tbl(ApiKeys::Table)
                            .from_col(ApiKeys::GroupId)
                            .to_tbl(Groups::Table)
                            .to_col(Groups::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除 api_keys 的 group_id 字段
        manager
            .alter_table(
                Table::alter()
                    .table(ApiKeys::Table)
                    .drop_foreign_key(Alias::new("fk_api_keys_group"))
                    .drop_column(ApiKeys::GroupId)
                    .to_owned(),
            )
            .await?;

        // 删除 accounts 的 group_id 字段
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .drop_foreign_key(Alias::new("fk_accounts_group"))
                    .drop_column(Accounts::GroupId)
                    .to_owned(),
            )
            .await?;

        // 删除 account_groups 表
        manager
            .drop_table(Table::drop().table(AccountGroups::Table).to_owned())
            .await?;

        // 删除 groups 表
        manager
            .drop_table(Table::drop().table(Groups::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Groups {
    Table,
    Id,
    Name,
    Description,
    Platform,
    Status,
    DailyLimitUsd,
    WeeklyLimitUsd,
    MonthlyLimitUsd,
    RateMultiplier,
    ModelRouting,
    ModelRoutingEnabled,
    FallbackGroupId,
    ClaudeCodeOnly,
    FallbackGroupIdOnInvalidRequest,
    SortOrder,
    IsExclusive,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
pub enum AccountGroups {
    Table,
    AccountId,
    GroupId,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum Accounts {
    Table,
    Id,
    GroupId,
}

#[derive(DeriveIden)]
pub enum ApiKeys {
    Table,
    GroupId,
}
