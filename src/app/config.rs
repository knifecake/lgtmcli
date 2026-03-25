use std::env;

use anyhow::{Context, Result, bail};

#[derive(Clone)]
pub struct GrafanaConfig {
    pub base_url: String,
    pub token: String,
}

impl GrafanaConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            base_url: required_env("GRAFANA_URL")?,
            token: required_env("GRAFANA_TOKEN")?,
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!("missing required env var {name}"))?;
    if value.trim().is_empty() {
        bail!("env var {name} is set but empty");
    }
    Ok(value)
}
