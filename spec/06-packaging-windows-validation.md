# Slice 6 — Packaging & the Blind-Windows Validation Protocol

**Goal:** installable builds for macOS + Windows, and a scripted test any Windows-having friend can run in 10 minutes, returning a Logs export that answers every open Windows question.

## Packaging

**macOS**
- `tauri build` → `.app` + `.dmg`. Dev: ad-hoc signing is fine. Distribution: Developer ID signing + notarization (required for anyone else to run it — line item, not v1 blocker).
- No installer steps needed: the app advertises the printer itself at runtime. Delightful consequence: *the .app is the entire install.*

**Windows**
- `tauri build` → NSIS `.exe` (or `.msi`). Unsigned for v1 testing (tester clicks through SmartScreen; instructions cover it).
- No driver, no service: app runs in tray, advertises via mDNS, in-box IPP class driver connects. If discovery fails, manual printer add (below) — still no third-party driver.
- Build via GitHub Actions windows runner (we develop on macOS; cross-compiling Tauri to Windows is not worth it vs CI).

## Open Windows questions this protocol must answer

| # | Question | Where the answer shows up |
|---|---|---|
| Q1 | Does Windows discover the mDNS-advertised printer in Settings → Add device? | Tester observation |
| Q2 | If not, does manual add by URL bind the IPP class driver? | Tester observation + capture logs |
| Q3 | **What `document-format` does Windows actually send** when we advertise PDF? | `capture` log line per job (the big one: vector PDF vs PWG raster) |
| Q4 | Does the Edge PDF viewer render our preview in WebView2? | Tester observation of picker |
| Q5 | Job metadata quality (job-name = doc title? user name?) | `capture` log |

## Tester script (goes in the README of the test build)

1. Install + launch. Tray icon appears. Open main window → add one endpoint: `https://webhook.site/<their-url>` (POST/multipart).
2. Settings → Printers → **Add device** → wait 30 s → does **Print Piper** appear? (Q1) If yes, add it. If no: **Add manually** → "shared printer by name" → `http://localhost:63163/ipp/print` (Q2).
3. Print the pre-supplied 2-page test doc (mixed text + vector chart) from Edge and from Word/WordPad.
4. Picker appears? Preview renders? (Q4) Click the endpoint → webhook.site shows the upload?
5. Logs tab → **Export** → send the file back. (Q3/Q5 read from the export; if Q3 says `pwg-raster`, the file lands as `.pwg` — send that too.)

**Decision gate after the protocol:** Q3 = `application/pdf` → Windows is done, architecture fully converged. Q3 = raster → activate fallback: **Microsoft Print to PDF + fixed-file-port** capture path for Windows (in-box driver, vector PDF; spec'd as an addendum only if needed).

## Acceptance criteria

1. CI produces installable artifacts for both OSes from a tag.
2. macOS `.dmg`: fresh-machine install → printing works with zero terminal usage.
3. Windows build boots, tray runs, IPP server binds, mDNS advertises (verifiable in logs even before a printer connects).
4. The tester script + Logs export answers Q1–Q5 without developer presence.
