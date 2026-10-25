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
    GetSettings,
    /// Adds or changes a provider. `key`: absent keeps the current key, an
    /// empty string removes it, anything else goes to the keychain.
    SaveProvider {
        name: String,
        protocol: String,
        base_url: String,
        model: String,
        #[ts(optional)]
        key: Option<String>,
        /// Which Settings preset it came from ("openai", "ollama", …), for
        /// its logo and links. Nothing depends on it.
        #[ts(optional)]
        preset: Option<String>,
    },
    /// The models an endpoint offers, for the model picker. Uses `key` if
    /// given, else the saved key of provider `name`.
    ListModels {
        protocol: String,
        base_url: String,
        #[ts(optional)]
        key: Option<String>,
        #[ts(optional)]
        name: Option<String>,
    },
    RemoveProvider {
        name: String,
    },
    TestProvider {
        name: String,
    },
    AssignRole {
        role: Role,
        #[ts(optional)]
        provider: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Reply {
    Today(TodayView),
    AppInfo(AppInfo),
    Settings(SettingsView),
    Models(Vec<String>),
}

/// What a provider is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Author,
    Checker,
    Tutor,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Author, Role::Checker, Role::Tutor];

    pub fn key(self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::Checker => "checker",
            Role::Tutor => "tutor",
        }
    }
}

/// Settings → Models.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsView {
    pub providers: Vec<ProviderView>,
    pub roles: RolesView,
    /// Advice and warnings, one line each.
    pub notes: Vec<String>,
    pub config_path: String,
    /// When config.toml can't be read, why (and providers is empty).
    pub config_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProviderView {
    pub name: String,
    pub preset: Option<String>,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub key: KeyView,
    /// The last test, if it still applies to this provider as configured.
    pub check: Option<CheckView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum KeyView {
    None,
    Keychain { present: bool },
    Env { var: String, present: bool },
    Refused { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CheckView {
    pub tested_at: String,
    pub reachable: bool,
    /// "native", "prompted" or "none".
    pub structured: String,
    pub streaming: bool,
    pub summary: String,
    pub problems: Vec<String>,
    /// The roles this provider can take, from this test.
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RolesView {
    pub author: Option<String>,
    pub checker: Option<String>,
    pub tutor: Option<String>,
}

/// Pushed to the UI (SSE in the browser loop, Tauri events in the window).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Event {
    /// config.toml changed on disk (by kizn or by hand).
    ConfigChanged,
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
    /// A request kizn won't carry out, and why.
    #[error("{0}")]
    Refused(String),
}

impl From<kizn_store::StoreError> for AppError {
    fn from(e: kizn_store::StoreError) -> Self {
        AppError::Store(e.to_string())
    }
}
