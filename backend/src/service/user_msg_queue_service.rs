//! User message queue service

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Queue message
#[derive(Debug, Clone)]
pub struct QueueMessage {
    /// Message ID
    pub id: u64,
    /// User ID
    pub user_id: i64,
    /// Message type
    pub msg_type: String,
    /// Message payload
    pub payload: Vec<u8>,
    /// Priority (higher = more important)
    pub priority: u8,
    /// Created at timestamp
    pub created_at: i64,
}

/// User message queue
pub struct UserMessageQueue {
    /// Queue per user
    queues: Arc<RwLock<HashMap<i64, VecDeque<QueueMessage>>>>,
    /// Message ID counter
    counter: Arc<RwLock<u64>>,
    /// Max queue size per user
    max_queue_size: usize,
}

impl Default for UserMessageQueue {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl UserMessageQueue {
    /// Create a new queue
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            queues: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(1)),
            max_queue_size,
        }
    }

    /// Enqueue a message
    pub async fn enqueue(
        &self,
        user_id: i64,
        msg_type: String,
        payload: Vec<u8>,
        priority: u8,
    ) -> Result<u64, String> {
        let mut queues = self.queues.write().await;
        let mut counter = self.counter.write().await;

        let queue = queues.entry(user_id).or_insert_with(VecDeque::new);

        if queue.len() >= self.max_queue_size {
            return Err("Queue is full".to_string());
        }

        let id = *counter;
        *counter += 1;

        let msg = QueueMessage {
            id,
            user_id,
            msg_type,
            payload,
            priority,
            created_at: chrono::Utc::now().timestamp(),
        };

        // Insert by priority
        let pos = queue
            .iter()
            .position(|m| m.priority < priority)
            .unwrap_or(queue.len());

        queue.insert(pos, msg);

        Ok(id)
    }

    /// Dequeue a message
    pub async fn dequeue(&self, user_id: i64) -> Option<QueueMessage> {
        let mut queues = self.queues.write().await;

        if let Some(queue) = queues.get_mut(&user_id) {
            queue.pop_front()
        } else {
            None
        }
    }

    /// Peek at the next message
    pub async fn peek(&self, user_id: i64) -> Option<QueueMessage> {
        let queues = self.queues.read().await;

        queues.get(&user_id).and_then(|q| q.front().cloned())
    }

    /// Get queue size for a user
    pub async fn queue_size(&self, user_id: i64) -> usize {
        let queues = self.queues.read().await;
        queues.get(&user_id).map(|q| q.len()).unwrap_or(0)
    }

    /// Clear queue for a user
    pub async fn clear_queue(&self, user_id: i64) {
        let mut queues = self.queues.write().await;
        queues.remove(&user_id);
    }

    /// Get total message count
    pub async fn total_messages(&self) -> usize {
        let queues = self.queues.read().await;
        queues.values().map(|q| q.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = UserMessageQueue::new(100);

        let id = queue
            .enqueue(123, "test".to_string(), b"payload".to_vec(), 5)
            .await
            .unwrap();
        assert_eq!(id, 1);

        let msg = queue.dequeue(123).await.unwrap();
        assert_eq!(msg.msg_type, "test");
    }

    #[tokio::test]
    async fn test_priority() {
        let queue = UserMessageQueue::new(100);

        queue
            .enqueue(123, "low".to_string(), vec![], 1)
            .await
            .unwrap();
        queue
            .enqueue(123, "high".to_string(), vec![], 10)
            .await
            .unwrap();

        let msg = queue.dequeue(123).await.unwrap();
        assert_eq!(msg.msg_type, "high");
    }
}
