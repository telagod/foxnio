//! Group capacity service

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Group capacity
#[derive(Debug, Clone)]
pub struct GroupCapacity {
    pub group_id: i64,
    pub total_capacity: u64,
    pub used_capacity: u64,
    pub reserved_capacity: u64,
}

impl GroupCapacity {
    pub fn available(&self) -> u64 {
        self.total_capacity
            .saturating_sub(self.used_capacity)
            .saturating_sub(self.reserved_capacity)
    }

    pub fn utilization(&self) -> f64 {
        if self.total_capacity == 0 {
            0.0
        } else {
            (self.used_capacity as f64 / self.total_capacity as f64) * 100.0
        }
    }
}

/// Group capacity service
pub struct GroupCapacityService {
    capacities: Arc<RwLock<HashMap<i64, GroupCapacity>>>,
}

impl Default for GroupCapacityService {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupCapacityService {
    pub fn new() -> Self {
        Self {
            capacities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_capacity(&self, group_id: i64, total: u64) {
        let mut capacities = self.capacities.write().await;
        capacities.insert(
            group_id,
            GroupCapacity {
                group_id,
                total_capacity: total,
                used_capacity: 0,
                reserved_capacity: 0,
            },
        );
    }

    pub async fn get_capacity(&self, group_id: i64) -> Option<GroupCapacity> {
        let capacities = self.capacities.read().await;
        capacities.get(&group_id).cloned()
    }

    pub async fn allocate(&self, group_id: i64, amount: u64) -> Result<(), String> {
        let mut capacities = self.capacities.write().await;
        let cap = capacities.get_mut(&group_id).ok_or("Group not found")?;

        if cap.available() < amount {
            return Err("Insufficient capacity".to_string());
        }

        cap.used_capacity += amount;
        Ok(())
    }

    pub async fn release(&self, group_id: i64, amount: u64) {
        let mut capacities = self.capacities.write().await;
        if let Some(cap) = capacities.get_mut(&group_id) {
            cap.used_capacity = cap.used_capacity.saturating_sub(amount);
        }
    }

    pub async fn reserve(&self, group_id: i64, amount: u64) -> Result<(), String> {
        let mut capacities = self.capacities.write().await;
        let cap = capacities.get_mut(&group_id).ok_or("Group not found")?;

        if cap.available() < amount {
            return Err("Insufficient capacity".to_string());
        }

        cap.reserved_capacity += amount;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capacity() {
        let service = GroupCapacityService::new();

        service.set_capacity(1, 1000).await;
        service.allocate(1, 500).await.unwrap();

        let cap = service.get_capacity(1).await.unwrap();
        assert_eq!(cap.used_capacity, 500);
        assert_eq!(cap.available(), 500);
    }
}
