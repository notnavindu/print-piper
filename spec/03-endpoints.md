# Slice 3 — Endpoints: Model, Storage, CRUD UI

**Goal:** first-run and ongoing management of the destinations a print job can be sent to.

## Model

```jsonc
{
  "id": "uuid",
  "name": "Invoice inbox (n8n)",        // shown in picker
  "url": "https://n8n.example.com/webhook/abc",
  "method": "POST",                      // POST | GET
  "body_mode": "multipart",              // multipart | raw | none
  "file_field": "file",                  // multipart only; default "file"
  "extra_fields": [{"key": "source", "value": "print-piper"}],   // multipart only
  "headers": [{"key": "Authorization", "value": "Bearer …"}],
  "created_at": "…", "updated_at": "…"
}
```

Semantics:
- **POST + multipart** (default): PDF as form-data file part (`filename={title}.pdf`, `Content-Type: application/pdf`) + `extra_fields` as text parts. Covers n8n, Zapier, Make, most custom APIs.
- **POST + raw**: body = PDF bytes, `Content-Type: application/pdf` (overridable via headers).
- **GET** (`body_mode: none` forced): no document body — trigger-style endpoints. Job metadata as query params: `title`, `bytes`, `job_id`. (A GET can't carry the PDF; UI copy must say this plainly.)

Validation: `name` non-empty; `url` parses as http(s); duplicate header keys allowed (HTTP permits); warn (don't block) on `http://` for non-localhost hosts.

## Storage

- `tauri-plugin-store` → `{app_data}/endpoints.json`, user-only file perms.
- **v1 accepts plaintext header values on disk** (documented trade-off; file perms + OS user boundary). First fast-follow: move header *values* to OS keychain via the `keyring` crate, keep refs in JSON. Model keeps `headers` shape so migration is invisible.

## UI (main window, Endpoints tab — functional, no polish)

- List: name, method chip, host, edit/delete. Empty state doubles as first-run setup: "Add your first endpoint".
- Editor (inline panel or modal): name, URL, method select, body-mode select (visible only for POST), headers key-value rows (add/remove), extra-fields rows (multipart only), file-field input (multipart only).
- **Test button** per endpoint → `test_endpoint`: sends the configured request shape with a 1-page built-in sample PDF (multipart/raw) or plain GET; shows status + latency inline and writes a Logs entry. This makes endpoint debugging self-service before any real print.
- Delete: plain `confirm()` is fine for v1.

## Acceptance criteria

1. Create → appears in picker immediately (no restart).
2. Endpoint config round-trips app restarts; malformed store file → error surfaced in Logs, app still boots with empty list (never crash on bad config).
3. Test button against a local `httpbin`-style echo shows correct method, headers, multipart shape.
4. `endpoints.json` is chmod 600 (macOS) / user-ACL (Windows).
