use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// View model for settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsView {
    pub categories: Vec<SettingsCategory>,
    pub total_settings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsCategory {
    pub name: String,
    pub display_name: String,
    pub settings: Vec<SettingItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingItem {
    pub key: String,
    pub value: String,
    pub display_name: String,
    pub description: Option<String>,
    pub setting_type: SettingType,
    pub is_public: bool,
    pub is_editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettingType {
    String,
    Integer,
    Float,
    Boolean,
    Json,
    Enum(Vec<String>),
}

impl SettingsView {
    /// Create from settings map
    pub fn from_settings(settings: HashMap<String, String>) -> Self {
        let mut categories: HashMap<String, Vec<SettingItem>> = HashMap::new();

        for (key, value) in settings {
            let category = key.split('.').next().unwrap_or("general").to_string();
            let setting = SettingItem {
                key: key.clone(),
                value,
                display_name: key.split('.').last().unwrap_or(&key).to_string(),
                description: None,
                setting_type: SettingType::String,
                is_public: false,
                is_editable: true,
            };

            categories
                .entry(category)
                .or_insert_with(Vec::new)
                .push(setting);
        }

        let categories: Vec<SettingsCategory> = categories
            .into_iter()
            .map(|(name, settings)| SettingsCategory {
                display_name: name.to_uppercase(),
                name,
                settings,
            })
            .collect();

        let total_settings = categories.iter().map(|c| c.settings.len()).sum();

        Self {
            categories,
            total_settings,
        }
    }

    /// Filter to public settings only
    pub fn public_only(&self) -> Self {
        let categories: Vec<SettingsCategory> = self
            .categories
            .iter()
            .map(|c| SettingsCategory {
                name: c.name.clone(),
                display_name: c.display_name.clone(),
                settings: c.settings.iter().filter(|s| s.is_public).cloned().collect(),
            })
            .filter(|c| !c.settings.is_empty())
            .collect();

        let total_settings = categories.iter().map(|c| c.settings.len()).sum();

        Self {
            categories,
            total_settings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_view_creation() {
        let mut settings = HashMap::new();
        settings.insert("app.name".to_string(), "FoxNIO".to_string());
        settings.insert("app.version".to_string(), "1.0".to_string());

        let view = SettingsView::from_settings(settings);

        assert_eq!(view.total_settings, 2);
        assert!(!view.categories.is_empty());
    }
}
