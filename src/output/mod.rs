mod json;
mod table;

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    Table,
    Json,
}

impl OutputMode {
    pub fn from_json_flag(json: bool) -> Self {
        if json { Self::Json } else { Self::Table }
    }
}

pub trait TableOutput {
    fn render_table(&self);
}

pub fn emit<T>(mode: OutputMode, value: &T) -> Result<()>
where
    T: Serialize + TableOutput,
{
    match mode {
        OutputMode::Table => {
            table::render(value);
            Ok(())
        }
        OutputMode::Json => json::render(value),
    }
}
