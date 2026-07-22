//! Embedded IPP server + mDNS advertising — the virtual printer itself.
//!
//! Built on ippper's SimpleIppService, with two additions per spec/01-capture.md:
//!  - PiperIppService: a delegating wrapper that recovers the `job-name` attribute
//!    (document title) that SimpleIppService discards. Print-Job carries it in the
//!    same request as the document (task-local scope); Create-Job announces it ahead
//!    of Send-Document (job-id → name map).
//!  - our own accept loop with a peer guard: loopback (or the host's own addresses)
//!    only, unless settings.allow_lan is set.

use crate::logging::plog;
use crate::models::Job;
use crate::state::{AppState, JOBS_CAPACITY};
use http::request::Parts as ReqParts;
use ipp::model::DelimiterTag;
use ipp::request::IppRequestResponse;
use ipp::value::IppValue;
use ippper::result::IppResult;
use ippper::server::wrap_as_http_service;
use ippper::service::simple::{
    PrinterInfoBuilder, SimpleIppDocument, SimpleIppService, SimpleIppServiceHandler,
};
use ippper::service::IppService;
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::TcpListener;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use uuid::Uuid;

tokio::task_local! {
    /// Document title for the job currently being handled on this task.
    static CURRENT_JOB_NAME: Option<String>;
}

// ---------------------------------------------------------------------------
// attribute helpers

fn ipp_value_to_string(v: &IppValue) -> Option<String> {
    match v {
        IppValue::NameWithoutLanguage(s) => Some(s.to_string()),
        IppValue::NameWithLanguage { name, .. } => Some(name.to_string()),
        IppValue::TextWithoutLanguage(s) => Some(s.to_string()),
        IppValue::TextWithLanguage { text, .. } => Some(text.to_string()),
        IppValue::Keyword(s) => Some(s.to_string()),
        _ => None,
    }
}

fn get_op_attr<'a>(req: &'a IppRequestResponse, name: &str) -> Option<&'a IppValue> {
    req.attributes()
        .groups_of(DelimiterTag::OperationAttributes)
        .find_map(|g| g.attributes().get(name))
        .map(|a| a.value())
}

fn take_job_name(req: &IppRequestResponse) -> Option<String> {
    get_op_attr(req, "job-name").and_then(ipp_value_to_string)
}

fn take_job_id_from_request(req: &IppRequestResponse) -> Option<i32> {
    match get_op_attr(req, "job-id") {
        Some(IppValue::Integer(id)) => Some(*id),
        _ => None,
    }
}

fn take_job_id_from_response(resp: &IppRequestResponse) -> Option<i32> {
    resp.attributes()
        .groups_of(DelimiterTag::JobAttributes)
        .find_map(|g| g.attributes().get("job-id"))
        .and_then(|a| match a.value() {
            IppValue::Integer(id) => Some(*id),
            _ => None,
        })
}

fn sanitize_title(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || "._- ".contains(c) { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim().replace(' ', "_");
    let capped: String = trimmed.chars().take(60).collect();
    if capped.is_empty() { "untitled".into() } else { capped }
}

fn extension_for_format(format: &str) -> &'static str {
    match format {
        "application/pdf" => "pdf",
        "image/pwg-raster" => "pwg",
        "application/PCLm" | "application/pclm" => "pclm",
        "image/urf" => "urf",
        "application/postscript" => "ps",
        _ => "bin",
    }
}

// ---------------------------------------------------------------------------
// document handler — receives the spooled document stream

struct PiperHandler {
    app: AppHandle,
}

impl SimpleIppServiceHandler for PiperHandler {
    fn handle_document(
        &self,
        document: SimpleIppDocument,
    ) -> impl futures::Future<Output = anyhow::Result<()>> + Send {
        let app = self.app.clone();
        async move { handle_incoming(app, document).await }
    }
}

async fn handle_incoming(app: AppHandle, document: SimpleIppDocument) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    if state.paused.load(Ordering::Relaxed) {
        plog(&app, "warn", "capture", "job rejected: capture is paused", json!({}));
        anyhow::bail!("Print Piper capture is paused");
    }

    let title_raw = CURRENT_JOB_NAME
        .try_with(|n| n.clone())
        .ok()
        .flatten()
        .unwrap_or_else(|| "Untitled print".to_string());
    let format = document
        .format
        .as_ref()
        .map(|f| f.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let user = document.job_attributes.originating_user_name.to_string();

    let id = Uuid::new_v4().to_string();
    let short_id = &id[..8];
    let filename = format!(
        "{}-{}.{}",
        short_id,
        sanitize_title(&title_raw),
        extension_for_format(&format)
    );
    let spool_path = state.spool_dir.join(&filename);

    let mut file = tokio::fs::File::create(&spool_path).await?;
    let bytes = tokio::io::copy(&mut document.payload.compat(), &mut file).await?;
    drop(file);

    if bytes == 0 {
        let _ = tokio::fs::remove_file(&spool_path).await;
        plog(&app, "warn", "capture", "empty document payload discarded", json!({ "title": title_raw }));
        return Ok(());
    }

    let job = Job {
        id: id.clone(),
        title: title_raw.clone(),
        user: Some(user.clone()),
        format: format.clone(),
        bytes,
        received_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        spool_path: spool_path.to_string_lossy().to_string(),
        status: "pending".into(),
        sends: vec![],
    };

    {
        let mut jobs = state.jobs.lock().unwrap();
        if jobs.len() >= JOBS_CAPACITY {
            jobs.remove(0);
        }
        jobs.push(job.clone());
    }
    if let Err(e) = state.save_jobs() {
        plog(&app, "error", "store", format!("failed to persist jobs: {e}"), json!({}));
    }

    let level = if format == "application/pdf" { "info" } else { "warn" };
    let msg = if format == "application/pdf" {
        format!("job spooled: \"{title_raw}\" ({bytes} bytes)")
    } else {
        format!("job spooled with NON-PDF format `{format}`: \"{title_raw}\" ({bytes} bytes) — Windows raster tripwire?")
    };
    plog(
        &app,
        level,
        "capture",
        msg,
        json!({
            "job_id": id, "title": title_raw, "user": user,
            "document_format": format, "bytes": bytes,
            "spool_path": job.spool_path,
        }),
    );

    let _ = app.emit("job:received", &job);

    // summon the picker
    if let Some(picker) = app.get_webview_window("picker") {
        let _ = picker.show();
        let _ = picker.set_focus();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// delegating IPP service — recovers job-name, passes everything else through

pub struct PiperIppService {
    inner: SimpleIppService<PiperHandler>,
    /// job-id → job-name announced in Create-Job, consumed by Send-Document
    pending_names: Mutex<HashMap<i32, String>>,
}

impl IppService for PiperIppService {
    fn version(&self) -> ipp::model::IppVersion {
        self.inner.version()
    }

    async fn print_job(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        let name = take_job_name(&req);
        CURRENT_JOB_NAME.scope(name, self.inner.print_job(head, req)).await
    }

    async fn create_job(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        let name = take_job_name(&req);
        let resp = self.inner.create_job(head, req).await;
        if let (Ok(r), Some(n)) = (&resp, name) {
            if let Some(id) = take_job_id_from_response(r) {
                self.pending_names.lock().unwrap().insert(id, n);
            }
        }
        resp
    }

    async fn send_document(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        let name = take_job_id_from_request(&req)
            .and_then(|id| self.pending_names.lock().unwrap().remove(&id));
        CURRENT_JOB_NAME.scope(name, self.inner.send_document(head, req)).await
    }

    async fn validate_job(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        self.inner.validate_job(head, req).await
    }

    async fn cancel_job(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        self.inner.cancel_job(head, req).await
    }

    async fn get_job_attributes(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        self.inner.get_job_attributes(head, req).await
    }

    async fn get_jobs(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        self.inner.get_jobs(head, req).await
    }

    async fn get_printer_attributes(&self, head: ReqParts, req: IppRequestResponse) -> IppResult {
        self.inner.get_printer_attributes(head, req).await
    }
}

// ---------------------------------------------------------------------------
// server + discovery

fn is_own_address(ip: &IpAddr) -> bool {
    if_addrs::get_if_addrs()
        .map(|addrs| addrs.iter().any(|a| a.ip() == *ip))
        .unwrap_or(false)
}

fn advertise_mdns(app: &AppHandle, printer_name: &str, port: u16, printer_uuid: &str) -> anyhow::Result<mdns_sd::ServiceDaemon> {
    let daemon = mdns_sd::ServiceDaemon::new()?;
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let host = format!("{hostname}.local.");

    let mut txt: HashMap<String, String> = HashMap::new();
    txt.insert("txtvers".into(), "1".into());
    txt.insert("qtotal".into(), "1".into());
    txt.insert("rp".into(), "ipp/print".into());
    txt.insert("ty".into(), printer_name.into());
    txt.insert("note".into(), "Print Piper virtual printer".into());
    txt.insert("pdl".into(), "application/pdf,application/octet-stream".into());
    txt.insert("product".into(), "(Print Piper)".into());
    txt.insert("Color".into(), "T".into());
    txt.insert("Duplex".into(), "F".into());
    txt.insert("UUID".into(), printer_uuid.into());
    txt.insert("adminurl".into(), format!("http://localhost:{port}/"));
    txt.insert("priority".into(), "0".into());

    let service = mdns_sd::ServiceInfo::new(
        "_ipp._tcp.local.",
        printer_name,
        &host,
        "",
        port,
        txt,
    )?
    .enable_addr_auto();

    daemon.register(service)?;
    plog(
        app,
        "info",
        "discovery",
        format!("mDNS advertising \"{printer_name}\" on port {port}"),
        json!({ "host": host, "port": port, "type": "_ipp._tcp" }),
    );
    Ok(daemon)
}

fn emit_status(app: &AppHandle) {
    let state = app.state::<AppState>();
    let status = state.capture.lock().unwrap().clone();
    let _ = app.emit("capture:status", &status);
}

/// Entry point: build the service, advertise, run the accept loop. Never returns
/// unless startup fails; spawned on the tauri async runtime.
pub async fn run_capture(app: AppHandle) {
    let (printer_name, port, printer_uuid) = {
        let state = app.state::<AppState>();
        let s = state.settings.read().unwrap();
        (s.printer_name.clone(), s.port, s.printer_uuid.clone())
    };

    let info = match PrinterInfoBuilder::default()
        .name(printer_name.as_str().try_into().expect("printer name"))
        .dnssd_name(Some(printer_name.as_str().try_into().expect("printer name")))
        .info(Some("Print Piper — print to an API endpoint".try_into().unwrap()))
        .make_and_model(Some("Print Piper Virtual Printer".try_into().unwrap()))
        .uuid(Uuid::parse_str(&printer_uuid).ok())
        .document_format_supported(vec![
            "application/pdf".try_into().unwrap(),
            "application/octet-stream".try_into().unwrap(),
        ])
        .build()
    {
        Ok(i) => i,
        Err(e) => {
            capture_failed(&app, format!("printer info: {e}"));
            return;
        }
    };

    let mut simple = SimpleIppService::new(info, PiperHandler { app: app.clone() });
    simple.set_basepath("/ipp/print");
    let service = Arc::new(PiperIppService {
        inner: simple,
        pending_names: Mutex::new(HashMap::new()),
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            capture_failed(&app, format!("cannot bind port {port}: {e}"));
            return;
        }
    };

    match advertise_mdns(&app, &printer_name, port, &printer_uuid) {
        Ok(daemon) => {
            let state = app.state::<AppState>();
            *state.mdns.lock().unwrap() = Some(daemon);
        }
        Err(e) => {
            // capture still works via manual ipp://localhost add — log, don't die
            plog(&app, "error", "discovery", format!("mDNS advertising failed: {e}"), json!({}));
        }
    }

    {
        let state = app.state::<AppState>();
        let mut cap = state.capture.lock().unwrap();
        cap.running = true;
        cap.port = port;
        cap.printer_name = printer_name.clone();
        cap.error = None;
    }
    plog(
        &app,
        "info",
        "capture",
        format!("IPP server listening on port {port}"),
        json!({ "port": port, "printer_name": printer_name }),
    );
    emit_status(&app);

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(x) => x,
            Err(e) => {
                plog(&app, "error", "capture", format!("accept error: {e}"), json!({}));
                continue;
            }
        };

        let allow_lan = {
            let state = app.state::<AppState>();
            let allowed = state.settings.read().unwrap().allow_lan;
            allowed
        };
        let peer_ip = peer.ip();
        if !peer_ip.is_loopback() && !allow_lan && !is_own_address(&peer_ip) {
            plog(
                &app,
                "warn",
                "capture",
                format!("rejected connection from non-local {peer_ip}"),
                json!({ "peer": peer.to_string() }),
            );
            continue; // drop the stream
        }

        let svc = wrap_as_http_service(service.clone());
        let app_conn = app.clone();
        tokio::spawn(async move {
            if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                hyper_util::rt::TokioExecutor::new(),
            )
            .serve_connection(hyper_util::rt::TokioIo::new(stream), svc)
            .await
            {
                plog(
                    &app_conn,
                    "debug",
                    "capture",
                    format!("connection ended: {e}"),
                    json!({}),
                );
            }
        });
    }
}

fn capture_failed(app: &AppHandle, error: String) {
    plog(app, "error", "capture", format!("capture failed to start: {error}"), json!({}));
    let state = app.state::<AppState>();
    {
        let mut cap = state.capture.lock().unwrap();
        cap.running = false;
        cap.error = Some(error);
    }
    emit_status(app);
}
