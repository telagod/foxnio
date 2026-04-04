//! Backup service — pg_dump/psql based file-system backup & restore.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

/// A single backup record returned to callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

/// File-system + pg_dump/psql backup service.
pub struct BackupService {
    db_url: String,
    backup_dir: String,
}

const FILENAME_PREFIX: &str = "foxnio_backup_";
const FILENAME_SUFFIX: &str = ".sql.gz";

// ── helpers ──────────────────────────────────────────────────────────

/// Reject any filename that could escape the backup directory.
fn validate_filename(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("filename must not be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        bail!("invalid filename: path traversal detected");
    }
    if !name.starts_with(FILENAME_PREFIX) || !name.ends_with(FILENAME_SUFFIX) {
        bail!("invalid backup filename pattern");
    }
    Ok(())
}

/// Try to extract a `DateTime<Utc>` from the timestamp embedded in the filename.
fn parse_timestamp_from_filename(name: &str) -> Option<DateTime<Utc>> {
    // foxnio_backup_20260405_153012.sql.gz  →  20260405_153012
    let stem = name.strip_prefix(FILENAME_PREFIX)?.strip_suffix(FILENAME_SUFFIX)?;
    let naive = NaiveDateTime::parse_from_str(stem, "%Y%m%d_%H%M%S").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

// ── implementation ───────────────────────────────────────────────────

impl BackupService {
    pub fn new(db_url: String, backup_dir: String) -> Self {
        Self { db_url, backup_dir }
    }

    /// Expose the database URL (used by the import façade).
    pub fn db_url(&self) -> &str {
        &self.db_url
    }

    /// Expose the backup directory path.
    pub fn backup_dir(&self) -> &str {
        &self.backup_dir
    }

    /// Ensure the backup directory exists.
    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.backup_dir)
            .await
            .with_context(|| format!("failed to create backup dir: {}", self.backup_dir))?;
        Ok(())
    }

    /// Create a new database backup via `pg_dump | gzip`.
    pub async fn create_backup(&self) -> Result<BackupRecord> {
        self.ensure_dir().await?;

        let now = Utc::now();
        let filename = format!(
            "{}{}{}",
            FILENAME_PREFIX,
            now.format("%Y%m%d_%H%M%S"),
            FILENAME_SUFFIX,
        );
        let filepath = Path::new(&self.backup_dir).join(&filename);

        info!("creating backup: {}", filepath.display());

        // pg_dump $DB_URL | gzip > $filepath
        let shell_cmd = format!(
            "pg_dump '{}' | gzip > '{}'",
            self.db_url,
            filepath.display(),
        );
        let output = Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .output()
            .await
            .context("failed to spawn pg_dump")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Clean up partial file on failure
            let _ = tokio::fs::remove_file(&filepath).await;
            bail!("pg_dump failed: {}", stderr);
        }

        let meta = tokio::fs::metadata(&filepath)
            .await
            .context("failed to stat backup file")?;

        let record = BackupRecord {
            id: uuid::Uuid::new_v4().to_string(),
            filename: filename.clone(),
            size_bytes: meta.len(),
            created_at: now,
        };

        info!("backup created: {} ({} bytes)", filename, meta.len());
        Ok(record)
    }

    /// List all backups in the backup directory, sorted by date descending.
    pub async fn list_backups(&self) -> Result<Vec<BackupRecord>> {
        self.ensure_dir().await?;

        let mut entries = tokio::fs::read_dir(&self.backup_dir)
            .await
            .context("failed to read backup dir")?;

        let mut records = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(FILENAME_PREFIX) || !name.ends_with(FILENAME_SUFFIX) {
                continue;
            }
            let meta = entry.metadata().await?;
            if !meta.is_file() {
                continue;
            }
            let created_at = parse_timestamp_from_filename(&name).unwrap_or_else(|| Utc::now());
            records.push(BackupRecord {
                id: uuid::Uuid::new_v4().to_string(),
                filename: name,
                size_bytes: meta.len(),
                created_at,
            });
        }

        // newest first
        records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(records)
    }

    /// Restore a backup by piping `gunzip -c <file> | psql <db_url>`.
    pub async fn restore_backup(&self, filename: &str) -> Result<()> {
        validate_filename(filename)?;
        let filepath = Path::new(&self.backup_dir).join(filename);

        if !filepath.exists() {
            bail!("backup file not found: {}", filename);
        }

        info!("restoring backup: {}", filepath.display());

        let shell_cmd = format!(
            "gunzip -c '{}' | psql '{}'",
            filepath.display(),
            self.db_url,
        );
        let output = Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .output()
            .await
            .context("failed to spawn restore pipeline")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("restore failed: {}", stderr);
        }

        info!("backup restored: {}", filename);
        Ok(())
    }

    /// Delete a backup file. Validates filename to prevent path traversal.
    pub async fn delete_backup(&self, filename: &str) -> Result<()> {
        validate_filename(filename)?;
        let filepath = Path::new(&self.backup_dir).join(filename);

        // Canonicalize both paths to ensure the file is truly inside backup_dir
        let canon_dir = tokio::fs::canonicalize(&self.backup_dir)
            .await
            .context("backup dir does not exist")?;
        let canon_file = tokio::fs::canonicalize(&filepath)
            .await
            .context("backup file does not exist")?;

        if !canon_file.starts_with(&canon_dir) {
            bail!("path traversal blocked");
        }

        tokio::fs::remove_file(&canon_file)
            .await
            .with_context(|| format!("failed to delete backup: {}", filename))?;

        info!("backup deleted: {}", filename);
        Ok(())
    }

    /// Return the validated path to a backup file (for download).
    pub fn get_backup_path(&self, filename: &str) -> Result<PathBuf> {
        validate_filename(filename)?;
        let filepath = Path::new(&self.backup_dir).join(filename);
        if !filepath.exists() {
            bail!("backup file not found: {}", filename);
        }
        Ok(filepath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_filename_ok() {
        assert!(validate_filename("foxnio_backup_20260405_153012.sql.gz").is_ok());
    }

    #[test]
    fn test_validate_filename_traversal() {
        assert!(validate_filename("../etc/passwd").is_err());
        assert!(validate_filename("foxnio_backup_../../evil.sql.gz").is_err());
        assert!(validate_filename("foxnio_backup_20260405.tar.gz").is_err());
    }

    #[test]
    fn test_parse_timestamp() {
        let dt = parse_timestamp_from_filename("foxnio_backup_20260405_153012.sql.gz");
        assert!(dt.is_some());
        let dt = dt.unwrap();
        assert_eq!(dt.format("%Y%m%d_%H%M%S").to_string(), "20260405_153012");
    }
}
