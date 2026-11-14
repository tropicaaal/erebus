//! What's known about running apps since the frontend started.

use std::collections::HashMap;

use erebus_ipc::{frontend::PeerId, protocol::LogRecord};
use gtk::glib;

use crate::logs;

/// A running process of a backend.
pub struct BackendInstance {
    pub peer: PeerId,
    pub pid: u32,
    pub backend: String,
    /// protocol version
    pub version: u32,
    pub connected_at: Option<glib::DateTime>,
}

struct AppRuntime {
    instances: Vec<BackendInstance>,
    logs: gtk::TextBuffer,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            logs: logs::new_buffer(),
        }
    }
}

#[derive(Default)]
pub struct Runtime {
    apps: HashMap<String, AppRuntime>,
    /// which app each connected peer belongs to
    peers: HashMap<PeerId, String>,
}

impl Runtime {
    pub fn connect(&mut self, process: String, instance: BackendInstance) {
        self.peers.insert(instance.peer, process.clone());
        self.apps
            .entry(process)
            .or_default()
            .instances
            .push(instance);
    }

    /// Returns the app the peer belonged to, if it was connected.
    pub fn disconnect(&mut self, peer: PeerId) -> Option<String> {
        let process = self.peers.remove(&peer)?;
        if let Some(app) = self.apps.get_mut(&process) {
            app.instances.retain(|instance| instance.peer != peer);
        }
        Some(process)
    }

    pub fn log(&self, peer: PeerId, record: &LogRecord) {
        let Some(app) = self
            .peers
            .get(&peer)
            .and_then(|process| self.apps.get(process))
        else {
            return;
        };

        if let Some(instance) = app.instances.iter().find(|instance| instance.peer == peer) {
            logs::append(&app.logs, instance.pid, record);
        }
    }

    /// Drops everything known about an app.
    pub fn forget(&mut self, process: &str) {
        self.apps.remove(process);
    }

    pub fn instances(&self, process: &str) -> &[BackendInstance] {
        self.apps.get(process).map_or(&[], |app| &app.instances)
    }

    /// Apps with at least one instance.
    pub fn running(&self) -> impl Iterator<Item = &String> {
        self.apps
            .iter()
            .filter(|(_, app)| !app.instances.is_empty())
            .map(|(process, _)| process)
    }

    pub fn logs(&mut self, process: &str) -> gtk::TextBuffer {
        self.apps
            .entry(process.to_owned())
            .or_default()
            .logs
            .clone()
    }
}
