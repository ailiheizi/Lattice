use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

use nextim_core::error::{NextImError, Result};
use nextim_core::traits::transport::Transport;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// WebSocket 客户端传输实现
pub struct WsTransport {
    stream: Option<Arc<Mutex<WsStream>>>,
    connected: AtomicBool,
}

impl WsTransport {
    pub fn new() -> Self {
        Self {
            stream: None,
            connected: AtomicBool::new(false),
        }
    }

    /// 从已有的 WebSocket 流创建（服务端 accept 后使用）
    pub fn from_stream(stream: WsStream) -> Self {
        Self {
            stream: Some(Arc::new(Mutex::new(stream))),
            connected: AtomicBool::new(true),
        }
    }
}

impl Default for WsTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for WsTransport {
    async fn connect(&mut self, addr: &str) -> Result<()> {
        let (ws_stream, _) = connect_async(addr)
            .await
            .map_err(|e| NextImError::Transport(format!("connect failed: {e}")))?;

        self.stream = Some(Arc::new(Mutex::new(ws_stream)));
        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, data: &[u8]) -> Result<()> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| NextImError::Transport("not connected".into()))?;

        let mut guard = stream.lock().await;
        guard
            .send(WsMessage::Binary(data.to_vec()))
            .await
            .map_err(|e| NextImError::Transport(format!("send failed: {e}")))?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<u8>> {
        let stream = self
            .stream
            .as_ref()
            .ok_or_else(|| NextImError::Transport("not connected".into()))?;

        let mut guard = stream.lock().await;
        loop {
            match guard.next().await {
                Some(Ok(WsMessage::Binary(data))) => return Ok(data.to_vec()),
                Some(Ok(WsMessage::Ping(payload))) => {
                    // 自动回复 Pong
                    let _ = guard.send(WsMessage::Pong(payload)).await;
                    continue;
                }
                Some(Ok(WsMessage::Close(_))) => {
                    self.connected.store(false, Ordering::SeqCst);
                    return Err(NextImError::Transport("connection closed".into()));
                }
                Some(Ok(_)) => continue, // 忽略 Text/Pong 等
                Some(Err(e)) => {
                    self.connected.store(false, Ordering::SeqCst);
                    return Err(NextImError::Transport(format!("recv error: {e}")));
                }
                None => {
                    self.connected.store(false, Ordering::SeqCst);
                    return Err(NextImError::Transport("stream ended".into()));
                }
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            let mut guard = stream.lock().await;
            let _ = guard.close(None).await;
        }
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

// === 服务端监听 ===

use nextim_core::traits::transport::TransportListener;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

/// WebSocket 服务端监听器
pub struct WsListener {
    listener: TcpListener,
}

impl TransportListener for WsListener {
    type Conn = WsTransport;

    async fn bind(addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| NextImError::Transport(format!("bind failed: {e}")))?;
        Ok(Self { listener })
    }

    async fn accept(&mut self) -> Result<Self::Conn> {
        let (tcp_stream, _addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| NextImError::Transport(format!("accept failed: {e}")))?;

        let ws_stream = accept_async(MaybeTlsStream::Plain(tcp_stream))
            .await
            .map_err(|e| NextImError::Transport(format!("ws handshake failed: {e}")))?;

        Ok(WsTransport::from_stream(ws_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_server_roundtrip() {
        // 启动服务端
        let mut server = WsListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.listener.local_addr().unwrap();
        let server_addr = format!("ws://127.0.0.1:{}", addr.port());

        let server_handle = tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            // 接收一条消息
            let data = conn.recv().await.unwrap();
            // 回传
            conn.send(&data).await.unwrap();
        });

        // 客户端连接
        let mut client = WsTransport::new();
        client.connect(&server_addr).await.unwrap();
        assert!(client.is_connected());

        // 发送
        let payload = b"hello nextim";
        client.send(payload).await.unwrap();

        // 接收回传
        let received = client.recv().await.unwrap();
        assert_eq!(received, payload);

        client.close().await.unwrap();
        assert!(!client.is_connected());

        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_multiple_messages() {
        let mut server = WsListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.listener.local_addr().unwrap();
        let server_addr = format!("ws://127.0.0.1:{}", addr.port());

        let server_handle = tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            for _ in 0..3 {
                let data = conn.recv().await.unwrap();
                conn.send(&data).await.unwrap();
            }
        });

        let mut client = WsTransport::new();
        client.connect(&server_addr).await.unwrap();

        for i in 0..3u8 {
            let msg = vec![i; 10];
            client.send(&msg).await.unwrap();
            let received = client.recv().await.unwrap();
            assert_eq!(received, msg);
        }

        client.close().await.unwrap();
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_protobuf_frame_roundtrip() {
        use nextim_proto::transport::{Frame, FrameType, Ping};
        use prost::Message;

        let mut server = WsListener::bind("127.0.0.1:0").await.unwrap();
        let addr = server.listener.local_addr().unwrap();
        let server_addr = format!("ws://127.0.0.1:{}", addr.port());

        let server_handle = tokio::spawn(async move {
            let mut conn = server.accept().await.unwrap();
            let data = conn.recv().await.unwrap();
            // 解码 Frame
            let frame = Frame::decode(data.as_slice()).unwrap();
            assert_eq!(frame.r#type, FrameType::Ping as i32);
            // 回传
            conn.send(&data).await.unwrap();
        });

        let mut client = WsTransport::new();
        client.connect(&server_addr).await.unwrap();

        // 构造 Protobuf Frame
        let frame = Frame {
            seq: 1,
            r#type: FrameType::Ping as i32,
            body: Some(nextim_proto::transport::frame::Body::Ping(Ping {
                timestamp: 1234567890,
            })),
        };
        let encoded = frame.encode_to_vec();

        client.send(&encoded).await.unwrap();
        let received = client.recv().await.unwrap();

        // 验证 roundtrip
        let decoded = Frame::decode(received.as_slice()).unwrap();
        assert_eq!(decoded.seq, 1);

        client.close().await.unwrap();
        let _ = server_handle.await;
    }
}
