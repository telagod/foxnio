//! Announcement migration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Announcement::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Announcement::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Announcement::Title).string().not_null())
                    .col(ColumnDef::new(Announcement::Content).text().not_null())
                    .col(
                        ColumnDef::new(Announcement::Status)
                            .string_len(20)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(Announcement::NotifyMode)
                            .string_len(20)
                            .not_null()
                            .default("silent"),
                    )
                    .col(ColumnDef::new(Announcement::Targeting).json().null())
                    .col(
                        ColumnDef::new(Announcement::StartsAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Announcement::EndsAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Announcement::CreatedBy).big_integer().null())
                    .col(ColumnDef::new(Announcement::UpdatedBy).big_integer().null())
                    .col(
                        ColumnDef::new(Announcement::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Announcement::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_announcements_status")
                    .table(Announcement::Table)
                    .col(Announcement::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_announcements_created_at")
                    .table(Announcement::Table)
                    .col(Announcement::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Create announcement_reads table
        manager
            .create_table(
                Table::create()
                    .table(AnnouncementRead::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AnnouncementRead::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AnnouncementRead::AnnouncementId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AnnouncementRead::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AnnouncementRead::ReadAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for announcement_reads
        manager
            .create_index(
                Index::create()
                    .name("idx_announcement_reads_unique")
                    .table(AnnouncementRead::Table)
                    .col(AnnouncementRead::AnnouncementId)
                    .col(AnnouncementRead::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AnnouncementRead::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Announcement::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Announcement {
    Table,
    Id,
    Title,
    Content,
    Status,
    NotifyMode,
    Targeting,
    StartsAt,
    EndsAt,
    CreatedBy,
    UpdatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AnnouncementRead {
    Table,
    Id,
    AnnouncementId,
    UserId,
    ReadAt,
}
