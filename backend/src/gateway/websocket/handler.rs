//! WebSocket 处理器
//!
//! 处理 WebSocket 连接、消息转发、心跳检测等

use super::pool::ConnectionPool;
use super::{WSConfig, WSError, WSMessage, WSProtocol};
use axum::{
    extract::{
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
        Extension, Query,
    },
    response::Response,
    Json,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::entity::usages;

/// WebSocket 处理器
pub struct WebSocketHandler {
    /// 连接池
    pool: ConnectionPool,
    /// 配置
    config: WSConfig,
    /// 数据库连接
    db: DatabaseConnection,
}

impl WebSocketHandler {
    /// 创建新的处理器
    pub fn new(pool: ConnectionPool, config: WSConfig, db: DatabaseConnection) -> Self {
        Self { pool, config, db }
    }

    /// 处理 WebSocket 升级请求
    pub async fn handle_upgrade(
        &self,
        protocol: WSProtocol,
        model: Option<String>,
        ws: WebSocketUpgrade,
    ) -> Response {
        let pool = self.pool.clone();
        let config = self.config.clone();
        let db = self.db.clone();

        ws.on_upgrade(move |socket| {
            Self::handle_connection(socket, pool, config, protocol, model, db)
        })
    }

    /// 处理 WebSocket 连接
    async fn handle_connection(
        mut socket: WebSocket,
        _pool: ConnectionPool,
        _config: WSConfig,
        protocol: WSProtocol,
        model: Option<String>,
        db: DatabaseConnection,
    ) {
        info!(
            protocol = ?protocol,
            model = ?model,
            "WebSocket connection established"
        );

        // 发送初始 session.created 事件
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_created = WSMessage::new(
            "session.created",
            serde_json::json!({
                "session": {
                    "id": session_id,
                    "model": model.unwrap_or_else(|| "gpt-4o-realtime-preview".to_string()),
                    "modalities": ["text", "audio"],
                    "instructions": "",
                    "voice": "alloy",
                    "input_format": "pcm16",
                    "output_format": "pcm16",
                    "input_transcription": null,
                    "turn_detection": null,
                    "tools": [],
                    "tool_choice": "auto",
                    "temperature": 0.8,
                    "max_response_output_tokens": "inf",
                }
            }),
        );

        if let Ok(msg) = session_created.to_json() {
            if socket.send(AxumMessage::Text(msg)).await.is_err() {
                error!("Failed to send session.created event");
                record_ws_failure(
                    &db,
                    None,
                    None,
                    "send_error",
                    "Failed to send session.created event",
                )
                .await;
                return;
            }
        }

        // 创建消息通道用于内部通信
        let (tx, mut rx) = mpsc::channel::<WSMessage>(100);

        // 心跳任务
        let heartbeat_tx = tx.clone();
        let session_id_heartbeat = session_id.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let ping = WSMessage::new(
                    "session.ping",
                    serde_json::json!({
                        "session_id": session_id_heartbeat
                    }),
                );
                if heartbeat_tx.send(ping).await.is_err() {
                    break;
                }
            }
        });

        // 消息处理循环
        loop {
            tokio::select! {
                // 接收客户端消息
                msg = socket.recv() => {
                    match msg {
                        Some(Ok(AxumMessage::Text(text))) => {
                            debug!(len = text.len(), "Received text message");
                            if let Err(e) = Self::handle_client_message(&tx, &text, &protocol).await {
                                error!(error = %e, "Failed to handle client message");
                            }
                        }
                        Some(Ok(AxumMessage::Binary(data))) => {
                            debug!(len = data.len(), "Received binary message");
                            // 处理二进制音频数据
                            if let Err(e) = Self::handle_binary_message(&tx, &data, &protocol).await {
                                error!(error = %e, "Failed to handle binary message");
                            }
                        }
                        Some(Ok(AxumMessage::Ping(data))) => {
                            debug!("Received ping");
                            let _ = socket.send(AxumMessage::Pong(data)).await;
                        }
                        Some(Ok(AxumMessage::Pong(_))) => {
                            debug!("Received pong");
                        }
                        Some(Ok(AxumMessage::Close(_))) => {
                            info!("Client requested close");
                            break;
                        }
                        Some(Err(e)) => {
                            error!(error = %e, "WebSocket error");
                            record_ws_failure(&db, None, None, "ws_error", &e.to_string()).await;
                            break;
                        }
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        }
                    }
                }

                // 发送消息给客户端
                msg = rx.recv() => {
                    if let Some(msg) = msg {
                        if let Ok(json) = msg.to_json() {
                            if socket.send(AxumMessage::Text(json)).await.is_err() {
                                error!("Failed to send message to client");
                                record_ws_failure(&db, None, None, "send_error", "Failed to send message to client").await;
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!(session_id = %session_id, "WebSocket connection closed");
    }

    /// 处理客户端文本消息
    async fn handle_client_message(
        tx: &mpsc::Sender<WSMessage>,
        text: &str,
        protocol: &WSProtocol,
    ) -> Result<(), WSError> {
        let msg = WSMessage::from_json(text)?;

        debug!(
            event_type = %msg.event_type,
            event_id = %msg.event_id,
            "Processing client message"
        );

        match msg.event_type.as_str() {
            "session.update" => {
                // 更新会话配置
                let response = WSMessage::new(
                    "session.updated",
                    serde_json::json!({
                        "session": msg.data.get("session").cloned().unwrap_or(serde_json::json!({}))
                    }),
                );
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            "response.create" => {
                // 创建响应
                Self::handle_response_create(tx, &msg, protocol).await?;
            }
            "response.cancel" => {
                // 取消响应
                let response = WSMessage::new(
                    "response.cancelled",
                    serde_json::json!({
                        "response_id": msg.data.get("response_id").cloned().unwrap_or(serde_json::Value::Null)
                    }),
                );
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            "input_audio_buffer.append" => {
                // 音频数据追加 - 实际实现需要转发到上游
                debug!("Audio buffer append received");
            }
            "input_audio_buffer.commit" => {
                // 提交音频缓冲区
                let response = WSMessage::new(
                    "input_audio_buffer.committed",
                    serde_json::json!({
                        "item_id": uuid::Uuid::new_v4().to_string()
                    }),
                );
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            "input_audio_buffer.clear" => {
                // 清空音频缓冲区
                let response = WSMessage::new("input_audio_buffer.cleared", serde_json::json!({}));
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            "conversation.item.create" => {
                // 创建会话项
                let response = WSMessage::new(
                    "conversation.item.created",
                    serde_json::json!({
                        "item": msg.data.get("item").cloned().unwrap_or(serde_json::json!({}))
                    }),
                );
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            "conversation.item.delete" => {
                // 删除会话项
                let response = WSMessage::new(
                    "conversation.item.deleted",
                    serde_json::json!({
                        "item_id": msg.data.get("item_id").cloned().unwrap_or(serde_json::Value::Null)
                    }),
                );
                tx.send(response)
                    .await
                    .map_err(|e| WSError::ConnectionError(e.to_string()))?;
            }
            _ => {
                debug!(event_type = %msg.event_type, "Unknown event type, forwarding as-is");
            }
        }

        Ok(())
    }

    /// 处理 response.create 请求
    async fn handle_response_create(
        tx: &mpsc::Sender<WSMessage>,
        _msg: &WSMessage,
        _protocol: &WSProtocol,
    ) -> Result<(), WSError> {
        let response_id = uuid::Uuid::new_v4().to_string();

        // 发送 response.created 事件
        let created = WSMessage::new(
            "response.created",
            serde_json::json!({
                "response": {
                    "id": response_id,
                    "object": "realtime.response",
                    "status": "created",
                    "status_details": null,
                    "output": [],
                    "usage": null
                }
            }),
        );
        tx.send(created)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.in_progress 事件
        let in_progress = WSMessage::new(
            "response.in_progress",
            serde_json::json!({
                "response": {
                    "id": response_id,
                    "status": "in_progress"
                }
            }),
        );
        tx.send(in_progress)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // NOTE: 实际转发请求到上游 API 并处理响应
        // 这里仅作为示例，发送一个简单的文本响应
        let item_id = uuid::Uuid::new_v4().to_string();

        // 发送 response.output_item.added
        let output_added = WSMessage::new(
            "response.output_item.added",
            serde_json::json!({
                "output_index": 0,
                "item": {
                    "id": item_id,
                    "object": "realtime.item",
                    "type": "message",
                    "status": "in_progress",
                    "role": "assistant",
                    "content": []
                }
            }),
        );
        tx.send(output_added)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.content_part.added
        let content_added = WSMessage::new(
            "response.content_part.added",
            serde_json::json!({
                "response_id": response_id,
                "item_id": item_id,
                "output_index": 0,
                "content_index": 0,
                "part": {
                    "type": "text",
                    "text": ""
                }
            }),
        );
        tx.send(content_added)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.text.delta (模拟)
        let text_delta = WSMessage::new(
            "response.text.delta",
            serde_json::json!({
                "response_id": response_id,
                "item_id": item_id,
                "output_index": 0,
                "content_index": 0,
                "delta": "Hello! This is a test response from FoxNIO WebSocket server."
            }),
        );
        tx.send(text_delta)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.text.done
        let text_done = WSMessage::new(
            "response.text.done",
            serde_json::json!({
                "response_id": response_id,
                "item_id": item_id,
                "output_index": 0,
                "content_index": 0,
                "text": "Hello! This is a test response from FoxNIO WebSocket server."
            }),
        );
        tx.send(text_done)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.content_part.done
        let content_done = WSMessage::new(
            "response.content_part.done",
            serde_json::json!({
                "response_id": response_id,
                "item_id": item_id,
                "output_index": 0,
                "content_index": 0,
                "part": {
                    "type": "text",
                    "text": "Hello! This is a test response from FoxNIO WebSocket server."
                }
            }),
        );
        tx.send(content_done)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.output_item.done
        let output_done = WSMessage::new(
            "response.output_item.done",
            serde_json::json!({
                "response_id": response_id,
                "item_id": item_id,
                "output_index": 0,
                "item": {
                    "id": item_id,
                    "object": "realtime.item",
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{
                        "type": "text",
                        "text": "Hello! This is a test response from FoxNIO WebSocket server."
                    }]
                }
            }),
        );
        tx.send(output_done)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.completed
        let completed = WSMessage::new(
            "response.completed",
            serde_json::json!({
                "response": {
                    "id": response_id,
                    "object": "realtime.response",
                    "status": "completed",
                    "status_details": {
                        "type": "completed"
                    },
                    "output": [{
                        "id": item_id,
                        "object": "realtime.item",
                        "type": "message",
                        "status": "completed",
                        "role": "assistant",
                        "content": [{
                            "type": "text",
                            "text": "Hello! This is a test response from FoxNIO WebSocket server."
                        }]
                    }],
                    "usage": {
                        "total_tokens": 100,
                        "input_tokens": 50,
                        "output_tokens": 50,
                        "input_token_details": {
                            "cached_tokens": 0,
                            "text_tokens": 50,
                            "audio_tokens": 0
                        },
                        "output_token_details": {
                            "text_tokens": 50,
                            "audio_tokens": 0
                        }
                    }
                }
            }),
        );
        tx.send(completed)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        // 发送 response.done
        let done = WSMessage::new(
            "response.done",
            serde_json::json!({
                "response_id": response_id
            }),
        );
        tx.send(done)
            .await
            .map_err(|e| WSError::ConnectionError(e.to_string()))?;

        Ok(())
    }

    /// 处理二进制消息（音频数据）
    async fn handle_binary_message(
        _tx: &mpsc::Sender<WSMessage>,
        data: &[u8],
        _protocol: &WSProtocol,
    ) -> Result<(), WSError> {
        // 音频数据处理 - 实际实现需要转发到上游
        debug!(len = data.len(), "Binary audio data received");
        Ok(())
    }
}

/// WebSocket 请求参数
#[derive(Debug, Deserialize)]
pub struct WSQueryParams {
    /// 模型名称
    pub model: Option<String>,
}

/// 创建 WebSocket 处理器工厂
pub fn create_handler(config: WSConfig, db: DatabaseConnection) -> WebSocketHandler {
    let pool = ConnectionPool::new(config.clone());
    WebSocketHandler::new(pool, config, db)
}

/// Axum 路由处理器 - V1 Realtime API
pub async fn ws_realtime_v1(
    Extension(handler): Extension<Arc<WebSocketHandler>>,
    Query(params): Query<WSQueryParams>,
    ws: WebSocketUpgrade,
) -> Response {
    handler
        .handle_upgrade(WSProtocol::OpenAIV1, params.model, ws)
        .await
}

/// Axum 路由处理器 - V2 Responses API
pub async fn ws_responses_v2(
    Extension(handler): Extension<Arc<WebSocketHandler>>,
    Query(params): Query<WSQueryParams>,
    ws: WebSocketUpgrade,
) -> Response {
    handler
        .handle_upgrade(WSProtocol::OpenAIV2, params.model, ws)
        .await
}

/// 获取连接池统计信息
pub async fn ws_pool_stats(
    Extension(handler): Extension<Arc<WebSocketHandler>>,
) -> Json<serde_json::Value> {
    let stats = handler.pool.stats();
    Json(serde_json::to_value(stats).unwrap_or(serde_json::json!({})))
}

/// 记录 WebSocket 请求失败的 usage
async fn record_ws_failure(
    db: &DatabaseConnection,
    user_id: Option<uuid::Uuid>,
    api_key_id: Option<uuid::Uuid>,
    error_type: &str,
    error_message: &str,
) {
    let usage = usages::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        user_id: Set(user_id.unwrap_or(uuid::Uuid::nil())),
        api_key_id: Set(api_key_id.unwrap_or(uuid::Uuid::nil())),
        account_id: Set(None),
        model: Set("websocket-realtime".to_string()),
        input_tokens: Set(0),
        output_tokens: Set(0),
        cost: Set(0),
        request_id: Set(None),
        success: Set(false),
        error_message: Set(Some(error_message.to_string())),
        metadata: Set(Some(serde_json::json!({
            "gateway": "websocket",
            "error_type": error_type,
        }))),
        created_at: Set(Utc::now()),
    };
    if let Err(e) = usage.insert(db).await {
        error!("Failed to record websocket failure usage: {}", e);
    }
}
