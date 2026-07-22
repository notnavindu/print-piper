//! Lightweight update check: ask GitHub for the latest published release and
//! compare its version to ours. No self-updating — just a hint and a link out.

use serde::Serialize;
use tauri::{AppHandle, Manager};

const LATEST_API: &str =
    "https://api.github.com/repos/notnavindu/print-piper/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/notnavindu/print-piper/releases";

#[derive(Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    /// where to send the user — the specific release, or the releases page
    pub url: String,
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> UpdateInfo {
    let current = app.package_info().version.to_string();
    let fallback = UpdateInfo {
        current: current.clone(),
        latest: None,
        update_available: false,
        url: RELEASES_PAGE.to_string(),
    };

    let client = app.state::<crate::state::AppState>().http.clone();
    let resp = client
        .get(LATEST_API)
        // GitHub's API rejects requests without a User-Agent
        .header(reqwest::header::USER_AGENT, "print-piper")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await;

    // any failure (offline, rate-limited, no published release yet) → no nag
    let body = match resp {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        _ => return fallback,
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return fallback,
    };

    let tag = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
    let latest = tag.strip_prefix("print-piper@").unwrap_or(tag).to_string();
    let url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(RELEASES_PAGE)
        .to_string();

    let update_available = matches!(
        (semver::Version::parse(&latest), semver::Version::parse(&current)),
        (Ok(l), Ok(c)) if l > c
    );

    UpdateInfo {
        current,
        latest: (!latest.is_empty()).then_some(latest),
        update_available,
        url,
    }
}

/// Open an http(s) URL in the user's default browser. Scheme-guarded so it can
/// only ever launch web links (the URL comes from our own release check).
#[tauri::command]
pub fn open_external(url: String) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
}
