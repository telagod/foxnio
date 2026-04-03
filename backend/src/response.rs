//! 统一响应格式模块
//!
//! 提供标准化的 API 响应格式，确保前后端接口一致

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

/// 标准成功响应
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
}

/// 分页信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
}

impl Pagination {
    pub fn new(page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = if per_page > 0 {
            ((total as f64) / (per_page as f64)).ceil() as u32
        } else {
            1
        };
        Self {
            page,
            per_page,
            total,
            total_pages,
        }
    }
}

/// 标准错误响应
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// API 错误类型
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    /// 创建 Bad Request 错误
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }

    /// 创建未授权错误
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self(StatusCode::UNAUTHORIZED, msg.into())
    }

    /// 创建禁止访问错误
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self(StatusCode::FORBIDDEN, msg.into())
    }

    /// 创建未找到错误
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, msg.into())
    }

    /// 创建内部服务器错误
    pub fn internal(msg: impl Into<String>) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, msg.into())
    }

    /// 创建冲突错误
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self(StatusCode::CONFLICT, msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = ApiErrorResponse {
            error: self.1,
            code: None,
        };
        (self.0, Json(body)).into_response()
    }
}

// ============ 辅助函数 ============

/// 创建成功响应
pub fn json_success<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        data,
        pagination: None,
    })
}

/// 创建分页响应
pub fn json_paginated<T: Serialize>(data: T, pagination: Pagination) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        data,
        pagination: Some(pagination),
    })
}

/// 创建错误响应
pub fn json_error(
    status: StatusCode,
    msg: impl Into<String>,
) -> (StatusCode, Json<ApiErrorResponse>) {
    let code = match status {
        StatusCode::BAD_REQUEST => "BAD_REQUEST",
        StatusCode::UNAUTHORIZED => "UNAUTHORIZED",
        StatusCode::FORBIDDEN => "FORBIDDEN",
        StatusCode::NOT_FOUND => "NOT_FOUND",
        StatusCode::CONFLICT => "CONFLICT",
        StatusCode::INTERNAL_SERVER_ERROR => "INTERNAL_ERROR",
        _ => "ERROR",
    };
    (
        status,
        Json(ApiErrorResponse {
            error: msg.into(),
            code: Some(code.to_string()),
        }),
    )
}

/// 快捷创建分页
pub fn paginate(total: u64, page: u32, per_page: u32) -> Pagination {
    Pagination::new(page, per_page, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination() {
        let p = Pagination::new(1, 10, 95);
        assert_eq!(p.total_pages, 10);

        let p2 = Pagination::new(1, 10, 100);
        assert_eq!(p2.total_pages, 10);
    }
}
