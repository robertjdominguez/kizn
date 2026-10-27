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
    AnswerView, App, AppDeps, AppError, AppInfo, CONFIG_FILE, Command, DeckView, DraftInput, DraftOutcome, Event,
    MemorySecrets, NextStep, Paths, ProviderView, Reply, Role, Secrets, SettingsView, SummaryView, TodayView,
    TopicView,
};
use kizn_core::bank::ItemOption;
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
    deck: Option<DeckView>,
    answers: Vec<AnswerView>,
    summary: Option<SummaryView>,
    /// How long the practice commands took, end to end.
    practice_took: std::time::Duration,
    clock: Arc<FakeClock>,
    concept: Option<kizn_app::ConceptPageView>,
    thread: Option<kizn_app::ThreadView>,
    threads: Vec<kizn_app::ThreadSummary>,
    deltas: usize,
    /// Progress and answers before something that mustn't move them.
    progress_before: Option<String>,
    resource: Option<kizn_app::ResourceView>,
    added: Option<Result<kizn_app::ConceptPageView, String>>,
    /// Writing batches started while the last tutor reply streamed.
    batches_during_reply: Option<i64>,
    reading: Option<kizn_app::ReadingListView>,
    topic_ids: std::collections::HashMap<String, String>,
    link_result: Option<Result<(), String>>,
    confirmed_topic_id: Option<String>,
    started: Option<TopicView>,
    map: Option<kizn_app::MapView>,
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
            deck: None,
            answers: vec![],
            summary: None,
            practice_took: std::time::Duration::ZERO,
            clock: Arc::new(FakeClock::at(Utc.with_ymd_and_hms(2026, 10, 25, 9, 0, 0).unwrap())),
            concept: None,
            thread: None,
            threads: vec![],
            deltas: 0,
            progress_before: None,
            resource: None,
            added: None,
            batches_during_reply: None,
            reading: None,
            topic_ids: Default::default(),
            link_result: None,
            confirmed_topic_id: None,
            started: None,
            map: None,
        }
    }

    /// Loads a fixture's seed.sql (tests/fixtures/<name>) into the open app's
    /// database, and reads its topic.
    async fn seed(&mut self, fixture: &str) {
        self.ensure_open();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/").to_string() + fixture + "/seed.sql";
        self.store().execute_batch(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let topic_id = self.store().all_topics().unwrap()[0].id.clone();
        self.topic_from(Command::GetTopic { topic_id }).await;
    }

    async fn start_practice(&mut self) -> DeckView {
        let topic_id = Some(self.topic_id());
        let started = std::time::Instant::now();
        let deck = match self.command(Command::StartPractice { topic_id }).await {
            Ok(Reply::Deck(d)) => d,
            other => panic!("expected a Deck, got {other:?}"),
        };
        self.practice_took += started.elapsed();
        self.deck = Some(deck.clone());
        deck
    }

    /// A card's stored options, read straight from the database (the deck
    /// never carries the key).
    fn stored_options(&self, item_id: &str) -> Vec<ItemOption> {
        let item = self.store().item(item_id).unwrap().unwrap();
        serde_json::from_str(item.options_json.as_deref().unwrap()).unwrap()
    }

    /// Answers the deck's card `n`, right or wrong.
    async fn answer_card(&mut self, n: usize, right: bool) -> AnswerView {
        let card = self.deck.as_ref().expect("not practising").cards.get(n).cloned();
        let card = card.unwrap_or_else(|| panic!("the deck has no card {n}"));
        let stored = self.stored_options(&card.item_id);
        let chosen = card.order.iter().position(|&i| stored[i as usize].correct == right).unwrap() as u32;
        let started = std::time::Instant::now();
        let a = match self.command(Command::Answer { item_id: card.item_id, order: card.order, chosen }).await {
            Ok(Reply::Answered(a)) => a,
            other => panic!("expected an answer, got {other:?}"),
        };
        self.practice_took += started.elapsed();
        self.answers.push(a.clone());
        a
    }

    /// Everything countable about the topic, as text: concept states, recall
    /// dates, answers and whether any is disputed.
    async fn progress_snapshot(&mut self) -> String {
        let topic_id = self.topic_id();
        let p = match self.command(Command::GetProgress { topic_id }).await {
            Ok(Reply::Progress(p)) => p,
            other => panic!("expected Progress, got {other:?}"),
        };
        let attempts = self.store().attempts(None).unwrap();
        let states: Vec<(String, Option<String>, i64)> =
            p.outcomes.iter().flat_map(|o| &o.concepts).map(|c| (c.state.clone(), c.due.clone(), c.answers)).collect();
        format!("{states:?}{attempts:?}")
    }

    async fn read_summary(&mut self) -> SummaryView {
        let attempt_ids = self.answers.iter().map(|a| a.attempt_id.clone()).collect();
        match self.command(Command::GetSummary { attempt_ids }).await {
            Ok(Reply::Summary(v)) => {
                self.summary = Some(v.clone());
                v
            }
            other => panic!("expected a Summary, got {other:?}"),
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
    /// for up to 60 seconds (generous: scenarios run side by side, and the
    /// slow-model ones need the room on a loaded machine).
    async fn wait_for(&mut self, what: &str, done: impl Fn(&TopicView) -> bool) -> TopicView {
        let topic_id = self.topic_id();
        for _ in 0..3000 {
            let t = self.topic_from(Command::GetTopic { topic_id: topic_id.clone() }).await;
            if done(&t) {
                return t;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("waited 60s for {what}: {:#?}", self.topic);
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
                clock: self.clock.clone(),
                rng: Arc::new(SeededRng::from_seed(1)),
                secrets: self.secrets.clone(),
                http: self.fake.client(),
                utc_offset: chrono::FixedOffset::east_opt(0).unwrap(),
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

#[given("a confirmed topic whose checker refuses every question")]
async fn topic_checker_refuses(w: &mut Kizn) {
    drafted_topic(w).await;
    w.use_for(Role::Checker, "picky", Behaviour::CheckerDisagrees, true).await;
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
        t.outcomes.iter().all(|o| matches!(o.bank_state.as_str(), "ready" | "failed" | "resting"))
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

#[then("each outcome rests, and kizn will try it again by itself")]
fn each_outcome_rests(w: &mut Kizn) {
    for o in &w.topic.as_ref().unwrap().outcomes {
        // Resting asks nothing of the learner, unlike "failed".
        assert_eq!(o.bank_state, "resting", "outcome {}", o.ord);
        assert!(o.retry_at.is_some(), "outcome {} has no time to try again", o.ord);
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

// The UI owns the words: every outcome's bank is waiting its turn or being
// written.
#[then("the topic page says its questions are being written")]
fn topic_page_writing(w: &mut Kizn) {
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

// --- First practice -----------------------------------------------------

#[given("a topic whose bank has recall questions for 5 unseen concepts")]
async fn topic_with_bank(w: &mut Kizn) {
    w.seed("topic-with-bank").await;
    let o = &w.topic.as_ref().unwrap().outcomes[0];
    assert_eq!(o.concepts.len(), 5, "the fixture's first outcome has 5 concepts");
}

#[given("I'm practising a topic")]
async fn practising(w: &mut Kizn) {
    topic_with_bank(w).await;
    w.start_practice().await;
}

#[when("I start practice")]
async fn start_practice(w: &mut Kizn) {
    w.start_practice().await;
}

#[then(expr = "I get {int} recall cards, one per concept")]
fn recall_cards(w: &mut Kizn, n: usize) {
    let deck = w.deck.as_ref().unwrap();
    assert_eq!(deck.cards.len(), n, "{deck:#?}");
    assert!(deck.first_set, "a topic never practised: its first set");
    let concepts: std::collections::HashSet<&str> = deck.cards.iter().map(|c| c.concept.as_str()).collect();
    assert_eq!(concepts.len(), n, "one card per concept");
    assert!(deck.cards.iter().all(|c| c.reason == "new" && c.outcome_ord == 1));
}

#[then("the correct option's position varies between cards")]
fn key_positions_vary(w: &mut Kizn) {
    let positions: std::collections::HashSet<usize> = w
        .deck
        .as_ref()
        .unwrap()
        .cards
        .iter()
        .map(|c| {
            let stored = w.stored_options(&c.item_id);
            c.order.iter().position(|&i| stored[i as usize].correct).unwrap()
        })
        .collect();
    assert!(positions.len() > 1, "every key in the same position: {positions:?}");
}

#[when("I tap a wrong option")]
async fn tap_wrong(w: &mut Kizn) {
    w.answer_card(0, false).await;
}

#[then("I see that it is wrong, every option's why, and the source")]
fn see_wrong(w: &mut Kizn) {
    let a = w.answers.last().unwrap();
    assert!(!a.correct);
    assert_ne!(a.picked, a.key);
    assert!(a.options[a.key as usize].correct);
    assert!(a.options.iter().all(|o| !o.why.is_empty()), "an option without its why");
    assert!(a.options[a.picked as usize].misconception.is_some(), "the picked option's misconception");
    assert!(a.source.is_some() || a.model_knowledge, "neither a source nor \"model knowledge\"");
}

#[then(expr = "the concept is {string}")]
fn concept_is(w: &mut Kizn, state: String) {
    let a = w.answers.last().unwrap();
    assert_eq!(a.concepts.len(), 1);
    assert_eq!((a.concepts[0].before.as_str(), a.concepts[0].after.as_str()), ("unseen", state.as_str()));
}

#[when("I finish the set")]
async fn finish_set(w: &mut Kizn) {
    let n = w.deck.as_ref().unwrap().cards.len();
    for i in 0..n {
        w.answer_card(i, i % 2 == 0).await;
    }
    w.read_summary().await;
}

#[then("I see what held, what slipped, the next due date")]
fn summary_held_slipped(w: &mut Kizn) {
    let s = w.summary.as_ref().unwrap();
    assert_eq!((s.answered, s.correct), (5, 3));
    assert_eq!((s.held.len(), s.slipped.len()), (3, 2));
    assert!(s.held.iter().all(|c| c.state == "recalled") && s.slipped.iter().all(|c| c.state == "seen"));
    // A miss comes back tomorrow.
    assert_eq!(s.next_due.as_deref(), Some("2026-10-26 09:00:00"));
}

#[then("each outcome's count and what's next for it")]
fn summary_outcomes(w: &mut Kizn) {
    let s = w.summary.as_ref().unwrap();
    assert_eq!(s.outcomes.len(), 1, "only the outcome practised");
    let o = &s.outcomes[0];
    assert_eq!((o.concepts, o.unseen, o.recalled, o.retained), (5, 0, 3, 0));
    assert!(o.next_due.is_some());
}

#[then(expr = "the {int} I got right are under \"you know these\"")]
fn know_these(w: &mut Kizn, n: usize) {
    let s = w.summary.as_ref().unwrap();
    assert_eq!(s.held.len(), n);
    assert!(s.held.iter().all(|c| !c.concept_id.is_empty()));
}

#[then(expr = "the {int} I missed are under \"learn these first\"")]
fn learn_these(w: &mut Kizn, n: usize) {
    assert_eq!(w.summary.as_ref().unwrap().slipped.len(), n);
}

#[then("it's my starting point, with the next recall tomorrow")]
fn starting_point_tomorrow(w: &mut Kizn) {
    let s = w.summary.as_ref().unwrap();
    assert!(s.first_set);
    assert_eq!(s.next_due_in_days, Some(1));
}

#[when(expr = "I answer {int} cards and stop")]
async fn answer_and_stop(w: &mut Kizn, n: usize) {
    for i in 0..n {
        w.answer_card(i, true).await;
    }
}

#[then(expr = "those {int} answers count")]
async fn answers_count(w: &mut Kizn, n: i64) {
    let topic_id = w.topic_id();
    match w.command(Command::GetProgress { topic_id }).await {
        Ok(Reply::Progress(p)) => {
            assert_eq!(p.attempts, n);
            let recalled = p.outcomes[0].concepts.iter().filter(|c| c.state == "recalled").count() as i64;
            assert_eq!(recalled, n);
        }
        other => panic!("expected Progress, got {other:?}"),
    }
}

#[then(expr = "the summary covers the {int} cards I answered")]
async fn summary_covers(w: &mut Kizn, n: i64) {
    let s = w.read_summary().await;
    assert_eq!(s.answered, n);
    assert_eq!(s.held.len() as i64, n);
}

#[given("no model can be reached")]
fn no_model_reachable(w: &mut Kizn) {
    let path = w.dir().join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path).unwrap();
    // Every provider's base_url at a closed port.
    let unreachable: Vec<String> = text
        .lines()
        .map(|l| if l.starts_with("base_url") { "base_url = \"http://127.0.0.1:9/v1\"".into() } else { l.into() })
        .collect();
    std::fs::write(&path, unreachable.join("\n")).unwrap();
}

#[when("I start practice and answer every card")]
async fn practise_everything(w: &mut Kizn) {
    let n = w.start_practice().await.cards.len();
    assert!(n > 0);
    for i in 0..n {
        w.answer_card(i, true).await;
    }
    w.read_summary().await;
}

#[then("nothing waited on a model")]
fn nothing_waited(w: &mut Kizn) {
    assert_eq!(w.summary.as_ref().unwrap().answered, w.deck.as_ref().unwrap().cards.len() as i64);
    assert!(w.practice_took < std::time::Duration::from_secs(1), "practice took {:?}", w.practice_took);
}

#[given("an author model that takes a while for each task")]
async fn slow_author(w: &mut Kizn) {
    w.use_for(Role::Author, "slow", Behaviour::Slow, false).await;
    w.use_for(Role::Checker, "slow", Behaviour::Slow, false).await;
}

#[when("I confirm a topic")]
async fn confirm_a_topic(w: &mut Kizn) {
    w.confirmed_topic().await;
}

#[then("practice is available once every concept on the first outcome has one question")]
async fn practice_available_early(w: &mut Kizn) {
    w.wait_for("the first set to be ready", |t| t.can_practise).await;
    // Read again: the view counts slots a moment before it decides practice
    // can start, and questions only ever get added.
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    let first = &t.outcomes[0];
    let recall: Vec<_> = first.slots.iter().filter(|s| s.kind == "recall").collect();
    assert!(recall.iter().all(|s| s.have >= 1), "a concept on outcome 1 without a question: {recall:#?}");
    let deck = w.start_practice().await;
    assert_eq!(deck.cards.len(), first.concepts.len().min(5));
}

#[then("the rest of the bank is still being written")]
fn rest_still_writing(w: &mut Kizn) {
    let t = w.topic.as_ref().unwrap();
    assert!(t.active < t.expected / 2, "{} of {} written already", t.active, t.expected);
    assert!(t.outcomes[1..].iter().all(|o| o.active == 0), "later outcomes were written before outcome 1's first set");
}

#[given("I finished my first set today")]
async fn finished_first_set(w: &mut Kizn) {
    practising(w).await;
    let n = w.deck.as_ref().unwrap().cards.len();
    for i in 0..n {
        w.answer_card(i, true).await;
    }
}

#[then("I see when my next recalls are due")]
fn see_next_recalls(w: &mut Kizn) {
    match &w.today().next {
        NextStep::NothingDue { next_at, .. } => assert_eq!(next_at.as_deref(), Some("2026-10-28 09:00:00")),
        other => panic!("expected nothing due until the next recalls, got {other:?}"),
    }
}

// --- Learn what you missed ---------------------------------------------------

impl Kizn {
    async fn concept_from(&mut self, cmd: Command) -> kizn_app::ConceptPageView {
        match self.command(cmd).await {
            Ok(Reply::Concept(c)) => {
                self.concept = Some(c.clone());
                c
            }
            other => panic!("expected a Concept, got {other:?}"),
        }
    }

    /// The concept missed on the first card.
    fn missed_concept(&self) -> String {
        let a = self.answers.first().expect("nothing answered");
        assert!(!a.correct);
        let item = self.store().item(&self.deck.as_ref().unwrap().cards[0].item_id).unwrap().unwrap();
        item.concept_ids[0].clone()
    }
}

#[given("I missed a concept in my starting point")]
async fn missed_one(w: &mut Kizn) {
    practising(w).await;
    w.answer_card(0, false).await;
}

#[when("I open it")]
async fn open_concept(w: &mut Kizn) {
    let concept_id = w.missed_concept();
    // kizn started looking when the answer was wrong; wait for it to finish.
    for _ in 0..500 {
        let c = w.concept_from(Command::GetConcept { concept_id: concept_id.clone() }).await;
        if c.finding != "finding" {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("still looking for sources after 10s");
}

#[then("I see the sources kizn fetched and could read")]
fn see_sources(w: &mut Kizn) {
    let c = w.concept.as_ref().unwrap();
    assert_eq!(c.finding, "found");
    let hosts: Vec<&str> = c.sources.iter().map(|s| s.host.as_str()).collect();
    assert_eq!(hosts, ["docs.example.org", "youtube.com", "blog.example.net"]);
    assert!(c.sources.iter().all(|s| s.words >= 150 && s.why.is_some()));
    assert_eq!(c.sources.iter().map(|s| s.kind.as_str()).collect::<Vec<_>>(), ["page", "video", "page"]);
}

#[then("none it couldn't")]
fn none_unverified(w: &mut Kizn) {
    let c = w.concept.as_ref().unwrap();
    assert!(c.sources.iter().all(|s| !s.url.contains("/web/shell") && !s.url.contains("/web/missing")));
}

#[then("the app shell and the dead link are recorded with their reasons")]
fn rejected_recorded(w: &mut Kizn) {
    let rows = w.store().sources(&w.missed_concept()).unwrap();
    let reason =
        |part: &str| rows.iter().find(|r| r.url.contains(part)).and_then(|r| r.reason.clone()).unwrap_or_default();
    assert!(reason("/web/shell").starts_with("empty app shell"), "{rows:#?}");
    assert!(reason("/web/missing").starts_with("unreachable: HTTP 404"), "{rows:#?}");
}

#[when("I open it and mark it studied")]
async fn open_and_study(w: &mut Kizn) {
    open_concept(w).await;
    let concept_id = w.missed_concept();
    let c = w.concept_from(Command::MarkStudied { concept_id }).await;
    assert!(c.studied);
}

#[then("the summary shows it studied")]
async fn summary_studied(w: &mut Kizn) {
    let s = w.read_summary().await;
    assert_eq!(s.slipped.len(), 1);
    assert!(s.slipped[0].studied);
}

#[then("it still comes back as recall tomorrow")]
fn back_tomorrow(w: &mut Kizn) {
    let c = w.concept.as_ref().unwrap();
    assert_eq!((c.state.as_str(), c.next_due_in_days), ("seen", Some(1)));
}

#[when("a day passes without studying it")]
fn a_day_passes(w: &mut Kizn) {
    w.clock.advance(chrono::Duration::days(1));
}

#[then("it's in the practice set anyway")]
async fn in_set_anyway(w: &mut Kizn) {
    let concept_id = w.missed_concept();
    let statement = w.store().concept(&concept_id).unwrap().unwrap().statement;
    let deck = w.start_practice().await;
    assert!(deck.cards.iter().any(|c| c.reason == "due" && c.concept == statement), "{deck:#?}");
}

#[given("I answer the rest of the set right")]
async fn rest_right(w: &mut Kizn) {
    let n = w.deck.as_ref().unwrap().cards.len();
    for i in 1..n {
        w.answer_card(i, true).await;
    }
}

#[when("I open Today after the set")]
async fn today_after_set(w: &mut Kizn) {
    open_today(w).await;
}

#[then(expr = "Today's next step is to read up on {int} concept")]
fn today_read_up(w: &mut Kizn, n: i64) {
    let missed = w.missed_concept();
    match &w.today().next {
        NextStep::Learn { count, concept_id, next_due_in_days } => {
            assert_eq!((*count, concept_id.as_str(), *next_due_in_days), (n, missed.as_str(), Some(1)));
        }
        other => panic!("expected the reading, got {other:?}"),
    }
}

#[then("this week shows today as practised")]
fn week_practised(w: &mut Kizn) {
    let t = w.today();
    assert_eq!(t.week.len(), 7);
    let today: Vec<_> = t.week.iter().filter(|d| d.today).collect();
    assert_eq!(today.len(), 1);
    assert!(today[0].practised && today[0].date == t.date);
    assert!(t.week.iter().filter(|d| !d.today).all(|d| !d.practised));
}

// --- This key is wrong ---------------------------------------------------------

#[given("I missed a card")]
async fn missed_a_card(w: &mut Kizn) {
    practising(w).await;
    w.answer_card(0, false).await;
}

#[when(expr = "I say {string} and confirm")]
async fn say_key_wrong(w: &mut Kizn, what: String) {
    assert_eq!(what, "This key is wrong");
    let attempt_id = w.answers.last().unwrap().attempt_id.clone();
    match w.command(Command::Dispute { attempt_id }).await {
        Ok(Reply::Disputed(d)) => {
            assert_eq!(d.concepts.len(), 1);
            assert_eq!((d.concepts[0].before.as_str(), d.concepts[0].after.as_str()), ("seen", "unseen"));
        }
        other => panic!("expected Disputed, got {other:?}"),
    }
}

#[then("that answer moves nothing")]
async fn answer_moves_nothing(w: &mut Kizn) {
    let topic_id = w.topic_id();
    match w.command(Command::GetProgress { topic_id }).await {
        Ok(Reply::Progress(p)) => {
            assert!(p.outcomes.iter().flat_map(|o| &o.concepts).all(|c| c.state == "unseen" && c.answers == 0));
        }
        other => panic!("expected Progress, got {other:?}"),
    }
    let s = w.read_summary().await;
    assert_eq!((s.answered, s.slipped.len()), (0, 0));
}

#[then("the question never appears again")]
async fn never_again(w: &mut Kizn) {
    let item_id = w.deck.as_ref().unwrap().cards[0].item_id.clone();
    assert_eq!(w.store().item(&item_id).unwrap().unwrap().status, "disputed");
    w.clock.advance(chrono::Duration::days(4));
    let deck = w.start_practice().await;
    assert!(deck.cards.iter().all(|c| c.item_id != item_id), "the disputed question came back");
}

#[then("a replacement is written for its slot")]
async fn replacement_written(w: &mut Kizn) {
    let item_id = w.deck.as_ref().unwrap().cards[0].item_id.clone();
    let item = w.store().item(&item_id).unwrap().unwrap();
    let concept = item.concept_ids[0].clone();
    let recall_for = |w: &Kizn| {
        w.store()
            .items(&item.outcome_id, Some("active"))
            .unwrap()
            .iter()
            .filter(|i| i.kind == "recall" && i.concept_ids.first() == Some(&concept))
            .count()
    };
    // The fixture held two; the disputed one leaves one until the writer refills.
    for _ in 0..3000 {
        if recall_for(w) >= 2 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("no replacement written for the disputed question's slot");
}

// --- Ask about this --------------------------------------------------------------

impl Kizn {
    fn card_anchor(&self) -> kizn_app::Anchor {
        kizn_app::Anchor::Card { item_id: self.deck.as_ref().unwrap().cards[0].item_id.clone() }
    }

    /// Asks about the card and waits for the whole reply; returns the deltas seen.
    async fn ask(&mut self, text: &str, new: bool) -> (kizn_app::ThreadView, usize) {
        let anchor = self.card_anchor();
        self.ask_on(anchor, text, None, new).await
    }

    async fn ask_on(
        &mut self,
        anchor: kizn_app::Anchor,
        text: &str,
        passage: Option<String>,
        new: bool,
    ) -> (kizn_app::ThreadView, usize) {
        self.ensure_open();
        let mut rx = self.app().subscribe();
        let thread_id = if new { None } else { self.thread.as_ref().map(|t| t.id.clone()) };
        let started = match self.command(Command::Ask { anchor, thread_id, text: text.into(), passage }).await {
            Ok(Reply::Thread(t)) => t,
            other => panic!("expected a Thread, got {other:?}"),
        };
        let batches = |w: &Kizn| -> i64 {
            let store = w.store();
            [
                "01SEEDOUTCOME0000000000001",
                "01SEEDOUTCOME0000000000002",
                "01SEEDOUTCOME0000000000003",
                "01SEEDOUTCOME0000000000004",
            ]
            .iter()
            .map(|o| store.batch_count(o).unwrap_or(0))
            .sum()
        };
        // Writing batches started between the 1st and the 15th word of the
        // reply: certainly while the tutor is answering.
        let mut first: Option<i64> = None;
        let mut deltas = 0;
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await {
                Ok(Ok(Event::TutorDelta { thread_id, .. })) if thread_id == started.id => {
                    deltas += 1;
                    if deltas == 1 {
                        first = Some(batches(self));
                    }
                    if deltas == 15 {
                        self.batches_during_reply = first.map(|f| batches(self) - f);
                    }
                }
                Ok(Ok(Event::TutorDone { thread_id, error })) if thread_id == started.id => {
                    assert!(error.is_none(), "the tutor failed: {error:?}");
                    break;
                }
                Ok(Ok(_)) | Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                other => panic!("no reply from the tutor: {other:?}"),
            }
        }
        let t = match self.command(Command::GetThread { thread_id: started.id.clone() }).await {
            Ok(Reply::Thread(t)) => t,
            other => panic!("expected a Thread, got {other:?}"),
        };
        self.thread = Some(t.clone());
        (t, deltas)
    }
}

#[when(expr = "I ask about it {string}")]
async fn ask_about_it(w: &mut Kizn, text: String) {
    let before = w.progress_snapshot().await;
    w.progress_before = Some(before);
    let (_, deltas) = w.ask(&text, true).await;
    w.deltas = deltas;
}

#[when(expr = "I ask about it again in a new conversation {string}")]
async fn ask_again_new(w: &mut Kizn, text: String) {
    w.ask(&text, true).await;
}

#[then("the tutor's reply streams in, about my pick and the key")]
fn reply_streams(w: &mut Kizn) {
    assert!(w.deltas > 3, "the reply arrived in {} pieces", w.deltas);
    let t = w.thread.as_ref().unwrap();
    assert_eq!(t.messages.len(), 2);
    assert_eq!(t.messages[0].role, "learner");
    let reply = &t.messages[1].text;
    let key = w.stored_options(&w.deck.as_ref().unwrap().cards[0].item_id).into_iter().find(|o| o.correct).unwrap();
    assert!(reply.contains("You picked") && reply.contains(&key.text) && reply.contains(&key.why), "{reply}");
}

#[then("the conversation is kept with the card")]
async fn kept_with_card(w: &mut Kizn) {
    let anchor = w.card_anchor();
    match w.command(Command::GetThreads { anchor }).await {
        Ok(Reply::Threads(ts)) => {
            assert_eq!(ts.len(), 1);
            assert_eq!(ts[0].messages, 2);
        }
        other => panic!("expected Threads, got {other:?}"),
    }
}

#[then("no answer, concept state or recall date changes")]
async fn nothing_moved(w: &mut Kizn) {
    let now = w.progress_snapshot().await;
    assert_eq!(Some(now), w.progress_before, "the tutor moved something");
}

#[then(expr = "the card shows {int} conversations")]
async fn card_conversations(w: &mut Kizn, n: usize) {
    let anchor = w.card_anchor();
    match w.command(Command::GetThreads { anchor }).await {
        Ok(Reply::Threads(ts)) => {
            assert_eq!(ts.len(), n);
            w.threads = ts;
        }
        other => panic!("expected Threads, got {other:?}"),
    }
}

#[then("I can reopen either")]
async fn reopen_either(w: &mut Kizn) {
    for t in w.threads.clone() {
        match w.command(Command::GetThread { thread_id: t.id.clone() }).await {
            Ok(Reply::Thread(v)) => assert_eq!((v.messages.len(), v.title.as_str()), (2, t.title.as_str())),
            other => panic!("expected a Thread, got {other:?}"),
        }
    }
}

// --- Read in kizn ------------------------------------------------------------------

impl Kizn {
    async fn open_resource(&mut self, kind: &str) -> kizn_app::ResourceView {
        let id = self
            .concept
            .as_ref()
            .expect("no concept open")
            .sources
            .iter()
            .find(|s| s.kind == kind)
            .unwrap_or_else(|| panic!("no {kind} offered"))
            .id
            .clone();
        match self.command(Command::GetResource { source_id: id }).await {
            Ok(Reply::Resource(r)) => {
                self.resource = Some(r.clone());
                r
            }
            other => panic!("expected a Resource, got {other:?}"),
        }
    }
}

#[given("a concept with a verified page")]
async fn concept_with_page(w: &mut Kizn) {
    missed_one(w).await;
    open_concept(w).await;
    assert!(w.concept.as_ref().unwrap().sources.iter().any(|s| s.kind == "page"));
}

#[when("I open it in kizn")]
async fn open_in_kizn(w: &mut Kizn) {
    w.open_resource("page").await;
}

#[then("I read its text with the concept on top")]
fn read_text(w: &mut Kizn) {
    let r = w.resource.as_ref().unwrap();
    assert_eq!(r.concept, w.concept.as_ref().unwrap().statement);
    assert!(r.blocks.iter().any(|b| b.kind == "h") && r.blocks.iter().filter(|b| b.kind == "p").count() >= 3);
    assert!(r.cues.is_empty());
}

#[then("not the page's scripts or navigation")]
fn no_scripts(w: &mut Kizn) {
    let r = w.resource.as_ref().unwrap();
    assert!(
        r.blocks.iter().all(|b| !b.text.contains("analytics()") && !b.text.contains("Docs Guides Reference")),
        "{:#?}",
        r.blocks
    );
}

#[when("kizn looks for resources for it")]
async fn looks_for_resources(w: &mut Kizn) {
    open_concept(w).await;
}

#[then("a video with captions is offered")]
fn video_offered(w: &mut Kizn) {
    let c = w.concept.as_ref().unwrap();
    let v = c.sources.iter().find(|s| s.kind == "video").expect("no video offered");
    assert!(v.url.contains("captioned-") && !v.automatic, "{v:?}");
}

#[then("opening it shows its transcript with timestamps")]
async fn transcript_shown(w: &mut Kizn) {
    let r = w.open_resource("video").await;
    assert!(r.blocks.is_empty());
    assert_eq!(r.cues.len(), 40);
    assert!(r.cues.windows(2).all(|p| p[0].start < p[1].start));
}

#[then("the video without captions is recorded as having no transcript")]
fn no_transcript(w: &mut Kizn) {
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    let rows = w.store().sources(&concept_id).unwrap();
    let row = rows.iter().find(|r| r.url.contains("nocaptions-")).expect("the uncaptioned video wasn't checked");
    assert_eq!(row.status, "rejected");
    assert!(row.reason.as_deref().unwrap_or("").starts_with("no transcript"), "{row:?}");
}

#[when("I write a note while reading it")]
async fn write_note(w: &mut Kizn) {
    w.open_resource("page").await;
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    let r = w.command(Command::SaveNote { concept_id, text: "Layers are shared, so rebuilds are cheap.".into() }).await;
    assert!(matches!(r, Ok(Reply::Note(_))), "{r:?}");
}

#[when("I come back to the concept later")]
async fn come_back_later(w: &mut Kizn) {
    // kizn closed and opened again.
    w.app = None;
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    w.concept_from(Command::GetConcept { concept_id }).await;
}

#[then("the note is there")]
fn note_there(w: &mut Kizn) {
    assert_eq!(w.concept.as_ref().unwrap().note.as_deref(), Some("Layers are shared, so rebuilds are cheap."));
}

#[when(expr = "I add {string} to it")]
async fn add_source(w: &mut Kizn, url: String) {
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    w.added = Some(match w.command(Command::AddSource { concept_id, url }).await {
        Ok(Reply::Concept(c)) => Ok(c),
        Err(AppError::Refused(why)) => Err(why),
        other => panic!("unexpected {other:?}"),
    });
}

#[then("kizn fetches it once and offers it")]
fn added_offered(w: &mut Kizn) {
    let c = w.added.clone().unwrap().expect("refused");
    let own = c.sources.iter().find(|s| s.own).expect("not offered");
    assert_eq!(own.host, "notes.example.com");
    assert_eq!(own.kind, "page");
}

#[then("I'm told it couldn't be read, and why")]
fn added_refused(w: &mut Kizn) {
    let why = w.added.clone().unwrap().expect_err("it was offered");
    assert!(why.contains("empty app shell"), "{why}");
}

// --- Asking about concepts, resources, passages -------------------------------------

impl Kizn {
    fn reply(&self) -> String {
        self.thread.as_ref().unwrap().messages.last().unwrap().text.clone()
    }

    async fn threads_on(&mut self, anchor: kizn_app::Anchor) -> Vec<kizn_app::ThreadSummary> {
        match self.command(Command::GetThreads { anchor }).await {
            Ok(Reply::Threads(ts)) => ts,
            other => panic!("expected Threads, got {other:?}"),
        }
    }
}

#[when(expr = "I ask about the concept {string}")]
async fn ask_concept(w: &mut Kizn, text: String) {
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    w.ask_on(kizn_app::Anchor::Concept { concept_id }, &text, None, true).await;
}

#[then("the tutor's reply draws on the concept's resources")]
fn reply_draws_resources(w: &mut Kizn) {
    let first = w.concept.as_ref().unwrap().sources[0].title.clone();
    let r = w.reply();
    assert!(r.contains(&format!("From {first}:")) && r.contains("This page explains"), "{r}");
}

#[then(expr = "the concept shows {int} conversation")]
async fn concept_conversations(w: &mut Kizn, n: usize) {
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    assert_eq!(w.threads_on(kizn_app::Anchor::Concept { concept_id }).await.len(), n);
}

#[when(expr = "I pick a passage in the page and ask {string}")]
async fn ask_passage(w: &mut Kizn, text: String) {
    let r = w.open_resource("page").await;
    let passage = r.blocks.iter().find(|b| b.kind == "p").unwrap().text.chars().take(60).collect::<String>();
    let source_id = r.source.id.clone();
    w.ask_on(kizn_app::Anchor::Resource { source_id }, &text, Some(passage), true).await;
}

#[then("the tutor's reply is about that passage, from the page")]
fn reply_passage(w: &mut Kizn) {
    let r = w.reply();
    let title = &w.resource.as_ref().unwrap().source.title;
    assert!(
        r.starts_with(&format!("About {title}")) && r.contains("On \"This page explains") && r.contains("From it:"),
        "{r}"
    );
    let asked = &w.thread.as_ref().unwrap().messages[0].text;
    assert!(asked.starts_with("> This page explains") && asked.ends_with("Explain this"), "{asked}");
}

#[then(expr = "the page shows {int} conversation")]
async fn page_conversations(w: &mut Kizn, n: usize) {
    let source_id = w.resource.as_ref().unwrap().source.id.clone();
    assert_eq!(w.threads_on(kizn_app::Anchor::Resource { source_id }).await.len(), n);
}

#[when(expr = "I pick a moment in its video and ask {string}")]
async fn ask_moment(w: &mut Kizn, text: String) {
    let r = w.open_resource("video").await;
    let cue = &r.cues[13];
    let passage = format!("{}s: {}", cue.start, cue.text);
    let source_id = r.source.id.clone();
    w.ask_on(kizn_app::Anchor::Resource { source_id }, &text, Some(passage), true).await;
}

#[then("the tutor's reply says when in the video")]
fn reply_moment(w: &mut Kizn) {
    let r = w.reply();
    assert!(r.contains("From it at ") && r.contains("On \"65s:"), "{r}");
}

#[when("a question about it is replaced")]
async fn question_replaced(w: &mut Kizn) {
    // The missed card's question disputed: its slot gets a replacement,
    // written now that the concept has resources.
    let attempt_id = w.answers[0].attempt_id.clone();
    assert!(matches!(w.command(Command::Dispute { attempt_id }).await, Ok(Reply::Disputed(_))));
}

#[then("the new question cites one of its resources")]
async fn cites_resource(w: &mut Kizn) {
    let concept_id = w.concept.as_ref().unwrap().concept_id.clone();
    let titles: Vec<String> = w.concept.as_ref().unwrap().sources.iter().map(|s| s.title.clone()).collect();
    let outcome_id = w.store().concept(&concept_id).unwrap().unwrap().outcome_id;
    for _ in 0..3000 {
        let cited = w
            .store()
            .items(&outcome_id, Some("active"))
            .unwrap()
            .into_iter()
            .find(|i| i.concept_ids.first() == Some(&concept_id) && i.source_ref.is_some());
        if let Some(i) = cited {
            assert!(!i.model_knowledge);
            assert!(titles.contains(i.source_ref.as_ref().unwrap()), "{:?} isn't one of {titles:?}", i.source_ref);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let dump: Vec<(String, String, Option<String>, Option<String>)> = w
        .store()
        .items(&outcome_id, None)
        .unwrap()
        .into_iter()
        .filter(|i| i.status == "refused")
        .map(|i| (i.kind, i.status, i.status_reason, i.source_ref))
        .collect();
    let o = w.store().outcome(&outcome_id).unwrap().unwrap();
    panic!("no question cites a resource: {dump:#?} {} {:?}", o.bank_state, o.bank_error);
}

#[given("an author and tutor that share a slow model")]
async fn shared_slow(w: &mut Kizn) {
    w.use_for(Role::Author, "slow", Behaviour::Slow, false).await;
    w.use_for(Role::Checker, "slow", Behaviour::Slow, false).await;
    w.use_for(Role::Tutor, "slow", Behaviour::Slow, false).await;
}

#[given("I missed a card while questions are being written")]
async fn missed_while_writing(w: &mut Kizn) {
    missed_a_card(w).await;
    // Writing is under way (outcomes 3 and 4 have no questions yet).
    for _ in 0..500 {
        if w.store().batch_count("01SEEDOUTCOME0000000000003").unwrap()
            + w.store().batch_count("01SEEDOUTCOME0000000000001").unwrap()
            > 0
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[then("no new writing starts until the reply is done")]
fn no_new_writing(w: &mut Kizn) {
    assert_eq!(w.batches_during_reply, Some(0), "the writer started batches while the tutor answered");
}

// --- Reading list -----------------------------------------------------------------

impl Kizn {
    async fn reading_list(&mut self) -> kizn_app::ReadingListView {
        match self.command(Command::GetReadingList { topic_id: None }).await {
            Ok(Reply::ReadingList(l)) => l,
            other => panic!("expected a ReadingList, got {other:?}"),
        }
    }
}

#[given(expr = "I missed {int} concepts in my starting point")]
async fn missed_n(w: &mut Kizn, n: usize) {
    practising(w).await;
    let total = w.deck.as_ref().unwrap().cards.len();
    for i in 0..total {
        w.answer_card(i, i >= n).await;
    }
}

#[when("I open my reading list")]
async fn open_reading_list(w: &mut Kizn) {
    // Opening it starts kizn looking; wait until it's found something for each.
    for _ in 0..500 {
        let l = w.reading_list().await;
        if l.concepts.iter().all(|c| !c.finding && c.pages + c.videos > 0) {
            w.reading = Some(l);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the reading list never filled: {:#?}", w.reading_list().await);
}

#[then("I see both concepts, with what there is to learn from for each")]
fn reading_both(w: &mut Kizn) {
    let l = w.reading.as_ref().unwrap();
    assert_eq!(l.concepts.len(), 2);
    assert!(l.concepts.iter().all(|c| c.pages == 2 && c.videos == 1), "{l:#?}");
}

#[then("none of them studied yet")]
fn none_studied(w: &mut Kizn) {
    assert_eq!(w.reading.as_ref().unwrap().studied, 0);
}

#[when("I mark one of them studied")]
async fn mark_one(w: &mut Kizn) {
    let concept_id = w.reading.as_ref().unwrap().concepts[0].concept_id.clone();
    w.concept_from(Command::MarkStudied { concept_id }).await;
}

#[then(expr = "the reading list shows {int} of {int} studied")]
async fn reading_progress(w: &mut Kizn, studied: i64, total: usize) {
    let l = w.reading_list().await;
    assert_eq!((l.studied, l.concepts.len()), (studied, total));
    assert!(l.concepts[0].studied && !l.concepts[1].studied);
}

// --- A second look ------------------------------------------------------------------

const REGISTRY: &str = "01SEEDCONCEPT00000000103";

#[given("I missed the concept about registries")]
async fn missed_registry(w: &mut Kizn) {
    practising(w).await;
    let n = w.deck.as_ref().unwrap().cards.len();
    let registry_card = (0..n)
        .find(|&i| {
            w.store().item(&w.deck.as_ref().unwrap().cards[i].item_id).unwrap().unwrap().concept_ids[0] == REGISTRY
        })
        .expect("no card for the registry concept");
    for i in 0..n {
        w.answer_card(i, i != registry_card).await;
    }
}

#[when("kizn looks for resources for that concept")]
async fn looks_for_registry(w: &mut Kizn) {
    for _ in 0..500 {
        let c = w.concept_from(Command::GetConcept { concept_id: REGISTRY.into() }).await;
        if c.finding != "finding" {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("still looking after 10s");
}

#[then("the first suggestions were all turned down")]
fn first_round_down(w: &mut Kizn) {
    let rows = w.store().sources(REGISTRY).unwrap();
    let first: Vec<_> = rows.iter().take(2).collect();
    assert!(first.iter().all(|r| r.status != "verified"), "{first:#?}");
}

#[then("it asked again and offers what it found")]
fn asked_again(w: &mut Kizn) {
    let c = w.concept.as_ref().unwrap();
    assert_eq!(c.finding, "found");
    assert_eq!(c.sources.len(), 3, "{:#?}", c.sources);
}

// --- Topics that rest on topics -------------------------------------------------------

impl Kizn {
    /// A confirmed topic straight in the store (links don't need a bank).
    fn make_topic(&mut self, title: &str) {
        self.ensure_open();
        let id = format!("01TOPIC{:0>19}", self.topic_ids.len() + 1);
        let store = self.store();
        store.create_draft_topic(&id, title, "to see how topics link", "2026-10-25 09:00:00").unwrap();
        store.confirm_topic(&id, "2026-10-25 09:00:00").unwrap();
        self.topic_ids.insert(title.to_string(), id);
    }

    fn tid(&self, title: &str) -> String {
        self.topic_ids.get(title).unwrap_or_else(|| panic!("no topic {title:?}")).clone()
    }

    async fn link(&mut self, upper: &str, lower: &str, kind: &str) -> Result<(), String> {
        let (upper_id, lower_id) = (self.tid(upper), self.tid(lower));
        match self.command(Command::LinkTopics { upper_id, lower_id, kind: kind.into() }).await {
            Ok(Reply::Done) => Ok(()),
            Err(AppError::Refused(why)) => Err(why),
            other => panic!("unexpected {other:?}"),
        }
    }

    async fn topic_named(&mut self, title: &str) -> TopicView {
        let topic_id = self.tid(title);
        self.topic_from(Command::GetTopic { topic_id }).await
    }
}

#[given(expr = "topics {string} and {string}")]
fn topics_two(w: &mut Kizn, a: String, b: String) {
    w.make_topic(&a);
    w.make_topic(&b);
}

#[given(expr = "topics {string}, {string} and {string}")]
fn topics_three(w: &mut Kizn, a: String, b: String, c: String) {
    for t in [a, b, c] {
        w.make_topic(&t);
    }
}

#[given(expr = "{string} rests on {string}, which rests on {string}")]
async fn chain(w: &mut Kizn, a: String, b: String, c: String) {
    w.link(&a, &b, "rests_on").await.unwrap();
    w.link(&b, &c, "rests_on").await.unwrap();
}

#[when(expr = "I make {string} a foundation of {string}")]
async fn make_foundation(w: &mut Kizn, lower: String, upper: String) {
    w.link_result = Some(w.link(&upper, &lower, "rests_on").await);
}

#[when(expr = "I make {string} part of {string}")]
async fn make_part(w: &mut Kizn, lower: String, upper: String) {
    w.link_result = Some(w.link(&upper, &lower, "part_of").await);
}

#[then(expr = "{string} rests on {string}")]
async fn rests_on(w: &mut Kizn, upper: String, lower: String) {
    assert_eq!(w.link_result.clone(), Some(Ok(())));
    let t = w.topic_named(&upper).await;
    assert_eq!(t.rests_on.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), [lower.as_str()]);
}

#[then(expr = "{string} supports {string}")]
async fn supports(w: &mut Kizn, lower: String, upper: String) {
    let t = w.topic_named(&lower).await;
    assert_eq!(t.supports.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), [upper.as_str()]);
}

#[then(expr = "{string} sits two layers above {string}")]
async fn two_layers(w: &mut Kizn, upper: String, lower: String) {
    let (u, l) = (w.topic_named(&upper).await.layer, w.topic_named(&lower).await.layer);
    assert_eq!(u - l, 2, "{upper} is layer {u}, {lower} layer {l}");
}

#[then("I'm told it would make a loop")]
fn told_loop(w: &mut Kizn) {
    let why = w.link_result.clone().expect("nothing linked").expect_err("the loop was linked");
    assert!(why.contains("would make a loop"), "{why}");
}

#[then(expr = "{string} is made up of {string}")]
async fn made_up_of(w: &mut Kizn, upper: String, part: String) {
    let t = w.topic_named(&upper).await;
    assert_eq!(t.parts.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), [part.as_str()]);
    let p = w.topic_named(&part).await;
    assert_eq!(p.part_of.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), [upper.as_str()]);
}

// --- Foundations at setup -------------------------------------------------------------

impl Kizn {
    async fn ground(&mut self) -> kizn_app::GroundView {
        let topic_id = self.topic_id();
        match self.command(Command::GetGround { topic_id }).await {
            Ok(Reply::Ground(g)) => g,
            other => panic!("expected Ground, got {other:?}"),
        }
    }

    /// Answers a foundation's check questions: right for each `true`.
    async fn answer_ground(&mut self, title: &str, right: &[bool]) {
        let g = self.ground().await;
        let f = g.foundations.iter().find(|f| f.title == title).unwrap_or_else(|| panic!("no foundation {title}"));
        let store = self.store();
        for (q, &r) in f.questions.iter().zip(right) {
            let row = store.ground_items(&f.suggestion_id).unwrap().into_iter().find(|g| g.id == q.id).unwrap();
            let opts: Vec<ItemOption> = serde_json::from_str(&row.options_json).unwrap();
            let chosen = q.order.iter().position(|&i| opts[i as usize].correct == r).unwrap() as u32;
            let res =
                self.command(Command::AnswerGround { item_id: q.id.clone(), order: q.order.clone(), chosen }).await;
            assert!(matches!(res, Ok(Reply::Ground(_))), "{res:?}");
        }
    }
}

#[then(expr = "the draft proposes {int} foundations, each with why it matters")]
fn draft_proposes(w: &mut Kizn, n: usize) {
    let f = &w.topic.as_ref().unwrap().foundations;
    assert_eq!(f.len(), n, "{f:#?}");
    assert!(f.iter().all(|f| f.why.is_some() && f.status == "kept" && f.source == "draft"));
}

#[when(expr = "I drop {string} while shaping it")]
async fn drop_foundation(w: &mut Kizn, title: String) {
    let id = w.topic.as_ref().unwrap().foundations.iter().find(|f| f.title == title).unwrap().id.clone();
    w.topic_from(Command::KeepFoundation { suggestion_id: id, keep: false }).await;
}

#[then(expr = "{int} foundations are kept")]
fn kept_count(w: &mut Kizn, n: usize) {
    assert_eq!(w.topic.as_ref().unwrap().foundations.iter().filter(|f| f.status == "kept").count(), n);
}

#[given(expr = "a topic I already have called {string}")]
fn topic_i_have(w: &mut Kizn, title: String) {
    w.make_topic(&title);
}

#[when("I confirm it with its foundations")]
async fn confirm_with_foundations(w: &mut Kizn) {
    let topic_id = w.topic_id();
    w.topic_from(Command::ConfirmTopic { topic_id }).await;
}

#[then(expr = "{string} rests on {string}, the topic I have")]
async fn rests_on_mine(w: &mut Kizn, _upper: String, lower: String) {
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    let link = t.rests_on.iter().find(|l| l.title == lower).expect("not linked");
    assert_eq!(link.topic_id, w.tid(&lower));
    let g = w.ground().await;
    assert!(g.foundations.iter().all(|f| f.title != lower), "a topic I have was checked");
}

#[given("I confirmed a topic keeping 2 foundations")]
async fn confirmed_keeping_two(w: &mut Kizn) {
    drafted_topic(w).await;
    drop_foundation(w, "Networking basics".into()).await;
    confirm_with_foundations(w).await;
    for _ in 0..500 {
        let g = w.ground().await;
        if !g.writing && g.foundations.iter().all(|f| !f.questions.is_empty()) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the check questions were never written: {:#?}", w.ground().await);
}

#[then(expr = "there are {int} questions on each foundation to check")]
async fn questions_each(w: &mut Kizn, n: usize) {
    let g = w.ground().await;
    assert_eq!(g.foundations.len(), 2);
    assert!(g.foundations.iter().all(|f| f.questions.len() == n), "{g:#?}");
}

#[when(expr = "I answer the questions on {string} mostly right")]
async fn ground_mostly_right(w: &mut Kizn, title: String) {
    w.answer_ground(&title, &[true, true, false]).await;
}

#[when(expr = "I answer the questions on {string} mostly wrong")]
async fn ground_mostly_wrong(w: &mut Kizn, title: String) {
    w.answer_ground(&title, &[true, false, false]).await;
}

#[then(expr = "{string} seems solid and {string} might be shaky")]
async fn solid_and_shaky(w: &mut Kizn, solid: String, shaky: String) {
    let g = w.ground().await;
    let result = |t: &str| g.foundations.iter().find(|f| f.title == t).and_then(|f| f.result.clone());
    assert_eq!((result(&solid).as_deref(), result(&shaky).as_deref()), (Some("solid"), Some("shaky")));
}

#[then(expr = "{string} is on my map to start with, and {string} isn't")]
async fn on_map_to_start(w: &mut Kizn, shaky: String, solid: String) {
    let map = match w.command(Command::GetMap).await {
        Ok(Reply::Map(m)) => m,
        other => panic!("expected the map, got {other:?}"),
    };
    let suggested = |t: &str| map.nodes.iter().any(|n| n.kind == "suggestion" && n.title == t);
    assert!(suggested(&shaky), "{shaky} isn't suggested on the map");
    assert!(!suggested(&solid), "{solid} is on the map, but it seems solid");
}

#[when(expr = "I start {string} from the check")]
async fn start_from_check(w: &mut Kizn, title: String) {
    let g = w.ground().await;
    let id = g.foundations.iter().find(|f| f.title == title).unwrap().suggestion_id.clone();
    w.confirmed_topic_id = Some(w.topic_id());
    match w.command(Command::ResolveFoundation { suggestion_id: id, action: "start".into() }).await {
        Ok(Reply::Topic(t)) => w.started = Some(t),
        other => panic!("expected the new topic, got {other:?}"),
    }
}

#[then(expr = "a new topic {string} is being set up")]
fn new_topic_set_up(w: &mut Kizn, title: String) {
    let t = w.started.as_ref().unwrap();
    assert_eq!((t.title.as_str(), t.status.as_str()), (title.as_str(), "draft"));
}

#[then("the topic I confirmed rests on it")]
async fn confirmed_rests_on_started(w: &mut Kizn) {
    let topic_id = w.confirmed_topic_id.clone().unwrap();
    let started = w.started.as_ref().unwrap().id.clone();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    assert!(t.rests_on.iter().any(|l| l.topic_id == started), "{:#?}", t.rests_on);
}

// --- Flags from answers ------------------------------------------------------------------

#[given(expr = "I missed {int} questions that point at {string}")]
async fn missed_pointing(w: &mut Kizn, n: usize, _title: String) {
    practising(w).await;
    for i in 0..n {
        w.answer_card(i, false).await;
    }
}

#[then(expr = "the topic and Today flag {string}")]
async fn topic_and_today_flag(w: &mut Kizn, title: String) {
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    assert_eq!(
        t.flags.iter().map(|f| (f.title.as_str(), f.kind.as_str(), f.misses)).collect::<Vec<_>>(),
        [(title.as_str(), "new", 2)]
    );
    open_today(w).await;
    assert!(w.today().flags.iter().any(|f| f.title == title));
}

#[when("I add it underneath")]
async fn add_underneath(w: &mut Kizn) {
    let f = w.topic.as_ref().unwrap().flags[0].clone();
    w.topic_from(Command::ResolveFlag { for_topic: f.for_topic, title: f.title, action: "add".into() }).await;
}

#[then(expr = "the flag is gone and {string} is suggested on my map")]
async fn flag_gone_suggested(w: &mut Kizn, title: String) {
    assert!(w.topic.as_ref().unwrap().flags.is_empty());
    let map = match w.command(Command::GetMap).await {
        Ok(Reply::Map(m)) => m,
        other => panic!("expected the map, got {other:?}"),
    };
    let node = map.nodes.iter().find(|n| n.title == title && n.kind == "suggestion").expect("not on the map");
    assert!(map.edges.iter().any(|e| e.lower == node.id && e.suggested && e.upper == w.topic_id()));
}

#[when(expr = r#"I say "not now" to the flag"#)]
async fn not_now(w: &mut Kizn) {
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    let f = t.flags[0].clone();
    w.topic_from(Command::ResolveFlag { for_topic: f.for_topic, title: f.title, action: "dismiss".into() }).await;
}

#[then("the flag is gone, after a restart too")]
async fn flag_gone_restart(w: &mut Kizn) {
    assert!(w.topic.as_ref().unwrap().flags.is_empty());
    w.app = None;
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    assert!(t.flags.is_empty(), "{:#?}", t.flags);
}

#[given(expr = "{string} is a topic my topic already rests on")]
async fn already_rests_on(w: &mut Kizn, title: String) {
    w.make_topic(&title);
    let (upper_id, lower_id) = (w.topic_id(), w.tid(&title));
    assert!(matches!(
        w.command(Command::LinkTopics { upper_id, lower_id, kind: "rests_on".into() }).await,
        Ok(Reply::Done)
    ));
}

#[then("the flag says it's worth a review")]
async fn flag_review(w: &mut Kizn) {
    let topic_id = w.topic_id();
    let t = w.topic_from(Command::GetTopic { topic_id }).await;
    assert_eq!(t.flags.len(), 1);
    assert_eq!(t.flags[0].kind, "review");
}

// --- The map ---------------------------------------------------------------------------------

#[given(expr = "{string} is suggested under {string}")]
fn suggested_under(w: &mut Kizn, title: String, upper: String) {
    let for_topic = w.tid(&upper);
    w.store()
        .insert_suggestion(&kizn_store::SuggestionRow {
            id: "01SUGGESTION00000000000001".into(),
            for_topic,
            title,
            why: Some("services talk over ports".into()),
            source: "flag".into(),
            status: "later".into(),
            topic_id: None,
            created_at: "2026-10-25 09:00:00".into(),
        })
        .unwrap();
}

#[when("I open the map")]
async fn open_map(w: &mut Kizn) {
    w.map = Some(match w.command(Command::GetMap).await {
        Ok(Reply::Map(m)) => m,
        other => panic!("expected the map, got {other:?}"),
    });
}

#[then(expr = "{string} is at the bottom and {string} at the top")]
fn bottom_and_top(w: &mut Kizn, bottom: String, top: String) {
    let m = w.map.as_ref().unwrap();
    let layer = |t: &str| m.nodes.iter().find(|n| n.title == t).unwrap().layer;
    let max = m.nodes.iter().map(|n| n.layer).max().unwrap();
    assert_eq!((layer(&bottom), layer(&top)), (0, max));
    assert_eq!(max, 2);
}

#[then(expr = "{string} is drawn as a suggestion under {string}")]
fn drawn_suggestion(w: &mut Kizn, title: String, upper: String) {
    let m = w.map.as_ref().unwrap();
    let n = m.nodes.iter().find(|n| n.title == title).unwrap();
    assert_eq!(n.kind, "suggestion");
    let upper_id = w.tid(&upper);
    assert!(m.edges.iter().any(|e| e.upper == upper_id && e.lower == n.id && e.suggested));
}

// The scenarios run on a thread with a roomy stack: the world and its step
// futures outgrow the 1 MB a Windows main thread gets.
fn main() {
    std::thread::Builder::new()
        .name("scenarios".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(8 * 1024 * 1024)
                .build()
                .expect("a tokio runtime")
                .block_on(run())
        })
        .expect("the scenarios thread")
        .join()
        .expect("the scenarios ran");
}

async fn run() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let extra = std::env::var("KIZN_BDD_TAG").ok().filter(|t| !t.is_empty());
    let extra = extra.map(|t| t.trim_start_matches('@').to_string());
    // A few at a time: scenarios share the machine with the fake model
    // server, and a small CI runner starved it when 64 ran at once.
    let at_once = std::env::var("KIZN_BDD_AT_ONCE").ok().and_then(|n| n.parse().ok()).unwrap_or(8);
    Kizn::cucumber()
        .max_concurrent_scenarios(at_once)
        .fail_on_skipped()
        .filter_run_and_exit(features, move |feature, rule, scenario| {
            let tags: Vec<&String> =
                feature.tags.iter().chain(rule.map(|r| &r.tags).into_iter().flatten()).chain(&scenario.tags).collect();
            let has = |t: &str| tags.iter().any(|x| x.as_str() == t);
            has("domain") && extra.as_deref().is_none_or(has)
        })
        .await;
}
