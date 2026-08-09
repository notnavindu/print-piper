//! Outbound delivery: build the request an endpoint describes, send the PDF, record the outcome.
//! No retries in v1 (spec decision) — a failed send stays visible in the picker for a manual re-click.

use crate::logging::plog;
use crate::models::{DispatchResult, Endpoint, Job, JobSend};
use crate::state::AppState;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const SAMPLE_PDF: &[u8] = include_bytes!("../assets/sample.pdf");
const RESPONSE_HEAD_BYTES: usize = 512;

enum Payload {
    Spooled { path: String, bytes: u64 },
    Sample,
}

/// A variable that survived resolution, with the transport it should ride on.
#[derive(Debug, Clone, PartialEq)]
struct ResolvedVar {
    key: String,
    value: String,
    /// "auto" | "query" | "header"
    transport: String,
}

/// Resolve declared variables against user-provided values: provided → default →
/// error if required and still empty. Only declared keys are ever sent.
///
/// Header-bound variables are validated here rather than at send time so the failure
/// arrives before a single byte of the document leaves the machine, and names the
/// variable instead of surfacing reqwest's opaque "invalid HTTP header value".
fn resolve_variables(
    endpoint: &Endpoint,
    provided: Option<&HashMap<String, String>>,
    fallback: Option<&str>,
) -> Result<Vec<ResolvedVar>, String> {
    let mut out = Vec::new();
    for var in &endpoint.variables {
        let mut value = provided
            .and_then(|m| m.get(&var.key))
            .filter(|v| !v.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| var.default_value.clone());
        if value.trim().is_empty() {
            match fallback {
                Some(f) => value = f.to_string(),
                None if var.required => {
                    return Err(format!("missing required variable \"{}\"", var.key))
                }
                None => continue, // optional and empty — don't send at all
            }
        }
        let transport = if var.transport.is_empty() { "auto" } else { var.transport.as_str() };
        // save_endpoint whitelists this, but endpoints.json is not re-validated on load, so
        // a hand-edited or downgraded file can carry anything. Refuse it rather than let
        // `rides_url`'s catch-all quietly route a header-bound value into the URL — that is
        // the access-log leak this feature exists to prevent.
        if !crate::models::VARIABLE_TRANSPORTS.contains(&transport) {
            return Err(format!(
                "variable \"{}\" has an unknown transport \"{}\"",
                var.key, transport
            ));
        }
        if transport == "header" {
            crate::models::validate_header_var(&var.key, &value)?;
        }
        out.push(ResolvedVar { key: var.key.clone(), value, transport: transport.to_string() });
    }
    Ok(out)
}

/// Does this variable ride in the URL? Split out of `build_and_send` so the routing
/// table is testable without a live AppHandle.
fn rides_url(transport: &str, auto_rides_url: bool) -> bool {
    match transport {
        "query" => true,
        "header" => false,
        _ => auto_rides_url, // "auto"; resolve_variables rejects anything unrecognised
    }
}

pub async fn dispatch_job(
    app: &AppHandle,
    job_id: &str,
    endpoint_id: &str,
    variables: Option<HashMap<String, String>>,
) -> DispatchResult {
    let state = app.state::<AppState>();
    let job: Option<Job> = state.jobs.lock().unwrap().iter().find(|j| j.id == job_id).cloned();
    let endpoint: Option<Endpoint> =
        state.endpoints.lock().unwrap().iter().find(|e| e.id == endpoint_id).cloned();

    let (job, endpoint) = match (job, endpoint) {
        (Some(j), Some(e)) => (j, e),
        _ => {
            return DispatchResult {
                job_id: job_id.into(),
                endpoint_id: endpoint_id.into(),
                ok: false,
                http_status: None,
                duration_ms: 0,
                response_head: None,
                error: Some("job or endpoint not found".into()),
            }
        }
    };

    // fail fast, before any bytes leave the machine
    let vars = match resolve_variables(&endpoint, variables.as_ref(), None) {
        Ok(v) => v,
        Err(msg) => {
            return DispatchResult {
                job_id: job_id.into(),
                endpoint_id: endpoint_id.into(),
                ok: false,
                http_status: None,
                duration_ms: 0,
                response_head: None,
                error: Some(msg),
            }
        }
    };

    let payload = Payload::Spooled { path: job.spool_path.clone(), bytes: job.bytes };
    let result = execute(app, &endpoint, &job.title, &job.format, payload, &job.id, &vars).await;

    // record the send on the job
    {
        let state = app.state::<AppState>();
        let mut jobs = state.jobs.lock().unwrap();
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job.id) {
            j.sends.push(JobSend {
                endpoint_id: endpoint.id.clone(),
                endpoint_name: endpoint.name.clone(),
                at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                ok: result.ok,
                http_status: result.http_status,
                duration_ms: result.duration_ms,
                response_head: result.response_head.clone(),
                error: result.error.clone(),
            });
            if result.ok {
                j.status = "sent".into();
            }
        }
    }
    let _ = app.state::<AppState>().save_jobs();

    let _ = app.emit("dispatch:result", &result);
    let body = if result.ok {
        format!("\"{}\" → {}", job.title, endpoint.name)
    } else {
        format!("\"{}\" → {} failed: {}", job.title, endpoint.name,
            result.error.clone().unwrap_or_else(|| format!("HTTP {}", result.http_status.unwrap_or(0))))
    };
    let _ = app
        .notification()
        .builder()
        .title(if result.ok { "Sent ✓" } else { "Send failed ✗" })
        .body(body)
        .show();

    result
}

/// Endpoint "Test" button: same request shape, tiny built-in sample PDF.
pub async fn test_endpoint(app: &AppHandle, endpoint_id: &str) -> DispatchResult {
    let state = app.state::<AppState>();
    let endpoint: Option<Endpoint> =
        state.endpoints.lock().unwrap().iter().find(|e| e.id == endpoint_id).cloned();
    match endpoint {
        Some(e) => {
            // tests never block on required vars: default → "test". They can still fail
            // on a malformed variable though, and that must surface — swallowing it would
            // send the sample with NO variables and report a green pass on a broken config.
            match resolve_variables(&e, None, Some("test")) {
                Ok(vars) => {
                    execute(app, &e, "print-piper-test", "application/pdf", Payload::Sample, "test", &vars)
                        .await
                }
                Err(msg) => DispatchResult {
                    job_id: "test".into(),
                    endpoint_id: endpoint_id.into(),
                    ok: false,
                    http_status: None,
                    duration_ms: 0,
                    response_head: None,
                    error: Some(msg),
                },
            }
        }
        None => DispatchResult {
            job_id: "test".into(),
            endpoint_id: endpoint_id.into(),
            ok: false,
            http_status: None,
            duration_ms: 0,
            response_head: None,
            error: Some("endpoint not found".into()),
        },
    }
}

async fn execute(
    app: &AppHandle,
    endpoint: &Endpoint,
    title: &str,
    format: &str,
    payload: Payload,
    job_id: &str,
    vars: &[ResolvedVar],
) -> DispatchResult {
    let started = Instant::now();
    let client = app.state::<AppState>().http.clone();
    let outcome = build_and_send(&client, endpoint, title, format, payload, vars).await;
    let duration_ms = started.elapsed().as_millis() as u64;

    let result = match outcome {
        Ok((status, head)) => DispatchResult {
            job_id: job_id.into(),
            endpoint_id: endpoint.id.clone(),
            ok: (200..300).contains(&status),
            http_status: Some(status),
            duration_ms,
            response_head: Some(head),
            error: None,
        },
        Err(e) => DispatchResult {
            job_id: job_id.into(),
            endpoint_id: endpoint.id.clone(),
            ok: false,
            http_status: None,
            duration_ms,
            response_head: None,
            error: Some(format!("{e:#}")),
        },
    };

    let host = reqwest::Url::parse(&endpoint.url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| "?".into());
    plog(
        app,
        if result.ok { "info" } else { "error" },
        "dispatch",
        format!(
            "{} {} \"{}\" → {} ({}, {} ms)",
            endpoint.method, host, title, endpoint.name,
            result.http_status.map(|s| s.to_string()).unwrap_or_else(|| "no response".into()),
            duration_ms
        ),
        json!({
            "job_id": job_id, "endpoint_id": endpoint.id, "url": endpoint.url,
            "method": endpoint.method, "body_mode": endpoint.body_mode,
            "http_status": result.http_status, "duration_ms": duration_ms,
            "response_head": result.response_head, "error": result.error,
            "header_keys": endpoint.headers.iter().map(|h| h.key.clone()).collect::<Vec<_>>(),
            // keys and transports only — variable *values* are never logged
            "variable_keys": vars.iter().map(|v| format!("{} via {}", v.key, v.transport)).collect::<Vec<_>>(),
        }),
    );

    result
}

async fn build_and_send(
    client: &reqwest::Client,
    endpoint: &Endpoint,
    title: &str,
    format: &str,
    payload: Payload,
    vars: &[ResolvedVar],
) -> anyhow::Result<(u16, String)> {
    let mut url = reqwest::Url::parse(&endpoint.url)?;

    // `auto` keeps the original rule: ride the URL for GET/raw, otherwise the multipart
    // form. `query` and `header` override it, so a raw-PDF endpoint can put a value in a
    // header instead of the URL (where it would land in access logs).
    let auto_rides_url = endpoint.method == "GET" || endpoint.body_mode == "raw";
    let query_vars: Vec<&ResolvedVar> =
        vars.iter().filter(|v| rides_url(&v.transport, auto_rides_url)).collect();
    let header_vars: Vec<&ResolvedVar> = vars.iter().filter(|v| v.transport == "header").collect();
    let form_vars: Vec<&ResolvedVar> = vars
        .iter()
        .filter(|v| v.transport != "header" && !rides_url(&v.transport, auto_rides_url))
        .collect();

    if !query_vars.is_empty() {
        url.query_pairs_mut()
            .extend_pairs(query_vars.iter().map(|v| (v.key.as_str(), v.value.as_str())));
    }

    let ext = if format == "application/pdf" { "pdf" } else { "bin" };
    let filename = format!("{}.{}", crate::capture_sanitize(title), ext);

    let mut req = match endpoint.method.as_str() {
        "GET" => {
            let bytes = match &payload {
                Payload::Spooled { bytes, .. } => *bytes,
                Payload::Sample => SAMPLE_PDF.len() as u64,
            };
            client.get(url).query(&[
                ("title", title.to_string()),
                ("bytes", bytes.to_string()),
                ("format", format.to_string()),
            ])
        }
        _ => match endpoint.body_mode.as_str() {
            "raw" => {
                let builder = client.post(url).header(reqwest::header::CONTENT_TYPE, format.to_string());
                match payload {
                    Payload::Spooled { path, .. } => {
                        let file = tokio::fs::File::open(&path).await?;
                        let stream = tokio_util::io::ReaderStream::new(file);
                        builder.body(reqwest::Body::wrap_stream(stream))
                    }
                    Payload::Sample => builder.body(SAMPLE_PDF.to_vec()),
                }
            }
            _ => {
                // multipart (default)
                let part = match payload {
                    Payload::Spooled { path, bytes } => {
                        let file = tokio::fs::File::open(&path).await?;
                        let stream = tokio_util::io::ReaderStream::new(file);
                        reqwest::multipart::Part::stream_with_length(
                            reqwest::Body::wrap_stream(stream),
                            bytes,
                        )
                    }
                    Payload::Sample => reqwest::multipart::Part::bytes(SAMPLE_PDF.to_vec()),
                }
                .file_name(filename.clone())
                .mime_str(format)?;

                let mut form = reqwest::multipart::Form::new();
                for f in &endpoint.extra_fields {
                    form = form.text(f.key.clone(), f.value.clone());
                }
                for v in &form_vars {
                    form = form.text(v.key.clone(), v.value.clone());
                }
                let field = if endpoint.file_field.is_empty() { "file".to_string() } else { endpoint.file_field.clone() };
                form = form.part(field, part);
                client.post(url).multipart(form)
            }
        },
    };

    // A header-bound variable replaces a static header of the same name instead of
    // appending a second copy of it (reqwest's `.header()` appends).
    let overridden = |key: &str| header_vars.iter().any(|v| v.key.eq_ignore_ascii_case(key));
    for h in endpoint.headers.iter().filter(|h| !overridden(&h.key)) {
        req = req.header(h.key.as_str(), h.value.as_str());
    }
    for v in &header_vars {
        req = req.header(v.key.as_str(), v.value.as_str());
    }

    let resp = req.send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    let head: String = text.chars().take(RESPONSE_HEAD_BYTES).collect();
    Ok((status, head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EndpointVariable;

    fn ep(vars: Vec<EndpointVariable>) -> Endpoint {
        Endpoint {
            id: "e1".into(),
            name: "test".into(),
            url: "http://localhost/hook".into(),
            method: "POST".into(),
            body_mode: "multipart".into(),
            file_field: "file".into(),
            extra_fields: vec![],
            headers: vec![],
            variables: vars,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn var(key: &str, required: bool, default_value: &str) -> EndpointVariable {
        EndpointVariable {
            key: key.into(),
            label: String::new(),
            required,
            default_value: default_value.into(),
            transport: "auto".into(),
        }
    }

    fn var_via(key: &str, transport: &str, default_value: &str) -> EndpointVariable {
        EndpointVariable { transport: transport.into(), ..var(key, false, default_value) }
    }

    /// existing assertions read as key/value pairs — transport is checked separately
    fn pairs(vars: &[ResolvedVar]) -> Vec<(String, String)> {
        vars.iter().map(|v| (v.key.clone(), v.value.clone())).collect()
    }

    #[test]
    fn provided_value_wins_over_default() {
        let e = ep(vec![var("name", true, "fallback")]);
        let provided: HashMap<String, String> = [("name".to_string(), "Ada".to_string())].into();
        let out = resolve_variables(&e, Some(&provided), None).unwrap();
        assert_eq!(pairs(&out), vec![("name".to_string(), "Ada".to_string())]);
    }

    #[test]
    fn default_fills_missing() {
        let e = ep(vec![var("name", true, "fallback")]);
        let out = resolve_variables(&e, None, None).unwrap();
        assert_eq!(pairs(&out), vec![("name".to_string(), "fallback".to_string())]);
    }

    #[test]
    fn required_and_empty_errors() {
        let e = ep(vec![var("name", true, "")]);
        let err = resolve_variables(&e, None, None).unwrap_err();
        assert!(err.contains("name"), "error should name the variable: {err}");
    }

    #[test]
    fn optional_and_empty_is_omitted() {
        let e = ep(vec![var("note", false, ""), var("kept", false, "v")]);
        let out = resolve_variables(&e, None, None).unwrap();
        assert_eq!(pairs(&out), vec![("kept".to_string(), "v".to_string())]);
    }

    #[test]
    fn whitespace_counts_as_empty() {
        let e = ep(vec![var("name", true, "")]);
        let provided: HashMap<String, String> = [("name".to_string(), "   ".to_string())].into();
        assert!(resolve_variables(&e, Some(&provided), None).is_err());
    }

    #[test]
    fn test_fallback_never_blocks() {
        let e = ep(vec![var("name", true, "")]);
        let out = resolve_variables(&e, None, Some("test")).unwrap();
        assert_eq!(pairs(&out), vec![("name".to_string(), "test".to_string())]);
    }

    #[test]
    fn transport_survives_resolution() {
        let e = ep(vec![var_via("X-Bill", "header", "B12"), var_via("note", "query", "hi")]);
        let out = resolve_variables(&e, None, None).unwrap();
        assert_eq!(out[0].transport, "header");
        assert_eq!(out[1].transport, "query");
    }

    #[test]
    fn blank_transport_reads_as_auto() {
        let e = ep(vec![var_via("name", "", "v")]);
        let out = resolve_variables(&e, None, None).unwrap();
        assert_eq!(out[0].transport, "auto");
    }

    /// endpoints.json written before transports existed must keep working untouched
    #[test]
    fn legacy_variable_json_defaults_to_auto() {
        let json = r#"{"key":"name","label":"Name","required":true,"default_value":""}"#;
        let v: EndpointVariable = serde_json::from_str(json).unwrap();
        assert_eq!(v.transport, "auto");
    }

    #[test]
    fn header_transport_rejects_non_ascii_value() {
        let e = ep(vec![var_via("X-Patient-Name", "header", "සුනිල්")]);
        let err = resolve_variables(&e, None, None).unwrap_err();
        assert!(err.contains("X-Patient-Name"), "error should name the variable: {err}");
        assert!(err.contains("query"), "error should point at a transport that works: {err}");
    }

    #[test]
    fn header_transport_rejects_illegal_header_name() {
        let e = ep(vec![var_via("patient name", "header", "Ada")]);
        let err = resolve_variables(&e, None, None).unwrap_err();
        assert!(err.contains("patient name"), "error should name the variable: {err}");
    }

    /// The Test button's "test" fallback fills blanks; it must not paper over a
    /// variable that can't be sent at all, or Test reports green on a broken endpoint.
    #[test]
    fn test_fallback_does_not_rescue_an_unsendable_header() {
        let e = ep(vec![var_via("X-Patient-Name", "header", "සුනිල්")]);
        assert!(resolve_variables(&e, None, Some("test")).is_err());
    }

    /// endpoints.json is not re-validated on load, so an unknown transport must fail
    /// loudly instead of falling back to auto and routing a header value into the URL.
    #[test]
    fn unknown_transport_is_rejected_not_silently_auto() {
        let e = ep(vec![var_via("X-Bill", "Header", "B-42")]);
        let err = resolve_variables(&e, None, None).unwrap_err();
        assert!(err.contains("unknown transport"), "{err}");
        assert!(err.contains("X-Bill"), "error should name the variable: {err}");
    }

    #[test]
    fn non_ascii_is_fine_on_other_transports() {
        let e = ep(vec![var_via("patientName", "query", "සුනිල්"), var_via("n", "auto", "සුනිල්")]);
        assert!(resolve_variables(&e, None, None).is_ok());
    }

    #[test]
    fn routing_table() {
        // auto follows the endpoint shape …
        assert!(rides_url("auto", true), "auto on GET/raw rides the URL");
        assert!(!rides_url("auto", false), "auto on multipart rides the form");
        // … the explicit transports do not
        assert!(rides_url("query", false), "query overrides multipart");
        assert!(!rides_url("header", true), "header overrides GET/raw");
    }

    /// Bind a throwaway listener, capture the first request in full, reply 200.
    /// Returns the URL to point an endpoint at, plus the captured bytes.
    async fn capture_one_request() -> (String, tokio::sync::oneshot::Receiver<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/ingest", listener.local_addr().unwrap());
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            // Read exactly the head plus its declared Content-Length. Waiting for EOF would
            // deadlock: the client holds the connection open for our response, so `read`
            // never returns Ok(0) and nothing would write the 200.
            loop {
                match sock.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
                let Some(head_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue; // head still arriving
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
                let want: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if buf.len() >= head_end + 4 + want {
                    break;
                }
            }
            let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
            let _ = sock.flush().await;
            let _ = tx.send(String::from_utf8_lossy(&buf).to_string());
        });
        (url, rx)
    }

    /// A timeout turns a wedged test server into a failing test rather than a
    /// `cargo test` that hangs forever in CI.
    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap()
    }

    fn resolved(key: &str, value: &str, transport: &str) -> ResolvedVar {
        ResolvedVar { key: key.into(), value: value.into(), transport: transport.into() }
    }

    /// The wiring the routing table can't prove: that a header-bound variable really
    /// leaves as a header and a query-bound one really leaves in the URL.
    #[tokio::test]
    async fn raw_post_puts_each_variable_on_its_declared_transport() {
        let (url, rx) = capture_one_request().await;
        let mut e = ep(vec![]);
        e.url = url;
        e.body_mode = "raw".into();
        e.headers = vec![crate::models::KeyValue {
            key: "Authorization".into(),
            value: "Bearer secret".into(),
        }];
        let vars = vec![
            resolved("X-Patient-Name", "Ada L", "header"),
            resolved("billNumber", "B-42", "query"),
            resolved("note", "auto-goes-to-url", "auto"),
        ];
        let client = test_client();
        let (status, _) =
            build_and_send(&client, &e, "report", "application/pdf", Payload::Sample, &vars)
                .await
                .unwrap();
        assert_eq!(status, 200);

        let req = rx.await.unwrap();
        let head = req.split("\r\n\r\n").next().unwrap().to_string();
        assert!(head.contains("x-patient-name: Ada L"), "header var missing:\n{head}");
        assert!(head.contains("authorization: Bearer secret"), "static header missing:\n{head}");
        assert!(head.contains("billNumber=B-42"), "query var not in the URL:\n{head}");
        assert!(head.contains("note=auto-goes-to-url"), "auto var not in the URL:\n{head}");
        // the header-bound one must NOT have also leaked into the URL
        assert!(!head.contains("X-Patient-Name=Ada"), "header var also rode the URL:\n{head}");
    }

    #[tokio::test]
    async fn multipart_query_variable_skips_the_form_and_rides_the_url() {
        let (url, rx) = capture_one_request().await;
        let mut e = ep(vec![]);
        e.url = url;
        let vars = vec![
            resolved("billNumber", "B-42", "query"),
            resolved("patientName", "සුනිල්", "auto"),
        ];
        let client = test_client();
        build_and_send(&client, &e, "report", "application/pdf", Payload::Sample, &vars)
            .await
            .unwrap();

        let req = rx.await.unwrap();
        assert!(req.contains("billNumber=B-42"), "query var not in the URL:\n{req}");
        assert!(!req.contains("name=\"billNumber\""), "query var duplicated into the form:\n{req}");
        // a multipart field is UTF-8, so non-ASCII survives here where a header couldn't
        assert!(req.contains("name=\"patientName\""), "auto var missing from the form:\n{req}");
        assert!(req.contains("සුනිල්"), "UTF-8 form value mangled:\n{req}");
    }

    /// A GET carries no body, so the capture server sees only a request head. This is the
    /// case that hung forever when the read loop waited for an EOF the client never sends.
    #[tokio::test]
    async fn get_endpoint_sends_metadata_and_variables_as_query() {
        let (url, rx) = capture_one_request().await;
        let mut e = ep(vec![]);
        e.url = url;
        e.method = "GET".into();
        e.body_mode = "none".into();
        let vars = vec![resolved("X-Bill", "B-42", "header"), resolved("note", "hi", "auto")];
        let client = test_client();
        let (status, _) = build_and_send(&client, &e, "report", "application/pdf", Payload::Sample, &vars)
            .await
            .unwrap();
        assert_eq!(status, 200);

        let req = rx.await.unwrap();
        assert!(req.contains("note=hi"), "auto var should ride the URL on GET:\n{req}");
        assert!(req.contains("bytes="), "GET metadata missing:\n{req}");
        assert!(req.contains("x-bill: B-42"), "header var should still be a header:\n{req}");
    }

    #[tokio::test]
    async fn header_variable_replaces_a_static_header_of_the_same_name() {
        let (url, rx) = capture_one_request().await;
        let mut e = ep(vec![]);
        e.url = url;
        e.body_mode = "raw".into();
        e.headers =
            vec![crate::models::KeyValue { key: "X-Bill-Number".into(), value: "static".into() }];
        let vars = vec![resolved("x-bill-number", "B-42", "header")];
        let client = test_client();
        build_and_send(&client, &e, "report", "application/pdf", Payload::Sample, &vars)
            .await
            .unwrap();

        let req = rx.await.unwrap();
        assert!(req.contains("x-bill-number: B-42"), "variable should win:\n{req}");
        assert!(!req.contains("static"), "static header should not also be sent:\n{req}");
    }

    #[test]
    fn undeclared_provided_keys_are_ignored() {
        let e = ep(vec![]);
        let provided: HashMap<String, String> = [("evil".to_string(), "x".to_string())].into();
        let out = resolve_variables(&e, Some(&provided), None).unwrap();
        assert!(out.is_empty());
    }
}
