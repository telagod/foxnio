//! 日志工具

use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// 初始化日志
pub fn init_logging() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();
}

/// 请求日志中间件
pub fn log_request(method: &str, path: &str, status: u16, duration_ms: u64) {
    tracing::info!(
        method = %method,
        path = %path,
        status = status,
        duration_ms = duration_ms,
        "Request processed"
    );
}

/// 错误日志
pub fn log_error(context: &str, error: &dyn std::error::Error) {
    tracing::error!(
        context = %context,
        error = %error,
        "Error occurred"
    );
}

/// 性能日志
pub fn log_performance(operation: &str, duration_ms: u64) {
    if duration_ms > 1000 {
        tracing::warn!(
            operation = %operation,
            duration_ms = duration_ms,
            "Slow operation detected"
        );
    } else {
        tracing::debug!(
            operation = %operation,
            duration_ms = duration_ms,
            "Operation completed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_init_logging() {
        // 不应该 panic
        init_logging();
    }
}
