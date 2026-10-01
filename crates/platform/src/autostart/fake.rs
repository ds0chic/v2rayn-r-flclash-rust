//! In-memory autostart registry for tests.

use std::collections::BTreeMap;
use std::sync::Mutex;

use super::AutoStartBackend;
use crate::error::Result;

/// A thread-safe fake of the Windows Run key.
#[derive(Debug, Default)]
pub struct FakeRegistry {
    values: Mutex<BTreeMap<String, String>>,
}

impl FakeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of all stored values, for assertions.
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.values.lock().expect("fake registry poisoned").clone()
    }
}

impl AutoStartBackend for FakeRegistry {
    fn query(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .values
            .lock()
            .expect("fake registry poisoned")
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, command: &str) -> Result<()> {
        self.values
            .lock()
            .expect("fake registry poisoned")
            .insert(name.to_string(), command.to_string());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<()> {
        self.values
            .lock()
            .expect("fake registry poisoned")
            .remove(name);
        Ok(())
    }
}
