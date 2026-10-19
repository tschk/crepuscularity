use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendMessage {
    pub channel: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Default, Clone)]
pub struct FrontendBridge {
    inbound: Arc<Mutex<VecDeque<FrontendMessage>>>,
    outbound: Arc<Mutex<VecDeque<FrontendMessage>>>,
}

fn lock_queue<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl FrontendBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send_to_runtime(&self, message: FrontendMessage) {
        lock_queue(&self.outbound).push_back(message);
    }

    pub fn recv_for_runtime(&self) -> Option<FrontendMessage> {
        lock_queue(&self.outbound).pop_front()
    }

    pub fn send_to_frontend(&self, message: FrontendMessage) {
        lock_queue(&self.inbound).push_back(message);
    }

    pub fn recv_for_frontend(&self) -> Option<FrontendMessage> {
        lock_queue(&self.inbound).pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(topic: &str) -> FrontendMessage {
        FrontendMessage {
            channel: "test".into(),
            topic: topic.into(),
            payload: Value::Null,
        }
    }

    #[test]
    fn messages_reach_only_the_named_destination() {
        let bridge = FrontendBridge::new();
        bridge.send_to_runtime(message("runtime"));
        assert!(bridge.recv_for_frontend().is_none());
        assert_eq!(bridge.recv_for_runtime().unwrap().topic, "runtime");
        assert!(bridge.recv_for_runtime().is_none());

        bridge.send_to_frontend(message("frontend"));
        assert!(bridge.recv_for_runtime().is_none());
        assert_eq!(bridge.recv_for_frontend().unwrap().topic, "frontend");
        assert!(bridge.recv_for_frontend().is_none());
    }

    #[test]
    fn cloned_bridges_preserve_independent_fifo_queues() {
        let bridge = FrontendBridge::new();
        let other = bridge.clone();
        bridge.send_to_runtime(message("runtime-1"));
        other.send_to_frontend(message("frontend-1"));
        other.send_to_runtime(message("runtime-2"));
        bridge.send_to_frontend(message("frontend-2"));

        assert_eq!(other.recv_for_runtime().unwrap().topic, "runtime-1");
        assert_eq!(bridge.recv_for_frontend().unwrap().topic, "frontend-1");
        assert_eq!(bridge.recv_for_runtime().unwrap().topic, "runtime-2");
        assert_eq!(other.recv_for_frontend().unwrap().topic, "frontend-2");
        assert!(other.recv_for_runtime().is_none());
        assert!(bridge.recv_for_frontend().is_none());
    }
}
