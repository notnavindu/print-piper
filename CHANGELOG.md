# Changelog

## 0.4.0

### Minor Changes

- c1d4142: Variables can now pick their transport: **auto**, **query**, or **header**.

  `auto` is the default and keeps the existing behaviour exactly — query param for
  GET/raw endpoints, multipart text field otherwise — so endpoints saved by older
  builds send what they always sent. Choosing `query` or `header` pins the transport
  regardless of method or body mode, so a raw-PDF endpoint can pass a value in a
  header instead of the URL, where it would otherwise land in the server's access
  logs.

  Header-bound variables use the variable key as the header name and are validated up
  front: an illegal header name, a non-ASCII value, or a `Content-Type` key (set by
  the body mode itself) is rejected when the endpoint is saved, and again before the
  document leaves the machine. Headers can't carry non-ASCII text — the receiving
  server decodes those bytes as latin-1 and gets mojibake — so such values need
  `query` or a multipart field, which are UTF-8. Header keys are also de-duplicated
  case-insensitively, since `X-Bill` and `x-bill` are one header.

### Patch Changes

- 69bdc1e: Fixed: the config window came up with an empty endpoint list on launch, and only
  showed saved endpoints once something else triggered a re-fetch.

  Tauri creates windows declared in `tauri.conf.json` _before_ the setup hook runs, so
  the webview could call `list_endpoints` before the app state was managed and get back
  an error that nothing surfaced. Both windows now declare `"create": false` and are
  built inside setup, after the state exists. A failed load shows a retry instead of a
  blank pane, and the settings file is no longer rewritten (with an fsync) on every
  launch — that write now sits before the first window is created, so it was stalling
  first paint to produce a byte-identical file.

## 0.3.0

### Minor Changes

- 7bbbc2e: In-app update check: the config window checks GitHub for a newer release on launch
  and shows a small "update available" badge that links to the release. No
  self-updating — just a heads-up and a link out.

## 0.2.2

### Patch Changes

- 62703fd: Automated multi-platform release pipeline: changesets-driven versioning plus a
  GitHub Actions workflow that builds macOS (Intel + Apple Silicon) and Windows
  (x64 + arm64) installers and attaches them to a GitHub Release.

  Renamed the bundle identifier from `com.printpiper.app` to `com.printpiper.desktop`
  (the `.app` suffix conflicted with the macOS bundle extension). Existing installs
  migrate their settings, endpoints, and spool automatically on first launch.

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
