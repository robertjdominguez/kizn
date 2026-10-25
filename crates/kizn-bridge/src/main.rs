//! Dev only, never bundled: the real `kizn-app` behind HTTP, so the UI can
//! run in a browser (and over SSH). Binds 127.0.0.1 only; Vite proxies
//! `/rpc` to it.
//!
//!   POST /rpc          a Command as JSON → its Reply, or an AppError with a 4xx/5xx
//!   POST /test/fresh   (`test` feature only) switch to a new, empty data
//!                      directory: "Given a fresh data directory" in @ui steps
//!
//! Env: KIZN_DATA_DIR / KIZN_CONFIG_DIR (where), KIZN_BRIDGE_PORT (default 7878).
//!
//! Dev subcommands, for the justfile:
//!   kizn-bridge paths        the data and config directories, as resolved
//!   kizn-bridge inspect db   read-only: schema version, tables, row counts

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use kizn_app::{App, AppError, Command, Paths};
use kizn_core::{SeededRng, SystemClock};

type Shared = Arc<RwLock<Arc<App>>>;

fn open(paths: Paths) -> Result<App, AppError> {
    App::open(paths, Arc::new(SystemClock), Arc::new(SeededRng::from_os()))
}

async fn rpc(State(app): State<Shared>, Json(cmd): Json<Command>) -> Response {
    let app = app.read().unwrap().clone();
    match app.handle(cmd).await {
        Ok(reply) => Json(reply).into_response(),
        Err(e) => {
            let status = match e {
                AppError::Config(_) => StatusCode::UNPROCESSABLE_ENTITY,
                AppError::Store(_) | AppError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(e)).into_response()
        }
    }
}

#[cfg(feature = "test")]
async fn fresh(State(app): State<Shared>) -> Response {
    // A new directory beside the current one, so a run's data stays in the
    // tempdir the test harness gave the bridge.
    let base = app.read().unwrap().paths().data_dir.clone();
    let dir = base.with_file_name(format!("fresh-{}", kizn_core::new_id(&SystemClock, &SeededRng::from_os())));
    match open(Paths::in_dir(&dir)) {
        Ok(next) => {
            *app.write().unwrap() = Arc::new(next);
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

fn inspect(what: Option<&str>) -> Result<(), String> {
    match what {
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
        other => Err(format!("inspect what? {other:?}: db (bank, progress and attempts come with later phases)")),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => serve().await,
        Some("paths") => paths(),
        Some("inspect") => {
            if let Err(e) = inspect(args.get(1).map(String::as_str)) {
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

async fn serve() {
    let app = match open(Paths::from_env()) {
        Ok(app) => Arc::new(app),
        Err(e) => {
            eprintln!("kizn-bridge: {e}");
            std::process::exit(1);
        }
    };
    let port = std::env::var("KIZN_BRIDGE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7878);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kizn-bridge: can't listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "kizn-bridge on http://{addr} (data: {}, config: {}){}",
        app.paths().data_dir.display(),
        app.paths().config_dir.display(),
        if cfg!(feature = "test") { " [test]" } else { "" }
    );
    let router = Router::new().route("/rpc", post(rpc));
    #[cfg(feature = "test")]
    let router = router.route("/test/fresh", post(fresh));
    let router = router.with_state(Arc::new(RwLock::new(app)));
    axum::serve(listener, router).await.unwrap();
}
