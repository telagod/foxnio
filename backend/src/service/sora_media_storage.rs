use serde::{Deserialize, Serialize};

/// Media storage service for Sora videos
pub struct SoraMediaStorage {
    config: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub storage_type: StorageType,
    pub bucket_name: String,
    pub region: String,
    pub cdn_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StorageType {
    S3,
    GCS,
    AzureBlob,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub url: String,
    pub key: String,
    pub size_bytes: u64,
    pub content_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("File not found")]
    NotFound,
}

impl SoraMediaStorage {
    pub fn new(config: StorageConfig) -> Self {
        Self { config }
    }

    /// Upload video
    pub async fn upload(
        &self,
        key: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<UploadResult, StorageError> {
        // In real implementation, would upload to S3/GCS/etc
        let url = format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            self.config.bucket_name, self.config.region, key
        );

        Ok(UploadResult {
            url,
            key: key.to_string(),
            size_bytes: data.len() as u64,
            content_type: content_type.to_string(),
        })
    }

    /// Generate signed URL
    pub fn generate_signed_url(&self, key: &str, expires_in_seconds: u64) -> String {
        // In real implementation, would generate signed URL
        format!(
            "https://{}.s3.{}.amazonaws.com/{}?signature=xxx&expires={}",
            self.config.bucket_name, self.config.region, key, expires_in_seconds
        )
    }

    /// Delete video
    pub async fn delete(&self, _key: &str) -> Result<(), StorageError> {
        // In real implementation, would delete from storage
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config() {
        let config = StorageConfig {
            storage_type: StorageType::S3,
            bucket_name: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            cdn_enabled: true,
        };

        assert_eq!(config.storage_type, StorageType::S3);
    }
}
