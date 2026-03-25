use anyhow::Result;
use serde::Serialize;

use crate::app::AppContext;
use crate::output::TableOutput;

#[derive(Debug, Serialize)]
pub struct AuthStatusResult {
    pub ok: bool,
    pub grafana_url: String,
    pub visible_datasources: usize,
}

pub fn status(ctx: &AppContext) -> Result<AuthStatusResult> {
    let datasources = ctx.grafana.fetch_datasources()?;

    Ok(AuthStatusResult {
        ok: true,
        grafana_url: ctx.config.base_url.clone(),
        visible_datasources: datasources.len(),
    })
}

impl TableOutput for AuthStatusResult {
    fn render_table(&self) {
        println!("✅ credentials look good");
        println!("Successfully reached Grafana API at {}", self.grafana_url);
        println!("Visible datasources: {}", self.visible_datasources);
    }
}
