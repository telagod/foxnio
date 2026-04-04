//! 数据备份管理 API Handler

use axum::{
    extract::{Extension, Path},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use super::ApiError;
use crate::gateway::middleware::permission::check_permission;
use crate::gateway::SharedState;
use crate::service::backup::{BackupFacade, ExportRequest, ImportRequest};
use crate::service::backup_service::BackupService;
use crate::service::permission::Permission;
use crate::service::user::Claims;

/// Build a [`BackupService`] from shared application state.
fn make_backup_svc(state: &SharedState) -> BackupService {
    let db_url = state.config.database_url();
    let backup_dir = std::env::var("FOXNIO_BACKUP_DIR").unwrap_or_else(|_| "./backups".into());
    BackupService::new(db_url, backup_dir)
}

/// 导出数据 (pg_dump)
pub async fn export_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ExportRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    let result = BackupFacade::export(&svc, body.tables)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "filename": result.filename,
        "metadata": result.metadata
    })))
}

/// 导入数据 (psql)
pub async fn import_data(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    let result = BackupFacade::import(&svc, &body.data)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "success": result.success,
        "message": result.message
    })))
}

/// 获取备份列表
pub async fn list_backups(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    let backups = svc
        .list_backups()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "object": "list",
        "data": backups
    })))
}

/// 创建备份
pub async fn create_backup(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    let record = svc
        .create_backup()
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "data": record
    })))
}

/// 下载备份文件
pub async fn download_backup(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    check_permission(&claims, Permission::BillingRead)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    let path = svc
        .get_backup_path(&filename)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let headers = [
        (header::CONTENT_TYPE, "application/gzip".to_string()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, bytes))
}

/// 删除备份文件
pub async fn delete_backup(
    Extension(state): Extension<SharedState>,
    Extension(claims): Extension<Claims>,
    Path(filename): Path<String>,
) -> Result<Json<Value>, ApiError> {
    check_permission(&claims, Permission::BillingWrite)
        .await
        .map_err(|e| ApiError(StatusCode::FORBIDDEN, e))?;

    let svc = make_backup_svc(&state);
    svc.delete_backup(&filename)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("backup {} deleted", filename)
    })))
}
