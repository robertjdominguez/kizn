//! The one command surface. Tauri IPC and the dev bridge both hand a
//! [`Command`] to [`App::handle`] and send back its [`Reply`]; the UI's
//! types are generated from these (`just types`).

mod command;
mod config;
mod paths;

use std::sync::Arc;

use kizn_core::{Clock, Rng};
use kizn_store::Store;

pub use command::{AppError, AppInfo, Command, Reply, TodayView, TopicSummary};
pub use config::{CONFIG_FILE, Config};
pub use paths::Paths;

pub const DB_FILE: &str = "kizn.db";

pub struct App {
    paths: Paths,
    store: Store,
    #[allow(dead_code)] // first used when topics are created (phase 3)
    clock: Arc<dyn Clock>,
    #[allow(dead_code)]
    rng: Arc<dyn Rng>,
}

impl App {
    /// Opens (creating if needed) the data directory and its database.
    pub fn open(paths: Paths, clock: Arc<dyn Clock>, rng: Arc<dyn Rng>) -> Result<App, AppError> {
        std::fs::create_dir_all(&paths.data_dir).map_err(|e| {
            AppError::Io(format!("couldn't create the data directory {}: {e}", paths.data_dir.display()))
        })?;
        let store = Store::open(&paths.data_dir.join(DB_FILE))?;
        Ok(App { paths, store, clock, rng })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub async fn handle(&self, cmd: Command) -> Result<Reply, AppError> {
        match cmd {
            Command::GetToday => {
                let config = Config::load(&self.paths.config_dir)?;
                let topics =
                    self.store.topics()?.into_iter().map(|t| TopicSummary { id: t.id, title: t.title }).collect();
                Ok(Reply::Today(TodayView { topics, model_configured: config.model_configured() }))
            }
            Command::GetAppInfo => Ok(Reply::AppInfo(AppInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                data_dir: self.paths.data_dir.display().to_string(),
                config_dir: self.paths.config_dir.display().to_string(),
                schema_version: self.store.schema_version()?,
            })),
        }
    }
}
