#![cfg_attr(not(windows), allow(dead_code))]

use super::UpdateProvider;
use crate::model::OutdatedApp;
use crate::runner;

pub struct Winget;

impl UpdateProvider for Winget {
    fn name(&self) -> &'static str {
        "winget"
    }

    fn missing_hint(&self) -> &'static str {
        "winget was not found. Install \"App Installer\" from the Microsoft Store."
    }

    fn is_available(&self) -> bool {
        runner::capture(runner::cmd("winget").arg("--version")).is_ok()
    }

    fn handles(&self, source: &str) -> bool {
        matches!(source, "winget" | "msstore")
    }

    fn list_outdated(&self) -> Result<Vec<OutdatedApp>, String> {
        // winget's exit code is non-zero when nothing is outdated, so only the text matters.
        let out = runner::capture(runner::cmd("winget").args([
            "upgrade",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]))?;
        Ok(parse_upgrade_table(&out.stdout))
    }

    fn update(&self, app: &OutdatedApp, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
        let mut c = runner::cmd("winget");
        c.args([
            "upgrade",
            "--id",
            &app.id,
            "--exact",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]);
        if !app.source.is_empty() {
            c.args(["--source", &app.source]);
        }
        runner::stream(&mut c, on_line)
    }
}

fn is_separator(line: &[char]) -> bool {
    line.len() >= 10 && line.iter().all(|&c| c == '-')
}

/// Parses the table(s) printed by `winget upgrade`.
///
/// Header words are localized, so columns are found by position: a column starts where
/// the header has a word after a space AND every data row has a space just before it.
/// Columns are, in order: Name, Id, Version, Available, Source.
pub fn parse_upgrade_table(text: &str) -> Vec<OutdatedApp> {
    let lines: Vec<Vec<char>> = text
        .trim_start_matches('\u{feff}')
        .split('\n')
        .map(|l| {
            // Progress spinners are overwritten with \r; keep only the final segment.
            l.split('\r')
                .filter(|s| !s.trim().is_empty())
                .last()
                .unwrap_or("")
                .trim_end()
                .chars()
                .collect()
        })
        .collect();

    let mut apps = Vec::new();
    for i in 1..lines.len() {
        if !is_separator(&lines[i]) {
            continue;
        }
        let header = &lines[i - 1];
        let mut starts: Vec<usize> = (0..header.len())
            .filter(|&j| header[j] != ' ' && (j == 0 || header[j - 1] == ' '))
            .collect();
        let Some(&last_start) = starts.last() else { continue };

        // Data rows are padded out to the last column; the first shorter line is the footer.
        let rows: Vec<&Vec<char>> = lines[i + 1..]
            .iter()
            .take_while(|l| l.len() > last_start && !is_separator(l))
            .collect();

        starts.retain(|&s| s == 0 || rows.iter().all(|r| r.get(s - 1).map_or(true, |&c| c == ' ')));
        if starts.len() < 4 {
            continue;
        }

        for row in rows {
            let field = |k: usize| -> String {
                let Some(&from) = starts.get(k) else { return String::new() };
                let to = starts.get(k + 1).copied().unwrap_or(row.len()).min(row.len());
                if from >= to {
                    return String::new();
                }
                row[from..to].iter().collect::<String>().trim().to_string()
            };
            let (name, id, current, available, source) = (field(0), field(1), field(2), field(3), field(4));
            if id.is_empty() || id.contains(' ') || available.is_empty() {
                continue;
            }
            let source = if source.is_empty() { "winget".to_string() } else { source };
            apps.push(OutdatedApp::new(&source, &id, &name, &current, &available));
        }
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_output() {
        let apps = parse_upgrade_table(include_str!("../../tests/fixtures/winget_upgrade.txt"));
        assert_eq!(apps.len(), 41);
        let zip = &apps[0];
        assert_eq!(zip.name, "7-Zip 25.00 (x64)");
        assert_eq!(zip.id, "7zip.7zip");
        assert_eq!(zip.current, "25.00");
        assert_eq!(zip.available, "26.06");
        assert_eq!(zip.source, "winget");
        assert_eq!(zip.key, "winget:7zip.7zip");

        let hw = apps.iter().find(|a| a.id == "REALiX.HWiNFO").unwrap();
        assert_eq!(hw.name, "HWiNFO® 64");
        let vc = apps.iter().find(|a| a.id == "Microsoft.VCRedist.2015+.x64").unwrap();
        assert_eq!(vc.current, "14.51.36247.0");
        let ubi = apps.iter().find(|a| a.id == "Ubisoft.Connect").unwrap();
        assert_eq!(ubi.current, "< 175.1.0.13416");
        let crystal = apps.iter().find(|a| a.name.starts_with("CrystalDiskInfo")).unwrap();
        assert_eq!(crystal.id, "CrystalDewWorld.CrystalDiskInfo.ShizukuEdition");
    }

    #[test]
    fn handles_spinner_crlf_and_second_table() {
        let text = "\r   - \r   \\ \rName      Id          Version Available Source\r\n\
                    ------------------------------------------------------\r\n\
                    Foo App   Foo.App     1.0     2.0       winget\r\n\
                    Bar       Bar.Bar     3.1     3.2       msstore\r\n\
                    2 upgrades available.\r\n\
                    \r\n\
                    The following packages have an upgrade available, but require explicit targeting for upgrade:\r\n\
                    Name Id      Version Available Source\r\n\
                    --------------------------------------\r\n\
                    Baz  Baz.Baz 1       2         winget\r\n";
        let apps = parse_upgrade_table(text);
        let ids: Vec<_> = apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["Foo.App", "Bar.Bar", "Baz.Baz"]);
        assert_eq!(apps[0].name, "Foo App");
        assert_eq!(apps[1].source, "msstore");
    }

    #[test]
    fn nothing_outdated() {
        assert!(parse_upgrade_table("No installed package found matching input criteria.\r\n").is_empty());
    }
}
