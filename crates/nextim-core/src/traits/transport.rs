use crate::error::Result;

/// 传输层抽象 — 所有通信协议实现此 Trait
pub trait Transport: Send + Sync {
    /// 连接到远程地址
    fn connect(&mut self, addr: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 发送二进制数据（Protobuf 序列化后的字节）
    fn send(&self, data: &[u8]) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 接收二进制数据
    fn recv(&mut self) -> impl std::future::Future<Output = Result<Vec<u8>>> + Send;

    /// 关闭连接
    fn close(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 连接是否存活
    fn is_connected(&self) -> bool;
}

/// 传输层服务端 — Store/Peer 节点监听连接
pub trait TransportListener: Send + Sync + Sized {
    type Conn: Transport;

    /// 绑定并监听地址
    fn bind(addr: &str) -> impl std::future::Future<Output = Result<Self>> + Send;

    /// 接受一个新连接
    fn accept(&mut self) -> impl std::future::Future<Output = Result<Self::Conn>> + Send;
}
