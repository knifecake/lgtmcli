use anyhow::Result;
use serde::Serialize;

use crate::app::AppContext;
use crate::grafana::models::DataSource;
use crate::output::{TableOutput, render_aligned_table};

#[derive(Debug, Serialize)]
pub struct DatasourceListResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_type: Option<String>,
    pub count: usize,
    pub datasources: Vec<DataSource>,
}

pub fn list(ctx: &AppContext, ds_type: Option<String>) -> Result<DatasourceListResult> {
    let datasources = ctx.grafana.fetch_datasources()?;
    let datasources = filter_and_sort(datasources, ds_type.as_deref());
    let count = datasources.len();

    Ok(DatasourceListResult {
        ds_type,
        count,
        datasources,
    })
}

fn filter_and_sort(mut datasources: Vec<DataSource>, ds_type: Option<&str>) -> Vec<DataSource> {
    if let Some(filter) = ds_type {
        datasources.retain(|ds| datasource_type_matches(filter, &ds.ds_type));
    }

    datasources.sort_by_key(|ds| {
        (
            ds.ds_type.to_ascii_lowercase(),
            ds.name.to_ascii_lowercase(),
        )
    });

    datasources
}

fn datasource_type_matches(filter: &str, ds_type: &str) -> bool {
    normalize_datasource_type(filter) == normalize_datasource_type(ds_type)
}

fn normalize_datasource_type(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "postgres" | "postgresql" | "grafana-postgresql-datasource" => "postgres".to_string(),
        "mysql" | "grafana-mysql-datasource" => "mysql".to_string(),
        "mssql" | "sqlserver" | "grafana-mssql-datasource" => "mssql".to_string(),
        _ => normalized,
    }
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

        let headers = ["ID", "UID", "TYPE", "NAME", "DEFAULT"];
        let rows: Vec<Vec<String>> = self
            .datasources
            .iter()
            .map(|ds| {
                vec![
                    ds.id.to_string(),
                    ds.uid.clone(),
                    ds.ds_type.clone(),
                    ds.name.clone(),
                    if ds.is_default { "yes" } else { "no" }.to_string(),
                ]
            })
            .collect();

        render_aligned_table(&headers, &rows);
    }
}

#[cfg(test)]
mod tests {
    use super::filter_and_sort;
    use crate::grafana::models::DataSource;

    fn ds(id: i64, uid: &str, ds_type: &str, name: &str) -> DataSource {
        DataSource {
            id,
            uid: uid.to_string(),
            name: name.to_string(),
            ds_type: ds_type.to_string(),
            is_default: false,
        }
    }

    #[test]
    fn filter_by_type_is_case_insensitive() {
        let input = vec![
            ds(1, "loki-1", "loki", "Loki A"),
            ds(2, "prom-1", "prometheus", "Prom A"),
            ds(3, "loki-2", "Loki", "Loki B"),
        ];

        let result = filter_and_sort(input, Some("LOKI"));

        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .all(|d| d.ds_type.eq_ignore_ascii_case("loki"))
        );
    }

    #[test]
    fn sorts_by_type_then_name_case_insensitive() {
        let input = vec![
            ds(1, "prom-z", "prometheus", "zeta"),
            ds(2, "loki-b", "loki", "Beta"),
            ds(3, "loki-a", "loki", "alpha"),
            ds(4, "prom-a", "prometheus", "Alpha"),
        ];

        let result = filter_and_sort(input, None);
        let ordered_uids: Vec<&str> = result.iter().map(|d| d.uid.as_str()).collect();

        assert_eq!(ordered_uids, vec!["loki-a", "loki-b", "prom-a", "prom-z"]);
    }

    #[test]
    fn filter_postgres_matches_grafana_postgres_plugin_type() {
        let input = vec![
            ds(
                1,
                "pg-ro",
                "grafana-postgresql-datasource",
                "Postgres Read Only",
            ),
            ds(2, "loki-1", "loki", "Loki"),
        ];

        let result = filter_and_sort(input, Some("postgres"));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uid, "pg-ro");
    }

    #[test]
    fn filter_plugin_type_matches_short_sql_alias() {
        let input = vec![
            ds(1, "pg-ro", "postgres", "Postgres Read Only"),
            ds(2, "mimir", "prometheus", "Mimir"),
        ];

        let result = filter_and_sort(input, Some("grafana-postgresql-datasource"));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uid, "pg-ro");
    }
}
