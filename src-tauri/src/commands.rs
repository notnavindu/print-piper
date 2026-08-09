use crate::logging::plog;
use crate::models::{CaptureStatus, DispatchResult, Endpoint, Job, Settings};
use crate::state::AppState;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

// -- endpoints ---------------------------------------------------------------

#[tauri::command]
pub fn list_endpoints(state: State<'_, AppState>) -> Vec<Endpoint> {
    state.endpoints.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_endpoint(app: AppHandle, state: State<'_, AppState>, mut endpoint: Endpoint) -> Result<Endpoint, String> {
    if endpoint.name.trim().is_empty() {
        return Err("name is required".into());
    }
    let url = reqwest::Url::parse(&endpoint.url).map_err(|e| format!("invalid URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("URL must be http(s)".into());
    }
    if !matches!(endpoint.method.as_str(), "POST" | "GET") {
        return Err("method must be POST or GET".into());
    }
    if endpoint.method == "GET" {
        endpoint.body_mode = "none".into();
    } else if !matches!(endpoint.body_mode.as_str(), "multipart" | "raw") {
        return Err("body_mode must be multipart or raw for POST".into());
    }

    // variables: drop unnamed rows, trim keys, reject duplicates
    endpoint.variables.retain(|v| !v.key.trim().is_empty());
    for v in endpoint.variables.iter_mut() {
        v.key = v.key.trim().to_string();
        if v.transport.is_empty() {
            v.transport = "auto".into();
        }
        if !crate::models::VARIABLE_TRANSPORTS.contains(&v.transport.as_str()) {
            return Err(format!("unknown transport \"{}\" for variable \"{}\"", v.transport, v.key));
        }
        // catch in the editor what would otherwise only fail at send time
        if v.transport == "header" {
            crate::models::validate_header_var(&v.key, &v.default_value)?;
            // Content-Type is set by the body mode itself (and carries the multipart
            // boundary), so a variable of that name would append a second, conflicting
            // copy rather than replace it.
            if v.key.eq_ignore_ascii_case("content-type") {
                return Err(
                    "Content-Type is set by the body mode and can't be a header variable".into()
                );
            }
        }
    }
    {
        // header names are case-insensitive, so `X-Bill` and `x-bill` are one header —
        // catch that here rather than appending two copies of it at send time
        let mut seen = std::collections::HashSet::new();
        for v in &endpoint.variables {
            let ident = if v.transport == "header" {
                v.key.to_ascii_lowercase()
            } else {
                v.key.clone()
            };
            if !seen.insert(ident) {
                return Err(format!("duplicate variable \"{}\"", v.key));
            }
        }
    }

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    endpoint.updated_at = now.clone();
    {
        let mut endpoints = state.endpoints.lock().unwrap();
        if endpoint.id.is_empty() {
            endpoint.id = uuid::Uuid::new_v4().to_string();
            endpoint.created_at = now;
            endpoints.push(endpoint.clone());
        } else if let Some(existing) = endpoints.iter_mut().find(|e| e.id == endpoint.id) {
            endpoint.created_at = existing.created_at.clone();
            *existing = endpoint.clone();
        } else {
            endpoints.push(endpoint.clone());
        }
    }
    state.save_endpoints().map_err(|e| e.to_string())?;
    plog(&app, "info", "store", format!("endpoint saved: {}", endpoint.name), json!({ "id": endpoint.id }));
    let _ = app.emit("endpoints:changed", ());
    Ok(endpoint)
}

#[tauri::command]
pub fn delete_endpoint(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.endpoints.lock().unwrap().retain(|e| e.id != id);
    state.save_endpoints().map_err(|e| e.to_string())?;
    plog(&app, "info", "store", "endpoint deleted", json!({ "id": id }));
    let _ = app.emit("endpoints:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn test_endpoint(app: AppHandle, id: String) -> DispatchResult {
    crate::dispatch::test_endpoint(&app, &id).await
}

// -- jobs --------------------------------------------------------------------

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Vec<Job> {
    state.jobs.lock().unwrap().clone()
}

#[tauri::command]
pub async fn dispatch_job(
    app: AppHandle,
    job_id: String,
    endpoint_id: String,
    variables: Option<std::collections::HashMap<String, String>>,
) -> DispatchResult {
    crate::dispatch::dispatch_job(&app, &job_id, &endpoint_id, variables).await
}

/// Cancel a job: drop it entirely and delete its spooled document. No retry,
/// nothing left on disk — the opposite of hiding the window (which keeps it).
#[tauri::command]
pub fn cancel_job(app: AppHandle, state: State<'_, AppState>, job_id: String) {
    let spool_path = {
        let mut jobs = state.jobs.lock().unwrap();
        let path = jobs.iter().find(|j| j.id == job_id).map(|j| j.spool_path.clone());
        jobs.retain(|j| j.id != job_id);
        path
    };
    if let Some(p) = spool_path {
        let _ = std::fs::remove_file(&p);
    }
    let _ = state.save_jobs();
    plog(&app, "info", "system", "job cancelled — document discarded", json!({ "job_id": job_id }));
}

// -- logs --------------------------------------------------------------------

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>, after_seq: Option<u64>) -> Vec<crate::logging::LogEntry> {
    state.logs.lock().unwrap().list_after(after_seq.unwrap_or(0))
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) {
    state.logs.lock().unwrap().clear();
}

#[tauri::command]
pub fn export_logs(app: AppHandle, state: State<'_, AppState>) {
    let entries = state.logs.lock().unwrap().list_after(0);
    let payload = serde_json::to_string_pretty(&entries).unwrap_or_default();
    app.dialog()
        .file()
        .set_file_name("print-piper-logs.json")
        .save_file(move |path| {
            if let Some(path) = path {
                if let Some(p) = path.as_path() {
                    let _ = std::fs::write(p, &payload);
                }
            }
        });
}

// -- settings / status -------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    if settings.port < 1024 {
        return Err("port must be >= 1024".into());
    }
    if settings.printer_name.trim().is_empty() {
        return Err("printer name is required".into());
    }
    let restart_needed = {
        let current = state.settings.read().unwrap();
        current.port != settings.port || current.printer_name != settings.printer_name
    };
    {
        let mut s = state.settings.write().unwrap();
        // printer_uuid is not editable from the UI
        let uuid = s.printer_uuid.clone();
        *s = settings.clone();
        s.printer_uuid = uuid;
    }
    state.save_settings().map_err(|e| e.to_string())?;

    // live-apply autostart
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    let _ = if settings.autostart { autolaunch.enable() } else { autolaunch.disable() };

    plog(
        &app,
        "info",
        "system",
        if restart_needed {
            "settings saved — printer name/port changes apply after restart"
        } else {
            "settings saved"
        },
        json!({ "restart_needed": restart_needed }),
    );
    Ok(state.settings.read().unwrap().clone())
}

#[tauri::command]
pub fn get_capture_status(state: State<'_, AppState>) -> CaptureStatus {
    let mut status = state.capture.lock().unwrap().clone();
    status.paused = state.paused.load(std::sync::atomic::Ordering::Relaxed);
    status
}

#[tauri::command]
pub fn clear_spool(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut removed = 0u32;
    if let Ok(entries) = std::fs::read_dir(&state.spool_dir) {
        for entry in entries.flatten() {
            if std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    state.jobs.lock().unwrap().clear();
    let _ = state.save_jobs();
    plog(&app, "info", "system", format!("spool cleared ({removed} files)"), json!({ "removed": removed }));
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}
