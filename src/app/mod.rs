mod config;

use anyhow::Result;

pub use config::{AuthOverrides, ConfigSource, GrafanaConfig, resolve_auth_inputs, save_profile};

use crate::grafana::client::GrafanaClient;

pub struct AppContext {
    pub config: GrafanaConfig,
    pub grafana: GrafanaClient,
}

impl AppContext {
    pub fn from_overrides(auth: &AuthOverrides) -> Result<Self> {
        let config = GrafanaConfig::resolve(auth)?;
        let grafana = GrafanaClient::new(config.clone())?;

        Ok(Self { config, grafana })
    }
}
