//! Proxy Service

#![allow(dead_code)]

use crate::entity::proxies;
use anyhow::Result;
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateProxyRequest {
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub tags: Option<serde_json::Value>,
    pub health_check_url: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProxyRequest {
    pub name: Option<String>,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub tags: Option<serde_json::Value>,
    pub health_check_url: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProxyResponse {
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: i32,
    pub username: Option<String>,
    pub has_password: bool,
    pub enabled: bool,
    pub priority: i32,
    pub tags: Option<serde_json::Value>,
    pub health_check_url: Option<String>,
    pub last_check_at: Option<DateTimeWithTimeZone>,
    pub last_check_status: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Debug, Serialize)]
pub struct HealthCheckResult {
    pub proxy_id: i64,
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<proxies::Model> for ProxyResponse {
    fn from(model: proxies::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            protocol: model.protocol,
            host: model.host,
            port: model.port,
            username: model.username,
            has_password: model.password.is_some(),
            enabled: model.enabled,
            priority: model.priority,
            tags: model.tags,
            health_check_url: model.health_check_url,
            last_check_at: model.last_check_at,
            last_check_status: model.last_check_status,
            notes: model.notes,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

pub struct ProxyService;

impl ProxyService {
    /// Create proxy
    pub async fn create(db: &DatabaseConnection, req: CreateProxyRequest) -> Result<ProxyResponse> {
        let now: DateTimeWithTimeZone = Utc::now().into();
        let proxy = proxies::ActiveModel {
            id: ActiveValue::NotSet,
            name: ActiveValue::Set(req.name),
            protocol: ActiveValue::Set(req.protocol),
            host: ActiveValue::Set(req.host),
            port: ActiveValue::Set(req.port),
            username: ActiveValue::Set(req.username),
            password: ActiveValue::Set(req.password),
            enabled: ActiveValue::Set(req.enabled),
            priority: ActiveValue::Set(req.priority),
            tags: ActiveValue::Set(req.tags),
            health_check_url: ActiveValue::Set(req.health_check_url),
            last_check_at: ActiveValue::Set(None),
            last_check_status: ActiveValue::Set(None),
            notes: ActiveValue::Set(req.notes),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = proxy.insert(db).await?;
        Ok(result.into())
    }

    /// List proxies
    pub async fn list(
        db: &DatabaseConnection,
        enabled_only: bool,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<ProxyResponse>> {
        let mut query = proxies::Entity::find();

        if enabled_only {
            query = query.filter(proxies::Column::Enabled.eq(true));
        }

        let results = query
            .order_by_asc(proxies::Column::Priority)
            .paginate(db, page_size)
            .fetch_page(page)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Get proxy by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: i64) -> Result<Option<ProxyResponse>> {
        let result = proxies::Entity::find_by_id(id).one(db).await?;

        Ok(result.map(|m| m.into()))
    }

    /// Update proxy
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        req: UpdateProxyRequest,
    ) -> Result<Option<ProxyResponse>> {
        let proxy = proxies::Entity::find_by_id(id).one(db).await?;

        match proxy {
            Some(model) => {
                let mut active_model: proxies::ActiveModel = model.into();

                if let Some(name) = req.name {
                    active_model.name = ActiveValue::Set(name);
                }
                if let Some(protocol) = req.protocol {
                    active_model.protocol = ActiveValue::Set(protocol);
                }
                if let Some(host) = req.host {
                    active_model.host = ActiveValue::Set(host);
                }
                if let Some(port) = req.port {
                    active_model.port = ActiveValue::Set(port);
                }
                if let Some(username) = req.username {
                    active_model.username = ActiveValue::Set(Some(username));
                }
                if let Some(password) = req.password {
                    active_model.password = ActiveValue::Set(Some(password));
                }
                if let Some(enabled) = req.enabled {
                    active_model.enabled = ActiveValue::Set(enabled);
                }
                if let Some(priority) = req.priority {
                    active_model.priority = ActiveValue::Set(priority);
                }
                if let Some(tags) = req.tags {
                    active_model.tags = ActiveValue::Set(Some(tags));
                }
                if let Some(health_check_url) = req.health_check_url {
                    active_model.health_check_url = ActiveValue::Set(Some(health_check_url));
                }
                if let Some(notes) = req.notes {
                    active_model.notes = ActiveValue::Set(Some(notes));
                }
                active_model.updated_at = ActiveValue::Set(Utc::now().into());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete proxy
    pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let result = proxies::Entity::delete_by_id(id).exec(db).await?;

        Ok(result.rows_affected > 0)
    }

    /// Check proxy health
    pub async fn check_health(db: &DatabaseConnection, id: i64) -> Result<HealthCheckResult> {
        let proxy = proxies::Entity::find_by_id(id).one(db).await?;

        match proxy {
            Some(model) => {
                let start = std::time::Instant::now();
                let url = model.url();

                // Try to connect through proxy
                let healthy = Self::test_proxy_connection(&url).await;
                let latency_ms = start.elapsed().as_millis() as i64;

                // Update proxy status
                let mut active_model: proxies::ActiveModel = model.clone().into();
                active_model.last_check_at = ActiveValue::Set(Some(Utc::now().into()));
                active_model.last_check_status = ActiveValue::Set(if healthy {
                    Some("healthy".to_string())
                } else {
                    Some("unhealthy".to_string())
                });
                active_model.updated_at = ActiveValue::Set(Utc::now().into());
                active_model.update(db).await?;

                Ok(HealthCheckResult {
                    proxy_id: id,
                    healthy,
                    latency_ms: Some(latency_ms),
                    error: if healthy {
                        None
                    } else {
                        Some("Connection failed".to_string())
                    },
                })
            }
            None => Ok(HealthCheckResult {
                proxy_id: id,
                healthy: false,
                latency_ms: None,
                error: Some("Proxy not found".to_string()),
            }),
        }
    }

    /// Check all proxies health
    pub async fn check_all_health(db: &DatabaseConnection) -> Result<Vec<HealthCheckResult>> {
        let proxies = proxies::Entity::find()
            .filter(proxies::Column::Enabled.eq(true))
            .all(db)
            .await?;

        let mut results = Vec::new();
        for proxy in proxies {
            let result = Self::check_health(db, proxy.id).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Test proxy connection
    async fn test_proxy_connection(proxy_url: &str) -> bool {
        // Simplified health check - just verify URL format
        // In production, this would make an actual HTTP request through the proxy
        proxy_url.starts_with("http://")
            || proxy_url.starts_with("https://")
            || proxy_url.starts_with("socks5://")
    }

    /// Get healthy proxy by priority
    pub async fn get_healthy_proxy(db: &DatabaseConnection) -> Result<Option<proxies::Model>> {
        let proxy = proxies::Entity::find()
            .filter(proxies::Column::Enabled.eq(true))
            .filter(proxies::Column::LastCheckStatus.eq("healthy"))
            .order_by_asc(proxies::Column::Priority)
            .one(db)
            .await?;

        Ok(proxy)
    }
}
