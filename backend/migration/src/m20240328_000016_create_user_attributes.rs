//! User attribute migration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create user_attribute_definitions table
        manager
            .create_table(
                Table::create()
                    .table(UserAttributeDefinition::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserAttributeDefinition::Key).string_len(100).not_null())
                    .col(ColumnDef::new(UserAttributeDefinition::Name).string_len(255).not_null())
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Description)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(UserAttributeDefinition::Type).string_len(20).not_null())
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Options)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Required)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Validation)
                            .json()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Placeholder)
                            .string_len(255)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::DisplayOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::Enabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::DeletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UserAttributeDefinition::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for user_attribute_definitions
        manager
            .create_index(
                Index::create()
                    .name("idx_user_attribute_definitions_key")
                    .table(UserAttributeDefinition::Table)
                    .col(UserAttributeDefinition::Key)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_attribute_definitions_enabled")
                    .table(UserAttributeDefinition::Table)
                    .col(UserAttributeDefinition::Enabled)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_attribute_definitions_deleted_at")
                    .table(UserAttributeDefinition::Table)
                    .col(UserAttributeDefinition::DeletedAt)
                    .to_owned(),
            )
            .await?;

        // Create user_attribute_values table
        manager
            .create_table(
                Table::create()
                    .table(UserAttributeValue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAttributeValue::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserAttributeValue::UserId).big_integer().not_null())
                    .col(ColumnDef::new(UserAttributeValue::AttributeId).big_integer().not_null())
                    .col(ColumnDef::new(UserAttributeValue::Value).text().not_null().default(""))
                    .col(
                        ColumnDef::new(UserAttributeValue::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UserAttributeValue::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for user_attribute_values
        manager
            .create_index(
                Index::create()
                    .name("idx_user_attribute_values_unique")
                    .table(UserAttributeValue::Table)
                    .col(UserAttributeValue::UserId)
                    .col(UserAttributeValue::AttributeId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_attribute_values_attribute_id")
                    .table(UserAttributeValue::Table)
                    .col(UserAttributeValue::AttributeId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserAttributeValue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserAttributeDefinition::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserAttributeDefinition {
    Table,
    Id,
    Key,
    Name,
    Description,
    Type,
    Options,
    Required,
    Validation,
    Placeholder,
    DisplayOrder,
    Enabled,
    DeletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserAttributeValue {
    Table,
    Id,
    UserId,
    AttributeId,
    Value,
    CreatedAt,
    UpdatedAt,
}
