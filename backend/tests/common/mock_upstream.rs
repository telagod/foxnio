//! Mock 上游服务器
//!
//! 用于测试网关代理功能，模拟上游 API 响应

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock 服务器状态
#[derive(Debug, Clone)]
pub struct MockServerState {
    pub should_fail: bool,
    pub delay_ms: u64,
    pub response_status: StatusCode,
    pub response_body: serde_json::Value,
}

impl Default for MockServerState {
    fn default() -> Self {
        Self {
            should_fail: false,
            delay_ms: 0,
            response_status: StatusCode::OK,
            response_body: json!({"status": "ok"}),
        }
    }
}

/// Mock 上游服务器
pub struct MockUpstream {
    addr: SocketAddr,
    state: Arc<RwLock<MockServerState>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl MockUpstream {
    /// 创建新的 Mock 服务器
    pub fn new(port: u16) -> Self {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        Self {
            addr,
            state: Arc::new(RwLock::new(MockServerState::default())),
            shutdown_tx: None,
        }
    }

    /// 启动服务器
    pub async fn start(&mut self) {
        let state = self.state.clone();
        
        let app = Router::new()
            .route("/v1/chat/completions", post(Self::handle_chat_completions))
            .route("/v1/models", get(Self::handle_list_models))
            .route("/health", get(Self::handle_health))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .expect("Failed to bind mock server");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .expect("Mock server error");
        });

        // 等待服务器启动
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    /// 停止服务器
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// 设置失败模式
    pub async fn set_should_fail(&self, should_fail: bool) {
        let mut state = self.state.write().await;
        state.should_fail = should_fail;
        if should_fail {
            state.response_status = StatusCode::INTERNAL_SERVER_ERROR;
            state.response_body = json!({"error": "Internal server error"});
        }
    }

    /// 设置延迟
    pub async fn set_delay(&self, delay_ms: u64) {
        let mut state = self.state.write().await;
        state.delay_ms = delay_ms;
    }

    /// 设置响应
    pub async fn set_response(&self, status: StatusCode, body: serde_json::Value) {
        let mut state = self.state.write().await;
        state.response_status = status;
        state.response_body = body;
    }

    /// 获取服务器地址
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// 获取服务器 URL
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    // 处理函数

    async fn handle_chat_completions(
        axum::extract::State(state): axum::extract::State<Arc<RwLock<MockServerState>>>,
        _req: Request<Body>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let state = state.read().await;
        
        if state.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(state.delay_ms)).await;
        }

        if state.should_fail {
            return Err(state.response_status);
        }

        Ok(Json(json!({
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "This is a mock response"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        })))
    }

    async fn handle_list_models(
        axum::extract::State(state): axum::extract::State<Arc<RwLock<MockServerState>>>,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        let state = state.read().await;
        
        if state.should_fail {
            return Err(state.response_status);
        }

        Ok(Json(json!({
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model", "owned_by": "openai"},
                {"id": "gpt-3.5-turbo", "object": "model", "owned_by": "openai"}
            ]
        })))
    }

    async fn handle_health() -> Json<serde_json::Value> {
        Json(json!({"status": "healthy"}))
    }
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        // 尝试关闭服务器
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_server_start_stop() {
        let mut server = MockUpstream::new(18080);
        server.start().await;
        
        // 测试健康检查
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("{}/health", server.url()))
            .send()
            .await
            .unwrap();
        
        assert_eq!(resp.status(), StatusCode::OK);
        
        server.stop().await;
    }

    #[tokio::test]
    async fn test_mock_server_chat_completions() {
        let mut server = MockUpstream::new(18081);
        server.start().await;
        
        let client = reqwest::Client::new();
        let resp = client
            .post(&format!("{}/v1/chat/completions", server.url()))
            .json(&json!({
                "model": "gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }))
            .send()
            .await
            .unwrap();
        
        assert_eq!(resp.status(), StatusCode::OK);
        
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
        
        server.stop().await;
    }

    #[tokio::test]
    async fn test_mock_server_failure_mode() {
        let mut server = MockUpstream::new(18082);
        server.start().await;
        server.set_should_fail(true).await;
        
        let client = reqwest::Client::new();
        let resp = client
            .post(&format!("{}/v1/chat/completions", server.url()))
            .json(&json!({"model": "gpt-4", "messages": []}))
            .send()
            .await
            .unwrap();
        
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        
        server.stop().await;
    }
}
