use std::io::{self, Write};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use crate::app::{
    AppContext, AuthOverrides, ConfigSource, GrafanaConfig, resolve_auth_inputs, save_profile,
};
use crate::grafana::client::GrafanaClient;
use crate::output::TableOutput;

#[derive(Debug, Serialize)]
pub struct AuthStatusResult {
    pub ok: bool,
    pub grafana_url: String,
    pub visible_datasources: usize,
    pub url_source: ConfigSource,
    pub token_source: ConfigSource,
}

#[derive(Debug, Serialize)]
pub struct AuthLoginResult {
    pub ok: bool,
    pub grafana_url: String,
    pub profile_path: String,
    pub verified: bool,
    pub visible_datasources: Option<usize>,
}

pub fn status(ctx: &AppContext) -> Result<AuthStatusResult> {
    let datasources = ctx.grafana.fetch_datasources()?;

    Ok(AuthStatusResult {
        ok: true,
        grafana_url: ctx.config.base_url.clone(),
        visible_datasources: datasources.len(),
        url_source: ctx.config.url_source,
        token_source: ctx.config.token_source,
    })
}

pub fn login(overrides: &AuthOverrides, no_verify: bool) -> Result<AuthLoginResult> {
    let resolved = resolve_auth_inputs(overrides)?;

    let base_url = match resolved.base_url {
        Some(value) => value.value,
        None => prompt_for_value("Grafana URL", false)?,
    };

    let token = match resolved.token {
        Some(value) => value.value,
        None => prompt_for_value("Grafana token", true)?,
    };

    let config = GrafanaConfig {
        base_url: base_url.clone(),
        token,
        url_source: ConfigSource::Flag,
        token_source: ConfigSource::Flag,
    };

    let visible_datasources = if no_verify {
        None
    } else {
        let grafana = GrafanaClient::new(config.clone())?;
        let datasources = grafana.fetch_datasources()?;
        Some(datasources.len())
    };

    let path = save_profile(&config.base_url, &config.token)?;

    Ok(AuthLoginResult {
        ok: true,
        grafana_url: config.base_url,
        profile_path: path.display().to_string(),
        verified: !no_verify,
        visible_datasources,
    })
}

impl TableOutput for AuthStatusResult {
    fn render_table(&self) {
        println!("✅ credentials look good");
        println!("Successfully reached Grafana API at {}", self.grafana_url);
        println!("Visible datasources: {}", self.visible_datasources);
        println!("URL source: {}", self.url_source.as_label());
        println!("Token source: {}", self.token_source.as_label());
    }
}

impl TableOutput for AuthLoginResult {
    fn render_table(&self) {
        println!("✅ saved Grafana credentials");
        println!("Grafana URL: {}", self.grafana_url);
        println!("Profile: {}", self.profile_path);

        if self.verified {
            println!("Verification: success");
            if let Some(count) = self.visible_datasources {
                println!("Visible datasources: {count}");
            }
        } else {
            println!("Verification: skipped (--no-verify)");
        }
    }
}

fn prompt_for_value(label: &str, secret: bool) -> Result<String> {
    let raw = if secret {
        rpassword::prompt_password(format!("{label}: ")).context("failed to read input")?
    } else {
        print!("{label}: ");
        io::stdout().flush().context("failed to flush stdout")?;

        let mut value = String::new();
        io::stdin()
            .read_line(&mut value)
            .context("failed to read input")?;
        value
    };

    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }

    Ok(trimmed)
}
