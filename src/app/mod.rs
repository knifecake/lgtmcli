mod config;

use anyhow::Result;

pub use config::GrafanaConfig;

use crate::grafana::client::GrafanaClient;

pub struct AppContext {
    pub config: GrafanaConfig,
    pub grafana: GrafanaClient,
}

impl AppContext {
    pub fn from_env() -> Result<Self> {
        let config = GrafanaConfig::from_env()?;
        let grafana = GrafanaClient::new(config.clone())?;

        Ok(Self { config, grafana })
    }
}
