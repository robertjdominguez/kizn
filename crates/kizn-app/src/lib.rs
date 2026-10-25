//! The one command surface. Tauri IPC and the dev bridge both hand a
//! [`Command`] to [`App::handle`] and send back its [`Reply`], and both
//! forward the [`Event`]s from [`App::subscribe`]. The UI's types are
//! generated from these (`just types`).

mod command;
mod config;
mod events;
mod paths;
mod secrets;
mod settings;

use std::sync::Arc;

use kizn_core::{Clock, Rng, SeededRng, SystemClock};
use kizn_store::Store;
use tokio::sync::broadcast;

pub use command::{
    AppError, AppInfo, CheckView, Command, Event, KeyView, ProviderView, Reply, Role, RolesView, SettingsView,
    TodayView, TopicSummary,
};
pub use config::{CONFIG_FILE, Config};
pub use kizn_model::Http;
pub use paths::Paths;
pub use secrets::{MemorySecrets, OsKeychain, Secrets};

pub const DB_FILE: &str = "kizn.db";

/// What the app needs from the outside world, injected so tests can say
/// "3 days later", reproduce shuffles, keep keys in memory and send model
/// traffic to a fake server.
#[derive(Clone)]
pub struct AppDeps {
    pub clock: Arc<dyn Clock>,
    pub rng: Arc<dyn Rng>,
    pub secrets: Arc<dyn Secrets>,
    pub http: Http,
}

impl AppDeps {
    /// The real thing: system clock, OS randomness and keychain, real HTTP.
    pub fn system() -> AppDeps {
        AppDeps {
            clock: Arc::new(SystemClock),
            rng: Arc::new(SeededRng::from_os()),
            secrets: Arc::new(OsKeychain),
            http: Http::real(),
        }
    }
}

pub struct App {
    paths: Paths,
    store: Store,
    deps: AppDeps,
    events: broadcast::Sender<Event>,
    _watcher: Option<events::ConfigWatcher>,
}

impl App {
    /// Opens (creating if needed) the data directory and its database, and
    /// starts watching config.toml.
    pub fn open(paths: Paths, deps: AppDeps) -> Result<App, AppError> {
        for dir in [&paths.data_dir, &paths.config_dir] {
            std::fs::create_dir_all(dir)
                .map_err(|e| AppError::Io(format!("couldn't create the directory {}: {e}", dir.display())))?;
        }
        let store = Store::open(&paths.data_dir.join(DB_FILE))?;
        let (events, _) = broadcast::channel(64);
        // Without a watcher kizn still works; Settings just won't follow
        // hand edits until it's reopened.
        let watcher = events::watch_config(&paths.config_dir, events.clone())
            .map_err(|e| eprintln!("kizn: not watching config.toml: {e}"))
            .ok();
        Ok(App { paths, store, deps, events, _watcher: watcher })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
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
            Command::GetSettings => self.settings().map(Reply::Settings),
            Command::SaveProvider { name, protocol, base_url, model, key, preset } => self
                .save_provider(&name, &protocol, &base_url, &model, key.as_deref(), preset.as_deref())
                .map(Reply::Settings),
            Command::ListModels { protocol, base_url, key, name } => {
                self.list_models(&protocol, &base_url, key.as_deref(), name.as_deref()).await.map(Reply::Models)
            }
            Command::RemoveProvider { name } => self.remove_provider(&name).map(Reply::Settings),
            Command::TestProvider { name } => self.test_provider(&name).await.map(Reply::Settings),
            Command::AssignRole { role, provider } => self.assign_role(role, provider.as_deref()).map(Reply::Settings),
        }
    }
}
