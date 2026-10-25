//! The @install layer: the real install scripts (`docs/install.sh`,
//! `docs/install.ps1`) run against a staged release served on localhost,
//! with a throwaway home directory. Only scenarios tagged for this OS run.
//!
//! Stubs by default; `KIZN_STAGE_ASSETS=<dir>` stages real bundles instead
//! (the CI install job). `KIZN_BDD_TAG` narrows the run further.

use std::path::{Path, PathBuf};
use std::process::Command;

use cucumber::{World, given, then, when};
use kizn_install_tests::{self as stage, Stage};

const OS_TAG: &str = if cfg!(target_os = "linux") {
    "linux"
} else if cfg!(target_os = "macos") {
    "macos"
} else {
    "windows"
};

#[derive(Debug, World)]
#[world(init = Self::new)]
struct Install {
    home: tempfile::TempDir,
    stage: Stage,
    _staged: tempfile::TempDir,
    base: String,
    out: Option<std::process::Output>,
}

/// Where this OS's install puts things, under the throwaway home.
struct Layout {
    app: PathBuf,
    /// The file compared byte for byte with the staged release.
    binary: PathBuf,
    launcher: PathBuf,
    data: PathBuf,
    config: PathBuf,
}

impl Install {
    async fn new() -> Self {
        let staged = tempfile::tempdir().unwrap();
        let root = staged.path().to_path_buf();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let serve_root = root.clone();
        tokio::spawn(async move {
            let app = axum::Router::new().fallback_service(tower_http::services::ServeDir::new(serve_root));
            axum::serve(listener, app).await.unwrap();
        });
        let real = std::env::var_os("KIZN_STAGE_ASSETS").map(PathBuf::from);
        Install { home: tempfile::tempdir().unwrap(), stage: Stage { root, real }, _staged: staged, base, out: None }
    }

    fn home(&self) -> &Path {
        self.home.path()
    }

    fn layout(&self) -> Layout {
        let h = self.home();
        match OS_TAG {
            "linux" => Layout {
                app: h.join(".local/bin/kizn"),
                binary: h.join(".local/bin/kizn"),
                launcher: h.join(".local/share/applications/kizn.desktop"),
                data: h.join(".local/share/kizn"),
                config: h.join(".config/kizn"),
            },
            "macos" => Layout {
                app: h.join("Applications/kizn.app"),
                binary: h.join("Applications").join(stage::MACOS_BINARY),
                launcher: h.join("Applications/kizn.app/Contents/Info.plist"),
                data: h.join("Library/Application Support/kizn"),
                config: h.join("Library/Application Support/kizn"),
            },
            _ => Layout {
                app: h.join("AppData/Local/Programs/kizn"),
                binary: h.join("AppData/Local/Programs/kizn").join(stage::WINDOWS_BINARY),
                launcher: h.join("AppData/Roaming/Microsoft/Windows/Start Menu/Programs/kizn.lnk"),
                data: h.join("AppData/Roaming/kizn/data"),
                config: h.join("AppData/Roaming/kizn/config"),
            },
        }
    }

    /// The bytes the install should have put at `layout().binary` for `version`.
    fn expected_binary(&self, version: &str) -> Vec<u8> {
        let dir = self.stage.version_dir(version);
        match OS_TAG {
            "linux" => std::fs::read(dir.join(stage::LINUX)).unwrap(),
            "macos" => stage::archive_member(&dir.join(stage::MACOS), stage::MACOS_BINARY).unwrap(),
            _ => stage::archive_member(&dir.join(stage::WINDOWS), stage::WINDOWS_BINARY).unwrap(),
        }
    }

    fn this_os_asset() -> &'static str {
        match OS_TAG {
            "linux" => stage::LINUX,
            "macos" => stage::MACOS,
            _ => stage::WINDOWS,
        }
    }

    /// Runs the real script with the scenario's flags (`--version X`,
    /// `--uninstall`, `--purge`), translated for PowerShell on Windows.
    fn run(&mut self, flags: &[&str]) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        let mut cmd = if OS_TAG == "windows" {
            let mut c = Command::new("pwsh");
            c.args(["-NoProfile", "-NonInteractive", "-File"]).arg(root.join("install.ps1"));
            for f in flags {
                c.arg(match *f {
                    "--version" => "-Version",
                    "--uninstall" => "-Uninstall",
                    "--purge" => "-Purge",
                    other => other,
                });
            }
            c.env("USERPROFILE", self.home())
                .env("LOCALAPPDATA", self.home().join("AppData/Local"))
                .env("APPDATA", self.home().join("AppData/Roaming"));
            c
        } else {
            let mut c = Command::new("sh");
            c.arg(root.join("install.sh")).args(flags);
            c.env_remove("XDG_DATA_HOME").env_remove("XDG_CONFIG_HOME");
            c
        };
        cmd.env("HOME", self.home()).env("KIZN_RELEASE_BASE", &self.base).env_remove("KIZN_VERSION");
        self.out = Some(cmd.output().expect("couldn't run the install script"));
    }

    fn out(&self) -> &std::process::Output {
        self.out.as_ref().expect("the install script hasn't run")
    }

    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.out().stdout).into_owned()
    }

    fn transcript(&self) -> String {
        format!("--- stdout\n{}--- stderr\n{}", self.stdout(), String::from_utf8_lossy(&self.out().stderr))
    }

    fn assert_succeeded(&self) {
        assert!(self.out().status.success(), "the install script failed:\n{}", self.transcript());
    }
}

// ---- given ---------------------------------------------------------------------

#[given(regex = r"^a staged release of kizn v(\S+)$")]
fn staged_release(w: &mut Install, version: String) {
    w.stage.release(&version, true).unwrap();
}

#[given(regex = r"^a staged prerelease of kizn v(\S+)$")]
fn staged_prerelease(w: &mut Install, version: String) {
    w.stage.release(&version, false).unwrap();
}

#[given("the release's checksum doesn't match its download")]
fn tampered(w: &mut Install) {
    w.stage.tamper(Install::this_os_asset()).unwrap();
}

#[given("kizn was installed with the script")]
fn installed(w: &mut Install) {
    w.run(&[]);
    w.assert_succeeded();
}

#[given("I have kizn data")]
fn have_data(w: &mut Install) {
    let l = w.layout();
    for (dir, file) in [(&l.data, "kizn.db"), (&l.config, "config.toml")] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(file), "mine").unwrap();
    }
}

// ---- when ----------------------------------------------------------------------

#[when("I run the install script")]
fn run_install(w: &mut Install) {
    w.run(&[]);
}

#[when(regex = r"^I run the install script asking for (\S+)$")]
fn run_install_version(w: &mut Install, version: String) {
    w.run(&["--version", &version]);
}

#[when(regex = r"^I run the install script with (--uninstall(?: --purge)?)$")]
fn run_uninstall(w: &mut Install, flags: String) {
    let flags: Vec<&str> = flags.split(' ').collect();
    w.run(&flags);
}

// ---- then ----------------------------------------------------------------------

#[then("the checksum is verified")]
fn checksum_verified(w: &mut Install) {
    w.assert_succeeded();
    assert!(w.stdout().contains("checksum verified"), "no checksum line:\n{}", w.transcript());
}

#[then(regex = r"^kizn v(\S+) is installed under my home directory$")]
fn installed_version(w: &mut Install, version: String) {
    let l = w.layout();
    let got = std::fs::read(&l.binary).unwrap_or_else(|e| panic!("{}: {e}\n{}", l.binary.display(), w.transcript()));
    assert!(got == w.expected_binary(&version), "{} isn't v{version}'s build", l.binary.display());
    assert!(w.stdout().contains(&format!("kizn v{version} is installed")), "{}", w.transcript());
}

#[then("kizn appears in my launcher")]
fn in_launcher(w: &mut Install) {
    let l = w.layout();
    assert!(l.launcher.exists(), "no launcher entry at {}", l.launcher.display());
    if OS_TAG == "linux" {
        let entry = std::fs::read_to_string(&l.launcher).unwrap();
        assert!(entry.contains(&format!("Exec=\"{}\"", l.app.display())), "launcher doesn't run kizn:\n{entry}");
    }
}

#[then("the data directory is reported")]
fn data_reported(w: &mut Install) {
    let data = w.layout().data.display().to_string();
    let data = if OS_TAG == "windows" { data.replace('/', "\\") } else { data };
    assert!(w.stdout().contains(&data), "{data} isn't reported:\n{}", w.transcript());
}

#[then("I'm told the download failed verification")]
fn told_failed(w: &mut Install) {
    assert!(!w.out().status.success(), "the install script succeeded:\n{}", w.transcript());
    assert!(w.transcript().contains("failed verification"), "{}", w.transcript());
}

#[then("nothing is installed")]
fn nothing_installed(w: &mut Install) {
    let l = w.layout();
    for p in [&l.app, &l.launcher] {
        assert!(!p.exists(), "{} exists", p.display());
    }
}

#[then("kizn is no longer installed")]
fn uninstalled(w: &mut Install) {
    w.assert_succeeded();
    assert!(!w.layout().app.exists(), "{} is still there", w.layout().app.display());
}

#[then("kizn no longer appears in my launcher")]
fn not_in_launcher(w: &mut Install) {
    assert!(!w.layout().launcher.exists(), "{} is still there", w.layout().launcher.display());
}

#[then("my kizn data is still there")]
fn data_kept(w: &mut Install) {
    let l = w.layout();
    assert!(l.data.join("kizn.db").exists() && l.config.join("config.toml").exists(), "data was removed");
}

#[then("my kizn data is gone")]
fn data_gone(w: &mut Install) {
    let l = w.layout();
    assert!(!l.data.exists() && !l.config.exists(), "data is still there");
}

#[tokio::main]
async fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../features");
    let extra = std::env::var("KIZN_BDD_TAG").ok().filter(|t| !t.is_empty());
    let extra = extra.map(|t| t.trim_start_matches('@').to_string());
    Install::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit(features, move |feature, rule, scenario| {
            let tags: Vec<&String> =
                feature.tags.iter().chain(rule.map(|r| &r.tags).into_iter().flatten()).chain(&scenario.tags).collect();
            let has = |t: &str| tags.iter().any(|x| x.as_str() == t);
            has("install") && has(OS_TAG) && extra.as_deref().is_none_or(has)
        })
        .await;
}
