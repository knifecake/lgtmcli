use anyhow::Result;
use serde::Serialize;

use crate::app::AppContext;
use crate::grafana::models::DataSource;
use crate::output::TableOutput;

#[derive(Debug, Serialize)]
pub struct DatasourceListResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_type: Option<String>,
    pub count: usize,
    pub datasources: Vec<DataSource>,
}

pub fn list(ctx: &AppContext, ds_type: Option<String>) -> Result<DatasourceListResult> {
    let mut datasources = ctx.grafana.fetch_datasources()?;

    if let Some(filter) = ds_type.as_deref() {
        datasources.retain(|ds| ds.ds_type.eq_ignore_ascii_case(filter));
    }

    datasources.sort_by_key(|ds| {
        (
            ds.ds_type.to_ascii_lowercase(),
            ds.name.to_ascii_lowercase(),
        )
    });

    let count = datasources.len();

    Ok(DatasourceListResult {
        ds_type,
        count,
        datasources,
    })
}

impl TableOutput for DatasourceListResult {
    fn render_table(&self) {
        if self.datasources.is_empty() {
            if let Some(filter) = self.ds_type.as_deref() {
                println!("No datasources found for --type '{filter}'.");
            } else {
                println!("No datasources found.");
            }
            return;
        }

        println!("ID\tUID\tTYPE\tNAME\tDEFAULT");
        for ds in &self.datasources {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                ds.id,
                ds.uid,
                ds.ds_type,
                ds.name,
                if ds.is_default { "yes" } else { "no" }
            );
        }
    }
}
