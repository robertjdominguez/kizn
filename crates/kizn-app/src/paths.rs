use std::path::PathBuf;

/// Where kizn keeps data and config. `KIZN_DATA_DIR` and `KIZN_CONFIG_DIR`
/// override the OS-standard locations (dev and tests use them).
#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> Paths {
        let dirs = directories::ProjectDirs::from("", "", "kizn");
        let pick = |var: &str, os: Option<PathBuf>| {
            std::env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .or(os)
                .unwrap_or_else(|| PathBuf::from(".kizn"))
        };
        Paths {
            data_dir: pick("KIZN_DATA_DIR", dirs.as_ref().map(|d| d.data_dir().to_path_buf())),
            config_dir: pick("KIZN_CONFIG_DIR", dirs.as_ref().map(|d| d.config_dir().to_path_buf())),
        }
    }

    /// Data and config in one directory: dev and tests.
    pub fn in_dir(dir: impl Into<PathBuf>) -> Paths {
        let dir = dir.into();
        Paths { data_dir: dir.clone(), config_dir: dir }
    }
}
