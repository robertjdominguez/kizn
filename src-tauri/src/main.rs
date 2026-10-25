//! The desktop shell. One IPC command, `rpc`, hands a Command to kizn-app,
//! and kizn-app's Events go to the window as `kizn` events. The window never
//! talks to anything else. No listening ports.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use kizn_app::{App, AppDeps, AppError, Command, Paths, Reply};
use tauri::{Emitter, Manager};

#[tauri::command]
async fn rpc(app: tauri::State<'_, Arc<App>>, cmd: Command) -> Result<Reply, AppError> {
    app.handle(cmd).await
}

fn main() {
    let app = match App::open(Paths::from_env(), AppDeps::system()) {
        Ok(app) => Arc::new(app),
        Err(e) => {
            eprintln!("kizn: {e}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app)
        .setup(|tauri_app| {
            let handle = tauri_app.handle().clone();
            let mut events = tauri_app.state::<Arc<App>>().subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(ev) = events.recv().await {
                    let _ = handle.emit("kizn", ev);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("kizn couldn't start its window");
}
