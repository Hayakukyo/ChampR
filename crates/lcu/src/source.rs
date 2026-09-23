use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceItem {
    #[serde(alias = "name")]
    pub label: String,
    #[serde(alias = "source")]
    pub value: String,
    pub is_aram: Option<bool>,
    #[serde(rename(serialize = "isUrf", deserialize = "isURF"))]
    pub is_urf: Option<bool>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub updated_at: String,
}

impl SourceItem {
    pub fn mode_label(&self) -> &'static str {
        if self.is_aram == Some(true) {
            "ARAM"
        } else if self.is_urf == Some(true) {
            "URF"
        } else {
            "SR"
        }
    }
}

fn source(label: &str, value: &str, is_aram: bool, is_urf: bool) -> SourceItem {
    SourceItem {
        label: label.to_string(),
        value: value.to_string(),
        is_aram: Some(is_aram),
        is_urf: Some(is_urf),
        version: String::new(),
        updated_at: String::new(),
    }
}

/// Sources supported by the restored multi-source UI.
///
/// OP.GG sources are fetched directly. Classic ChampR sources are backed by
/// their historical @champ-r/* packages from the public npm registry.
pub fn builtin_sources() -> Vec<SourceItem> {
    vec![
        source("OP.GG", "op.gg", false, false),
        source("OP.GG ARAM", "op.gg-aram", true, false),
        source("Lolalytics", "lolalytics", false, false),
        source("MurderBridge", "murderbridge", true, false),
    ]
}
