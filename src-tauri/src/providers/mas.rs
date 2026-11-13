#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::PathBuf;

use super::UpdateProvider;
use crate::model::OutdatedApp;
use crate::runner;

/// Mac App Store, via the `mas` CLI (`brew install mas`).
pub struct Mas;

fn mas_path() -> Option<PathBuf> {
    ["/opt/homebrew/bin/mas", "/usr/local/bin/mas"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

impl UpdateProvider for Mas {
    fn name(&self) -> &'static str {
        "App Store"
    }

    fn missing_hint(&self) -> &'static str {
        "Mac App Store apps are skipped. Run `brew install mas` to include them."
    }

    fn is_available(&self) -> bool {
        mas_path().is_some()
    }

    fn handles(&self, source: &str) -> bool {
        source == "mas"
    }

    fn list_outdated(&self) -> Result<Vec<OutdatedApp>, String> {
        let mas = mas_path().ok_or("mas not found")?;
        let out = runner::capture(runner::cmd(mas).arg("outdated"))?;
        Ok(parse_outdated(&out.stdout))
    }

    fn update(&self, app: &OutdatedApp, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
        let mas = mas_path().ok_or("mas not found")?;
        runner::stream(runner::cmd(mas).args(["upgrade", &app.id]), on_line)
    }
}

/// Parses `mas outdated` lines: `497799835  Xcode  (15.0 -> 15.1)`.
pub fn parse_outdated(text: &str) -> Vec<OutdatedApp> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (id, rest) = line.split_once(char::is_whitespace)?;
            if !id.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let open = rest.rfind('(')?;
            let versions = rest[open + 1..].strip_suffix(')')?;
            let (current, available) = versions.split_once("->")?;
            let name = rest[..open].trim();
            Some(OutdatedApp::new("mas", id, name, current.trim(), available.trim()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines() {
        let apps = parse_outdated(include_str!("../../tests/fixtures/mas_outdated.txt"));
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0], OutdatedApp::new("mas", "497799835", "Xcode", "15.0", "15.1"));
        assert_eq!(apps[1].name, "Pages (Productivity)");
        assert_eq!(apps[1].available, "14.2");
    }
}
