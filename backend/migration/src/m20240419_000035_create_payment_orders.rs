//! 支付订单表

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PaymentOrders::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(PaymentOrders::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(PaymentOrders::UserId).uuid().not_null())
                    .col(ColumnDef::new(PaymentOrders::OrderNo).string_len(64).not_null().unique_key())
                    .col(ColumnDef::new(PaymentOrders::Provider).string_len(32).not_null())
                    .col(ColumnDef::new(PaymentOrders::PaymentType).string_len(32).not_null())
                    .col(ColumnDef::new(PaymentOrders::AmountCents).big_integer().not_null())
                    .col(ColumnDef::new(PaymentOrders::Currency).string_len(8).not_null().default("CNY"))
                    .col(ColumnDef::new(PaymentOrders::Status).string_len(32).not_null().default("pending"))
                    .col(ColumnDef::new(PaymentOrders::ProviderOrderId).string_len(255).null())
                    .col(ColumnDef::new(PaymentOrders::ProviderData).json_binary().null())
                    .col(ColumnDef::new(PaymentOrders::PaymentUrl).text().null())
                    .col(ColumnDef::new(PaymentOrders::ClientSecret).string_len(255).null())
                    .col(ColumnDef::new(PaymentOrders::ExpiresAt).timestamp_with_time_zone().null())
                    .col(ColumnDef::new(PaymentOrders::PaidAt).timestamp_with_time_zone().null())
                    .col(ColumnDef::new(PaymentOrders::CompletedAt).timestamp_with_time_zone().null())
                    .col(ColumnDef::new(PaymentOrders::CreatedAt).timestamp_with_time_zone().not_null())
                    .col(ColumnDef::new(PaymentOrders::UpdatedAt).timestamp_with_time_zone().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(PaymentOrders::Table, PaymentOrders::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager.create_index(Index::create().name("idx_payment_orders_user_id").table(PaymentOrders::Table).col(PaymentOrders::UserId).to_owned()).await?;
        manager.create_index(Index::create().name("idx_payment_orders_status").table(PaymentOrders::Table).col(PaymentOrders::Status).to_owned()).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(PaymentOrders::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
pub enum PaymentOrders {
    Table, Id, UserId, OrderNo, Provider, PaymentType, AmountCents, Currency,
    Status, ProviderOrderId, ProviderData, PaymentUrl, ClientSecret,
    ExpiresAt, PaidAt, CompletedAt, CreatedAt, UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Users { Table, Id }
