---
"print-piper": patch
---

Automated multi-platform release pipeline: changesets-driven versioning plus a
GitHub Actions workflow that builds macOS (Intel + Apple Silicon) and Windows
(x64 + arm64) installers and attaches them to a GitHub Release.

Renamed the bundle identifier from `com.printpiper.app` to `com.printpiper.desktop`
(the `.app` suffix conflicted with the macOS bundle extension). Existing installs
migrate their settings, endpoints, and spool automatically on first launch.
