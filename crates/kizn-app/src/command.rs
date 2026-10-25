use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything the UI can ask for. One variant per intent; each has exactly
/// one [`Reply`] variant (the pairing is spelled out in `ui/src/rpc.ts`).
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Command {
    GetToday,
    GetAppInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Reply {
    Today(TodayView),
    AppInfo(AppInfo),
}

/// The home screen.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TodayView {
    pub topics: Vec<TopicSummary>,
    /// Whether any role (author, checker, tutor) points at a provider.
    pub model_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TopicSummary {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub config_dir: String,
    pub schema_version: u32,
}

/// What went wrong, in words the UI can show as they are.
#[derive(Debug, Clone, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
#[ts(export)]
pub enum AppError {
    #[error("{0}")]
    Store(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Io(String),
}

impl From<kizn_store::StoreError> for AppError {
    fn from(e: kizn_store::StoreError) -> Self {
        AppError::Store(e.to_string())
    }
}
