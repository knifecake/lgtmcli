use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::app::AppContext;
use crate::cli::SqlQueryArgs;
use crate::output::{TableOutput, render_aligned_table};

const SUPPORTED_SQL_TYPES: [&str; 3] = ["postgres", "mysql", "mssql"];

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
    ensure_supported_sql_type(&datasource.ds_type)?;

    let response =
        ctx.grafana
            .query_datasource_sql(&args.datasource_uid, &datasource.ds_type, &args.query)?;

    let parsed = parse_ds_query_result(&response)?;

    let total_row_count = parsed.rows.len();
    let rows: Vec<BTreeMap<String, String>> = parsed.rows.into_iter().take(args.limit).collect();
    let row_count = rows.len();
    let truncated = row_count < total_row_count;

    Ok(SqlQueryResult {
        datasource_uid: args.datasource_uid,
        datasource_type: datasource.ds_type,
        query: args.query,
        columns: parsed.columns,
        row_count,
        total_row_count,
        truncated,
        rows,
    })
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

fn ensure_supported_sql_type(ds_type: &str) -> Result<()> {
    if SUPPORTED_SQL_TYPES
        .iter()
        .any(|allowed| ds_type.eq_ignore_ascii_case(allowed))
    {
        return Ok(());
    }

    bail!(
        "datasource type '{ds_type}' is not a supported SQL datasource type (supported: postgres, mysql, mssql)"
    )
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

    use super::{parse_ds_query_result, validate_read_only_sql};

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
}
