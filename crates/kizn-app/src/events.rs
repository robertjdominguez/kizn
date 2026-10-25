//! Watches config.toml and broadcasts `Event::ConfigChanged`, so Settings
//! follows hand edits without a restart.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::{CONFIG_FILE, Event};

/// Keeps the watcher alive; dropping it stops watching.
pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

pub fn watch_config(config_dir: &Path, events: broadcast::Sender<Event>) -> notify::Result<ConfigWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res
            && ev.paths.iter().any(|p| p.file_name().is_some_and(|n| n == CONFIG_FILE))
        {
            let _ = tx.send(());
        }
    })?;
    watcher.watch(config_dir, RecursiveMode::NonRecursive)?;
    // Editors and atomic writes fire several events per save: one
    // ConfigChanged per burst.
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
            let _ = events.send(Event::ConfigChanged);
        }
    });
    Ok(ConfigWatcher { _watcher: watcher })
}
