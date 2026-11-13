#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::UpdateProvider;
use crate::model::OutdatedApp;
use crate::runner;

pub struct Brew;

/// GUI apps on macOS don't inherit the shell PATH, so look in the standard install locations.
fn brew_path() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

fn brew_cmd(brew: &Path) -> Command {
    let mut c = runner::cmd(brew);
    let bin = brew.parent().map(|p| p.display().to_string()).unwrap_or_default();
    c.env("PATH", format!("{bin}:/usr/bin:/bin:/usr/sbin:/sbin"))
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .env("NONINTERACTIVE", "1");
    c
}

impl UpdateProvider for Brew {
    fn name(&self) -> &'static str {
        "Homebrew"
    }

    fn missing_hint(&self) -> &'static str {
        "Homebrew was not found. Install it from https://brew.sh to check apps and command-line tools."
    }

    fn is_available(&self) -> bool {
        brew_path().is_some()
    }

    fn handles(&self, source: &str) -> bool {
        matches!(source, "brew" | "cask")
    }

    fn list_outdated(&self) -> Result<Vec<OutdatedApp>, String> {
        let brew = brew_path().ok_or("brew not found")?;
        // Refresh package metadata first; a failure here (offline) still lets us show what we know.
        let _ = runner::capture(brew_cmd(&brew).arg("update"));
        let out = runner::capture(brew_cmd(&brew).args(["outdated", "--json=v2", "--greedy"]))?;
        parse_outdated_json(&out.stdout).map_err(|e| {
            if out.stderr.trim().is_empty() { e } else { format!("{e}: {}", out.stderr.trim()) }
        })
    }

    fn update(&self, app: &OutdatedApp, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
        let brew = brew_path().ok_or("brew not found")?;
        let mut c = brew_cmd(&brew);
        if app.source == "cask" {
            c.args(["upgrade", "--cask", "--greedy", &app.id]);
        } else {
            c.args(["upgrade", &app.id]);
        }
        runner::stream(&mut c, on_line)
    }
}

/// Parses `brew outdated --json=v2`.
pub fn parse_outdated_json(text: &str) -> Result<Vec<OutdatedApp>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("unexpected brew output ({e})"))?;
    let mut apps = Vec::new();
    for (list, source) in [("formulae", "brew"), ("casks", "cask")] {
        for item in v[list].as_array().into_iter().flatten() {
            let Some(name) = item["name"].as_str() else { continue };
            let installed = match &item["installed_versions"] {
                Value::Array(a) => a.last().and_then(Value::as_str).unwrap_or("").to_string(),
                Value::String(s) => s.clone(),
                _ => String::new(),
            };
            let current = item["current_version"].as_str().unwrap_or("");
            if item["pinned"].as_bool() == Some(true) {
                continue;
            }
            apps.push(OutdatedApp::new(source, name, name, &installed, current));
        }
    }
    Ok(apps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formulae_and_casks() {
        let apps = parse_outdated_json(include_str!("../../tests/fixtures/brew_outdated.json")).unwrap();
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0], OutdatedApp::new("brew", "git", "git", "2.46.0", "2.47.1"));
        assert_eq!(apps[1].source, "cask");
        assert_eq!(apps[1].id, "firefox");
        assert_eq!(apps[1].current, "131.0");
        assert_eq!(apps[2].current, "1.94.0");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_outdated_json("Error: oops").is_err());
    }
}
