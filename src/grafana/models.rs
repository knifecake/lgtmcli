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
