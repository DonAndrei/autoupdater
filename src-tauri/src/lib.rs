mod model;
mod providers;
mod runner;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use model::OutdatedApp;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;

const TRAY_ID: &str = "main";

#[derive(Default)]
struct AppState {
    apps: Mutex<Vec<OutdatedApp>>,
    scanning: AtomicBool,
    updating: AtomicBool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    apps: Vec<OutdatedApp>,
    warnings: Vec<String>,
    checked_at: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    key: String,
    /// "running" | "done" | "failed"
    status: &'static str,
    line: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
struct Finished {
    ok: usize,
    failed: usize,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn set_tray_count(app: &AppHandle, n: usize) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let text = match n {
            0 => "AutoUpdater - all up to date".to_string(),
            1 => "AutoUpdater - 1 update available".to_string(),
            n => format!("AutoUpdater - {n} updates available"),
        };
        let _ = tray.set_tooltip(Some(text));
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Asks every available package manager for outdated apps. Blocking.
fn do_scan(app: &AppHandle) -> Result<ScanResult, String> {
    let state = app.state::<AppState>();
    if state.updating.load(Ordering::SeqCst) {
        return Err("Updates are running. Scan again when they finish.".into());
    }
    if state.scanning.swap(true, Ordering::SeqCst) {
        return Err("A scan is already running.".into());
    }
    let _ = app.emit("scan-started", ());

    let mut apps = Vec::new();
    let mut warnings = Vec::new();
    for p in providers::all() {
        if !p.is_available() {
            warnings.push(p.missing_hint().to_string());
            continue;
        }
        match p.list_outdated() {
            Ok(mut found) => apps.append(&mut found),
            Err(e) => warnings.push(format!("{}: {e}", p.name())),
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());

    *state.apps.lock().unwrap() = apps.clone();
    state.scanning.store(false, Ordering::SeqCst);
    set_tray_count(app, apps.len());

    let result = ScanResult { apps, warnings, checked_at: now_ms() };
    let _ = app.emit("scan-result", &result);
    Ok(result)
}

#[tauri::command]
async fn scan(app: AppHandle) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || do_scan(&app))
        .await
        .map_err(|e| e.to_string())?
}

/// Starts updating the given apps one at a time; progress arrives as `update-progress` events.
#[tauri::command]
fn update_apps(app: AppHandle, state: State<AppState>, keys: Vec<String>) -> Result<(), String> {
    if state.scanning.load(Ordering::SeqCst) {
        return Err("Wait for the scan to finish.".into());
    }
    if state.updating.swap(true, Ordering::SeqCst) {
        return Err("Updates are already running.".into());
    }
    let targets: Vec<OutdatedApp> = {
        let apps = state.apps.lock().unwrap();
        keys.iter().filter_map(|k| apps.iter().find(|a| &a.key == k).cloned()).collect()
    };

    thread::spawn(move || {
        let providers = providers::all();
        let (mut ok, mut failed) = (0, 0);
        let emit = |key: &str, status: &'static str, line: Option<String>, error: Option<String>| {
            let _ = app.emit("update-progress", Progress { key: key.to_string(), status, line, error });
        };

        for t in &targets {
            emit(&t.key, "running", None, None);
            let result = match providers.iter().find(|p| p.handles(&t.source)) {
                Some(p) => p.update(t, &mut |line| emit(&t.key, "running", Some(line.to_string()), None)),
                None => Err(format!("no updater for source '{}'", t.source)),
            };
            match result {
                Ok(()) => {
                    ok += 1;
                    emit(&t.key, "done", None, None);
                    app.state::<AppState>().apps.lock().unwrap().retain(|a| a.key != t.key);
                }
                Err(e) => {
                    failed += 1;
                    emit(&t.key, "failed", None, Some(e));
                }
            }
        }

        let state = app.state::<AppState>();
        let remaining = state.apps.lock().unwrap().len();
        state.updating.store(false, Ordering::SeqCst);
        set_tray_count(&app, remaining);
        let _ = app.emit("update-finished", Finished { ok, failed });

        let body = match failed {
            0 => format!("Updated {ok} app(s)."),
            _ => format!("Updated {ok} app(s), {failed} failed."),
        };
        notify(&app, "Updates finished", &body);
    });
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![scan, update_apps])
        .setup(|app| {
            let open = MenuItem::with_id(app, "open", "Open AutoUpdater", true, None::<&str>)?;
            let check = MenuItem::with_id(app, "check", "Check for updates", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &check, &sep, &quit])?;

            TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("AutoUpdater")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "check" => {
                        let app = app.clone();
                        thread::spawn(move || match do_scan(&app) {
                            Ok(r) if r.apps.is_empty() => notify(&app, "All up to date", "No app updates found."),
                            Ok(r) => notify(
                                &app,
                                &format!("{} update(s) available", r.apps.len()),
                                "Open AutoUpdater to choose which apps to update.",
                            ),
                            Err(e) => notify(&app, "AutoUpdater", &e),
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app alive in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running AutoUpdater");
}
