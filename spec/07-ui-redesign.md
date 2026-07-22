# 07 — UI redesign: brand, sidebar shell, the pipe

Status: implemented (this slice documents the redesign shipped after v0.1).

## Brand

The core idea in one line: **⌘P → pipe to anywhere.**

- Wordmark: "Print Piper" with a pipe glyph (rounded elbow-pipe SVG, drawn inline —
  no image assets, tints with `currentColor`/accent).
- Accent: **piper green** `#3ddc97` on a deep neutral dark theme. The green is used
  sparingly: the pipe flow, the latched endpoint, primary buttons, the "ready" status dot.
- Palette (CSS custom properties in `+layout.svelte`):
  `--bg #0b0d10`, `--panel #13161b`, `--raised #1a1f26`, `--border #262c35`,
  `--text #e8eaed`, `--muted #8b93a1`, `--accent #3ddc97`, `--danger #ff6b70`, `--warn #ffd479`.
- Type: system stack (SF Pro on macOS). Monospace only for logs/URLs.
- Both windows use macOS `titleBarStyle: Overlay` + `hiddenTitle` for full-bleed chrome
  (ignored on Windows, where the normal title bar renders).

## Main window — sidebar shell

```
┌──────────────┬────────────────────────────────────┐
│ ⌘ Print Piper│  [Endpoint name]        Configure ⟷ Activity
│              │                                    │
│ ENDPOINTS    │  form: name, url, method,          │
│ ● Invoices   │  body mode, headers, variables,    │
│ ● Archive    │  extra fields · Test · Delete      │
│ + New        │                                    │
│              │  (Activity = every send to this    │
│ ──────────   │   endpoint, derived from jobs)     │
│ System logs  │                                    │
│ Settings     │                                    │
│ ● ready :63163                                    │
└──────────────┴────────────────────────────────────┘
```

- Left sidebar lists endpoints; clicking one opens its detail view with two tabs:
  **Configure** (the editor) and **Activity** (per-endpoint send history — time, job
  title, HTTP status, duration — derived from `Job.sends`, no new backend surface).
- "System logs" and "Settings" are sidebar footer items (global log ring stays — it is
  the Windows blind-test instrument).
- No endpoint selected / no endpoints: branded hero ("⌘P → pipe to anywhere") with a
  create button.
- Capture status pill lives at the bottom of the sidebar.

## Picker — the pipe window

```
┌──────────────────────────────────────────────────┐
│                                      1 of 2   ✕  │
│  ┌────────┐                       ┌ endpoint ┐   │
│  │  PDF   │   ════════╪═══════▶   │ LATCHED  │   │
│  │ card   │   (pulsating pipe)    └──────────┘   │
│  │ title  │                       ┌ endpoint ┐   │
│  └────────┘                       └──────────┘   │
│         esc dismiss · ↑↓ choose · ⏎ send         │
└──────────────────────────────────────────────────┘
```

- **Left**: the captured document as a floating card — small PDF preview, title,
  size/time. Gentle idle float; flies in when a job arrives.
- **Middle**: the pipe — an SVG tube with an animated dash "flow" pulsing left→right
  toward the latched endpoint. Sending speeds up/brightens the flow; success fires a
  green burst and the doc card is "sucked into" the pipe; failure tints the junction red.
- **Right**: vertically scrollable endpoint rail with `scroll-snap` centering. The
  endpoint that rests in the middle **latches** onto the pipe (scale + accent ring +
  connector nub). Click a non-latched endpoint to latch it; click the latched card (or
  press ⏎) to send. ↑/↓ move the latch, Esc dismisses.
- **Variables** (see below): the latched card expands to show one input per variable;
  send is gated until required ones are filled.
- Queue: FIFO "1 of N"; auto-advance ~1s after a successful send.

## Endpoint variables

Automators often need a per-send value (a name, an order id, a tag). Each endpoint can
declare variables the user fills in the picker before sending.

Model (`Endpoint.variables`, serde-default so v0.1 JSON loads unchanged):

```rust
pub struct EndpointVariable {
    pub key: String,           // field/param name sent to the endpoint
    pub label: String,         // optional human label ("" → key shown)
    pub required: bool,        // gates the Send button; backend re-validates
    pub default_value: String, // prefilled in the picker
}
```

Dispatch semantics — variables ride the same channel as endpoint metadata:

| method / body     | variables become                              |
|-------------------|-----------------------------------------------|
| GET               | query params (after `title`/`bytes`/`format`) |
| POST multipart    | text form fields (after `extra_fields`)       |
| POST raw          | query params on the URL                       |

`dispatch_job(job_id, endpoint_id, variables?)` takes a `{key: value}` map; the backend
resolves each declared variable as provided value → `default_value` → error if
`required` and still empty (fails fast, before any bytes leave the machine). Unknown
keys in the map are ignored — only declared variables are sent. "Test" resolves
defaults and falls back to `"test"` so it never blocks.

## Boot re-summon

If pending jobs exist at startup (app restarted with an unsent capture), the picker is
shown at boot instead of waiting for the next job — closes the "you have to send the
PDF again" gap from v0.1 testing.

## Non-goals (unchanged)

Retries, keychain secrets, Windows validation, Linux — tracked in 06/roadmap.
