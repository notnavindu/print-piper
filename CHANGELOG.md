# Changelog

All notable changes to Print Piper are documented here. This project adheres to
[Semantic Versioning](https://semver.org).

## [0.2.1]

Initial public release.

### Capture & routing
- Virtual IPP Everywhere printer ("`>>> PIPE >>>`") advertised over mDNS/DNS-SD —
  discovered by the OS's native print stack, no driver code.
- Print jobs captured as true vector PDF (macOS spools PDF natively), preserving
  document title and user.
- Dispatch to user-configured HTTP endpoints as `multipart/form-data`, raw
  `application/pdf`, or a GET trigger with metadata.
- **Endpoint variables**: per-send values (e.g. `name`) filled in the pipe window
  before sending; sent as form fields (multipart) or query params (GET/raw).

### Interface
- **Pipe window**: the captured document on the left, a pulsating pipe in the
  middle, a scrollable endpoint rail on the right; the centered endpoint latches
  onto the pipe and sends on ⏎ or click. ↑/↓ move the latch.
- **Cancel** discards a document entirely (spool file deleted, no retry); closing
  the window instead keeps the job for later.
- **Config window**: sidebar of endpoints, each with a Configure and an Activity
  (per-endpoint send history) view; plus System Logs and Settings.
- Branding, icons, and the "pipe" visual language.

### Security & platform
- Capture is loopback-only by default; LAN printing is an explicit opt-in.
- Spool retention (default 50 jobs / 7 days), owner-only file permissions, strict
  Tauri capabilities + CSP, asset protocol scoped to the spool directory.
- Endpoint header values are never logged (keys only).
- macOS: working end-to-end. Windows: builds via CI, pending validation. Linux:
  not yet wired up.

### Known gaps
- Endpoint secrets are stored in a `0600` plaintext JSON file, not the OS keychain
  (top roadmap item — see [`SECURITY.md`](SECURITY.md)).
- No dispatch retries.
