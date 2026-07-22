# Slice 5 — Logs & Observability

**Goal:** the Logs tab is the product's flight recorder — and the *primary instrument* for validating Windows without a Windows machine on hand. Over-log on purpose in v1.

## Model

```jsonc
{ "seq": 1024, "ts": "…", "level": "info|warn|error|debug",
  "area": "capture|discovery|dispatch|store|system",
  "msg": "human-readable line",
  "detail": { /* structured, area-specific */ } }
```

- **Ring buffer** in Rust: last 2000 entries, `seq`-addressed; `get_logs(after_seq)` for incremental fetch; `log:appended` event streams new entries when the Logs tab is open.
- **Persisted** via `tauri-plugin-log` to `{app_data}/logs/` (rotating, keep ~5 MB). Ring feeds UI; file feeds "export".

## What we log (deliberately verbose)

**capture** — every IPP request: peer address, operation name/code, ipp version, status returned; per job: `document-format` **as received** (the Windows PDF-vs-raster tripwire), `job-name`, `requesting-user-name`, byte count, spool path, duration. Rejected non-loopback peers → `warn`.

**discovery** — mDNS service registered (name, port, TXT), conflicts/renames, re-announcements, shutdown unregister.

**dispatch** — endpoint id/name, method, URL host, request body mode, bytes sent, HTTP status, duration, response head (512 B), redirect chain if any. Errors with full reqwest error chain.

**store/system** — config load/save, migrations, prune runs, port bind failures, app start/stop with version + OS.

Never log: header *values* (log keys only, values as `•••`), full response bodies, endpoint URLs beyond host in `msg` (full URL in `detail`, which the UI reveals on expand).

## UI (main window, Logs tab)

- Virtualized list, newest first · level + area filter chips · text filter.
- Row expands to pretty-printed `detail` JSON.
- Buttons: **Copy visible**, **Export file**, **Clear**. Export = the debugging artifact a Windows tester sends back.
- Live-follows while visible (via `log:appended`), no polling.

## Acceptance criteria

1. Printing one macOS job produces a legible capture story: connection → Get-Printer-Attributes → Create-Job/Send-Document → format=application/pdf → spooled (path, bytes).
2. A dispatch appears with status + duration; header values never appear anywhere in logs.
3. Filters and expand work over 2000 entries without jank (virtualized).
4. Export produces a single text/JSON file a non-developer could email back.
