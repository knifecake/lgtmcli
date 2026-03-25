use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::app::AppContext;
use crate::cli::{SqlDescribeArgs, SqlQueryArgs, SqlTablesArgs};
use crate::output::{TableOutput, render_aligned_table};

#[derive(Debug, Serialize)]
pub struct SqlQueryResult {
    pub datasource_uid: String,
    pub datasource_type: String,
    pub query: String,
    pub columns: Vec<String>,
    pub row_count: usize,
    pub total_row_count: usize,
    pub truncated: bool,
    pub rows: Vec<BTreeMap<String, String>>,
}

pub fn query(ctx: &AppContext, args: SqlQueryArgs) -> Result<SqlQueryResult> {
    validate_read_only_sql(&args.query)?;

    let datasource = ctx.grafana.fetch_datasource_by_uid(&args.datasource_uid)?;

    if normalize_sql_type(&datasource.ds_type).is_none() && !args.force {
        bail!(
            "datasource type '{}' is not recognized as SQL by lgtmcli (supported: postgres/mysql/mssql and known aliases). Use --force to skip this check.",
            datasource.ds_type
        );
    }

    run_sql_query(
        ctx,
        datasource.uid,
        datasource.ds_type,
        args.query,
        args.limit,
    )
}

pub fn tables(ctx: &AppContext, args: SqlTablesArgs) -> Result<SqlQueryResult> {
    let datasource = ctx.grafana.fetch_datasource_by_uid(&args.datasource_uid)?;
    let sql_type = require_supported_sql_type(&datasource.ds_type)?;

    let query = build_tables_query(sql_type, args.schema.as_deref(), args.like.as_deref())?;

    run_sql_query(ctx, datasource.uid, datasource.ds_type, query, args.limit)
}

pub fn describe(ctx: &AppContext, args: SqlDescribeArgs) -> Result<SqlQueryResult> {
    let datasource = ctx.grafana.fetch_datasource_by_uid(&args.datasource_uid)?;
    let sql_type = require_supported_sql_type(&datasource.ds_type)?;

    let (schema, table) = resolve_schema_and_table(&args.table, args.schema.as_deref())?;
    let query = build_describe_query(sql_type, schema.as_deref(), &table)?;

    run_sql_query(ctx, datasource.uid, datasource.ds_type, query, args.limit)
}

impl TableOutput for SqlQueryResult {
    fn render_table(&self) {
        if self.columns.is_empty() {
            println!("No tabular result returned.");
            return;
        }

        let headers: Vec<&str> = self.columns.iter().map(String::as_str).collect();
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|row| {
                self.columns
                    .iter()
                    .map(|col| row.get(col).cloned().unwrap_or_default())
                    .collect()
            })
            .collect();

        render_aligned_table(&headers, &rows);

        if self.truncated {
            println!(
                "\nShowing {} of {} rows. Re-run with a higher --limit to see more.",
                self.row_count, self.total_row_count
            );
        }
    }
}

#[derive(Debug)]
struct ParsedSqlTable {
    columns: Vec<String>,
    rows: Vec<BTreeMap<String, String>>,
}

fn run_sql_query(
    ctx: &AppContext,
    datasource_uid: String,
    datasource_type: String,
    query: String,
    limit: usize,
) -> Result<SqlQueryResult> {
    let response = ctx
        .grafana
        .query_datasource_sql(&datasource_uid, &datasource_type, &query)?;

    let parsed = parse_ds_query_result(&response)?;

    let total_row_count = parsed.rows.len();
    let rows: Vec<BTreeMap<String, String>> = parsed.rows.into_iter().take(limit).collect();
    let row_count = rows.len();
    let truncated = row_count < total_row_count;

    Ok(SqlQueryResult {
        datasource_uid,
        datasource_type,
        query,
        columns: parsed.columns,
        row_count,
        total_row_count,
        truncated,
        rows,
    })
}

fn require_supported_sql_type(ds_type: &str) -> Result<&'static str> {
    normalize_sql_type(ds_type).ok_or_else(|| {
        anyhow::anyhow!(
            "datasource type '{ds_type}' is not a supported SQL datasource type (supported: postgres, mysql, mssql; aliases: grafana-postgresql-datasource, grafana-mysql-datasource, grafana-mssql-datasource)"
        )
    })
}

fn normalize_sql_type(ds_type: &str) -> Option<&'static str> {
    match ds_type.to_ascii_lowercase().as_str() {
        "postgres" | "postgresql" | "grafana-postgresql-datasource" => Some("postgres"),
        "mysql" | "grafana-mysql-datasource" => Some("mysql"),
        "mssql" | "sqlserver" | "grafana-mssql-datasource" | "grafana-sqlserver-datasource" => {
            Some("mssql")
        }
        _ => None,
    }
}

fn build_tables_query(ds_type: &str, schema: Option<&str>, like: Option<&str>) -> Result<String> {
    let mut query = String::from(
        "SELECT table_schema AS schema_name, table_name\nFROM information_schema.tables\nWHERE table_type = 'BASE TABLE'",
    );

    match ds_type.to_ascii_lowercase().as_str() {
        "postgres" => {
            query.push_str("\n  AND table_schema NOT IN ('pg_catalog', 'information_schema')")
        }
        "mysql" => query.push_str(
            "\n  AND table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')",
        ),
        "mssql" => {
            query.push_str("\n  AND table_schema NOT IN ('INFORMATION_SCHEMA', 'sys')")
        }
        other => bail!("unsupported SQL datasource type '{other}'"),
    }

    if let Some(schema) = schema {
        query.push_str(&format!(
            "\n  AND table_schema = {}",
            sql_string_literal(schema)
        ));
    }
    if let Some(pattern) = like {
        query.push_str(&format!(
            "\n  AND table_name LIKE {}",
            sql_string_literal(pattern)
        ));
    }

    query.push_str("\nORDER BY table_schema, table_name");
    Ok(query)
}

fn resolve_schema_and_table(
    table_input: &str,
    schema_flag: Option<&str>,
) -> Result<(Option<String>, String)> {
    let trimmed = table_input.trim();
    if trimmed.is_empty() {
        bail!("table name must not be empty");
    }

    let dot_count = trimmed.matches('.').count();
    if dot_count > 1 {
        bail!("table must be either <table> or <schema.table>");
    }

    if let Some((schema_from_table, table_name)) = trimmed.split_once('.') {
        if schema_flag.is_some() {
            bail!("use either <schema.table> or --schema, not both");
        }
        if schema_from_table.trim().is_empty() || table_name.trim().is_empty() {
            bail!("table must be either <table> or <schema.table>");
        }

        return Ok((
            Some(schema_from_table.trim().to_string()),
            table_name.trim().to_string(),
        ));
    }

    Ok((schema_flag.map(ToOwned::to_owned), trimmed.to_string()))
}

fn build_describe_query(ds_type: &str, schema: Option<&str>, table: &str) -> Result<String> {
    let mut query = String::from(
        "SELECT table_schema AS schema_name, column_name, data_type, is_nullable, column_default, ordinal_position\nFROM information_schema.columns\nWHERE table_name = ",
    );
    query.push_str(&sql_string_literal(table));

    match ds_type.to_ascii_lowercase().as_str() {
        "postgres" => {
            if let Some(schema) = schema {
                query.push_str(&format!(
                    "\n  AND table_schema = {}",
                    sql_string_literal(schema)
                ));
            } else {
                query.push_str("\n  AND table_schema NOT IN ('pg_catalog', 'information_schema')");
            }
        }
        "mysql" | "mssql" => {
            if let Some(schema) = schema {
                query.push_str(&format!(
                    "\n  AND table_schema = {}",
                    sql_string_literal(schema)
                ));
            }
        }
        other => bail!("unsupported SQL datasource type '{other}'"),
    }

    query.push_str("\nORDER BY table_schema, ordinal_position");
    Ok(query)
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn parse_ds_query_result(payload: &Value) -> Result<ParsedSqlTable> {
    let results = payload
        .get("results")
        .and_then(Value::as_object)
        .context("invalid /api/ds/query response: missing results")?;

    let first_result = results
        .values()
        .next()
        .context("/api/ds/query response had no query results")?;

    if let Some(error) = first_result.get("error").and_then(Value::as_str) {
        bail!("datasource query failed: {error}");
    }

    let frames = first_result
        .get("frames")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if frames.is_empty() {
        return Ok(ParsedSqlTable {
            columns: vec![],
            rows: vec![],
        });
    }

    let mut columns: Vec<String> = Vec::new();
    let mut rows: Vec<BTreeMap<String, String>> = Vec::new();

    for frame in frames {
        let frame_columns = extract_frame_columns(&frame)?;

        if columns.is_empty() {
            columns = frame_columns.clone();
        } else if columns != frame_columns {
            bail!("query returned multiple frames with different column sets");
        }

        let frame_rows = extract_frame_rows(&frame, &columns)?;
        rows.extend(frame_rows);
    }

    Ok(ParsedSqlTable { columns, rows })
}

fn extract_frame_columns(frame: &Value) -> Result<Vec<String>> {
    let fields = frame
        .get("schema")
        .and_then(|v| v.get("fields"))
        .and_then(Value::as_array)
        .context("invalid frame: missing schema.fields")?;

    fields
        .iter()
        .map(|field| {
            field
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .context("invalid frame field: missing name")
        })
        .collect()
}

fn extract_frame_rows(frame: &Value, columns: &[String]) -> Result<Vec<BTreeMap<String, String>>> {
    let values_by_column = frame
        .get("data")
        .and_then(|v| v.get("values"))
        .and_then(Value::as_array)
        .context("invalid frame: missing data.values")?;

    if values_by_column.len() != columns.len() {
        bail!(
            "invalid frame: data.values column count ({}) does not match schema.fields ({})",
            values_by_column.len(),
            columns.len()
        );
    }

    let column_arrays: Vec<&Vec<Value>> = values_by_column
        .iter()
        .map(|column| {
            column
                .as_array()
                .context("invalid frame: each data.values entry must be an array")
        })
        .collect::<Result<Vec<_>>>()?;

    let row_count = column_arrays.iter().map(|col| col.len()).max().unwrap_or(0);

    let mut rows = Vec::with_capacity(row_count);
    for row_idx in 0..row_count {
        let mut row = BTreeMap::new();

        for (col_idx, column_name) in columns.iter().enumerate() {
            let cell = column_arrays[col_idx]
                .get(row_idx)
                .map(value_to_string)
                .unwrap_or_default();
            row.insert(column_name.clone(), cell);
        }

        rows.push(row);
    }

    Ok(rows)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::String(v) => v.clone(),
        Value::Number(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Array(v) => Value::Array(v.clone()).to_string(),
        Value::Object(v) => Value::Object(v.clone()).to_string(),
    }
}

fn validate_read_only_sql(sql: &str) -> Result<()> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        bail!("SQL query must not be empty");
    }

    let without_trailing = trimmed.trim_end_matches(';').trim_end();
    if without_trailing.contains(';') {
        bail!("multiple SQL statements are not allowed");
    }

    let first_word = first_sql_word(without_trailing)
        .context("could not determine SQL statement type")?
        .to_ascii_lowercase();

    let allowed = ["select", "with", "show", "explain", "values"];
    if allowed.contains(&first_word.as_str()) {
        return Ok(());
    }

    bail!("only read-only SQL statements are allowed (SELECT/WITH/SHOW/EXPLAIN/VALUES)")
}

fn first_sql_word(sql: &str) -> Option<&str> {
    sql.split_whitespace().next()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_describe_query, build_tables_query, normalize_sql_type, parse_ds_query_result,
        resolve_schema_and_table, validate_read_only_sql,
    };

    #[test]
    fn allows_select_queries() {
        let result = validate_read_only_sql("SELECT * FROM users LIMIT 10;");
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_write_queries() {
        let result = validate_read_only_sql("UPDATE users SET admin=true");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_multiple_statements() {
        let result = validate_read_only_sql("SELECT 1; SELECT 2");
        assert!(result.is_err());
    }

    #[test]
    fn parses_table_frame_rows() {
        let payload = json!({
            "results": {
                "A": {
                    "frames": [
                        {
                            "schema": {
                                "fields": [
                                    {"name": "id"},
                                    {"name": "email"}
                                ]
                            },
                            "data": {
                                "values": [
                                    [1, 2],
                                    ["a@example.com", "b@example.com"]
                                ]
                            }
                        }
                    ]
                }
            }
        });

        let parsed = parse_ds_query_result(&payload).expect("parse result");
        assert_eq!(parsed.columns, vec!["id", "email"]);
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0].get("id").unwrap(), "1");
        assert_eq!(parsed.rows[0].get("email").unwrap(), "a@example.com");
    }

    #[test]
    fn resolve_schema_and_table_from_qualified_name() {
        let (schema, table) = resolve_schema_and_table("public.users", None).expect("parse");
        assert_eq!(schema.as_deref(), Some("public"));
        assert_eq!(table, "users");
    }

    #[test]
    fn resolve_schema_and_table_rejects_both_inputs() {
        let result = resolve_schema_and_table("public.users", Some("public"));
        assert!(result.is_err());
    }

    #[test]
    fn build_tables_query_for_postgres_excludes_system_schemas() {
        let query = build_tables_query("postgres", None, None).expect("query");
        assert!(query.contains("pg_catalog"));
        assert!(query.contains("information_schema"));
    }

    #[test]
    fn build_describe_query_for_schema() {
        let query = build_describe_query("postgres", Some("public"), "users").expect("query");
        assert!(query.contains("table_name = 'users'"));
        assert!(query.contains("table_schema = 'public'"));
    }

    #[test]
    fn normalizes_grafana_sql_plugin_aliases() {
        assert_eq!(
            normalize_sql_type("grafana-postgresql-datasource"),
            Some("postgres")
        );
        assert_eq!(
            normalize_sql_type("grafana-mysql-datasource"),
            Some("mysql")
        );
        assert_eq!(
            normalize_sql_type("grafana-mssql-datasource"),
            Some("mssql")
        );
    }
}
