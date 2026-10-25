//! The one command surface. Tauri IPC and the dev bridge both hand a
//! [`Command`] to [`App::handle`] and send back its [`Reply`], and both
//! forward the [`Event`]s from [`App::subscribe`]. The UI's types are
//! generated from these (`just types`).

mod bank;
mod command;
mod config;
mod events;
mod paths;
mod secrets;
mod settings;
mod topics;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use kizn_core::{Clock, Rng, SeededRng, SystemClock};
use kizn_store::Store;
use tokio::sync::broadcast;

pub use command::{
    AppError, AppInfo, CheckView, Command, ConceptView, DraftInput, DraftOutcome, Event, KeyView, OutcomeView,
    ProviderView, Reply, Role, RolesView, SettingsView, TodayView, TopicSummary, TopicView,
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

/// What background jobs (drafting, bank writing) need: cheap to clone.
#[derive(Clone)]
pub struct Ctx {
    paths: Paths,
    store: Arc<Store>,
    deps: AppDeps,
    events: broadcast::Sender<Event>,
}

pub struct App {
    ctx: Ctx,
    /// Drafts being drafted, and the flag that stops each between calls.
    drafting: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Wakes the bank writer; it starts on first use (it needs the runtime).
    bank_wake: Arc<tokio::sync::Notify>,
    bank_started: AtomicBool,
    _watcher: Option<events::ConfigWatcher>,
}

impl std::ops::Deref for App {
    type Target = Ctx;
    fn deref(&self) -> &Ctx {
        &self.ctx
    }
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
        Ok(App {
            ctx: Ctx { paths, store: Arc::new(store), deps, events },
            drafting: Mutex::new(HashMap::new()),
            bank_wake: Arc::new(tokio::sync::Notify::new()),
            bank_started: AtomicBool::new(false),
            _watcher: watcher,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    pub async fn handle(&self, cmd: Command) -> Result<Reply, AppError> {
        // Picks up banks left mid-write when kizn last closed.
        self.ensure_bank();
        match cmd {
            Command::GetToday => {
                let config = Config::load(&self.paths.config_dir)?;
                let topics = self.topic_summaries()?;
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
            Command::StartTopic { title, goal } => self.start_topic(&title, &goal).map(Reply::Topic),
            Command::StopDrafting { topic_id } => self.stop_drafting(&topic_id).map(Reply::Topic),
            Command::SaveDraft { topic_id, draft } => self.save_draft(&topic_id, &draft).map(Reply::Topic),
            Command::ConfirmTopic { topic_id } => self.confirm_topic(&topic_id).map(Reply::Topic),
            Command::GetTopic { topic_id } => self.topic_view(&topic_id).map(Reply::Topic),
            Command::RetryBank { outcome_id } => self.retry_bank(&outcome_id).map(Reply::Topic),
        }
    }
}

impl App {
    /// Starts the bank writer if it isn't running yet.
    fn ensure_bank(&self) {
        if !self.bank_started.swap(true, std::sync::atomic::Ordering::SeqCst) {
            tokio::spawn(self.ctx.clone().bank_loop(self.bank_wake.clone()));
        }
    }

    /// Wakes the bank writer (starting it if needed).
    fn wake_bank(&self) {
        self.ensure_bank();
        self.bank_wake.notify_one();
    }

    fn retry_bank(&self, outcome_id: &str) -> Result<TopicView, AppError> {
        let o = self.store.outcome(outcome_id)?.ok_or_else(|| AppError::Refused("there's no such outcome".into()))?;
        self.store.set_bank_state(&o.id, "waiting", None, None)?;
        self.wake_bank();
        self.topic_view(&o.topic_id)
    }
}
