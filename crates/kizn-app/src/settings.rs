//! Settings → Models: providers, their connection tests, and roles.
//!
//! A provider's last test is stored with a fingerprint of what was tested
//! (protocol, URL, model, key reference). Change any of them, in Settings or
//! by hand, and the test no longer applies: roles can't be confirmed on an
//! untested provider.

use kizn_core::sql_time;
use kizn_model::{Check, Client, Endpoint, Protocol, Structured, probe};
use kizn_store::CheckRow;

use crate::command::{CheckView, KeyView, ProviderView, Role, RolesView, SettingsView};
use crate::config::{self, Config, ConfigEdit, KEYCHAIN_PREFIX, KeyRef, ProviderEntry};
use crate::{App, AppError};

impl App {
    pub(crate) fn settings(&self) -> Result<SettingsView, AppError> {
        let config_path = config::path(&self.paths.config_dir).display().to_string();
        let config = match Config::load(&self.paths.config_dir) {
            Ok(c) => c,
            Err(e) => {
                return Ok(SettingsView {
                    providers: vec![],
                    roles: RolesView::default(),
                    notes: vec![],
                    config_path,
                    config_error: Some(e.to_string()),
                });
            }
        };
        let checks = self.store.checks()?;
        let mut notes = vec![];
        let providers = config
            .providers
            .iter()
            .map(|(name, p)| {
                let key = self.key_view(name, p);
                if let KeyView::Env { var, .. } = &key {
                    notes.push(format!(
                        "{name}'s key comes from the environment variable {var}, not the keychain. That works, but it's \
                         only as private as your environment."
                    ));
                }
                let check = checks
                    .iter()
                    .find(|c| &c.provider == name && c.fingerprint == fingerprint(p))
                    .and_then(|c| Some((c, serde_json::from_str::<Check>(&c.check_json).ok()?)))
                    .map(|(row, check)| check_view(&row.tested_at, &check));
                ProviderView {
                    name: name.clone(),
                    preset: p.preset.clone(),
                    protocol: p.protocol.clone(),
                    base_url: base_url(p),
                    model: p.model.clone(),
                    key,
                    check,
                }
            })
            .collect();
        let r = &config.roles;
        if r.author.is_some() && r.author == r.checker {
            notes.push(
                "The checker is the same model as the author. A different model checks items more independently: it \
                 doesn't share the author's blind spots, so it catches more ambiguous items and wrong keys."
                    .into(),
            );
        }
        Ok(SettingsView {
            providers,
            roles: RolesView { author: r.author.clone(), checker: r.checker.clone(), tutor: r.tutor.clone() },
            notes,
            config_path,
            config_error: None,
        })
    }

    fn key_view(&self, name: &str, p: &ProviderEntry) -> KeyView {
        match (&p.key, &p.key_env) {
            (Some(k), _) if k.starts_with(KEYCHAIN_PREFIX) => {
                let account = &k[KEYCHAIN_PREFIX.len()..];
                KeyView::Keychain { present: matches!(self.deps.secrets.get(account), Ok(Some(_))) }
            }
            (Some(_), _) => KeyView::Refused {
                reason: format!(
                    "config.toml has a key written into it for {name}. kizn only reads keys from the keychain \
                     (key = \"{KEYCHAIN_PREFIX}{name}\") or an environment variable (key_env)."
                ),
            },
            (None, Some(var)) => KeyView::Env { var: var.clone(), present: std::env::var_os(var).is_some() },
            (None, None) => KeyView::None,
        }
    }

    pub(crate) fn save_provider(
        &self,
        name: &str,
        protocol: &str,
        base_url: &str,
        model: &str,
        key: Option<&str>,
        preset: Option<&str>,
    ) -> Result<SettingsView, AppError> {
        validate_name(name)?;
        let protocol = parse_protocol(protocol)?;
        let model = model.trim();
        if model.is_empty() {
            return Err(AppError::Refused("a provider needs a model id (for example qwen3:32b)".into()));
        }
        let base_url = match base_url.trim() {
            "" => protocol.default_base_url().to_string(),
            u if u.starts_with("http://") || u.starts_with("https://") => u.to_string(),
            u => return Err(AppError::Refused(format!("{u} isn't a URL: it should start with http:// or https://"))),
        };
        let key_ref = match key.map(str::trim) {
            None => None,
            Some("") => {
                self.deps.secrets.delete(name).map_err(keychain_error)?;
                Some(KeyRef::None)
            }
            Some(k) => {
                self.deps.secrets.set(name, k).map_err(keychain_error)?;
                Some(KeyRef::Keychain(name.to_string()))
            }
        };
        let mut edit = ConfigEdit::open(&self.paths.config_dir)?;
        edit.set_provider(name, protocol_name(protocol), &base_url, model, key_ref, preset);
        edit.save()?;
        self.settings()
    }

    pub(crate) fn remove_provider(&self, name: &str) -> Result<SettingsView, AppError> {
        let mut edit = ConfigEdit::open(&self.paths.config_dir)?;
        edit.remove_provider(name);
        edit.save()?;
        self.store.delete_check(name)?;
        self.deps.secrets.delete(name).map_err(keychain_error)?;
        self.settings()
    }

    pub(crate) async fn list_models(
        &self,
        protocol: &str,
        base_url: &str,
        key: Option<&str>,
        name: Option<&str>,
    ) -> Result<Vec<String>, AppError> {
        let protocol = parse_protocol(protocol)?;
        let key = match (key.map(str::trim).filter(|k| !k.is_empty()), name) {
            (Some(k), _) => Some(k.to_string()),
            (None, Some(n)) => {
                let config = Config::load(&self.paths.config_dir)?;
                match config.providers.get(n) {
                    Some(p) => self.resolve_key(n, p)?,
                    None => None,
                }
            }
            (None, None) => None,
        };
        let base_url = match base_url.trim() {
            "" => protocol.default_base_url().to_string(),
            u => u.to_string(),
        };
        let endpoint = Endpoint { protocol, base_url, model: String::new(), headers: vec![], key };
        let mut models = Client::new(endpoint, self.deps.http.clone())
            .with_timeout(std::time::Duration::from_secs(10))
            .list_models_strict()
            .await
            .map_err(|e| AppError::Refused(e.to_string()))?;
        models.sort();
        Ok(models)
    }

    pub(crate) async fn test_provider(&self, name: &str) -> Result<SettingsView, AppError> {
        let config = Config::load(&self.paths.config_dir)?;
        let p = config
            .providers
            .get(name)
            .ok_or_else(|| AppError::Refused(format!("there's no provider called {name}")))?;
        let endpoint = Endpoint {
            protocol: parse_protocol(&p.protocol)?,
            base_url: base_url(p),
            model: p.model.clone(),
            headers: p.headers.clone().into_iter().collect(),
            key: self.resolve_key(name, p)?,
        };
        let check = probe(&Client::new(endpoint, self.deps.http.clone())).await;
        self.store.save_check(&CheckRow {
            provider: name.to_string(),
            fingerprint: fingerprint(p),
            tested_at: sql_time(self.deps.clock.now()),
            check_json: serde_json::to_string(&check).expect("a Check serialises"),
        })?;
        // Roles nobody has yet go to the first provider that can take them.
        let allowed = roles_for(&check);
        let unset: Vec<Role> =
            Role::ALL.into_iter().filter(|r| config.roles.get(*r).is_none() && allowed.contains(r)).collect();
        if !unset.is_empty() {
            let mut edit = ConfigEdit::open(&self.paths.config_dir)?;
            for role in unset {
                edit.set_role(role, Some(name));
            }
            edit.save()?;
        }
        self.settings()
    }

    pub(crate) fn assign_role(&self, role: Role, provider: Option<&str>) -> Result<SettingsView, AppError> {
        if let Some(name) = provider {
            let view = self.settings()?;
            let p = view
                .providers
                .iter()
                .find(|p| p.name == name)
                .ok_or_else(|| AppError::Refused(format!("there's no provider called {name}")))?;
            match &p.check {
                None => {
                    return Err(AppError::Refused(format!(
                        "{name} hasn't been tested as it's set up now. Test the connection first."
                    )));
                }
                Some(c) if !c.roles.contains(&role) => {
                    return Err(AppError::Refused(format!("{name} can't be the {}: {}.", role.key(), c.summary)));
                }
                Some(_) => {}
            }
        }
        let mut edit = ConfigEdit::open(&self.paths.config_dir)?;
        edit.set_role(role, provider);
        edit.save()?;
        self.settings()
    }

    fn resolve_key(&self, name: &str, p: &ProviderEntry) -> Result<Option<String>, AppError> {
        match self.key_view(name, p) {
            KeyView::None => Ok(None),
            KeyView::Refused { reason } => Err(AppError::Refused(reason)),
            KeyView::Env { var, .. } => std::env::var(&var).map(Some).map_err(|_| {
                AppError::Refused(format!("{name}'s key should be in the environment variable {var}, which isn't set"))
            }),
            KeyView::Keychain { .. } => {
                let account = &p.key.as_deref().unwrap_or_default()[KEYCHAIN_PREFIX.len()..];
                match self.deps.secrets.get(account).map_err(keychain_error)? {
                    Some(k) => Ok(Some(k)),
                    None => Err(AppError::Refused(format!(
                        "{name}'s key isn't in the keychain any more. Add it again in Settings."
                    ))),
                }
            }
        }
    }
}

/// What a provider can do, from its test: authoring and checking need JSON,
/// tutoring needs streaming.
pub fn roles_for(check: &Check) -> Vec<Role> {
    let mut roles = vec![];
    if check.reachable && check.structured != Structured::None {
        roles.extend([Role::Author, Role::Checker]);
    }
    if check.reachable && check.streaming {
        roles.push(Role::Tutor);
    }
    roles
}

fn check_view(tested_at: &str, check: &Check) -> CheckView {
    CheckView {
        tested_at: tested_at.to_string(),
        reachable: check.reachable,
        structured: match check.structured {
            Structured::Native => "native",
            Structured::Prompted => "prompted",
            Structured::None => "none",
        }
        .into(),
        streaming: check.streaming,
        summary: check.summary.clone(),
        problems: check.problems.clone(),
        roles: roles_for(check),
    }
}

fn fingerprint(p: &ProviderEntry) -> String {
    let headers: Vec<String> = p.headers.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!(
        "{}|{}|{}|{}|{}|{}",
        p.protocol,
        base_url(p),
        p.model,
        p.key.as_deref().unwrap_or(""),
        p.key_env.as_deref().unwrap_or(""),
        headers.join(",")
    )
}

fn base_url(p: &ProviderEntry) -> String {
    match &p.base_url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => parse_protocol(&p.protocol).map(|pr| pr.default_base_url().to_string()).unwrap_or_default(),
    }
}

fn parse_protocol(s: &str) -> Result<Protocol, AppError> {
    match s.trim() {
        "openai" => Ok(Protocol::OpenAi),
        "anthropic" => Ok(Protocol::Anthropic),
        other => Err(AppError::Refused(format!(
            "kizn speaks two protocols, \"openai\" (OpenAI-compatible: Ollama, llama.cpp, vLLM, LM Studio, OpenRouter, \
             OpenAI, Gemini) and \"anthropic\"; \"{other}\" isn't one of them"
        ))),
    }
}

fn protocol_name(p: Protocol) -> &'static str {
    match p {
        Protocol::OpenAi => "openai",
        Protocol::Anthropic => "anthropic",
    }
}

fn validate_name(name: &str) -> Result<(), AppError> {
    let ok = !name.is_empty()
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(AppError::Refused(format!(
            "\"{name}\" can't be a provider name: use lowercase letters, digits, - and _ (for example local or claude)"
        )))
    }
}

fn keychain_error(e: String) -> AppError {
    AppError::Refused(format!(
        "couldn't use the system keychain ({e}). You can put the key in an environment variable instead and set key_env \
         in config.toml."
    ))
}
