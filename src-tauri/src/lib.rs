mod capture;
mod commands;
mod dispatch;
mod logging;
mod models;
mod state;
mod store;
mod update;

use logging::plog;
use serde_json::json;
use state::AppState;
use std::sync::atomic::Ordering;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

/// Re-exported for dispatch (filename building shares capture's sanitizer).
pub(crate) fn capture_sanitize(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_alphanumeric() || "._- ".contains(c) { c } else { '_' })
        .collect::<String>()
        .trim()
        .replace(' ', "_")
        .chars()
        .take(60)
        .collect::<String>()
}

fn prune_spool(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (max_jobs, max_days) = {
        let s = state.settings.read().unwrap();
        (s.retention_max_jobs, s.retention_max_days)
    };
    let cutoff = chrono::Utc::now() - chrono::Duration::days(max_days);
    let mut dropped: Vec<String> = Vec::new();
    {
        let mut jobs = state.jobs.lock().unwrap();
        let total = jobs.len();
        let mut keep: Vec<bool> = jobs
            .iter()
            .map(|j| {
                chrono::DateTime::parse_from_rfc3339(&j.received_at)
                    .map(|t| t.with_timezone(&chrono::Utc) > cutoff)
                    .unwrap_or(true)
            })
            .collect();
        // enforce max_jobs: oldest first (jobs are newest-last)
        let excess = total.saturating_sub(max_jobs);
        for k in keep.iter_mut().take(excess) {
            *k = false;
        }
        let mut i = 0;
        jobs.retain(|j| {
            let keep_it = keep[i];
            i += 1;
            if !keep_it {
                dropped.push(j.spool_path.clone());
            }
            keep_it
        });
    }
    if !dropped.is_empty() {
        for path in &dropped {
            let _ = std::fs::remove_file(path);
        }
        let _ = state.save_jobs();
        plog(app, "info", "system", format!("pruned {} old jobs from spool", dropped.len()), json!({ "count": dropped.len() }));
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open", "Open Print Piper").build(app)?;
    let pause = CheckMenuItemBuilder::with_id("pause", "Pause capture")
        .checked(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app).item(&open).item(&pause).separator().item(&quit).build()?;

    // decoded at compile time; icon_as_template lets macOS tint the white
    // silhouette for light/dark menu bars (no-op elsewhere)
    let tray_icon = tauri::include_image!("icons/tray.png");

    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "pause" => {
                let state = app.state::<AppState>();
                let was = state.paused.load(Ordering::Relaxed);
                state.paused.store(!was, Ordering::Relaxed);
                plog(
                    app,
                    "info",
                    "capture",
                    if was { "capture resumed" } else { "capture paused" },
                    json!({}),
                );
                let status = commands::get_capture_status(app.state::<AppState>());
                let _ = app.emit("capture:status", &status);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("print-piper".into()),
                    }),
                ])
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::init(app.handle())?;
            app.manage(state);

            // surface any config-corruption found during load
            let warnings: Vec<String> = app
                .state::<AppState>()
                .boot_warnings
                .lock()
                .unwrap()
                .drain(..)
                .collect();
            for w in warnings {
                plog(app.handle(), "error", "store", w, json!({}));
            }

            // apply autostart preference
            {
                use tauri_plugin_autostart::ManagerExt;
                let want = app.state::<AppState>().settings.read().unwrap().autostart;
                let autolaunch = app.autolaunch();
                let is = autolaunch.is_enabled().unwrap_or(false);
                if want && !is {
                    let _ = autolaunch.enable();
                } else if !want && is {
                    let _ = autolaunch.disable();
                }
            }

            build_tray(app.handle())?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                capture::run_capture(handle).await;
            });

            // re-summon the picker if we restarted with unsent captures
            {
                let pending = app
                    .state::<AppState>()
                    .jobs
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|j| j.status == "pending")
                    .count();
                if pending > 0 {
                    if let Some(w) = app.get_webview_window("picker") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    plog(
                        app.handle(),
                        "info",
                        "system",
                        format!("{pending} pending job(s) from last run — picker re-summoned"),
                        json!({ "pending": pending }),
                    );
                }
            }

            // retention prune: at boot and hourly
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
                loop {
                    interval.tick().await;
                    prune_spool(&handle);
                }
            });

            plog(
                app.handle(),
                "info",
                "system",
                format!("Print Piper {} started", env!("CARGO_PKG_VERSION")),
                json!({ "os": std::env::consts::OS }),
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // both windows hide instead of closing; quit lives in the tray
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_endpoints,
            commands::save_endpoint,
            commands::delete_endpoint,
            commands::test_endpoint,
            commands::list_jobs,
            commands::dispatch_job,
            commands::cancel_job,
            commands::get_logs,
            commands::clear_logs,
            commands::export_logs,
            commands::get_settings,
            commands::save_settings,
            commands::get_capture_status,
            commands::clear_spool,
            commands::show_main_window,
            update::check_update,
            update::open_external,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
