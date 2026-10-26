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
    App, AppDeps, AppError, AppInfo, CONFIG_FILE, Command, DraftInput, DraftOutcome, Event, MemorySecrets, Paths,
    ProviderView, Reply, Role, Secrets, SettingsView, TodayView, TopicView,
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
    topic: Option<TopicView>,
    goal: Option<String>,
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
            topic: None,
            goal: None,
        }
    }

    async fn topic_from(&mut self, cmd: Command) -> TopicView {
        match self.command(cmd).await {
            Ok(Reply::Topic(t)) => {
                self.topic = Some(t.clone());
                t
            }
            other => panic!("expected a Topic, got {other:?}"),
        }
    }

    fn topic_id(&self) -> String {
        self.topic.as_ref().expect("no topic: start one first").id.clone()
    }

    /// Re-reads the topic until `done` holds (background jobs move it on),
    /// for up to 10 seconds.
    async fn wait_for(&mut self, what: &str, done: impl Fn(&TopicView) -> bool) -> TopicView {
        let topic_id = self.topic_id();
        for _ in 0..1500 {
            let t = self.topic_from(Command::GetTopic { topic_id: topic_id.clone() }).await;
            if done(&t) {
                return t;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("waited 30s for {what}: {:#?}", self.topic);
    }

    /// The scenario's database, read directly: refused items aren't in any view.
    fn store(&self) -> kizn_store::Store {
        kizn_store::Store::open(&self.dir().join(kizn_app::DB_FILE)).unwrap()
    }

    /// A provider with a fake behaviour, given a role. Roles normally need a
    /// passing test; `by_hand` writes the role into config.toml, as a learner
    /// could, for providers that can't pass one.
    async fn use_for(&mut self, role: Role, name: &str, behaviour: Behaviour, by_hand: bool) {
        let url = self.fake.openai_url(behaviour);
        self.add_provider(name, "openai", &url, "m", None).await;
        if by_hand {
            let path = self.dir().join(CONFIG_FILE);
            let text = std::fs::read_to_string(&path).unwrap();
            let line = format!("{} = \"local\"", role_key(role));
            assert!(text.contains(&line), "{line} isn't in config.toml:\n{text}");
            std::fs::write(&path, text.replace(&line, &format!("{} = \"{name}\"", role_key(role)))).unwrap();
        } else {
            self.settings_from(Command::TestProvider { name: name.into() }).await;
            let r = self.command(Command::AssignRole { role, provider: Some(name.into()) }).await;
            assert!(r.is_ok(), "assigning {role:?} to {name}: {r:?}");
        }
    }

    async fn confirmed_topic(&mut self) {
        drafted_topic(self).await;
        let topic_id = self.topic_id();
        self.topic_from(Command::ConfirmTopic { topic_id }).await;
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

// ---- start a topic -------------------------------------------------------------

#[given("a configured author model")]
async fn configured_author(w: &mut Kizn) {
    let url = w.fake.openai_url(Behaviour::Works);
    w.add_provider("local", "openai", &url, "qwen3:32b", None).await;
    w.settings_from(Command::TestProvider { name: "local".into() }).await;
}

#[when(expr = "I start a topic {string} with the goal {string}")]
async fn start_topic(w: &mut Kizn, title: String, goal: String) {
    w.goal = Some(goal.clone());
    w.topic_from(Command::StartTopic { title, goal }).await;
    w.wait_for("the draft", |t| t.draft_state != "drafting").await;
}

#[given("a drafted topic")]
async fn drafted_topic(w: &mut Kizn) {
    start_topic(w, "Container fundamentals".into(), "explain to customers why deployments behave differently".into())
        .await;
    assert_eq!(w.topic.as_ref().unwrap().draft_state, "drafted");
}

#[then(expr = "I see {int} to {int} drafted outcomes, each starting with a verb")]
fn drafted_outcomes(w: &mut Kizn, min: usize, max: usize) {
    let t = w.topic.as_ref().unwrap();
    assert!((min..=max).contains(&t.outcomes.len()), "{} outcomes", t.outcomes.len());
    for o in &t.outcomes {
        let first = o.statement.split_whitespace().next().unwrap_or("");
        let looks_like_a_verb = first.chars().next().is_some_and(char::is_uppercase)
            && first.chars().all(char::is_alphabetic)
            && !["The", "A", "An", "Understanding", "Knowledge", "Introduction"].contains(&first);
        assert!(looks_like_a_verb, "\"{}\" doesn't start with a verb", o.statement);
    }
}

#[then("each outcome has a target with a reason that quotes my goal")]
fn targets_quote_goal(w: &mut Kizn) {
    let goal = w.goal.clone().unwrap();
    for o in &w.topic.as_ref().unwrap().outcomes {
        assert!(!o.target_plain.is_empty(), "outcome {} has no target", o.ord);
        assert!(o.reason.contains(&goal), "outcome {}'s reason doesn't quote the goal: {}", o.ord, o.reason);
    }
}

#[then(expr = "each outcome has {int} to {int} concepts")]
fn concept_counts(w: &mut Kizn, min: usize, max: usize) {
    for o in &w.topic.as_ref().unwrap().outcomes {
        assert!((min..=max).contains(&o.concepts.len()), "outcome {} has {} concepts", o.ord, o.concepts.len());
    }
}

#[when(expr = "I rename a concept, delete an outcome and change a target to {string}")]
async fn shape(w: &mut Kizn, target: String) {
    let t = w.topic.clone().unwrap();
    let mut outcomes: Vec<DraftOutcome> = t
        .outcomes
        .iter()
        .map(|o| DraftOutcome {
            id: Some(o.id.clone()),
            statement: o.statement.clone(),
            target: o.target.clone(),
            reason: o.reason.clone(),
            concepts: o.concepts.iter().map(|c| c.statement.clone()).collect(),
        })
        .collect();
    outcomes[0].concepts[0] = "An image is immutable layers plus config".into();
    outcomes.pop();
    outcomes[0].target = target;
    let topic_id = t.id.clone();
    let shaped = w.topic_from(Command::SaveDraft { topic_id, draft: DraftInput { outcomes } }).await;
    assert_eq!(shaped.outcomes.len(), t.outcomes.len() - 1);
    assert_eq!(shaped.outcomes[0].concepts[0].statement, "An image is immutable layers plus config");
    assert_eq!(shaped.outcomes[0].target, "evaluate");
}

#[when("I confirm the topic")]
async fn confirm(w: &mut Kizn) {
    let topic_id = w.topic_id();
    w.topic_from(Command::ConfirmTopic { topic_id }).await;
}

#[then("bank writing is queued for each outcome")]
fn bank_queued(w: &mut Kizn) {
    let t = w.topic.as_ref().unwrap();
    assert_eq!(t.status, "confirmed");
    for o in &t.outcomes {
        assert!(o.bank_state != "failed", "outcome {} failed: {:?}", o.ord, o.bank_error);
    }
}

fn role_key(role: Role) -> &'static str {
    match role {
        Role::Author => "author",
        Role::Checker => "checker",
        Role::Tutor => "tutor",
    }
}

#[given("the author writes an item whose correct option is the longest by far")]
async fn author_longest_cue(w: &mut Kizn) {
    w.use_for(Role::Author, "cue", Behaviour::LongestCue, false).await;
    w.confirmed_topic().await;
}

#[given("the checker picks a different option than the key")]
async fn checker_disagrees(w: &mut Kizn) {
    w.use_for(Role::Checker, "contrary", Behaviour::CheckerDisagrees, false).await;
    w.confirmed_topic().await;
}

#[given("a confirmed topic whose bank is being written")]
async fn topic_being_written(w: &mut Kizn) {
    w.confirmed_topic().await;
}

#[given("a confirmed topic whose author account has no credit")]
async fn topic_no_credit(w: &mut Kizn) {
    drafted_topic(w).await;
    w.use_for(Role::Author, "broke", Behaviour::NoCredit, true).await;
    let topic_id = w.topic_id();
    w.topic_from(Command::ConfirmTopic { topic_id }).await;
}

#[given("drafting failed because the author couldn't run requests")]
async fn drafting_failed(w: &mut Kizn) {
    w.use_for(Role::Author, "broke", Behaviour::NoCredit, true).await;
    w.goal = Some("explain to customers why deployments behave differently".into());
    w.topic_from(Command::StartTopic {
        title: "Container fundamentals".into(),
        goal: "explain to customers why deployments behave differently".into(),
    })
    .await;
    let t = w.wait_for("the draft to fail", |t| t.draft_state != "drafting").await;
    assert_eq!(t.draft_state, "failed");
    assert!(
        t.draft_error.as_deref().unwrap_or("").contains("the account can't run requests yet"),
        "{:?}",
        t.draft_error
    );
}

#[when("I fix the author and try again")]
async fn fix_and_retry(w: &mut Kizn) {
    let path = w.dir().join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path).unwrap().replace("author = \"broke\"", "author = \"local\"");
    std::fs::write(&path, text).unwrap();
    let topic_id = w.topic_id();
    w.topic_from(Command::ResumeDrafting { topic_id }).await;
}

#[then("drafting carries on and every outcome gets its concepts")]
async fn drafting_carries_on(w: &mut Kizn) {
    let t = w.wait_for("the draft to finish", |t| t.draft_state != "drafting").await;
    assert_eq!(t.draft_state, "drafted", "{:?}", t.draft_error);
    assert!(!t.outcomes.is_empty());
    for o in &t.outcomes {
        assert!(!o.concepts.is_empty(), "outcome {} has no concepts", o.ord);
    }
}

#[when("the bank is written")]
async fn bank_written(w: &mut Kizn) {
    w.wait_for("every outcome's bank to finish", |t| {
        t.outcomes.iter().all(|o| o.bank_state == "ready" || o.bank_state == "failed")
    })
    .await;
}

#[when("every outcome has items")]
async fn every_outcome_has_items(w: &mut Kizn) {
    w.wait_for("every outcome to be ready", |t| t.outcomes.iter().all(|o| o.bank_state == "ready")).await;
}

#[then(expr = "that item is refused with the reason {string}")]
fn refused_with(w: &mut Kizn, label: String) {
    let store = w.store();
    let outcome = &w.topic.as_ref().unwrap().outcomes[0];
    let refused = store.items(&outcome.id, Some("refused")).unwrap();
    let hit = refused.iter().find(|i| i.status_reason.as_deref().is_some_and(|r| r.starts_with(&format!("{label}:"))));
    let hit = hit.unwrap_or_else(|| {
        panic!(
            "no item refused for {label:?}; refused: {:?}",
            refused.iter().map(|i| &i.status_reason).collect::<Vec<_>>()
        )
    });
    w.goal = Some(hit.stem.clone()); // remember which item
}

#[then("it never becomes active")]
fn never_active(w: &mut Kizn) {
    let stem = w.goal.clone().unwrap();
    let store = w.store();
    for o in &w.topic.as_ref().unwrap().outcomes {
        assert!(
            !store.items(&o.id, Some("active")).unwrap().iter().any(|i| i.stem == stem),
            "the refused item is active"
        );
    }
}

#[then("the item is not active")]
fn blind_refused(w: &mut Kizn) {
    let store = w.store();
    let mut refused_blind = 0;
    for o in &w.topic.as_ref().unwrap().outcomes {
        let active = store.items(&o.id, Some("active")).unwrap();
        assert!(active.iter().all(|i| i.kind == "explain"), "a scored item went live past a disagreeing checker");
        refused_blind += store
            .items(&o.id, Some("refused"))
            .unwrap()
            .iter()
            .filter(|i| i.status_reason.as_deref().is_some_and(|r| r.starts_with("blind check:")))
            .count();
    }
    assert!(refused_blind > 0, "nothing was refused by the blind check");
}

#[then("I see the topic as preparing")]
fn topic_preparing(w: &mut Kizn) {
    let t = &w.today().topics[0];
    assert!(t.preparing, "Today shows {} as ready already", t.title);
}

#[then("I see the topic as ready")]
async fn topic_ready(w: &mut Kizn) {
    // Ready means full: every outcome holds its whole bank.
    for o in &w.topic.as_ref().unwrap().outcomes {
        assert_eq!(o.active, o.expected, "outcome {} is ready with {} of {} questions", o.ord, o.active, o.expected);
    }
    open_kizn(w).await;
    let t = &w.today().topics[0];
    assert!(
        !t.preparing && t.active > 0,
        "Today shows {} as {}",
        t.title,
        if t.preparing { "preparing" } else { "empty" }
    );
}

#[when("I open Today")]
async fn open_today(w: &mut Kizn) {
    open_kizn(w).await;
}

#[then("each outcome says the account can't run requests yet")]
fn each_outcome_account(w: &mut Kizn) {
    for o in &w.topic.as_ref().unwrap().outcomes {
        assert_eq!(o.bank_state, "failed", "outcome {}", o.ord);
        let why = o.bank_error.as_deref().unwrap_or("");
        assert!(why.starts_with("the account can't run requests yet"), "outcome {}: {why}", o.ord);
    }
}

#[when("I retry an outcome")]
async fn retry_outcome(w: &mut Kizn) {
    let outcome_id = w.topic.as_ref().unwrap().outcomes[0].id.clone();
    w.closed_port = Some(w.store().batch_count(&outcome_id).unwrap() as u16);
    w.topic_from(Command::RetryBank { outcome_id }).await;
}

#[then("kizn tries writing it again straight away")]
async fn tried_again(w: &mut Kizn) {
    let outcome_id = w.topic.as_ref().unwrap().outcomes[0].id.clone();
    let before = w.closed_port.unwrap() as i64;
    for _ in 0..250 {
        if w.store().batch_count(&outcome_id).unwrap() > before {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("no new attempt within 5s of retrying");
}

// The UI owns the words: "writing items" is an outcome whose bank is
// waiting its turn or being written.
#[then(expr = "the topic page shows {string} on each outcome")]
fn topic_page_shows(w: &mut Kizn, text: String) {
    assert_eq!(text, "writing items", "no @domain meaning for {text:?}");
    for o in &w.topic.as_ref().expect("no topic").outcomes {
        assert!(["waiting", "writing"].contains(&o.bank_state.as_str()), "outcome {} is {}", o.ord, o.bank_state);
    }
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

#[given("a provider whose account has no credit")]
async fn provider_no_credit(w: &mut Kizn) {
    let url = w.fake.anthropic_url(Behaviour::NoCredit);
    w.add_provider("broke", "anthropic", &url, "m", Some(TEST_KEY)).await;
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

#[then("I see that the account can't run requests yet, in the provider's own words")]
fn account_cant_run(w: &mut Kizn) {
    let c = w.current_check();
    assert!(c.reachable, "the provider was reached");
    assert!(
        c.summary.starts_with("the account can't run requests yet: Your credit balance is too low"),
        "{}",
        c.summary
    );
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
