//! The one command surface. Tauri IPC and the dev bridge both hand a
//! [`Command`] to [`App::handle`] and send back its [`Reply`], and both
//! forward the [`Event`]s from [`App::subscribe`]. The UI's types are
//! generated from these (`just types`).

mod bank;
mod command;
mod config;
mod events;
mod flags;
mod foundations;
mod ground;
mod lecture;
mod map;
mod paths;
mod practice;
mod secrets;
mod settings;
mod study;
mod topics;
mod tutor;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use kizn_core::{Clock, Rng, SeededRng, SystemClock};
use kizn_store::Store;
use tokio::sync::broadcast;

pub use command::{
    Anchor, AnswerView, AppError, AppInfo, BlockView, CardView, CheckView, Command, ConceptChange, ConceptLine,
    ConceptPageView, ConceptProgress, ConceptView, CueView, DayView, DeckView, DisputeView, DraftInput, DraftOutcome,
    Event, FlagView, GroundFoundation, GroundQuestion, GroundView, KeyView, LectureView, LinkedTopic, MapEdge, MapNode,
    MapView, MessageView, NextStep, NoteView, OptionFeedback, OutcomeProgress, OutcomeStanding, OutcomeView,
    PrefillView, ProgressView, ProviderView, ReadingItem, ReadingListView, Reply, ResourceView, Role, RolesView,
    SettingsView, SlotView, SourceView, StateCounts, SuggestionView, SummaryView, ThreadSummary, ThreadView, TodayView,
    TopicSummary, TopicView,
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
    /// The learner's offset from UTC: where their day starts and ends (the
    /// daily caps, the week on Today). UTC in scenarios.
    pub utc_offset: chrono::FixedOffset,
}

impl AppDeps {
    /// The real thing: system clock, OS randomness and keychain, real HTTP.
    pub fn system() -> AppDeps {
        AppDeps {
            clock: Arc::new(SystemClock),
            rng: Arc::new(SeededRng::from_os()),
            secrets: Arc::new(OsKeychain),
            http: Http::real(),
            utc_offset: *chrono::Local::now().offset(),
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
    /// Concepts whose sources are being looked for right now.
    sourcing: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Topics whose ground-check questions are being written.
    grounding: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Conversations the tutor is answering right now, and the provider
    /// answering each (the bank writer waits for it on the same one).
    tutoring: Arc<Mutex<HashMap<String, String>>>,
    /// Wakes the bank writer (the same one App starts).
    bank_wake: Arc<tokio::sync::Notify>,
    /// Concepts whose lectures are being written right now.
    lecturing: Arc<Mutex<std::collections::HashSet<String>>>,
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
        // Room for a tutor reply streaming word by word while the bank writer
        // reports; the UI re-reads a finished reply anyway.
        let (events, _) = broadcast::channel(1024);
        // Without a watcher kizn still works; Settings just won't follow
        // hand edits until it's reopened.
        let watcher = events::watch_config(&paths.config_dir, events.clone())
            .map_err(|e| eprintln!("kizn: not watching config.toml: {e}"))
            .ok();
        let bank_wake = Arc::new(tokio::sync::Notify::new());
        Ok(App {
            ctx: Ctx {
                paths,
                store: Arc::new(store),
                deps,
                events,
                sourcing: Arc::default(),
                tutoring: Arc::default(),
                grounding: Arc::default(),
                bank_wake: bank_wake.clone(),
                lecturing: Arc::default(),
            },
            drafting: Mutex::new(HashMap::new()),
            bank_wake,
            bank_started: AtomicBool::new(false),
            _watcher: watcher,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// Concept states and next recalls, read without starting background
    /// work (for `just inspect progress`).
    pub fn read_progress(&self, topic_id: &str) -> Result<ProgressView, AppError> {
        self.ctx.progress(topic_id)
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
                let next = self.next_step()?;
                let (date, week, counts) = self.week()?;
                Ok(Reply::Today(TodayView {
                    topics,
                    model_configured: config.model_configured(),
                    next,
                    date,
                    week,
                    counts,
                    flags: self.all_flags()?,
                }))
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
            Command::StartTopic { title, goal, foundation } => {
                self.start_topic(&title, &goal, foundation.as_deref()).map(Reply::Topic)
            }
            Command::DeleteTopic { topic_id } => self.delete_topic(&topic_id).map(|()| Reply::Done),
            Command::FoundationPrefill { suggestion_id } => self.foundation_prefill(&suggestion_id).map(Reply::Prefill),
            Command::StopDrafting { topic_id } => self.stop_drafting(&topic_id).map(Reply::Topic),
            Command::ResumeDrafting { topic_id } => self.resume_drafting(&topic_id).map(Reply::Topic),
            Command::SaveDraft { topic_id, draft } => self.save_draft(&topic_id, &draft).map(Reply::Topic),
            Command::ConfirmTopic { topic_id } => self.confirm_topic(&topic_id).map(Reply::Topic),
            Command::GetTopic { topic_id } => self.topic_view(&topic_id).map(Reply::Topic),
            Command::RetryBank { outcome_id } => self.retry_bank(&outcome_id).map(Reply::Topic),
            Command::StartPractice { topic_id } => self.start_practice(topic_id.as_deref()).map(Reply::Deck),
            Command::Answer { item_id, order, chosen } => {
                let reply = self.answer(&item_id, &order, chosen).map(Reply::Answered);
                // Used-up questions get refilled.
                self.wake_bank();
                reply
            }
            Command::GetSummary { attempt_ids } => self.summary(&attempt_ids).map(Reply::Summary),
            Command::GetProgress { topic_id } => self.progress(&topic_id).map(Reply::Progress),
            Command::GetConcept { concept_id } => self.open_concept(&concept_id).map(Reply::Concept),
            Command::FindSources { concept_id } => self.find_again(&concept_id).map(Reply::Concept),
            Command::MarkStudied { concept_id } => self.mark_studied(&concept_id).map(Reply::Concept),
            Command::LinkTopics { upper_id, lower_id, kind } => {
                self.link_topics(&upper_id, &lower_id, &kind).map(|_| Reply::Done)
            }
            Command::Unlink { link_id } => self.unlink(&link_id).map(|_| Reply::Done),
            Command::GetGround { topic_id } => self.ground_view(&topic_id).map(Reply::Ground),
            Command::AnswerGround { item_id, order, chosen } => {
                self.answer_and_prepare(&item_id, &order, chosen).map(Reply::Ground)
            }
            Command::ResolveFoundation { suggestion_id, action } => self.resolve_foundation(&suggestion_id, &action),
            Command::GetMap => self.map_view().map(Reply::Map),
            Command::ResolveFlag { for_topic, title, action } => {
                self.resolve_flag(&for_topic, &title, &action)?;
                self.topic_view(&for_topic).map(Reply::Topic)
            }
            Command::KeepFoundation { suggestion_id, keep } => {
                let topic_id = self.keep_foundation(&suggestion_id, keep)?;
                self.topic_view(&topic_id).map(Reply::Topic)
            }
            Command::GetReadingList { topic_id } => self.reading_list(topic_id.as_deref()).map(Reply::ReadingList),
            Command::GetResource { source_id } => self.resource(&source_id).await.map(Reply::Resource),
            Command::SaveNote { concept_id, text } => self.save_note(&concept_id, &text).map(Reply::Note),
            Command::AddSource { concept_id, url } => self.add_source(&concept_id, &url).await.map(Reply::Concept),
            Command::Ask { anchor, thread_id, text, passage } => {
                self.ask(&anchor, thread_id.as_deref(), &text, passage.as_deref()).map(Reply::Thread)
            }
            Command::GetThreads { anchor } => self.threads(&anchor).map(Reply::Threads),
            Command::GetThread { thread_id } => self.get_thread(&thread_id).map(Reply::Thread),
            Command::Dispute { attempt_id } => {
                let reply = self.dispute(&attempt_id).map(Reply::Disputed);
                // The slot is short now: write its replacement.
                self.wake_bank();
                reply
            }
        }
    }
}

impl App {
    /// Starts the bank writer if it isn't running yet. `KIZN_BANK_PAUSED=1`
    /// (`just dev mock paused`) holds it, so seeded states stay as seeded
    /// while the UI is reviewed.
    fn ensure_bank(&self) {
        if std::env::var("KIZN_BANK_PAUSED").is_ok_and(|v| v == "1") {
            return;
        }
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
