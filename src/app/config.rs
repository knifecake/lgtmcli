use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const PROFILE_SCHEMA_VERSION: u32 = 1;
const DEFAULT_PROFILE_NAME: &str = "default";

#[derive(Clone, Debug, Default)]
pub struct AuthOverrides {
    pub base_url: Option<String>,
    pub token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GrafanaConfig {
    pub base_url: String,
    pub token: String,
    pub url_source: ConfigSource,
    pub token_source: ConfigSource,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    Flag,
    Env,
    Profile,
}

impl ConfigSource {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Flag => "--url/--token flag",
            Self::Env => "environment variable",
            Self::Profile => "saved profile",
        }
    }
}

#[derive(Debug)]
pub struct ResolvedAuthInputs {
    pub base_url: Option<ResolvedValue>,
    pub token: Option<ResolvedValue>,
}

#[derive(Debug)]
pub struct ResolvedValue {
    pub value: String,
    pub source: ConfigSource,
}

impl ResolvedAuthInputs {
    pub fn into_required(self) -> Result<GrafanaConfig> {
        let base_url = self.base_url.ok_or_else(|| {
            anyhow::anyhow!(
                "missing Grafana URL. Provide --url, set GRAFANA_URL, or run `lgtmcli auth login`."
            )
        })?;

        let token = self.token.ok_or_else(|| {
            anyhow::anyhow!(
                "missing Grafana token. Provide --token, set GRAFANA_TOKEN, or run `lgtmcli auth login`."
            )
        })?;

        Ok(GrafanaConfig {
            base_url: base_url.value,
            token: token.value,
            url_source: base_url.source,
            token_source: token.source,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ProfilesFile {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default = "default_profile_name")]
    active_profile: String,
    #[serde(default)]
    profiles: BTreeMap<String, StoredProfile>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredProfile {
    grafana_url: String,
    grafana_token: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyProfileFile {
    grafana_url: String,
    grafana_token: String,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl GrafanaConfig {
    pub fn resolve(overrides: &AuthOverrides) -> Result<Self> {
        resolve_auth_inputs(overrides)?.into_required()
    }
}

pub fn resolve_auth_inputs(overrides: &AuthOverrides) -> Result<ResolvedAuthInputs> {
    let profile = load_saved_profile()?;

    let base_url = resolve_value(
        overrides.base_url.clone(),
        "--url",
        "GRAFANA_URL",
        profile.as_ref().map(|p| p.grafana_url.clone()),
    )?;

    let token = resolve_value(
        overrides.token.clone(),
        "--token",
        "GRAFANA_TOKEN",
        profile.as_ref().map(|p| p.grafana_token.clone()),
    )?;

    Ok(ResolvedAuthInputs { base_url, token })
}

pub fn save_profile(base_url: &str, token: &str) -> Result<PathBuf> {
    let path = profile_path().ok_or_else(|| {
        anyhow::anyhow!(
            "could not determine profile path. Set XDG_CONFIG_HOME or HOME to use `auth login`."
        )
    })?;

    let parent = path
        .parent()
        .context("failed to determine profile parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create profile directory {}", parent.display()))?;

    let mut profiles = load_profiles_file()?.unwrap_or_default();
    if profiles.schema_version < PROFILE_SCHEMA_VERSION {
        profiles.schema_version = PROFILE_SCHEMA_VERSION;
    }

    if profiles.active_profile.trim().is_empty() {
        profiles.active_profile = DEFAULT_PROFILE_NAME.to_string();
    }

    profiles.profiles.insert(
        profiles.active_profile.clone(),
        StoredProfile::new(base_url.to_string(), token.to_string())?,
    );

    let payload =
        serde_json::to_string_pretty(&profiles).context("failed to encode profile JSON")?;
    write_profile_payload(&path, &payload)?;

    Ok(path)
}

pub fn profile_path() -> Option<PathBuf> {
    profile_dir().map(|dir| dir.join("lgtmcli").join("profiles.json"))
}

fn legacy_profile_path() -> Option<PathBuf> {
    profile_dir().map(|dir| dir.join("lgtmcli").join("profile.json"))
}

fn profile_dir() -> Option<PathBuf> {
    if let Some(path) = non_empty_env_path("XDG_CONFIG_HOME") {
        return Some(path);
    }

    env::var_os("HOME").and_then(|home| {
        if home.is_empty() {
            None
        } else {
            Some(PathBuf::from(home).join(".config"))
        }
    })
}

fn non_empty_env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
}

fn load_saved_profile() -> Result<Option<StoredProfile>> {
    let Some(profiles) = load_profiles_file()? else {
        return Ok(None);
    };

    if let Some(active) = profiles.profiles.get(&profiles.active_profile) {
        return Ok(Some(active.clone()));
    }

    Ok(profiles.profiles.values().next().cloned())
}

fn load_profiles_file() -> Result<Option<ProfilesFile>> {
    let path = match select_existing_profile_path() {
        Some(path) => path,
        None => return Ok(None),
    };

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read profile file {}", path.display()))?;

    let parsed_profiles = serde_json::from_str::<ProfilesFile>(&raw);
    match parsed_profiles {
        Ok(file) => Ok(Some(file.sanitize()?)),
        Err(profiles_err) => {
            let parsed_legacy = serde_json::from_str::<LegacyProfileFile>(&raw);
            match parsed_legacy {
                Ok(legacy) => Ok(Some(ProfilesFile::from_legacy(legacy)?.sanitize()?)),
                Err(legacy_err) => {
                    bail!(
                        "failed to parse profile file {} (profiles schema error: {}; legacy schema error: {})",
                        path.display(),
                        profiles_err,
                        legacy_err
                    )
                }
            }
        }
    }
}

fn select_existing_profile_path() -> Option<PathBuf> {
    let primary = profile_path()?;
    if primary.exists() {
        return Some(primary);
    }

    let legacy = legacy_profile_path()?;
    if legacy.exists() {
        return Some(legacy);
    }

    None
}

impl ProfilesFile {
    fn sanitize(mut self) -> Result<Self> {
        if self.schema_version == 0 {
            self.schema_version = PROFILE_SCHEMA_VERSION;
        }

        if self.active_profile.trim().is_empty() {
            self.active_profile = DEFAULT_PROFILE_NAME.to_string();
        } else {
            self.active_profile = self.active_profile.trim().to_string();
        }

        let mut sanitized = BTreeMap::new();
        for (name, profile) in self.profiles {
            let normalized_name = normalize_profile_name(name);
            sanitized.insert(normalized_name, profile.sanitize()?);
        }

        self.profiles = sanitized;

        if !self.profiles.contains_key(&self.active_profile) {
            if self.profiles.contains_key(DEFAULT_PROFILE_NAME) {
                self.active_profile = DEFAULT_PROFILE_NAME.to_string();
            } else if let Some(first_profile_name) = self.profiles.keys().next().cloned() {
                self.active_profile = first_profile_name;
            }
        }

        Ok(self)
    }

    fn from_legacy(legacy: LegacyProfileFile) -> Result<Self> {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            DEFAULT_PROFILE_NAME.to_string(),
            StoredProfile::new(legacy.grafana_url, legacy.grafana_token)?,
        );

        Ok(Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            active_profile: DEFAULT_PROFILE_NAME.to_string(),
            profiles,
            extra: BTreeMap::new(),
        })
    }
}

impl StoredProfile {
    fn new(grafana_url: String, grafana_token: String) -> Result<Self> {
        Ok(Self {
            grafana_url: sanitize_non_empty(grafana_url, "Grafana URL")?,
            grafana_token: sanitize_non_empty(grafana_token, "Grafana token")?,
            extra: BTreeMap::new(),
        })
    }

    fn sanitize(mut self) -> Result<Self> {
        self.grafana_url = sanitize_non_empty(self.grafana_url, "saved profile grafana_url")?;
        self.grafana_token = sanitize_non_empty(self.grafana_token, "saved profile grafana_token")?;
        Ok(self)
    }
}

fn resolve_value(
    flag_value: Option<String>,
    flag_name: &str,
    env_name: &str,
    profile_value: Option<String>,
) -> Result<Option<ResolvedValue>> {
    if let Some(value) = flag_value {
        return Ok(Some(ResolvedValue {
            value: sanitize_non_empty(value, flag_name)?,
            source: ConfigSource::Flag,
        }));
    }

    if let Some(value) = read_env(env_name)? {
        return Ok(Some(ResolvedValue {
            value,
            source: ConfigSource::Env,
        }));
    }

    if let Some(value) = profile_value {
        return Ok(Some(ResolvedValue {
            value: sanitize_non_empty(value, "saved profile")?,
            source: ConfigSource::Profile,
        }));
    }

    Ok(None)
}

fn read_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(sanitize_non_empty(value, name)?)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            bail!("environment variable {name} contains non-UTF-8 data")
        }
    }
}

fn normalize_profile_name(value: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        DEFAULT_PROFILE_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_non_empty(value: String, field_name: &str) -> Result<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        bail!("{field_name} is set but empty");
    }
    Ok(trimmed)
}

#[cfg(unix)]
fn write_profile_payload(path: &Path, payload: &str) -> Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to open profile file {}", path.display()))?;

    file.write_all(payload.as_bytes())
        .with_context(|| format!("failed to write profile file {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to sync profile file {}", path.display()))?;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set profile file permissions {}", path.display()))?;

    Ok(())
}

#[cfg(not(unix))]
fn write_profile_payload(path: &Path, payload: &str) -> Result<()> {
    fs::write(path, payload)
        .with_context(|| format!("failed to write profile file {}", path.display()))
}

fn default_schema_version() -> u32 {
    PROFILE_SCHEMA_VERSION
}

fn default_profile_name() -> String {
    DEFAULT_PROFILE_NAME.to_string()
}
