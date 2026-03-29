//! Announcement Service

#![allow(dead_code)]

use crate::entity::{announcement_reads, announcements};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub notify_mode: String,
    pub targeting: Option<JsonValue>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnouncementRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub notify_mode: Option<String>,
    pub targeting: Option<JsonValue>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub updated_by: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AnnouncementResponse {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub status: String,
    pub notify_mode: String,
    pub targeting: Option<JsonValue>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_read: Option<bool>,
}

impl From<announcements::Model> for AnnouncementResponse {
    fn from(model: announcements::Model) -> Self {
        Self {
            id: model.id,
            title: model.title,
            content: model.content,
            status: model.status,
            notify_mode: model.notify_mode,
            targeting: model.targeting,
            starts_at: model.starts_at,
            ends_at: model.ends_at,
            created_by: model.created_by,
            updated_by: model.updated_by,
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_read: None,
        }
    }
}

pub struct AnnouncementService;

impl AnnouncementService {
    /// Create a new announcement
    pub async fn create(
        db: &DatabaseConnection,
        req: CreateAnnouncementRequest,
    ) -> Result<AnnouncementResponse> {
        let now = Utc::now();
        let announcement = announcements::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(req.title),
            content: ActiveValue::Set(req.content),
            status: ActiveValue::Set(req.status),
            notify_mode: ActiveValue::Set(req.notify_mode),
            targeting: ActiveValue::Set(req.targeting),
            starts_at: ActiveValue::Set(req.starts_at),
            ends_at: ActiveValue::Set(req.ends_at),
            created_by: ActiveValue::Set(req.created_by),
            updated_by: ActiveValue::Set(None),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
        };

        let result = announcement.insert(db).await?;
        Ok(result.into())
    }

    /// Get announcement by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<Option<AnnouncementResponse>> {
        let result = announcements::Entity::find_by_id(id).one(db).await?;

        Ok(result.map(|m| m.into()))
    }

    /// List all announcements
    pub async fn list(
        db: &DatabaseConnection,
        status: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<AnnouncementResponse>> {
        let mut query = announcements::Entity::find();

        if let Some(s) = status {
            query = query.filter(announcements::Column::Status.eq(s));
        }

        let results = query
            .order_by_desc(announcements::Column::CreatedAt)
            .paginate(db, page_size)
            .fetch_page(page)
            .await?;

        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    /// Update announcement
    pub async fn update(
        db: &DatabaseConnection,
        id: i64,
        req: UpdateAnnouncementRequest,
    ) -> Result<Option<AnnouncementResponse>> {
        let announcement = announcements::Entity::find_by_id(id).one(db).await?;

        match announcement {
            Some(model) => {
                let mut active_model: announcements::ActiveModel = model.into();

                if let Some(title) = req.title {
                    active_model.title = ActiveValue::Set(title);
                }
                if let Some(content) = req.content {
                    active_model.content = ActiveValue::Set(content);
                }
                if let Some(status) = req.status {
                    active_model.status = ActiveValue::Set(status);
                }
                if let Some(notify_mode) = req.notify_mode {
                    active_model.notify_mode = ActiveValue::Set(notify_mode);
                }
                if let Some(targeting) = req.targeting {
                    active_model.targeting = ActiveValue::Set(Some(targeting));
                }
                if let Some(starts_at) = req.starts_at {
                    active_model.starts_at = ActiveValue::Set(Some(starts_at));
                }
                if let Some(ends_at) = req.ends_at {
                    active_model.ends_at = ActiveValue::Set(Some(ends_at));
                }
                if let Some(updated_by) = req.updated_by {
                    active_model.updated_by = ActiveValue::Set(Some(updated_by));
                }
                active_model.updated_at = ActiveValue::Set(Utc::now());

                let result = active_model.update(db).await?;
                Ok(Some(result.into()))
            }
            None => Ok(None),
        }
    }

    /// Delete announcement
    pub async fn delete(db: &DatabaseConnection, id: i64) -> Result<bool> {
        let result = announcements::Entity::delete_by_id(id).exec(db).await?;

        Ok(result.rows_affected > 0)
    }

    /// Mark announcement as read
    pub async fn mark_as_read(
        db: &DatabaseConnection,
        announcement_id: i64,
        user_id: i64,
    ) -> Result<()> {
        let now = Utc::now();
        let read = announcement_reads::ActiveModel {
            id: ActiveValue::NotSet,
            announcement_id: ActiveValue::Set(announcement_id),
            user_id: ActiveValue::Set(user_id),
            read_at: ActiveValue::Set(now),
        };

        read.insert(db).await?;
        Ok(())
    }

    /// Get active announcements for user
    pub async fn get_active_for_user(
        db: &DatabaseConnection,
        user_id: i64,
    ) -> Result<Vec<AnnouncementResponse>> {
        let now = Utc::now();
        let announcements = announcements::Entity::find()
            .filter(announcements::Column::Status.eq("active"))
            .filter(
                Condition::any()
                    .add(announcements::Column::StartsAt.is_null())
                    .add(announcements::Column::StartsAt.lte(now)),
            )
            .filter(
                Condition::any()
                    .add(announcements::Column::EndsAt.is_null())
                    .add(announcements::Column::EndsAt.gte(now)),
            )
            .order_by_desc(announcements::Column::CreatedAt)
            .all(db)
            .await?;

        // Check which ones are read
        let announcement_ids: Vec<i64> = announcements.iter().map(|a| a.id).collect();
        let reads = announcement_reads::Entity::find()
            .filter(announcement_reads::Column::UserId.eq(user_id))
            .filter(announcement_reads::Column::AnnouncementId.is_in(announcement_ids))
            .all(db)
            .await?;

        let read_ids: std::collections::HashSet<i64> =
            reads.iter().map(|r| r.announcement_id).collect();

        let responses: Vec<AnnouncementResponse> = announcements
            .into_iter()
            .map(|a| {
                let mut response: AnnouncementResponse = a.into();
                response.is_read = Some(read_ids.contains(&response.id));
                response
            })
            .collect();

        Ok(responses)
    }
}
