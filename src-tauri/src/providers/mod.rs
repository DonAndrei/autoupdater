//! Package-manager backends. Parsers compile on every OS (so they can be unit-tested
//! anywhere); `all()` only registers the providers that make sense for the current OS.

pub mod brew;
pub mod mas;
pub mod winget;

use crate::model::OutdatedApp;

pub trait UpdateProvider: Send + Sync {
    fn name(&self) -> &'static str;
    /// Hint shown in the UI when the package manager isn't installed.
    fn missing_hint(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn handles(&self, source: &str) -> bool;
    fn list_outdated(&self) -> Result<Vec<OutdatedApp>, String>;
    fn update(&self, app: &OutdatedApp, on_line: &mut dyn FnMut(&str)) -> Result<(), String>;
}

pub fn all() -> Vec<Box<dyn UpdateProvider>> {
    #[allow(unused_mut)]
    let mut v: Vec<Box<dyn UpdateProvider>> = Vec::new();
    #[cfg(windows)]
    v.push(Box::new(winget::Winget));
    #[cfg(target_os = "macos")]
    {
        v.push(Box::new(brew::Brew));
        v.push(Box::new(mas::Mas));
    }
    v
}
