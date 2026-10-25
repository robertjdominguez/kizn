use std::path::Path;

use serde::Deserialize;

use crate::AppError;

pub const CONFIG_FILE: &str = "config.toml";

/// `config.toml`. Providers and their checks arrive in phase 2; for now the
/// roles are all that's read. A missing file is an empty config: nothing is
/// written on first run.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub roles: Roles,
}

#[derive(Debug, Default, Deserialize)]
pub struct Roles {
    pub author: Option<String>,
    pub checker: Option<String>,
    pub tutor: Option<String>,
}

impl Config {
    pub fn load(config_dir: &Path) -> Result<Config, AppError> {
        let path = config_dir.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(AppError::Io(format!("couldn't read {}: {e}", path.display()))),
        };
        toml::from_str(&text).map_err(|e| AppError::Config(format!("{} isn't valid: {}", path.display(), e.message())))
    }

    pub fn model_configured(&self) -> bool {
        let r = &self.roles;
        [&r.author, &r.checker, &r.tutor].iter().any(|v| v.is_some())
    }
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
}
