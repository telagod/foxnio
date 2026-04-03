use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 添加 api_keys 表的关键索引
        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_key")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::Key)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_keys_user_status")
                    .table(ApiKeys::Table)
                    .col(ApiKeys::UserId)
                    .col(ApiKeys::Status)
                    .to_owned(),
            )
            .await?;

        // 添加 accounts 表的关键索引
        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_provider_status")
                    .table(Accounts::Table)
                    .col(Accounts::Provider)
                    .col(Accounts::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_accounts_status")
                    .table(Accounts::Table)
                    .col(Accounts::Status)
                    .to_owned(),
            )
            .await?;

        // 添加 usages 表的关键索引
        manager
            .create_index(
                Index::create()
                    .name("idx_usages_model_created")
                    .table(Usages::Table)
                    .col(Usages::Model)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_usages_success_created")
                    .table(Usages::Table)
                    .col(Usages::Success)
                    .col(Usages::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // 添加 users 表的关键索引
        manager
            .create_index(
                Index::create()
                    .name("idx_users_status")
                    .table(Users::Table)
                    .col(Users::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_role_status")
                    .table(Users::Table)
                    .col(Users::Role)
                    .col(Users::Status)
                    .to_owned(),
            )
            .await?;

        // 添加 model_configs 表的关键索引
        manager
            .create_index(
                Index::create()
                    .name("idx_model_configs_provider")
                    .table(ModelConfigs::Table)
                    .col(ModelConfigs::Provider)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_model_configs_enabled")
                    .table(ModelConfigs::Table)
                    .col(ModelConfigs::Enabled)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除 api_keys 表的索引
        manager
            .drop_index(Index::drop().name("idx_api_keys_key").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_api_keys_user_status").to_owned())
            .await?;

        // 删除 accounts 表的索引
        manager
            .drop_index(
                Index::drop()
                    .name("idx_accounts_provider_status")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(Index::drop().name("idx_accounts_status").to_owned())
            .await?;

        // 删除 usages 表的索引
        manager
            .drop_index(Index::drop().name("idx_usages_model_created").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_usages_success_created").to_owned())
            .await?;

        // 删除 users 表的索引
        manager
            .drop_index(Index::drop().name("idx_users_status").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_users_role_status").to_owned())
            .await?;

        // 删除 model_configs 表的索引
        manager
            .drop_index(Index::drop().name("idx_model_configs_provider").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_model_configs_enabled").to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum ApiKeys {
    Table,
    Key,
    UserId,
    Status,
}

#[derive(DeriveIden)]
pub enum Accounts {
    Table,
    Provider,
    Status,
}

#[derive(DeriveIden)]
pub enum Usages {
    Table,
    Model,
    Success,
    CreatedAt,
}

#[derive(DeriveIden)]
pub enum Users {
    Table,
    Status,
    Role,
}

#[derive(DeriveIden)]
pub enum ModelConfigs {
    Table,
    Provider,
    Enabled,
}
