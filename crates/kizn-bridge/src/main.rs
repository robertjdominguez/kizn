//! Dev only, never bundled: the real `kizn-app` behind HTTP, so the UI can
//! run in a browser (and over SSH). Binds 127.0.0.1 only; Vite proxies
//! `/rpc` and `/events` to it.
//!
//!   POST /rpc          a Command as JSON → its Reply, or an AppError with a 4xx/5xx
//!   GET  /events       server-sent Events (config changes, later job progress)
//!   POST /test/fresh   (`test` feature only) switch to a new, empty data
//!                      directory: "Given a fresh data directory" in @ui steps
//!
//! Env: KIZN_DATA_DIR / KIZN_CONFIG_DIR (where), KIZN_BRIDGE_PORT (default
//! 7878), KIZN_MOCK=1 (`just dev mock`: model traffic goes to the fake model
//! server on KIZN_FAKE_PORT, default 7880, and keys stay in memory).
//! The `test` feature always runs mock mode.
//!
//! Dev subcommands, for the justfile:
//!   kizn-bridge paths        the data and config directories, as resolved
//!   kizn-bridge inspect db   read-only: schema version, tables, row counts
//!   kizn-bridge inspect bank [topic]   questions by outcome and status, refusal reasons
//!   kizn-bridge seed <file>  migrate the dev database, then run a fixture's SQL

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use kizn_app::{App, AppDeps, AppError, Command, MemorySecrets, Paths};
use kizn_model::fake::FakeServer;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Clone)]
struct Bridge {
    app: Arc<RwLock<Arc<App>>>,
    /// For `/test/fresh`, which opens a new App with the same deps.
    #[cfg_attr(not(feature = "test"), allow(dead_code))]
    deps: AppDeps,
}

impl Bridge {
    fn app(&self) -> Arc<App> {
        self.app.read().unwrap().clone()
    }
}

async fn rpc(State(bridge): State<Bridge>, Json(cmd): Json<Command>) -> Response {
    match bridge.app().handle(cmd).await {
        Ok(reply) => Json(reply).into_response(),
        Err(e) => {
            let status = match e {
                AppError::Config(_) | AppError::Refused(_) => StatusCode::UNPROCESSABLE_ENTITY,
                AppError::Store(_) | AppError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(e)).into_response()
        }
    }
}

async fn events(State(bridge): State<Bridge>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = BroadcastStream::new(bridge.app().subscribe()).filter_map(|ev| async move {
        let ev = ev.ok()?;
        Some(Ok(SseEvent::default().data(serde_json::to_string(&ev).ok()?)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(feature = "test")]
async fn fresh(State(bridge): State<Bridge>) -> Response {
    // A new directory beside the current one, so a run's data stays in the
    // tempdir the test harness gave the bridge. Keys start empty too.
    let base = bridge.app().paths().data_dir.clone();
    let id = kizn_core::new_id(&kizn_core::SystemClock, &kizn_core::SeededRng::from_os());
    let dir = base.with_file_name(format!("fresh-{id}"));
    let deps = AppDeps { secrets: Arc::new(MemorySecrets::default()), ..bridge.deps.clone() };
    match App::open(Paths::in_dir(&dir), deps) {
        Ok(next) => {
            *bridge.app.write().unwrap() = Arc::new(next);
            Json(serde_json::json!({ "data_dir": dir.display().to_string() })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e)).into_response(),
    }
}

fn paths() {
    let p = Paths::from_env();
    println!("data:   {}", p.data_dir.display());
    println!("config: {}", p.config_dir.display());
}

fn inspect(what: Option<&str>, topic: Option<&str>) -> Result<(), String> {
    match what {
        Some("bank") => inspect_bank(topic),
        Some("db") => {
            let path = Paths::from_env().data_dir.join(kizn_app::DB_FILE);
            if !path.exists() {
                return Err(format!("no database at {} yet (run `just dev` once)", path.display()));
            }
            let i = kizn_store::inspect(&path).map_err(|e| e.to_string())?;
            println!("{}", path.display());
            println!("schema version {} (this build: {})", i.schema_version, kizn_store::SCHEMA_VERSION);
            for (name, rows) in i.tables {
                println!("  {name:<24} {rows} rows");
            }
            Ok(())
        }
        other => Err(format!("inspect what? {other:?}: db or bank (progress and attempts come with later phases)")),
    }
}

/// The bank as it stands: per outcome, active questions by kind against what
/// a full bank holds, refusals grouped by reason, and the writing runs.
fn inspect_bank(topic: Option<&str>) -> Result<(), String> {
    let path = Paths::from_env().data_dir.join(kizn_app::DB_FILE);
    if !path.exists() {
        return Err(format!("no database at {} yet", path.display()));
    }
    let store = kizn_store::Store::open(&path).map_err(|e| e.to_string())?;
    let topics: Vec<_> = store
        .all_topics()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|t| topic.is_none_or(|q| t.id.starts_with(q) || t.title.to_lowercase().contains(&q.to_lowercase())))
        .collect();
    if topics.is_empty() {
        return Err(match topic {
            Some(q) => format!("no topic matches {q:?}"),
            None => "no topics yet".into(),
        });
    }
    for t in topics {
        println!("{}  ({}, {})", t.title, t.id, t.status);
        for o in store.outcomes(&t.id).map_err(|e| e.to_string())? {
            let tier = kizn_core::Tier::parse(&o.target_tier).unwrap_or(kizn_core::Tier::Understand);
            let concepts = store.concepts(&o.id).map_err(|e| e.to_string())?;
            let items = store.items(&o.id, None).map_err(|e| e.to_string())?;
            let count = |status: &str| items.iter().filter(|i| i.status == status).count();
            let full = kizn_core::bank::bank_size(tier, concepts.len());
            println!("  {}. {}", o.ord, o.statement);
            println!(
                "     {}  {}/{} active · {} refused · {} retired{}",
                o.bank_state,
                count("active"),
                full,
                count("refused"),
                count("retired"),
                o.bank_error.as_deref().map(|e| format!("\n     error: {e}")).unwrap_or_default()
            );
            let kinds: Vec<String> = ["recall", "apply", "target", "explain"]
                .iter()
                .map(|k| (k, items.iter().filter(|i| i.status == "active" && i.kind == *k).count()))
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| format!("{k} {n}"))
                .collect();
            if !kinds.is_empty() {
                println!("     active by kind: {}", kinds.join(" · "));
            }
            let mut reasons: Vec<(String, usize)> = vec![];
            for i in items.iter().filter(|i| i.status == "refused") {
                let label = i.status_reason.as_deref().unwrap_or("?").split(':').next().unwrap_or("?").to_string();
                match reasons.iter_mut().find(|(l, _)| *l == label) {
                    Some((_, n)) => *n += 1,
                    None => reasons.push((label, 1)),
                }
            }
            reasons.sort_by_key(|r| std::cmp::Reverse(r.1));
            for (label, n) in reasons {
                println!("       {n} × {label}");
            }
            let runs = store.batch_count(&o.id).map_err(|e| e.to_string())?;
            if runs > 0 {
                println!("     {runs} writing runs");
            }
        }
        println!();
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => serve().await,
        Some("paths") => paths(),
        Some("seed") => {
            let Some(file) = args.get(1) else { exit("seed needs a SQL file".into()) };
            let sql = std::fs::read_to_string(file).unwrap_or_else(|e| exit(format!("can't read {file}: {e}")));
            let path = Paths::from_env().data_dir;
            std::fs::create_dir_all(&path).unwrap_or_else(|e| exit(format!("can't create {}: {e}", path.display())));
            let store = kizn_store::Store::open(&path.join(kizn_app::DB_FILE)).unwrap_or_else(|e| exit(e.to_string()));
            store.execute_batch(&sql).unwrap_or_else(|e| exit(format!("{file}: {e}")));
        }
        Some("inspect") => {
            if let Err(e) = inspect(args.get(1).map(String::as_str), args.get(2).map(String::as_str)) {
                eprintln!("kizn-bridge: {e}");
                std::process::exit(1);
            }
        }
        Some(other) => {
            eprintln!("kizn-bridge: unknown command {other:?} (serve, paths, inspect db)");
            std::process::exit(2);
        }
    }
}

fn exit(msg: String) -> ! {
    eprintln!("kizn-bridge: {msg}");
    std::process::exit(1);
}

async fn serve() {
    let mock = cfg!(feature = "test") || std::env::var("KIZN_MOCK").is_ok_and(|v| v == "1");
    let mut deps = AppDeps::system();
    if mock {
        let port = if cfg!(feature = "test") {
            0
        } else {
            std::env::var("KIZN_FAKE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7880)
        };
        let fake =
            FakeServer::start(port).await.unwrap_or_else(|e| exit(format!("can't start the fake model server: {e}")));
        deps.http = fake.client();
        deps.secrets = Arc::new(MemorySecrets::default());
        eprintln!(
            "kizn-bridge: mock mode. Model traffic goes to the fake model server on http://{} (keys stay in memory).\n  \
             Any host works, e.g. http://gpu:11434/v1. Behaviours by URL: {}",
            fake.addr(),
            kizn_model::fake::Behaviour::ALL.map(|b| fake.openai_url(b)).join("  ")
        );
    }
    let app = App::open(Paths::from_env(), deps.clone()).unwrap_or_else(|e| exit(e.to_string()));
    let port = std::env::var("KIZN_BRIDGE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7878);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener =
        tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| exit(format!("can't listen on {addr}: {e}")));
    eprintln!(
        "kizn-bridge on http://{addr} (data: {}, config: {}){}",
        app.paths().data_dir.display(),
        app.paths().config_dir.display(),
        if cfg!(feature = "test") { " [test]" } else { "" }
    );
    let router = Router::new().route("/rpc", post(rpc)).route("/events", get(events));
    #[cfg(feature = "test")]
    let router = router.route("/test/fresh", post(fresh));
    let router = router.with_state(Bridge { app: Arc::new(RwLock::new(Arc::new(app))), deps });
    axum::serve(listener, router).await.unwrap();
}
