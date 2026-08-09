# Slice 2 — App Shell: Tauri 2 Scaffold, Tray, Windows, IPC, Security Config

**Goal:** the resident application skeleton everything else plugs into.

## Scaffold

- `create-tauri-app`: Svelte 5 + TypeScript + Vite. Identifier `com.printpiper.app`, product name **Print Piper**.
- Rust workspace layout inside `src-tauri/src/`:
  - `capture/` — IPP server + mdns (slice 1)
  - `dispatch/` — outbound requests (slice 4)
  - `store/` — endpoints, jobs, settings persistence (slice 3)
  - `logging/` — ring buffer + plugin-log wiring (slice 5)
  - `commands.rs` — `#[tauri::command]` surface · `events.rs` — typed event names/payloads

## Plugins

| Plugin | Purpose |
|---|---|
| `tauri-plugin-single-instance` | Second launch focuses main window instead of double-binding the IPP port |
| `tauri-plugin-autostart` | Launch at login (toggle in settings; default ON — a printer that isn't running isn't a printer) |
| `tauri-plugin-store` | Endpoints/settings JSON |
| `tauri-plugin-log` | File + console logging, feeds Logs tab |
| `tauri-plugin-notification` | "Sent ✓ / Failed ✗" toasts after picker closes |

## Windows

| Window | Label | Behavior |
|---|---|---|
| Main | `main` | Tabs: **Endpoints**, **Logs** (+ minimal Settings section). Closing hides to tray, does not quit. Also `create: false` + built in `setup`, same reason as the picker. |
| Picker | `picker` | Defined in config with `visible: false` and `create: false`, built in the `setup` hook (after `manage(AppState)`, so no webview can invoke a command before the state exists) and thus still pre-created at startup. On `job:received`: populate, `show + set_focus`, always-on-top, centered. Esc hides (job → `dismissed`). |

- Tray: icon + menu `Open Print Piper` / `Pause capture` (stops accepting jobs, printer stays visible) / `Quit`. Left-click opens main window.
- macOS: keep dock icon in v1 (simpler); `ActivationPolicy::Accessory` is a later polish item.

## IPC surface (complete v1 list)

Commands (frontend → Rust):
`list_endpoints` · `save_endpoint(e)` · `delete_endpoint(id)` · `test_endpoint(id)` (HEAD/GET ping, result to Logs) ·
`list_jobs` · `dispatch_job(job_id, endpoint_id)` · `dismiss_job(job_id)` ·
`get_logs(after_seq?)` · `clear_logs` · `get_settings` · `save_settings(s)` · `clear_spool`

Events (Rust → frontend):
`job:received {job}` · `dispatch:result {job_id, endpoint_id, ok, http_status, duration_ms, error?}` ·
`log:appended {entry}` · `capture:status {running, port, printer_name, discovered_ip_hint}`

## Security configuration (do this at scaffold time, not later)

- **Capabilities:** one capability file per window. `picker` gets the bare minimum (window show/hide, event listen, the dispatch/dismiss commands). `main` gets CRUD + settings. No shell, no fs, no http plugin on the frontend — **all network I/O happens in Rust**.
- **CSP:** `default-src 'self'; img-src 'self' asset: http://asset.localhost; object-src 'self' asset:` (object/embed needed for PDF preview — tighten while implementing slice 4).
- **Asset protocol:** enabled, scoped to `{app_data}/spool/**` only — the webview can render spooled PDFs and nothing else on disk.
- Config/store files live under `{app_data}` with user-only permissions.

## Acceptance criteria

1. App starts to tray; main window opens/hides; quit only via tray menu.
2. Second launch focuses the existing instance (no port conflict).
3. Picker window summons in <150 ms from event to visible (pre-created, warm).
4. `capture:status` reflects server lifecycle (running/port/errors) in the UI footer.
5. Capability files pass `tauri` build lint; frontend has no fs/shell/http permissions.
