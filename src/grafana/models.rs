use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataSource {
    pub id: i64,
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ds_type: String,
    #[serde(rename = "isDefault", default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LokiQueryRangeResponse {
    pub status: String,
    pub data: LokiStreamsData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LokiStreamsData {
    #[serde(rename = "result")]
    pub streams: Vec<LokiStream>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LokiStream {
    pub stream: BTreeMap<String, String>,
    pub values: Vec<(String, String)>,
}
