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

/// Sources that existed in the classic ChampR UI/package feed.
///
/// The live service source list is merged with this catalog at runtime, so
/// current server-provided sources win while older package-backed sources
/// remain available when the service no longer advertises them.
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
            label: "U.GG".to_string(),
            value: "u.gg".to_string(),
            is_aram: Some(false),
            is_urf: Some(false),
        },
        SourceItem {
            label: "U.GG ARAM".to_string(),
            value: "u.gg-aram".to_string(),
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
