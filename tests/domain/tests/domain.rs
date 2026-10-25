//! The @domain layer: the feature files in `features/`, driven against
//! `kizn-app` in-process with a fake clock, a fixed seed and a throwaway
//! data directory. Never a real model.
//!
//! `KIZN_BDD_TAG` (set by `just bdd <tag>`) narrows the run to scenarios that
//! also carry that tag.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use cucumber::{World, given, then, when};
use kizn_app::{App, CONFIG_FILE, Command, Paths, Reply, TodayView};
use kizn_core::{FakeClock, SeededRng};

#[derive(Debug, Default, World)]
struct Kizn {
    dir: Option<tempfile::TempDir>,
    today: Option<TodayView>,
}

impl Kizn {
    fn dir(&self) -> &std::path::Path {
        self.dir.as_ref().expect("no data directory: start with \"Given a fresh data directory\"").path()
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
    let clock = Arc::new(FakeClock::at(Utc.with_ymd_and_hms(2026, 10, 25, 9, 0, 0).unwrap()));
    let app = App::open(Paths::in_dir(w.dir()), clock, Arc::new(SeededRng::from_seed(1))).unwrap();
    match app.handle(Command::GetToday).await.unwrap() {
        Reply::Today(t) => w.today = Some(t),
        other => panic!("GetToday replied {other:?}"),
    }
}

// The UI owns the words; here each phrase a scenario can "see" maps to the
// state of the view that makes the UI show it.
#[then(expr = "I see {string}")]
fn i_see(w: &mut Kizn, text: String) {
    let today = w.today();
    match text.as_str() {
        "Start a topic" => assert!(today.topics.is_empty(), "Today lists topics, so its empty state isn't shown"),
        other => panic!("no @domain meaning for seeing {other:?}: add it to the I see step"),
    }
}

#[then("no model is configured")]
fn no_model_configured(w: &mut Kizn) {
    assert!(!w.today().model_configured, "Today says a model is configured");
    assert!(!w.dir().join(CONFIG_FILE).exists(), "first run wrote a config.toml");
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
