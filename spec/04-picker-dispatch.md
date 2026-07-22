# Slice 4 — Picker Window + Dispatch

**Goal:** the moment that defines the product: print lands → small window appears → PDF preview left, endpoints right → click → sent.

## Picker flow

```
job:received ──► picker populates ──► show + focus (always-on-top)
                      │
        ┌─────────────┴──────────────┐
        │ PDF preview    │ Endpoint  │
        │ (first page,   │ list      │
        │  scrollable)   │ [Send]    │
        │ title · pages  │ per row   │
        │ · size · time  │           │
        └─────────────┬──────────────┘
                      │ click endpoint
                      ▼
            spinner on that row ──► dispatch:result
                      │
         ok: row ✓ → auto-hide after ~1.2 s + system toast
         err: row ✗ + error inline, window stays open
```

- **Esc / close** → hide window, job → `dismissed` (file stays in spool per retention; visible in Logs).
- **Multiple jobs:** FIFO queue in picker state; header shows "1 of 3"; after send/dismiss, advance to next. No parallel pickers.
- Send is **synchronous UX, async under the hood**: row-level spinner; other rows disabled while one send is in flight (v1 simplification).
- A job can be re-sent to a second endpoint before dismissing (rows re-enable after result) — cheap and genuinely useful.

## PDF preview

- **Attempt 1 (zero deps):** `<embed>`/iframe of the spooled file via asset protocol (`convertFileSrc`). WKWebView (macOS) and WebView2 (Windows, Edge PDF viewer) both render PDFs natively.
- **Fallback (if inconsistent):** bundle `pdf.js` and render page 1 to canvas. No CDN — CSP stays closed.
- Non-PDF job (possible on Windows until validated): "No preview — received `image/pwg-raster`" + still sendable. This is a debug feature, not an error state.

## Dispatch (Rust, `dispatch/`)

- `reqwest` with rustls; timeout 30 s connect+total; redirects capped at 5 (final URL logged).
- Build request per endpoint `method`/`body_mode`/`headers` (see slice 3). PDF is **streamed** from spool, not buffered.
- Result → update job record (`sends[]` entry), emit `dispatch:result`, write Logs entry with method, URL host, status, duration, response head (first 512 bytes, for webhook debugging).
- **No retries in v1** (decided). A failed send leaves the job in picker/spool — manual re-click is the retry.

## Spool retention

- Keep job files: last **50 jobs or 7 days**, whichever first; prune on startup + hourly.
- "Clear spool now" button in settings area. Printed documents are sensitive — retention is a security feature, not housekeeping.

## Acceptance criteria

1. Print from any app → picker visible with correct title/size and rendered first page in <1 s on macOS.
2. Send to a local echo server: correct multipart shape, filename `{title}.pdf`, spinner → ✓ → auto-hide → toast.
3. Failing endpoint (connection refused, 500): inline error, window stays, second endpoint still clickable, Logs entry complete.
4. Esc → job dismissed, file present in spool, prune obeys 50/7d policy.
5. Two prints in quick succession → "1 of 2" queue behavior works.
