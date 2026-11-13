use serde::{Deserialize, Serialize};

/// One installed app that has a newer version available.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutdatedApp {
    /// Unique across providers: "<source>:<id>".
    pub key: String,
    /// Package id the package manager understands (winget id, brew name, App Store id).
    pub id: String,
    pub name: String,
    pub current: String,
    pub available: String,
    /// "winget" | "msstore" | "brew" | "cask" | "mas"
    pub source: String,
}

impl OutdatedApp {
    pub fn new(source: &str, id: &str, name: &str, current: &str, available: &str) -> Self {
        Self {
            key: format!("{source}:{id}"),
            id: id.to_string(),
            name: name.to_string(),
            current: current.to_string(),
            available: available.to_string(),
            source: source.to_string(),
        }
    }
}
