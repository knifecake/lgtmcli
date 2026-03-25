use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusQueryResponse {
    pub status: String,
    pub data: PrometheusData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusData {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub result: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TempoSearchResponse {
    #[serde(default)]
    pub traces: Vec<Value>,
}
