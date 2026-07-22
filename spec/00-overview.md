# Print Piper — v1 Spec Overview

> A virtual printer for automators. Hit ⌘P anywhere → pick an endpoint → your PDF lands in an API.
> (Name styled "Print Piper", à la Pied Piper. Bundle id: `com.printpiper.app`.)

## Principles

1. **Simple** — smallest v1 that completes the loop: print → preview → pick endpoint → sent.
2. **Fast** — resident tray app, picker window pre-created and summoned instantly on job arrival.
3. **Secure** — localhost-only capture by default, strict Tauri capabilities/CSP, printed documents treated as sensitive data (scoped asset protocol, explicit retention).

## Validated foundations (from prototyping, 2026-07-19)

- macOS print dialog discovers a DNS-SD-advertised IPP printer with **no driver, no admin, no install** (tested with `/usr/bin/ippeveprinter`).
- Jobs arrive as **true vector PDF** (embedded fonts, `/Text` ProcSet — verified on a real 2-page print).
- Job metadata (document title) rides along via IPP attributes.
- Windows is mid-migration to IPP-only printing (third-party driver deprecation, WPP mode) — the in-box **Microsoft IPP Class Driver** connecting to our server is the aligned, future-proof path. Format negotiation (PDF vs PWG raster) is **unvalidated** — v1 ships with deep logging so it can be tested on any Windows machine later (no Windows box available now).

## Architecture

```
                         ┌──────────────────────────────────────────────┐
 ⌘P / Ctrl-P             │  Print Piper (single Tauri 2 process)        │
 any app                 │                                              │
   │    IPP over         │  ┌────────────┐   job:received   ┌────────┐  │
   ├────localhost───────►│  │ IPP server │ ────────────────►│ Picker │  │
   │  (in-box OS         │  │ (Rust,     │   spool/*.pdf    │ window │  │
   │   print stack)      │  │  embedded) │                  └───┬────┘  │
   │                     │  └────────────┘                      │ click │
 mDNS/DNS-SD ◄───────────│   advertises "Print Piper"           ▼       │
 (printer discovery)     │                                 ┌──────────┐ │
                         │  Main window                    │ Dispatch │ │
                         │  (Endpoints CRUD | Logs)        │ (reqwest)│ │
                         │  Tray icon                      └────┬─────┘ │
                         └───────────────────────────────────────│──────┘
                                                                 ▼
                                                    webhook / API endpoint
```

One process. The Rust side hosts: embedded IPP server, mDNS advertiser, job spool, dispatcher, config store, log ring. The webview side hosts: main window (Endpoints + Logs tabs) and the picker window.

## Tech stack

| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri 2** | Tray, multi-window, small resident footprint, capability-based security |
| Core | **Rust** (tokio) | Same language for IPP server, dispatch, supervision |
| IPP server | evaluate **`ippper`** crate; fallback: hand-rolled on **`ipp`** crate codec + hyper | See `01-capture.md` |
| Discovery | **`mdns-sd`** crate | Pure-Rust DNS-SD, no Bonjour SDK dependency |
| HTTP out | **`reqwest`** (rustls) | Dispatch to endpoints |
| Storage | JSON via **`tauri-plugin-store`** | v1-simple; SQLite only if job history outgrows it |
| Frontend | **Svelte 5 + TS + Vite** | Snappy in a webview; UI is throwaway (redesign planned) so keep it thin |
| Logging | **`tauri-plugin-log`** + in-memory ring | Logs tab is a first-class debugging surface (Windows validation depends on it) |

## Spec slices

| Slice | File | Delivers |
|---|---|---|
| 1 | `01-capture.md` | Embedded IPP server + discovery; jobs land in spool as PDF |
| 2 | `02-shell.md` | Tauri app shell: tray, windows, IPC surface, security config |
| 3 | `03-endpoints.md` | Endpoint model + CRUD (name, URL, method, headers, body mode) |
| 4 | `04-picker-dispatch.md` | Picker window (preview left, endpoints right) + send |
| 5 | `05-logs.md` | Logs tab, ring buffer, persisted logs, Windows debug surface |
| 6 | `06-packaging-windows-validation.md` | Builds, installers, and the blind-Windows test protocol |

Slices 1–2 are parallel-friendly; 3–5 build on 2; 6 is last.

## Explicit v1 non-goals

- Retries / offline queue for dispatch (deferred by decision)
- Linux (nearly free later — same IPP server + CUPS — but out of scope now)
- Secrets in OS keychain (v1: config file with user-only perms; keychain is the first fast-follow — see `03-endpoints.md`)
- Polished UI (functional only; full redesign planned)
- Raster→PDF conversion, OCR, multi-format ingest (PDF-only advertised)
- LAN/tailnet printing from other devices (deliberately off by default; revisit as a feature)
