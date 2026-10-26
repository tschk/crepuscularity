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
        lock_queue(&self.inbound).pop_front()
    }

    pub fn send_to_frontend(&self, message: FrontendMessage) {
        lock_queue(&self.inbound).push_back(message);
    }

    pub fn recv_for_frontend(&self) -> Option<FrontendMessage> {
        lock_queue(&self.outbound).pop_front()
    }
}
