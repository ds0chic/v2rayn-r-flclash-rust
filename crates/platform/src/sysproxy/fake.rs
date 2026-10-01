//! In-memory system proxy backend for tests. Never touches the real OS.

use std::sync::Mutex;

use super::{ProxyField, ProxyState, SystemProxyBackend};
use crate::error::Result;

/// A thread-safe, fully in-memory implementation of [`SystemProxyBackend`].
///
/// It also records every `set_field` write so tests can assert that `apply`
/// and `restore` only write the fields they own.
#[derive(Debug, Default)]
pub struct FakeSystemProxyBackend {
    state: Mutex<ProxyState>,
    writes: Mutex<Vec<(ProxyField, Option<String>)>>,
    notify_count: Mutex<u32>,
}

impl FakeSystemProxyBackend {
    /// Build a fake starting from an explicit state.
    pub fn new(initial: ProxyState) -> Self {
        Self {
            state: Mutex::new(initial),
            ..Self::default()
        }
    }

    /// Current state without going through the trait.
    pub fn state(&self) -> ProxyState {
        self.state
            .lock()
            .expect("fake proxy state poisoned")
            .clone()
    }

    /// All `(field, value)` writes recorded so far, in order.
    pub fn writes(&self) -> Vec<(ProxyField, Option<String>)> {
        self.writes
            .lock()
            .expect("fake proxy writes poisoned")
            .clone()
    }

    /// Number of change notifications observed.
    pub fn notify_count(&self) -> u32 {
        *self.notify_count.lock().expect("fake notify poisoned")
    }

    fn record(&self, field: ProxyField, value: Option<&str>) {
        self.writes
            .lock()
            .expect("fake proxy writes poisoned")
            .push((field, value.map(str::to_string)));
    }
}

impl SystemProxyBackend for FakeSystemProxyBackend {
    fn snapshot(&self) -> Result<ProxyState> {
        Ok(self.state())
    }

    fn set_field(&self, field: ProxyField, value: Option<&str>) -> Result<()> {
        self.record(field, value);
        let mut state = self.state.lock().expect("fake proxy state poisoned");
        state.set_field(field, value);
        Ok(())
    }

    fn notify_changed(&self) -> Result<()> {
        let mut count = self
            .notify_count
            .lock()
            .expect("fake proxy notify poisoned");
        *count += 1;
        Ok(())
    }
}
