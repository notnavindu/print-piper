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

/// Resolve declared variables against user-provided values: provided → default →
/// error if required and still empty. Only declared keys are ever sent.
fn resolve_variables(
    endpoint: &Endpoint,
    provided: Option<&HashMap<String, String>>,
    fallback: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
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
        out.push((var.key.clone(), value));
    }
    Ok(out)
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
            // tests never block on required vars: default → "test"
            let vars = resolve_variables(&e, None, Some("test")).unwrap_or_default();
            execute(app, &e, "print-piper-test", "application/pdf", Payload::Sample, "test", &vars).await
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
    vars: &[(String, String)],
) -> DispatchResult {
    let started = Instant::now();
    let outcome = build_and_send(app, endpoint, title, format, payload, vars).await;
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
            "variable_keys": vars.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
        }),
    );

    result
}

async fn build_and_send(
    app: &AppHandle,
    endpoint: &Endpoint,
    title: &str,
    format: &str,
    payload: Payload,
    vars: &[(String, String)],
) -> anyhow::Result<(u16, String)> {
    let state = app.state::<AppState>();
    let client = state.http.clone();
    let mut url = reqwest::Url::parse(&endpoint.url)?;

    // variables ride the URL for GET/raw (multipart carries them as form fields)
    if !vars.is_empty() && (endpoint.method == "GET" || endpoint.body_mode == "raw") {
        url.query_pairs_mut()
            .extend_pairs(vars.iter().map(|(k, v)| (k.as_str(), v.as_str())));
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
                for (k, v) in vars {
                    form = form.text(k.clone(), v.clone());
                }
                let field = if endpoint.file_field.is_empty() { "file".to_string() } else { endpoint.file_field.clone() };
                form = form.part(field, part);
                client.post(url).multipart(form)
            }
        },
    };

    for h in &endpoint.headers {
        req = req.header(h.key.as_str(), h.value.as_str());
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
        }
    }

    #[test]
    fn provided_value_wins_over_default() {
        let e = ep(vec![var("name", true, "fallback")]);
        let provided: HashMap<String, String> = [("name".to_string(), "Ada".to_string())].into();
        let out = resolve_variables(&e, Some(&provided), None).unwrap();
        assert_eq!(out, vec![("name".to_string(), "Ada".to_string())]);
    }

    #[test]
    fn default_fills_missing() {
        let e = ep(vec![var("name", true, "fallback")]);
        let out = resolve_variables(&e, None, None).unwrap();
        assert_eq!(out, vec![("name".to_string(), "fallback".to_string())]);
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
        assert_eq!(out, vec![("kept".to_string(), "v".to_string())]);
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
        assert_eq!(out, vec![("name".to_string(), "test".to_string())]);
    }

    #[test]
    fn undeclared_provided_keys_are_ignored() {
        let e = ep(vec![]);
        let provided: HashMap<String, String> = [("evil".to_string(), "x".to_string())].into();
        let out = resolve_variables(&e, Some(&provided), None).unwrap();
        assert!(out.is_empty());
    }
}
