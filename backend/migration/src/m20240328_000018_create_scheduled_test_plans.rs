//! Scheduled test plan migration

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScheduledTestPlan::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScheduledTestPlan::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ScheduledTestPlan::Description).text().null())
                    .col(
                        ColumnDef::new(ScheduledTestPlan::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::CronExpr)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::TestConfig)
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::LastRunAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::NextRunAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(ScheduledTestPlan::LastResult).json().null())
                    .col(
                        ColumnDef::new(ScheduledTestPlan::CreatedBy)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestPlan::UpdatedAt)
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
                    .name("idx_scheduled_test_plans_enabled")
                    .table(ScheduledTestPlan::Table)
                    .col(ScheduledTestPlan::Enabled)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_scheduled_test_plans_next_run_at")
                    .table(ScheduledTestPlan::Table)
                    .col(ScheduledTestPlan::NextRunAt)
                    .to_owned(),
            )
            .await?;

        // Create scheduled_test_results table
        manager
            .create_table(
                Table::create()
                    .table(ScheduledTestResult::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScheduledTestResult::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestResult::PlanId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestResult::Status)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ScheduledTestResult::Result).json().null())
                    .col(
                        ColumnDef::new(ScheduledTestResult::ErrorMessage)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestResult::DurationMs)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledTestResult::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for scheduled_test_results
        manager
            .create_index(
                Index::create()
                    .name("idx_scheduled_test_results_plan_id")
                    .table(ScheduledTestResult::Table)
                    .col(ScheduledTestResult::PlanId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScheduledTestResult::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ScheduledTestPlan::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ScheduledTestPlan {
    Table,
    Id,
    Name,
    Description,
    Enabled,
    CronExpr,
    TestConfig,
    LastRunAt,
    NextRunAt,
    LastResult,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ScheduledTestResult {
    Table,
    Id,
    PlanId,
    Status,
    Result,
    ErrorMessage,
    DurationMs,
    CreatedAt,
}
