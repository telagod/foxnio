//! 告警管理器模块
//!
//! 提供告警规则管理、检查和发送功能
//!
//! 预留功能：告警管理器（扩展功能）

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{
    channels::AlertChannel,
    history::{AlertHistory, AlertHistoryEntry, AlertHistoryFilter},
    rules::{AlertRule, MetricsSnapshot, RuleCheckResult, RuleState},
    Alert, AlertChannelType, AlertSendResult, SilenceRule,
};
use crate::alert::channels::create_channel;

/// 告警管理器配置
#[derive(Debug, Clone)]
pub struct AlertManagerConfig {
    /// 历史记录最大数量
    pub max_history_entries: usize,
    /// 默认告警通道
    pub default_channels: Vec<AlertChannelType>,
}

impl Default for AlertManagerConfig {
    fn default() -> Self {
        Self {
            max_history_entries: 1000,
            default_channels: vec![AlertChannelType::Email],
        }
    }
}

/// 告警管理器
pub struct AlertManager {
    /// 告警规则
    rules: Arc<RwLock<Vec<AlertRule>>>,
    /// 规则状态
    rule_states: Arc<RwLock<HashMap<String, RuleState>>>,
    /// 告警通道
    channels: Arc<RwLock<HashMap<String, Box<dyn AlertChannel>>>>,
    /// 告警历史
    history: AlertHistory,
    /// 静默规则
    silence_rules: Arc<RwLock<Vec<SilenceRule>>>,
    /// 配置
    config: AlertManagerConfig,
}

impl AlertManager {
    /// 创建新的告警管理器
    pub fn new(config: AlertManagerConfig) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_states: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
            history: AlertHistory::new(config.max_history_entries),
            silence_rules: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(AlertManagerConfig::default())
    }

    // ============ 规则管理 ============

    /// 添加规则
    pub async fn add_rule(&self, rule: AlertRule) -> String {
        let rule_id = rule.id.clone();
        let mut rules = self.rules.write().await;
        rules.push(rule);
        rule_id
    }

    /// 获取规则
    pub async fn get_rule(&self, id: &str) -> Option<AlertRule> {
        let rules = self.rules.read().await;
        rules.iter().find(|r| r.id == id).cloned()
    }

    /// 列出所有规则
    pub async fn list_rules(&self) -> Vec<AlertRule> {
        self.rules.read().await.clone()
    }

    /// 更新规则
    pub async fn update_rule(&self, id: &str, rule: AlertRule) -> bool {
        let mut rules = self.rules.write().await;
        if let Some(existing) = rules.iter_mut().find(|r| r.id == id) {
            *existing = rule;
            true
        } else {
            false
        }
    }

    /// 删除规则
    pub async fn delete_rule(&self, id: &str) -> bool {
        let mut rules = self.rules.write().await;
        let initial_len = rules.len();
        rules.retain(|r| r.id != id);
        rules.len() != initial_len
    }

    /// 启用规则
    pub async fn enable_rule(&self, id: &str) -> bool {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = true;
            rule.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// 禁用规则
    pub async fn disable_rule(&self, id: &str) -> bool {
        let mut rules = self.rules.write().await;
        if let Some(rule) = rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = false;
            rule.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    // ============ 通道管理 ============

    /// 注册告警通道
    pub async fn register_channel(
        &self,
        id: String,
        channel_type: AlertChannelType,
        config: serde_json::Value,
    ) -> Result<(), String> {
        let channel = create_channel(channel_type, &config)?;
        let mut channels = self.channels.write().await;
        channels.insert(id, channel);
        Ok(())
    }

    /// 移除告警通道
    pub async fn remove_channel(&self, id: &str) -> bool {
        let mut channels = self.channels.write().await;
        channels.remove(id).is_some()
    }

    /// 列出所有通道
    pub async fn list_channels(&self) -> Vec<(String, AlertChannelType, String)> {
        let channels = self.channels.read().await;
        channels
            .iter()
            .map(|(id, channel)| {
                (
                    id.clone(),
                    channel.channel_type(),
                    channel.name().to_string(),
                )
            })
            .collect()
    }

    // ============ 静默管理 ============

    /// 添加静默规则
    pub async fn add_silence(&self, silence: SilenceRule) {
        let mut silence_rules = self.silence_rules.write().await;
        silence_rules.push(silence);
    }

    /// 移除静默规则
    pub async fn remove_silence(&self, id: &str) -> bool {
        let mut silence_rules = self.silence_rules.write().await;
        let initial_len = silence_rules.len();
        silence_rules.retain(|s| s.id != id);
        silence_rules.len() != initial_len
    }

    /// 列出静默规则
    pub async fn list_silences(&self) -> Vec<SilenceRule> {
        let silence_rules = self.silence_rules.read().await;
        silence_rules.clone()
    }

    /// 检查规则是否被静默
    pub async fn is_silenced(&self, rule_name: &str) -> bool {
        let silence_rules = self.silence_rules.read().await;
        silence_rules
            .iter()
            .any(|s| s.is_active() && s.matches(rule_name))
    }

    // ============ 规则检查 ============

    /// 检查所有规则
    pub async fn check_rules(&self, metrics: &MetricsSnapshot) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let rules = self.rules.read().await;
        let mut rule_states = self.rule_states.write().await;

        for rule in rules.iter() {
            // 获取或创建规则状态
            let state = rule_states
                .entry(rule.id.clone())
                .or_insert_with(RuleState::new);

            // 检查静默
            if self.is_silenced(&rule.name).await {
                continue;
            }

            // 检查规则
            let result = rule.check(metrics, state);

            match result {
                RuleCheckResult::ConditionStarted => {
                    state.start_condition();
                }
                RuleCheckResult::Triggered => {
                    let alert = rule.generate_alert(metrics);
                    alerts.push(alert);
                    state.end_condition();
                }
                RuleCheckResult::ConditionEnded => {
                    state.end_condition();
                }
                _ => {}
            }
        }

        alerts
    }

    /// 手动触发告警（用于测试）
    pub async fn trigger_alert(&self, rule_id: &str) -> Option<Alert> {
        let rules = self.rules.read().await;
        let rule = rules.iter().find(|r| r.id == rule_id)?;

        let metrics = MetricsSnapshot::new();
        let alert = rule.generate_alert(&metrics);
        Some(alert)
    }

    // ============ 告警发送 ============

    /// 发送告警
    pub async fn send_alert(
        &self,
        alert: Alert,
        rule_id: Option<String>,
        rule_name: Option<String>,
    ) -> AlertHistoryEntry {
        let mut entry = AlertHistoryEntry::new(alert.clone(), rule_id.clone(), rule_name.clone());

        // 检查静默
        if let Some(ref name) = rule_name {
            if self.is_silenced(name).await {
                entry.silenced = true;
                self.history.add(entry.clone()).await;
                return entry;
            }
        }

        let channels = self.channels.read().await;

        // 如果规则指定了通道，使用指定的通道；否则使用默认通道
        let channel_ids: Vec<String> = if let Some(ref rule) = rule_id {
            if let Some(rule) = self.get_rule(rule).await {
                rule.channels
                    .iter()
                    .map(|ct| ct.as_str().to_string())
                    .collect()
            } else {
                self.config
                    .default_channels
                    .iter()
                    .map(|ct| ct.as_str().to_string())
                    .collect()
            }
        } else {
            channels.keys().cloned().collect()
        };

        // 发送到所有通道
        for channel_id in channel_ids {
            if let Some(channel) = channels.get(&channel_id) {
                // 检查通道是否可用
                if !channel.is_available() {
                    entry.add_result(AlertSendResult::failure(
                        channel.channel_type(),
                        format!("Channel {} is not available", channel.name()),
                    ));
                    continue;
                }
                let result = channel.send(&alert).await;
                entry.add_result(result);
            } else if let Some(channel) = channels
                .values()
                .find(|c| c.channel_type().as_str() == channel_id)
            {
                // 检查通道是否可用
                if !channel.is_available() {
                    entry.add_result(AlertSendResult::failure(
                        channel.channel_type(),
                        format!("Channel {} is not available", channel.name()),
                    ));
                    continue;
                }
                let result = channel.send(&alert).await;
                entry.add_result(result);
            }
        }

        // 更新规则触发计数
        if let Some(ref rule_id) = rule_id {
            let mut rules = self.rules.write().await;
            if let Some(rule) = rules.iter_mut().find(|r| &r.id == rule_id) {
                rule.record_trigger();
            }
        }

        // 添加到历史
        self.history.add(entry.clone()).await;

        entry
    }

    /// 测试告警通道
    pub async fn test_channel(&self, channel_id: &str) -> AlertSendResult {
        let channels = self.channels.read().await;

        if let Some(channel) = channels.get(channel_id) {
            let test_alert = Alert::new(
                super::AlertLevel::Info,
                "测试告警",
                "这是一条测试告警消息，用于验证通道配置是否正确。",
            )
            .with_source("test");

            channel.send(&test_alert).await
        } else {
            AlertSendResult::failure(
                AlertChannelType::Email,
                format!("Channel {channel_id} not found"),
            )
        }
    }

    // ============ 历史查询 ============

    /// 获取告警历史
    pub async fn get_history(&self, id: &str) -> Option<AlertHistoryEntry> {
        self.history.get(id).await
    }

    /// 查询告警历史
    pub async fn query_history(&self, filter: &AlertHistoryFilter) -> Vec<AlertHistoryEntry> {
        self.history.query(filter).await
    }

    /// 获取告警统计
    pub async fn get_stats(
        &self,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> super::history::AlertHistoryStats {
        self.history.stats(start_time, end_time).await
    }

    /// 清理历史记录
    pub async fn cleanup_history(&self, before: DateTime<Utc>) -> usize {
        self.history.cleanup(before).await
    }

    /// 清理过期的静默规则
    pub async fn cleanup_silences(&self) -> usize {
        let mut silence_rules = self.silence_rules.write().await;
        let initial_len = silence_rules.len();
        silence_rules.retain(|s| s.is_active());
        initial_len - silence_rules.len()
    }
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alert::rules::AlertCondition;
    use crate::alert::AlertLevel;

    fn create_test_rule(name: &str) -> AlertRule {
        AlertRule::new(
            name,
            AlertCondition::ErrorRateAbove { threshold: 5.0 },
            AlertLevel::Warning,
            vec![AlertChannelType::Email],
        )
    }

    #[tokio::test]
    async fn test_add_and_get_rule() {
        let manager = AlertManager::with_defaults();
        let rule = create_test_rule("测试规则");
        let rule_id = rule.id.clone();

        manager.add_rule(rule).await;

        let retrieved = manager.get_rule(&rule_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "测试规则");
    }

    #[tokio::test]
    async fn test_update_and_delete_rule() {
        let manager = AlertManager::with_defaults();
        let rule = create_test_rule("原始规则");
        let rule_id = rule.id.clone();

        manager.add_rule(rule).await;

        // 更新
        let updated_rule = AlertRule::new(
            "更新后规则",
            AlertCondition::LatencyAbove { threshold_ms: 1000 },
            AlertLevel::Error,
            vec![AlertChannelType::Slack],
        );
        let updated_id = updated_rule.id.clone();

        manager.update_rule(&rule_id, updated_rule).await;

        let retrieved = manager.get_rule(&updated_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "更新后规则");

        // 删除
        let deleted = manager.delete_rule(&updated_id).await;
        assert!(deleted);

        let retrieved = manager.get_rule(&updated_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_enable_disable_rule() {
        let manager = AlertManager::with_defaults();
        let rule = create_test_rule("测试规则");
        let rule_id = rule.id.clone();

        manager.add_rule(rule).await;

        manager.disable_rule(&rule_id).await;
        let retrieved = manager.get_rule(&rule_id).await;
        assert!(!retrieved.unwrap().enabled);

        manager.enable_rule(&rule_id).await;
        let retrieved = manager.get_rule(&rule_id).await;
        assert!(retrieved.unwrap().enabled);
    }

    #[tokio::test]
    async fn test_silence_rules() {
        let manager = AlertManager::with_defaults();

        let silence = SilenceRule {
            id: "silence-1".to_string(),
            rule_pattern: "test-*".to_string(),
            start_time: Utc::now() - chrono::Duration::hours(1),
            end_time: Utc::now() + chrono::Duration::hours(1),
            reason: "测试静默".to_string(),
            created_by: Some("admin".to_string()),
        };

        manager.add_silence(silence).await;

        // 检查静默
        assert!(manager.is_silenced("test-alert").await);
        assert!(!manager.is_silenced("other-alert").await);

        // 移除静默
        manager.remove_silence("silence-1").await;
        assert!(!manager.is_silenced("test-alert").await);
    }

    #[tokio::test]
    async fn test_check_rules() {
        let manager = AlertManager::with_defaults();
        let rule = create_test_rule("错误率规则");
        manager.add_rule(rule).await;

        // 正常指标
        let metrics_ok = MetricsSnapshot::new().with_error_rate(3.0);
        let alerts = manager.check_rules(&metrics_ok).await;
        assert!(alerts.is_empty());

        // 触发指标
        let metrics_trigger = MetricsSnapshot::new().with_error_rate(10.0);
        let alerts = manager.check_rules(&metrics_trigger).await;
        assert_eq!(alerts.len(), 1);
        assert!(alerts[0].title.contains("错误率规则"));
    }

    #[tokio::test]
    async fn test_send_alert() {
        let manager = AlertManager::with_defaults();

        // 注册一个测试通道
        let _result = manager
            .register_channel(
                "test-email".to_string(),
                AlertChannelType::Webhook,
                serde_json::json!({
                    "url": "https://httpbin.org/post",
                    "method": "POST"
                }),
            )
            .await;

        // 即使通道注册失败，我们也测试告警发送逻辑
        let alert = Alert::new(AlertLevel::Warning, "测试告警", "测试消息");
        let entry = manager.send_alert(alert, None, None).await;

        // 验证历史记录已创建
        assert!(!entry.id.is_empty());
    }

    #[tokio::test]
    async fn test_history_stats() {
        let manager = AlertManager::with_defaults();

        // 添加一些规则和告警
        let rule = create_test_rule("测试规则");
        let rule_id = rule.id.clone();
        let rule_name = rule.name.clone();
        manager.add_rule(rule).await;

        // 发送一些告警
        for i in 0..3 {
            let alert = Alert::new(AlertLevel::Warning, format!("告警 {i}"), "测试");
            manager
                .send_alert(alert, Some(rule_id.clone()), Some(rule_name.clone()))
                .await;
        }

        let stats = manager.get_stats(None, None).await;
        assert_eq!(stats.total_count, 3);
    }
}
