//! Backup Service — export/import façade over BackupService (pg_dump/psql).

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

use super::backup_service::{BackupRecord, BackupService};

// ── request / response types (kept for handler compat) ───────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupData {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub sql_dump: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub tables: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub filename: String,
    pub metadata: BackupMetadata,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub success: bool,
    pub message: String,
}

// ── façade ───────────────────────────────────────────────────────────

/// Thin wrapper that delegates to [`BackupService`] for the handler layer.
pub struct BackupFacade;

impl BackupFacade {
    /// Run pg_dump via BackupService and return the filename + metadata.
    pub async fn export(svc: &BackupService, _tables: Option<Vec<String>>) -> Result<ExportResponse> {
        let record: BackupRecord = svc.create_backup().await?;
        Ok(ExportResponse {
            filename: record.filename.clone(),
            metadata: BackupMetadata {
                filename: record.filename,
                size_bytes: record.size_bytes,
                created_at: record.created_at,
            },
        })
    }

    /// Accept raw SQL bytes, write to a temp file, then restore via psql.
    pub async fn import(svc: &BackupService, sql_bytes: &[u8]) -> Result<ImportResponse> {
        // Write to a temp file in the backup dir, then restore
        let tmp_name = format!("foxnio_import_{}.sql", Utc::now().format("%Y%m%d_%H%M%S"));
        let tmp_path = Path::new(svc.backup_dir()).join(&tmp_name);

        tokio::fs::write(&tmp_path, sql_bytes)
            .await
            .context("failed to write import temp file")?;

        // Pipe through psql
        let shell_cmd = format!("psql '{}' < '{}'", svc.db_url(), tmp_path.display());
        let output = Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .output()
            .await
            .context("failed to spawn psql for import")?;

        // Clean up temp file regardless of outcome
        let _ = tokio::fs::remove_file(&tmp_path).await;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("import failed: {}", stderr);
        }

        info!("import completed from {} bytes of SQL", sql_bytes.len());
        Ok(ImportResponse {
            success: true,
            message: format!("imported {} bytes of SQL", sql_bytes.len()),
        })
    }
}
