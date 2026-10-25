//! The @domain layer: the feature files in `features/`, driven against
//! `kizn-app` in-process with a fake clock, a fixed seed, keys in memory, a
//! throwaway data directory, and model traffic sent to the fake model
//! server. Never a real model.
//!
//! `KIZN_BDD_TAG` (set by `just bdd <tag>`) narrows the run to scenarios that
//! also carry that tag.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use cucumber::{World, given, then, when};
use kizn_app::{
    App, AppDeps, AppError, AppInfo, CONFIG_FILE, Command, Event, MemorySecrets, Paths, ProviderView, Reply, Role,
    Secrets, SettingsView, TodayView,
};
use kizn_core::{FakeClock, SeededRng};
use kizn_model::fake::{Behaviour, FakeServer};
use tokio::sync::broadcast;

#[derive(World)]
#[world(init = Self::new)]
struct Kizn {
    fake: FakeServer,
    secrets: Arc<MemorySecrets>,
    dir: Option<tempfile::TempDir>,
    app: Option<App>,
    today: Option<TodayView>,
    about: Option<AppInfo>,
    /// The provider the scenario is working with.
    provider: Option<String>,
    settings: Option<SettingsView>,
    /// A closed port, for "nothing is listening".
    closed_port: Option<u16>,
    events: Option<broadcast::Receiver<Event>>,
}

impl std::fmt::Debug for Kizn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kizn").field("dir", &self.dir).field("provider", &self.provider).finish()
    }
}

impl Kizn {
    async fn new() -> Self {
        Kizn {
            fake: FakeServer::start(0).await.expect("the fake model server"),
            secrets: Arc::new(MemorySecrets::default()),
            dir: None,
            app: None,
            today: None,
            about: None,
            provider: None,
            settings: None,
            closed_port: None,
            events: None,
        }
    }

    async fn command(&mut self, cmd: Command) -> Result<Reply, AppError> {
        self.ensure_open();
        self.app().handle(cmd).await
    }

    async fn settings_from(&mut self, cmd: Command) -> SettingsView {
        match self.command(cmd).await {
            Ok(Reply::Settings(v)) => {
                self.settings = Some(v.clone());
                v
            }
            other => panic!("expected Settings, got {other:?}"),
        }
    }

    async fn add_provider(&mut self, name: &str, protocol: &str, base_url: &str, model: &str, key: Option<&str>) {
        self.settings_from(Command::SaveProvider {
            name: name.into(),
            protocol: protocol.into(),
            base_url: base_url.into(),
            model: model.into(),
            key: key.map(str::to_string),
            preset: None,
        })
        .await;
        self.provider = Some(name.into());
    }

    fn current(&self) -> &ProviderView {
        let name = self.provider.as_deref().expect("no provider: add one first");
        let view = self.settings.as_ref().expect("Settings hasn't been read");
        view.providers.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no provider {name} in Settings"))
    }

    fn current_check(&self) -> &kizn_app::CheckView {
        self.current().check.as_ref().expect("the provider hasn't been tested")
    }

    async fn assign(&mut self, role: Role) -> Result<Reply, AppError> {
        let provider = self.provider.clone();
        self.command(Command::AssignRole { role, provider }).await
    }

    fn dir(&self) -> &std::path::Path {
        self.dir.as_ref().expect("no data directory: start with \"Given a fresh data directory\"").path()
    }

    /// Opens kizn on this scenario's data directory (a fresh one if none was
    /// given yet). Scenarios that start mid-journey ("Given no model is
    /// configured") get an open app without saying "When I open kizn".
    fn ensure_open(&mut self) -> &App {
        if self.dir.is_none() {
            self.dir = Some(tempfile::tempdir().unwrap());
        }
        if self.app.is_none() {
            let deps = AppDeps {
                clock: Arc::new(FakeClock::at(Utc.with_ymd_and_hms(2026, 10, 25, 9, 0, 0).unwrap())),
                rng: Arc::new(SeededRng::from_seed(1)),
                secrets: self.secrets.clone(),
                http: self.fake.client(),
            };
            self.app = Some(App::open(Paths::in_dir(self.dir()), deps).unwrap());
        }
        self.app.as_ref().unwrap()
    }

    fn app(&self) -> &App {
        self.app.as_ref().expect("kizn isn't open: \"When I open kizn\" first")
    }

    fn about(&self) -> &AppInfo {
        self.about.as_ref().expect("About isn't open: \"And I open About\" first")
    }

    fn today(&self) -> &TodayView {
        self.today.as_ref().expect("Today hasn't been opened")
    }
}

#[given("a fresh data directory")]
fn fresh_data_directory(w: &mut Kizn) {
    w.dir = Some(tempfile::tempdir().unwrap());
}

#[when("I open kizn")]
async fn open_kizn(w: &mut Kizn) {
    match w.ensure_open().handle(Command::GetToday).await.unwrap() {
        Reply::Today(t) => w.today = Some(t),
        other => panic!("GetToday replied {other:?}"),
    }
}

#[when("I open About")]
async fn open_about(w: &mut Kizn) {
    match w.app().handle(Command::GetAppInfo).await.unwrap() {
        Reply::AppInfo(i) => w.about = Some(i),
        other => panic!("GetAppInfo replied {other:?}"),
    }
}

#[then("I see the version of kizn I'm running")]
fn see_version(w: &mut Kizn) {
    // kizn-bdd shares the workspace version with kizn-app.
    assert_eq!(w.about().version, env!("CARGO_PKG_VERSION"));
}

#[then("I see where my data is kept")]
fn see_data_dir(w: &mut Kizn) {
    assert_eq!(std::path::Path::new(&w.about().data_dir), w.dir());
}

// The UI owns the words; here each phrase a scenario can "see" maps to the
// state of the view that makes the UI show it.
#[then(expr = "I see {string}")]
fn i_see(w: &mut Kizn, text: String) {
    match text.as_str() {
        "Start a topic" => assert!(w.today().topics.is_empty(), "Today lists topics, so its empty state isn't shown"),
        "works" => assert_eq!(w.current_check().summary, "works"),
        other => panic!("no @domain meaning for seeing {other:?}: add it to the I see step"),
    }
}

#[then("no model is configured")]
fn no_model_configured(w: &mut Kizn) {
    assert!(!w.today().model_configured, "Today says a model is configured");
    assert!(!w.dir().join(CONFIG_FILE).exists(), "first run wrote a config.toml");
}

// ---- connect a model -----------------------------------------------------------

#[given("no model is configured")]
async fn given_no_model(w: &mut Kizn) {
    match w.command(Command::GetToday).await.unwrap() {
        Reply::Today(t) => assert!(!t.model_configured, "a model is already configured"),
        other => panic!("GetToday replied {other:?}"),
    }
}

#[when(expr = "I add a provider {string} with protocol {string} at {string} and model {string}")]
async fn add_provider_at(w: &mut Kizn, name: String, protocol: String, url: String, model: String) {
    w.add_provider(&name, &protocol, &url, &model, None).await;
}

#[when(expr = "I add a provider {string} with protocol {string} and a key")]
async fn add_provider_with_key(w: &mut Kizn, name: String, protocol: String) {
    w.add_provider(&name, &protocol, "", "claude-sonnet-5", Some(TEST_KEY)).await;
}

const TEST_KEY: &str = "sk-kizn-test-0001";

#[given("a provider whose replies are not valid JSON")]
async fn provider_no_json(w: &mut Kizn) {
    let url = w.fake.openai_url(Behaviour::NoJson);
    w.add_provider("chatty", "openai", &url, "chatty-7b", None).await;
}

#[given("a provider at an address where nothing is listening")]
async fn provider_nowhere(w: &mut Kizn) {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    w.closed_port = Some(port);
    w.add_provider("nowhere", "openai", &format!("http://127.0.0.1:{port}/v1"), "m", None).await;
}

#[given("a provider that ignores JSON schemas but answers in JSON when asked")]
async fn provider_prompted(w: &mut Kizn) {
    let url = w.fake.openai_url(Behaviour::PromptedJson);
    w.add_provider("parsed", "openai", &url, "m", None).await;
}

#[when("I test the connection")]
async fn test_connection(w: &mut Kizn) {
    let name = w.provider.clone().expect("no provider to test");
    w.settings_from(Command::TestProvider { name }).await;
}

#[then(expr = "all three roles use {string}")]
fn all_roles_use(w: &mut Kizn, name: String) {
    let r = &w.settings.as_ref().unwrap().roles;
    for (role, got) in [("author", &r.author), ("checker", &r.checker), ("tutor", &r.tutor)] {
        assert_eq!(got.as_deref(), Some(name.as_str()), "the {role} role");
    }
}

#[then("I see that it can tutor but can't author items")]
fn can_tutor_not_author(w: &mut Kizn) {
    let c = w.current_check();
    assert_eq!(c.roles, vec![Role::Tutor]);
    assert!(c.summary.contains("it can tutor, not author items"), "{}", c.summary);
}

#[then("I can't assign it the author role")]
async fn cant_assign_author(w: &mut Kizn) {
    let r = w.assign(Role::Author).await;
    assert!(matches!(r, Err(AppError::Refused(_))), "assigning author gave {r:?}");
}

#[then("config.toml references the key by name only")]
fn key_by_name(w: &mut Kizn) {
    let text = std::fs::read_to_string(w.dir().join(CONFIG_FILE)).unwrap();
    assert!(text.contains("key = \"keychain:kizn/claude\""), "{text}");
    assert!(!text.contains(TEST_KEY), "the key itself is in config.toml:\n{text}");
    assert_eq!(w.secrets.get("claude").unwrap().as_deref(), Some(TEST_KEY), "the key isn't in the keychain");
}

#[then("I see that nothing answered at that address")]
fn nothing_answered(w: &mut Kizn) {
    let port = w.closed_port.unwrap();
    assert_eq!(w.current_check().summary, format!("nothing answered at http://127.0.0.1:{port}"));
}

#[then("no role can be assigned to it")]
async fn no_role(w: &mut Kizn) {
    assert!(w.current_check().roles.is_empty());
    for role in [Role::Author, Role::Checker, Role::Tutor] {
        let r = w.assign(role).await;
        assert!(matches!(r, Err(AppError::Refused(_))), "assigning {role:?} gave {r:?}");
    }
}

#[then("I see that it works and kizn will parse its replies")]
fn works_parsed(w: &mut Kizn) {
    let c = w.current_check();
    assert_eq!(c.structured, "prompted");
    assert!(c.summary.starts_with("works") && c.summary.contains("kizn will parse"), "{}", c.summary);
}

#[then("I can assign it the author role")]
async fn can_assign_author(w: &mut Kizn) {
    match w.assign(Role::Author).await {
        Ok(Reply::Settings(v)) => assert_eq!(v.roles.author, w.provider),
        other => panic!("assigning author gave {other:?}"),
    }
}

#[given("kizn is open on Settings")]
async fn open_on_settings(w: &mut Kizn) {
    w.ensure_open();
    w.events = Some(w.app().subscribe());
    w.settings_from(Command::GetSettings).await;
}

#[when("I add a provider to config.toml by hand")]
fn hand_edit(w: &mut Kizn) {
    let text = "# added by hand\n[providers.handmade]\nprotocol = \"openai\"\nbase_url = \"http://gpu:11434/v1\"\nmodel = \"qwen3:32b\"\n";
    std::fs::write(w.dir().join(CONFIG_FILE), text).unwrap();
}

#[then("Settings shows it without a restart")]
async fn shows_without_restart(w: &mut Kizn) {
    let rx = w.events.as_mut().expect("not subscribed: \"Given kizn is open on Settings\" first");
    let ev = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
    assert!(matches!(ev, Ok(Ok(Event::ConfigChanged))), "no ConfigChanged event within 5s: {ev:?}");
    let v = w.settings_from(Command::GetSettings).await;
    assert!(v.providers.iter().any(|p| p.name == "handmade"), "Settings doesn't list the hand-added provider");
}

#[tokio::main]
async fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let extra = std::env::var("KIZN_BDD_TAG").ok().filter(|t| !t.is_empty());
    let extra = extra.map(|t| t.trim_start_matches('@').to_string());
    Kizn::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit(features, move |feature, rule, scenario| {
            let tags: Vec<&String> =
                feature.tags.iter().chain(rule.map(|r| &r.tags).into_iter().flatten()).chain(&scenario.tags).collect();
            let has = |t: &str| tags.iter().any(|x| x.as_str() == t);
            has("domain") && extra.as_deref().is_none_or(has)
        })
        .await;
}
