use std::sync::Arc;
use tokio::sync::RwLock;

/// Buffer pool for SSE (Server-Sent Events) scanner
pub struct SseScannerBufferPool {
    pool: Arc<RwLock<Vec<Vec<u8>>>>,
    buffer_size: usize,
    max_buffers: usize,
}

impl SseScannerBufferPool {
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        Self {
            pool: Arc::new(RwLock::new(Vec::with_capacity(max_buffers))),
            buffer_size,
            max_buffers,
        }
    }

    /// Acquire buffer from pool
    pub async fn acquire(&self) -> Vec<u8> {
        let mut pool = self.pool.write().await;
        pool.pop().unwrap_or_else(|| vec![0u8; self.buffer_size])
    }

    /// Return buffer to pool
    pub async fn release(&self, buffer: Vec<u8>) {
        let mut pool = self.pool.write().await;
        if pool.len() < self.max_buffers {
            pool.push(buffer);
        }
    }

    /// Get pool stats
    pub async fn stats(&self) -> PoolStats {
        let pool = self.pool.read().await;
        PoolStats {
            available_buffers: pool.len(),
            buffer_size: self.buffer_size,
            max_buffers: self.max_buffers,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available_buffers: usize,
    pub buffer_size: usize,
    pub max_buffers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_buffer_pool() {
        let pool = SseScannerBufferPool::new(4096, 10);

        let buffer = pool.acquire().await;
        assert_eq!(buffer.len(), 4096);

        pool.release(buffer).await;

        let stats = pool.stats().await;
        assert_eq!(stats.available_buffers, 1);
    }
}
