# Slice 1 — Capture: Embedded IPP Server + Discovery

**Goal:** printing to "Print Piper" from any app on macOS (and later Windows) produces a PDF file in the app's spool directory plus a `job:received` event — with zero drivers and zero admin rights on macOS.

## Why embedded (not ippeveprinter as child process)

- One code path for macOS **and** Windows (Windows has no system ippeveprinter).
- `ippeveprinter` cannot bind localhost-only — our prototype was visible to the whole tailnet. Owning the server lets us enforce **loopback-only by default** (security principle).
- No child-process supervision, no bundling/signing of an external binary.
- The system `ippeveprinter` + `ipptool` remain our **reference implementation and test harness** (see Testing).

## Server

- tokio + hyper (or axum) HTTP/1.1 server; must handle `Expect: 100-continue` and chunked bodies (the CUPS client uses both).
- Default port: `63163` (configurable in settings). Path: `/ipp/print`.
- **Library decision (RESOLVED 2026-07-19):** `ippper` 0.6.0 (BSD-3-Clause, maintained — v0.6.0 June 2026, built on `ipp` 6 + hyper). Its `SimpleIppService` handles all IPP ops; we add: (a) a delegating `IppService` wrapper to extract the `job-name` attribute (SimpleIppService discards it — needed for picker titles; Print-Job via task-local, Create-Job/Send-Document via job-id→name map), (b) our own accept loop (clone of its `serve_http`) with the peer-IP guard, (c) `mdns-sd` 0.20 for advertising. Rejected: `ipp-printer-app` (callback receives PWG raster — the vector-destroying path).

### IPP operations (minimum viable set)

| Op | Code | Behavior |
|---|---|---|
| Get-Printer-Attributes | 0x000B | Static attribute set (see below) |
| Validate-Job | 0x0004 | Always `successful-ok` |
| Print-Job | 0x0002 | Strip IPP preamble → stream document to spool → return job-id |
| Create-Job / Send-Document | 0x0005/0x0006 | Same as Print-Job split in two (the macOS `ipp` backend prefers this path) |
| Get-Jobs | 0x000A | Return recent jobs (minimal) |
| Get-Job-Attributes | 0x0009 | Job state: report `completed` once spooled |
| Cancel-Job | 0x0008 | Ack; mark job canceled if not yet spooled |

### Advertised attributes (key ones)

- `printer-name`: user-configurable, default `Print Piper`
- `document-format-supported`: `application/pdf`, `application/octet-stream` (IPP requires octet-stream; log a warning if a job actually arrives as non-PDF — this is the Windows-format tripwire)
- `ipp-versions-supported`: 1.1, 2.0 · `printer-uuid`: stable per-install UUID
- media/sides/copies defaults: mirror what `ippeveprinter -f application/pdf` reports

**De-risk step:** capture the reference attribute set from the running prototype:
`ipptool -tv ipp://localhost:8632/ipp/print get-printer-attributes.test` → commit output to `spec/reference/` and mirror it.

## Discovery / registration

- **macOS:** advertise `_ipp._tcp` (+ `_universal._sub`) via `mdns-sd` with TXT: `txtvers=1 qtotal=1 rp=ipp/print ty=Print Piper pdl=application/pdf Color=T Duplex=F UUID=…`. This alone made the prototype appear in ⌘P → validated. No `lpadmin`, no admin.
- **Windows:** advertise the same; Windows 11 Settings → Add device *should* find mDNS IPP printers and bind the in-box **Microsoft IPP Class Driver**. Fallback: manual add by URL (`http://localhost:63163/ipp/print`). Exact flow is part of the Windows validation protocol (`06-…`). Everything about the add/negotiate/print sequence gets logged.

## Security

- Bind `0.0.0.0` but **reject non-loopback peers at accept time** (mDNS advertises the host's LAN name, so the local CUPS client may connect via a non-127.0.0.1 route; binding loopback-only would break discovery printing). Setting `allow_lan: false` default; flipping it is a deliberate future feature (print from phone/tailnet).
- No auth in v1 (loopback-only makes it moot); revisit with `allow_lan`.

## Spool & job record

- Spool dir: `{app_data}/spool/`, files `{job_id}-{sanitized_title}.pdf`, perms 0600.
- Job record (JSON, ring of last 200): `id, title (job-name attr), user (requesting-user-name), format (document-format as received), bytes, received_at, spool_path, status: pending|sent|dismissed|canceled, sends: [{endpoint_id, at, http_status, duration_ms, response_head}]`.
- Non-PDF payloads (Windows unknown): **spool anyway** with extension from format (`.pwg`, `.pclm`, `.bin`), log prominently, still emit `job:received` (picker shows "no preview — format X").
- On spooled job: emit `job:received {job}` to all windows.

## Acceptance criteria

1. With the app running, "Print Piper" appears in the macOS print dialog within 5 s of launch; printing a 2-page doc yields a vector PDF in the spool and a `job:received` event carrying the doc title.
2. `ipptool` conformance: Get-Printer-Attributes, Validate-Job, Print-Job, Create-Job+Send-Document tests all pass against our server.
3. A connection from a non-loopback address is refused and logged.
4. Killing/restarting the app re-advertises cleanly (no ghost printers accumulating).
