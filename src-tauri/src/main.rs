//! The desktop shell. One IPC command, `rpc`, hands a Command to kizn-app;
//! the window never talks to anything else. No listening ports.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use kizn_app::{App, AppError, Command, Paths, Reply};
use kizn_core::{SeededRng, SystemClock};

#[tauri::command]
async fn rpc(app: tauri::State<'_, App>, cmd: Command) -> Result<Reply, AppError> {
    app.handle(cmd).await
}

fn main() {
    let app = match App::open(Paths::from_env(), Arc::new(SystemClock), Arc::new(SeededRng::from_os())) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("kizn: {e}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .manage(app)
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("kizn couldn't start its window");
}
