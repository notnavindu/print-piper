use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

/// A per-send value the user fills in the picker before dispatch (e.g. "name").
/// Sent as a query param (GET/raw) or text form field (multipart) under `key`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EndpointVariable {
    pub key: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub url: String,
    /// "POST" | "GET"
    pub method: String,
    /// "multipart" | "raw" | "none"
    pub body_mode: String,
    /// multipart only: name of the file field
    #[serde(default = "default_file_field")]
    pub file_field: String,
    /// multipart only: extra text fields
    #[serde(default)]
    pub extra_fields: Vec<KeyValue>,
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    /// values the user provides at send time
    #[serde(default)]
    pub variables: Vec<EndpointVariable>,
    pub created_at: String,
    pub updated_at: String,
}

fn default_file_field() -> String {
    "file".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSend {
    pub endpoint_id: String,
    pub endpoint_name: String,
    pub at: String,
    pub ok: bool,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
    pub response_head: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub title: String,
    pub user: Option<String>,
    /// document-format as received over IPP — the Windows PDF-vs-raster tripwire
    pub format: String,
    pub bytes: u64,
    pub received_at: String,
    pub spool_path: String,
    /// "pending" | "sent" | "dismissed"
    pub status: String,
    #[serde(default)]
    pub sends: Vec<JobSend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub printer_name: String,
    pub port: u16,
    pub allow_lan: bool,
    pub autostart: bool,
    pub retention_max_jobs: usize,
    pub retention_max_days: i64,
    /// stable per-install printer UUID
    pub printer_uuid: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            printer_name: ">>> PIPE >>>".into(),
            port: 63163,
            allow_lan: false,
            // default off during development — flip at packaging (spec 00: a printer
            // that isn't running isn't a printer)
            autostart: false,
            retention_max_jobs: 50,
            retention_max_days: 7,
            printer_uuid: uuid::Uuid::new_v4().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CaptureStatus {
    pub running: bool,
    pub paused: bool,
    pub port: u16,
    pub printer_name: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchResult {
    pub job_id: String,
    pub endpoint_id: String,
    pub ok: bool,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
    pub response_head: Option<String>,
    pub error: Option<String>,
}
