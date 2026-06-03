use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::Mutex;

pub type SharedPeerObservability = Arc<Mutex<PeerObservability>>;

const DEFAULT_HISTORY_LIMIT: usize = 100;

#[derive(Debug)]
pub struct PeerObservability {
    total_relayed: u64,
    total_delivered: u64,
    error_count: u64,
    next_connection_id: u64,
    history_limit: usize,
    active_connections: HashMap<String, ActiveConnection>,
    connection_history: VecDeque<ConnectionHistory>,
}

#[derive(Debug)]
struct ActiveConnection {
    remote_addr: String,
    connected_at: u64,
    message_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub remote_addr: String,
    pub connected_at: u64,
    pub message_count: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionHistory {
    pub remote_addr: String,
    pub connected_at: u64,
    pub disconnected_at: u64,
    pub duration_seconds: u64,
    pub message_count: u64,
}

#[derive(Debug, Clone)]
pub struct PeerObservabilitySnapshot {
    pub total_relayed: u64,
    pub total_delivered: u64,
    pub error_count: u64,
    pub active: Vec<ConnectionInfo>,
    pub history: Vec<ConnectionHistory>,
}

impl Default for PeerObservability {
    fn default() -> Self {
        Self::new(DEFAULT_HISTORY_LIMIT)
    }
}

impl PeerObservability {
    pub fn new(history_limit: usize) -> Self {
        Self {
            total_relayed: 0,
            total_delivered: 0,
            error_count: 0,
            next_connection_id: 0,
            history_limit,
            active_connections: HashMap::new(),
            connection_history: VecDeque::new(),
        }
    }

    pub fn register_connection(&mut self, remote_addr: String) -> String {
        self.next_connection_id += 1;
        let id = format!("conn-{}", self.next_connection_id);

        self.active_connections.insert(
            id.clone(),
            ActiveConnection {
                remote_addr,
                connected_at: now_seconds(),
                message_count: 0,
            },
        );

        id
    }

    pub fn unregister_connection(&mut self, id: &str) {
        if let Some(connection) = self.active_connections.remove(id) {
            let disconnected_at = now_seconds();
            self.connection_history.push_back(ConnectionHistory {
                remote_addr: connection.remote_addr,
                connected_at: connection.connected_at,
                disconnected_at,
                duration_seconds: disconnected_at.saturating_sub(connection.connected_at),
                message_count: connection.message_count,
            });

            while self.connection_history.len() > self.history_limit {
                self.connection_history.pop_front();
            }
        }
    }

    pub fn record_connection_message(&mut self, id: &str) {
        if let Some(connection) = self.active_connections.get_mut(id) {
            connection.message_count += 1;
        }
    }

    pub fn record_relayed_message(&mut self) {
        self.total_relayed += 1;
    }

    pub fn record_delivered_messages(&mut self, delivered_count: usize) {
        self.total_delivered += delivered_count as u64;
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    pub fn snapshot(&self) -> PeerObservabilitySnapshot {
        let mut active = self
            .active_connections
            .iter()
            .map(|(id, connection)| ConnectionInfo {
                id: id.clone(),
                remote_addr: connection.remote_addr.clone(),
                connected_at: connection.connected_at,
                message_count: connection.message_count,
                status: "connected".to_string(),
            })
            .collect::<Vec<_>>();
        active.sort_by(|left, right| left.id.cmp(&right.id));

        PeerObservabilitySnapshot {
            total_relayed: self.total_relayed,
            total_delivered: self.total_delivered,
            error_count: self.error_count,
            active,
            history: self.connection_history.iter().cloned().collect(),
        }
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_active_connections_and_history() {
        let mut observability = PeerObservability::new(10);

        let connection_id = observability.register_connection("127.0.0.1:9200".to_string());
        observability.record_connection_message(&connection_id);
        observability.record_relayed_message();
        observability.record_delivered_messages(2);

        let active_snapshot = observability.snapshot();
        assert_eq!(active_snapshot.active.len(), 1);
        assert_eq!(active_snapshot.active[0].message_count, 1);
        assert_eq!(active_snapshot.total_relayed, 1);
        assert_eq!(active_snapshot.total_delivered, 2);

        observability.unregister_connection(&connection_id);

        let closed_snapshot = observability.snapshot();
        assert!(closed_snapshot.active.is_empty());
        assert_eq!(closed_snapshot.history.len(), 1);
        assert_eq!(closed_snapshot.history[0].remote_addr, "127.0.0.1:9200");
        assert_eq!(closed_snapshot.history[0].message_count, 1);
    }

    #[test]
    fn enforces_history_limit() {
        let mut observability = PeerObservability::new(1);

        let first = observability.register_connection("first".to_string());
        observability.unregister_connection(&first);

        let second = observability.register_connection("second".to_string());
        observability.unregister_connection(&second);

        let snapshot = observability.snapshot();
        assert_eq!(snapshot.history.len(), 1);
        assert_eq!(snapshot.history[0].remote_addr, "second");
    }
}
