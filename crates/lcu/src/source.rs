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

/// Classic ChampR package-backed sources.
///
/// These are merged with the live service source list at runtime. Live
/// service entries override labels for duplicate keys, while the classic
/// package sources stay available for the old multi-source workflow.
pub fn builtin_sources() -> Vec<SourceItem> {
    vec![
        SourceItem {
            label: "OP.GG".to_string(),
            value: "op.gg".to_string(),
            is_aram: Some(false),
            is_urf: Some(false),
        },
        SourceItem {
            label: "OP.GG ARAM".to_string(),
            value: "op.gg-aram".to_string(),
            is_aram: Some(true),
            is_urf: Some(false),
        },
        SourceItem {
            label: "Lolalytics".to_string(),
            value: "lolalytics".to_string(),
            is_aram: Some(false),
            is_urf: Some(false),
        },
        SourceItem {
            label: "MurderBridge".to_string(),
            value: "murderbridge".to_string(),
            is_aram: Some(true),
            is_urf: Some(false),
        },
    ]
}
