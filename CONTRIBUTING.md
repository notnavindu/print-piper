# Contributing to Print Piper

Thanks for your interest! Print Piper is a virtual IPP printer that routes print
jobs to API endpoints. This guide gets you building and explains how the project
is organized.

## Prerequisites

- **Node.js** 20+ and npm
- **Rust** (stable) via [rustup](https://rustup.rs)
- Platform toolchain for [Tauri 2](https://v2.tauri.app/start/prerequisites/)
  (Xcode Command Line Tools on macOS; WebView2 + Build Tools on Windows)

## Getting started

```sh
npm install
npm run tauri dev
```

The virtual printer ("`>>> PIPE >>>`") appears in your system print dialog within
a few seconds of launch (port 63163, configurable in Settings).

## Project layout

- `spec/` — **design docs, one slice per subsystem.** This is a spec-first
  project: read `spec/00-overview.md` first, and for non-trivial changes, update
  or add a spec slice alongside the code.
- `src-tauri/` — the Rust core (IPP server, mDNS, capture, dispatch, commands).
- `src/` — the SvelteKit UI (main config window + the "pipe" picker window).
- `scripts/echo_server.py` — a local endpoint that echoes what it receives.

## Testing

```sh
# Rust unit tests (dispatch/variable resolution, etc.)
cd src-tauri && cargo test

# Frontend type/scaffold check
npm run check

# Inject a print job without printing (macOS/Linux ship ipptool)
ipptool -t -f /usr/share/cups/ipptool/document-letter.pdf \
  ipp://localhost:63163/ipp/print print-job.test

# Receive dispatches locally
python3 scripts/echo_server.py   # then add http://localhost:9999/hook as an endpoint
```

## Pull requests

- Keep PRs focused; one concern per PR.
- Run `cargo test` and `npm run check` before pushing.
- Match the surrounding code style (no new formatter/lint config needed).
- If your change alters behavior described in a spec slice, update that slice.
- Describe *why*, not just *what* — link the spec slice or issue where relevant.

## Platform notes

Development happens on macOS; Windows is built via CI (`.github/workflows`) and
still needs validation (see `spec/06-packaging-windows-validation.md`). If you have
a Windows machine, that blind-test is one of the most valuable contributions right
now.

## Versioning & releases (maintainers)

Versions are automated — never hand-edit them. `package.json` is the single source
of truth; `tauri.conf.json` reads its version from there, and `scripts/sync-version.mjs`
mirrors it into `Cargo.toml`/`Cargo.lock` during the version step.

Flow:

1. Every user-facing PR includes a changeset (`npx changeset`) — see
   [`.changeset/README.md`](.changeset/README.md).
2. `.github/workflows/version.yml` opens a **"version packages"** PR that consumes
   the changesets, bumps the version, and updates `CHANGELOG.md`.
3. Merging that PR pushes a `print-piper@X.Y.Z` tag.
4. `.github/workflows/release.yml` builds macOS (Intel + Apple Silicon) and Windows
   (x64 + arm64) installers and attaches them to a **draft** GitHub Release. Review
   the binaries, then publish the draft.

### One-time setup

- Add a repo secret **`RELEASE_PAT`** — a fine-grained personal access token with
  `contents: write` + `pull requests: write`. It is required because a tag pushed
  with the default `GITHUB_TOKEN` will not trigger the release workflow; the PAT's
  push does. (Settings → Secrets and variables → Actions → New repository secret.)

Builds are currently **unsigned**; signing/notarization can be added later by
supplying the platform certs as secrets to `tauri-action` (no workflow changes).

## Security

Found a vulnerability? Please follow [`SECURITY.md`](SECURITY.md) rather than
opening a public issue.
