//! 审计日志处理器 - Audit Log Handler

use axum::{
    Extension,
    Json,
    http::StatusCode,
    extract::{Query, Path},
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::gateway::SharedState;
use crate::service::{AuditService, AuditFilter};
use crate::service::user::Claims;
use crate::entity::audit_logs::SanitizedAuditLog;
use super::ApiError;

/// 分页查询参数
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

/// 审计日志查询参数
#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// 审计日志列表响应
#[derive(Debug, Serialize)]
pub struct AuditLogListResponse {
    pub object: String,
    pub data: Vec<SanitizedAuditLog>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub total_pages: u64,
}

/// 管理员查询审计日志
/// GET /api/v1/admin/audit-logs
pub async fn list_audit_logs(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<AuditLogListResponse>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let audit_service = AuditService::new(state.db.clone());
    
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50).min(100);

    // 构建过滤条件
    let filter = AuditFilter {
        user_id: None,
        action: query.action,
        resource_type: query.resource_type,
        start_time: query.start_time.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
        end_time: query.end_time.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
        page: Some(page),
        page_size: Some(page_size),
        ..Default::default()
    };

    // 查询日志
    let logs = audit_service.list(filter.clone())
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 统计总数
    let total = audit_service.count(filter)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 脱敏处理
    let sanitized: Vec<SanitizedAuditLog> = logs.into_iter().map(|l| l.sanitized()).collect();

    let total_pages = (total + page_size - 1) / page_size;

    Ok(Json(AuditLogListResponse {
        object: "list".to_string(),
        data: sanitized,
        total,
        page,
        page_size,
        total_pages,
    }))
}

/// 管理员查询指定用户的审计日志
/// GET /api/v1/admin/audit-logs/users/:user_id
pub async fn list_user_audit_logs(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<AuditLogListResponse>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let user_id = Uuid::parse_str(&user_id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let audit_service = AuditService::new(state.db.clone());
    
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50).min(100);

    // 查询用户日志
    let logs = audit_service.get_user_logs(user_id, page, page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 统计总数
    let filter = AuditFilter {
        user_id: Some(user_id),
        page: Some(page),
        page_size: Some(page_size),
        ..Default::default()
    };
    let total = audit_service.count(filter)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 脱敏处理
    let sanitized: Vec<SanitizedAuditLog> = logs.into_iter().map(|l| l.sanitized()).collect();

    let total_pages = (total + page_size - 1) / page_size;

    Ok(Json(AuditLogListResponse {
        object: "list".to_string(),
        data: sanitized,
        total,
        page,
        page_size,
        total_pages,
    }))
}

/// 管理员查询敏感操作日志
/// GET /api/v1/admin/audit-logs/sensitive
pub async fn list_sensitive_audit_logs(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<AuditLogListResponse>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let audit_service = AuditService::new(state.db.clone());
    
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(50).min(100);

    // 查询敏感操作日志
    let logs = audit_service.get_sensitive_logs(page, page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 脱敏处理
    let sanitized: Vec<SanitizedAuditLog> = logs.into_iter().map(|l| l.sanitized()).collect();

    let total = sanitized.len() as u64;
    let total_pages = (total + page_size - 1) / page_size;

    Ok(Json(AuditLogListResponse {
        object: "list".to_string(),
        data: sanitized,
        total,
        page,
        page_size,
        total_pages,
    }))
}

/// 用户查询自己的操作日志
/// GET /api/v1/users/me/audit-logs
pub async fn list_my_audit_logs(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<AuditLogListResponse>, ApiError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let audit_service = AuditService::new(state.db.clone());
    
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20).min(50);

    // 查询用户自己的日志
    let logs = audit_service.get_user_logs(user_id, page, page_size)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 统计总数
    let filter = AuditFilter {
        user_id: Some(user_id),
        page: Some(page),
        page_size: Some(page_size),
        ..Default::default()
    };
    let total = audit_service.count(filter)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 脱敏处理
    let sanitized: Vec<SanitizedAuditLog> = logs.into_iter().map(|l| l.sanitized()).collect();

    let total_pages = (total + page_size - 1) / page_size;

    Ok(Json(AuditLogListResponse {
        object: "list".to_string(),
        data: sanitized,
        total,
        page,
        page_size,
        total_pages,
    }))
}

/// 清理过期审计日志（管理员）
/// DELETE /api/v1/admin/audit-logs/cleanup
pub async fn cleanup_audit_logs(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let audit_service = AuditService::new(state.db.clone());

    // 清理 90 天前的日志
    let deleted = audit_service.cleanup_old_logs(90)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted_count": deleted,
        "message": format!("Deleted {} old audit logs", deleted)
    })))
}

/// 获取审计日志统计（管理员）
/// GET /api/v1/admin/audit-logs/stats
pub async fn get_audit_stats(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 验证管理员权限
    if claims.role != "admin" {
        return Err(ApiError(StatusCode::FORBIDDEN, "Admin only".into()));
    }

    let audit_service = AuditService::new(state.db.clone());

    // 获取各类型操作统计
    let total = audit_service.count(AuditFilter::default())
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let logins = audit_service.count(AuditFilter {
        action: Some("USER_LOGIN".to_string()),
        ..Default::default()
    })
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let api_requests = audit_service.count(AuditFilter {
        action: Some("API_REQUEST".to_string()),
        ..Default::default()
    })
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let admin_actions = audit_service.count(AuditFilter {
        action: Some("ADMIN_ACTION".to_string()),
        ..Default::default()
    })
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "total": total,
        "by_action": {
            "logins": logins,
            "api_requests": api_requests,
            "admin_actions": admin_actions,
        }
    })))
}
