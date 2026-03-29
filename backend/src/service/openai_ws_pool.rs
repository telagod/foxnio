use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// WebSocket connection pool for OpenAI Realtime API
pub struct OpenAIWsPool {
    connections: Arc<RwLock<HashMap<String, PoolConnection>>>,
    config: PoolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConnection {
    pub id: String,
    pub account_id: i64,
    pub model: String,
    pub status: ConnectionStatus,
    pub created_at: i64,
    pub last_used_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Idle,
    Busy,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub max_connections: usize,
    pub idle_timeout_seconds: u64,
    pub max_requests_per_connection: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            idle_timeout_seconds: 300,
            max_requests_per_connection: 1000,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("Pool exhausted")]
    PoolExhausted,
    #[error("Connection not found")]
    ConnectionNotFound,
    #[error("Connection disconnected")]
    ConnectionDisconnected,
}

impl OpenAIWsPool {
    pub fn new(config: PoolConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get or create connection
    pub async fn get_or_create(&self, account_id: i64, model: &str) -> Result<String, PoolError> {
        let mut connections = self.connections.write().await;

        // Find idle connection
        for (id, conn) in connections.iter_mut() {
            if conn.account_id == account_id
                && conn.model == model
                && conn.status == ConnectionStatus::Idle
            {
                conn.status = ConnectionStatus::Busy;
                conn.last_used_at = chrono::Utc::now().timestamp();
                debug!("Reusing connection {} for account {}", id, account_id);
                return Ok(id.clone());
            }
        }

        // Check pool limit
        if connections.len() >= self.config.max_connections {
            return Err(PoolError::PoolExhausted);
        }

        // Create new connection
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let conn = PoolConnection {
            id: id.clone(),
            account_id,
            model: model.to_string(),
            status: ConnectionStatus::Busy,
            created_at: now,
            last_used_at: now,
        };

        connections.insert(id.clone(), conn);
        info!("Created new connection {} for account {}", id, account_id);
        Ok(id)
    }

    /// Release connection back to pool
    pub async fn release(&self, connection_id: &str) -> Result<(), PoolError> {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(connection_id) {
            conn.status = ConnectionStatus::Idle;
            conn.last_used_at = chrono::Utc::now().timestamp();
            debug!("Released connection {}", connection_id);
        }
        Ok(())
    }

    /// Remove connection from pool
    pub async fn remove(&self, connection_id: &str) {
        let mut connections = self.connections.write().await;
        connections.remove(connection_id);
        debug!("Removed connection {}", connection_id);
    }

    /// Cleanup idle connections
    pub async fn cleanup_idle(&self) {
        let mut connections = self.connections.write().await;
        let now = chrono::Utc::now().timestamp();
        connections
            .retain(|_, conn| now - conn.last_used_at < self.config.idle_timeout_seconds as i64);
    }

    /// Get pool stats
    pub async fn get_stats(&self) -> PoolStats {
        let connections = self.connections.read().await;
        let total = connections.len();
        let idle = connections
            .values()
            .filter(|c| c.status == ConnectionStatus::Idle)
            .count();
        PoolStats {
            total,
            idle,
            busy: total - idle,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub total: usize,
    pub idle: usize,
    pub busy: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_operations() {
        let pool = OpenAIWsPool::new(PoolConfig::default());

        let conn_id = pool.get_or_create(1, "gpt-4o-realtime").await.unwrap();
        assert!(!conn_id.is_empty());

        let stats = pool.get_stats().await;
        assert_eq!(stats.total, 1);
        assert_eq!(stats.busy, 1);

        pool.release(&conn_id).await.unwrap();
        let stats = pool.get_stats().await;
        assert_eq!(stats.idle, 1);
    }
}
