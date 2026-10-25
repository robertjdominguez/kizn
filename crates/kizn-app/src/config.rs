//! `config.toml`: providers and roles. Read with serde; written with
//! toml_edit, so comments and hand edits survive kizn's own writes. A
//! missing file is an empty config, and nothing is written on first run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::AppError;
use crate::command::Role;

pub const CONFIG_FILE: &str = "config.toml";
/// `key = "keychain:kizn/<account>"`.
pub const KEYCHAIN_PREFIX: &str = "keychain:kizn/";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderEntry>,
    #[serde(default)]
    pub roles: Roles,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEntry {
    pub protocol: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub key_env: Option<String>,
    #[serde(default)]
    pub preset: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Roles {
    pub author: Option<String>,
    pub checker: Option<String>,
    pub tutor: Option<String>,
}

impl Roles {
    pub fn get(&self, role: Role) -> Option<&String> {
        match role {
            Role::Author => self.author.as_ref(),
            Role::Checker => self.checker.as_ref(),
            Role::Tutor => self.tutor.as_ref(),
        }
    }
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join(CONFIG_FILE)
}

fn read_text(config_dir: &Path) -> Result<Option<String>, AppError> {
    let path = path(config_dir);
    match std::fs::read_to_string(&path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::Io(format!("couldn't read {}: {e}", path.display()))),
    }
}

impl Config {
    pub fn load(config_dir: &Path) -> Result<Config, AppError> {
        let Some(text) = read_text(config_dir)? else { return Ok(Config::default()) };
        toml::from_str(&text)
            .map_err(|e| AppError::Config(format!("{} isn't valid: {}", path(config_dir).display(), e.message())))
    }

    pub fn model_configured(&self) -> bool {
        let r = &self.roles;
        [&r.author, &r.checker, &r.tutor].iter().any(|v| v.is_some())
    }
}

/// An edit to config.toml that keeps everything else in the file as it was.
pub struct ConfigEdit {
    dir: PathBuf,
    doc: DocumentMut,
}

impl ConfigEdit {
    pub fn open(config_dir: &Path) -> Result<ConfigEdit, AppError> {
        let text = read_text(config_dir)?.unwrap_or_default();
        let doc = text
            .parse::<DocumentMut>()
            .map_err(|e| AppError::Config(format!("{} isn't valid TOML: {e}", path(config_dir).display())))?;
        Ok(ConfigEdit { dir: config_dir.to_path_buf(), doc })
    }

    fn table(&mut self, name: &str) -> &mut Table {
        if !self.doc.contains_table(name) {
            let mut t = Table::new();
            t.set_implicit(name == "providers");
            self.doc.insert(name, Item::Table(t));
        }
        self.doc[name].as_table_mut().expect("a table")
    }

    pub fn set_provider(
        &mut self,
        name: &str,
        protocol: &str,
        base_url: &str,
        model: &str,
        key: Option<KeyRef>,
        preset: Option<&str>,
    ) {
        let providers = self.table("providers");
        if !providers.contains_table(name) {
            providers.insert(name, Item::Table(Table::new()));
        }
        let t = providers[name].as_table_mut().expect("a table");
        t["protocol"] = value(protocol);
        t["base_url"] = value(base_url);
        t["model"] = value(model);
        if let Some(p) = preset {
            t["preset"] = value(p);
        }
        match key {
            Some(KeyRef::Keychain(account)) => {
                t["key"] = value(format!("{KEYCHAIN_PREFIX}{account}"));
                t.remove("key_env");
            }
            Some(KeyRef::None) => {
                t.remove("key");
                t.remove("key_env");
            }
            None => {}
        }
    }

    pub fn remove_provider(&mut self, name: &str) {
        if let Some(t) = self.doc.get_mut("providers").and_then(|p| p.as_table_mut()) {
            t.remove(name);
        }
        for role in Role::ALL {
            if self.doc.get("roles").and_then(|r| r.get(role.key())).and_then(|v| v.as_str()) == Some(name) {
                self.set_role(role, None);
            }
        }
    }

    pub fn set_role(&mut self, role: Role, provider: Option<&str>) {
        let roles = self.table("roles");
        match provider {
            Some(p) => roles[role.key()] = value(p),
            None => {
                roles.remove(role.key());
            }
        }
    }

    /// Writes atomically (a temp file renamed over the old one).
    pub fn save(self) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| AppError::Io(format!("couldn't create {}: {e}", self.dir.display())))?;
        let path = path(&self.dir);
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.doc.to_string())
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| AppError::Io(format!("couldn't write {}: {e}", path.display())))
    }
}

/// What to do with a provider's key on save.
pub enum KeyRef {
    Keychain(String),
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_means_no_model() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!Config::load(dir.path()).unwrap().model_configured());
    }

    #[test]
    fn a_role_means_a_model() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "[roles]\ntutor = \"local\"\n").unwrap();
        assert!(Config::load(dir.path()).unwrap().model_configured());
    }

    #[test]
    fn bad_toml_is_a_plain_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "[roles\n").unwrap();
        let err = Config::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("config.toml isn't valid"), "{err}");
    }

    #[test]
    fn edits_keep_comments_and_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let hand = "# my models\n[providers.gpu]\nprotocol = \"openai\" # the box\nbase_url = \"http://gpu:11434/v1\"\nmodel = \"qwen3:32b\"\n";
        std::fs::write(dir.path().join(CONFIG_FILE), hand).unwrap();
        let mut edit = ConfigEdit::open(dir.path()).unwrap();
        edit.set_provider(
            "claude",
            "anthropic",
            "https://api.anthropic.com",
            "claude-sonnet-5",
            Some(KeyRef::Keychain("claude".into())),
            Some("anthropic"),
        );
        edit.set_role(Role::Author, Some("gpu"));
        edit.save().unwrap();
        let text = std::fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        assert!(text.contains("# my models") && text.contains("# the box"), "{text}");
        assert!(text.contains("key = \"keychain:kizn/claude\""), "{text}");
        let c = Config::load(dir.path()).unwrap();
        assert_eq!(c.providers.len(), 2);
        assert_eq!(c.roles.author.as_deref(), Some("gpu"));
        let mut edit = ConfigEdit::open(dir.path()).unwrap();
        edit.remove_provider("gpu");
        edit.save().unwrap();
        let c = Config::load(dir.path()).unwrap();
        assert!(c.roles.author.is_none() && !c.providers.contains_key("gpu"));
    }
}
