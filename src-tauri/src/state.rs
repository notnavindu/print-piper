use crate::logging::LogRing;
use crate::models::{CaptureStatus, Endpoint, Job, Settings};
use crate::store;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};
use tauri::Manager;

pub const JOBS_CAPACITY: usize = 200;

pub struct AppState {
    pub app_data: PathBuf,
    pub spool_dir: PathBuf,
    endpoints_path: PathBuf,
    jobs_path: PathBuf,
    settings_path: PathBuf,

    pub endpoints: Mutex<Vec<Endpoint>>,
    pub jobs: Mutex<Vec<Job>>, // newest last
    pub settings: RwLock<Settings>,
    pub logs: Mutex<LogRing>,
    pub paused: AtomicBool,
    pub capture: Mutex<CaptureStatus>,
    pub http: reqwest::Client,
    pub mdns: Mutex<Option<mdns_sd::ServiceDaemon>>,
    /// load-time corruption notices, logged once the ring is live
    pub boot_warnings: Mutex<Vec<String>>,
}

/// One-time migration for the bundle-identifier rename
/// (`com.printpiper.app` → `com.printpiper.desktop`). If the new app-data dir
/// doesn't exist yet but the old one does, move it over so settings, endpoints,
/// and spool carry across untouched. Must run before the new dir is created.
fn migrate_legacy_app_data(new_dir: &std::path::Path) {
    const LEGACY_ID: &str = "com.printpiper.app";
    if new_dir.exists() {
        return;
    }
    let Some(parent) = new_dir.parent() else {
        return;
    };
    let legacy = parent.join(LEGACY_ID);
    if legacy.is_dir() {
        // same parent → same filesystem, so a rename is atomic and cheap
        let _ = std::fs::rename(&legacy, new_dir);
    }
}

impl AppState {
    pub fn init(app: &tauri::AppHandle) -> anyhow::Result<Self> {
        let app_data = app.path().app_data_dir()?;
        migrate_legacy_app_data(&app_data);
        let spool_dir = app_data.join("spool");
        std::fs::create_dir_all(&spool_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&spool_dir, std::fs::Permissions::from_mode(0o700))?;
        }

        let endpoints_path = app_data.join("endpoints.json");
        let jobs_path = app_data.join("jobs.json");
        let settings_path = app_data.join("settings.json");

        let mut warnings = Vec::new();
        let (endpoints, w1): (Vec<Endpoint>, _) = store::read_json(&endpoints_path);
        let (jobs, w2): (Vec<Job>, _) = store::read_json(&jobs_path);
        let (settings, w3): (Settings, _) = store::read_json(&settings_path);
        for w in [w1, w2, w3].into_iter().flatten() {
            warnings.push(w);
        }
        // persist settings back so the generated printer_uuid survives first run
        let _ = store::write_json_atomic(&settings_path, &settings);

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;

        Ok(Self {
            app_data,
            spool_dir,
            endpoints_path,
            jobs_path,
            settings_path,
            endpoints: Mutex::new(endpoints),
            jobs: Mutex::new(jobs),
            settings: RwLock::new(settings),
            logs: Mutex::new(LogRing::default()),
            paused: AtomicBool::new(false),
            capture: Mutex::new(CaptureStatus::default()),
            http,
            mdns: Mutex::new(None),
            boot_warnings: Mutex::new(warnings),
        })
    }

    pub fn save_endpoints(&self) -> anyhow::Result<()> {
        let endpoints = self.endpoints.lock().unwrap().clone();
        store::write_json_atomic(&self.endpoints_path, &endpoints)
    }

    pub fn save_jobs(&self) -> anyhow::Result<()> {
        let jobs = self.jobs.lock().unwrap().clone();
        store::write_json_atomic(&self.jobs_path, &jobs)
    }

    pub fn save_settings(&self) -> anyhow::Result<()> {
        let settings = self.settings.read().unwrap().clone();
        store::write_json_atomic(&self.settings_path, &settings)
    }
}
