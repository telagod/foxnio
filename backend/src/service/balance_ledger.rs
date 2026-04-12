//! Balance Ledger Service
//!
//! Records all balance mutations with before/after snapshots for auditability.

#![allow(dead_code)]

use anyhow::{bail, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::{balance_ledger, users};

/// Aggregated balance summary per source_type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSummary {
    pub user_id: Uuid,
    pub current_balance: i64,
    pub entries: Vec<SourceAggregate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceAggregate {
    pub source_type: String,
    pub total_credits: i64,
    pub total_debits: i64,
    pub count: u64,
}

pub struct BalanceLedgerService {
    db: DatabaseConnection,
}

impl BalanceLedgerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Record a balance mutation atomically:
    /// 1. Fetch current user.balance
    /// 2. Insert ledger entry with balance_before / balance_after
    /// 3. Update user.balance
    pub async fn record(
        &self,
        user_id: Uuid,
        source_type: &str,
        source_id: Option<String>,
        delta_cents: i64,
        description: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<balance_ledger::Model> {
        let txn = self.db.begin().await?;
        let entry = Self::record_with_txn(
            &txn,
            user_id,
            source_type,
            source_id,
            delta_cents,
            description,
            metadata,
        )
        .await?;
        txn.commit().await?;
        Ok(entry)
    }

    /// Record a ledger entry inside an existing transaction.
    /// Caller is responsible for committing.
    pub async fn record_with_txn<C: ConnectionTrait>(
        txn: &C,
        user_id: Uuid,
        source_type: &str,
        source_id: Option<String>,
        delta_cents: i64,
        description: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<balance_ledger::Model> {
        let user = users::Entity::find_by_id(user_id)
            .one(txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let balance_before = user.balance;
        let balance_after = balance_before + delta_cents;

        if balance_after < 0 {
            bail!(
                "Insufficient balance: current={}, delta={}",
                balance_before,
                delta_cents
            );
        }

        // Insert ledger entry
        let entry = balance_ledger::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            source_type: Set(source_type.to_string()),
            source_id: Set(source_id),
            delta_cents: Set(delta_cents),
            balance_before: Set(balance_before),
            balance_after: Set(balance_after),
            description: Set(description),
            metadata: Set(metadata),
            created_at: Set(Utc::now()),
        };
        let entry = entry.insert(txn).await?;

        // Update user balance
        let mut user_am: users::ActiveModel = user.into();
        user_am.balance = Set(balance_after);
        user_am.updated_at = Set(Utc::now());
        user_am.update(txn).await?;

        Ok(entry)
    }

    /// Insert a ledger row only (no user.balance update).
    /// Use when the caller already handles the balance mutation.
    pub async fn insert_entry_with_txn<C: ConnectionTrait>(
        txn: &C,
        user_id: Uuid,
        source_type: &str,
        source_id: Option<String>,
        delta_cents: i64,
        balance_before: i64,
        balance_after: i64,
        description: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<balance_ledger::Model> {
        let entry = balance_ledger::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            source_type: Set(source_type.to_string()),
            source_id: Set(source_id),
            delta_cents: Set(delta_cents),
            balance_before: Set(balance_before),
            balance_after: Set(balance_after),
            description: Set(description),
            metadata: Set(metadata),
            created_at: Set(Utc::now()),
        };
        Ok(entry.insert(txn).await?)
    }

    /// Paginated ledger for a user, newest first.
    pub async fn get_user_ledger(
        &self,
        user_id: Uuid,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<balance_ledger::Model>, u64)> {
        let paginator = balance_ledger::Entity::find()
            .filter(balance_ledger::Column::UserId.eq(user_id))
            .order_by_desc(balance_ledger::Column::CreatedAt)
            .paginate(&self.db, per_page);

        let total = paginator.num_items().await?;
        let entries = paginator.fetch_page(page.saturating_sub(1)).await?;

        Ok((entries, total))
    }

    /// Aggregate credits/debits per source_type for a user.
    pub async fn get_balance_summary(&self, user_id: Uuid) -> Result<BalanceSummary> {
        let user = users::Entity::find_by_id(user_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let all_entries = balance_ledger::Entity::find()
            .filter(balance_ledger::Column::UserId.eq(user_id))
            .all(&self.db)
            .await?;

        let mut map = std::collections::HashMap::<String, SourceAggregate>::new();

        for entry in &all_entries {
            let agg = map
                .entry(entry.source_type.clone())
                .or_insert_with(|| SourceAggregate {
                    source_type: entry.source_type.clone(),
                    total_credits: 0,
                    total_debits: 0,
                    count: 0,
                });
            agg.count += 1;
            if entry.delta_cents >= 0 {
                agg.total_credits += entry.delta_cents;
            } else {
                agg.total_debits += entry.delta_cents;
            }
        }

        let mut entries: Vec<SourceAggregate> = map.into_values().collect();
        entries.sort_by(|a, b| a.source_type.cmp(&b.source_type));

        Ok(BalanceSummary {
            user_id,
            current_balance: user.balance,
            entries,
        })
    }
}
