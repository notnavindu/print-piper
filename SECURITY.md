# Security Policy

Print Piper captures documents you print and forwards them to HTTP endpoints you
configure. That makes it security-sensitive by nature — please read the threat
model below before pointing it at anything that matters.

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Use GitHub's private vulnerability reporting: go to the repository's **Security**
tab → **Report a vulnerability**. If that is unavailable, open a minimal public
issue asking for a private contact channel (no details) and we'll follow up.

We aim to acknowledge reports within a few days. Please include repro steps, the
affected version, and your platform.

## Supported versions

Only the latest released version is supported. This is early software (0.x) — the
security model is still evolving.

## Threat model & known considerations

Print Piper runs entirely on your machine; there is no Print Piper cloud service.
Documents and endpoint configuration never leave your computer except when *you*
dispatch a job to an endpoint you configured.

Things to understand before relying on it:

- **Endpoint secrets are stored in plaintext.** Header values (e.g. API keys /
  `Authorization` tokens) live in a JSON file in the app-data directory with
  `0600` (owner-only) permissions — **not** the OS keychain yet. Anything that can
  read your user account's files can read them. Moving secrets to the OS keychain
  is the top roadmap item; until then, avoid storing high-value production
  credentials, and prefer scoped/revocable tokens.
- **Capture is loopback-only by default.** The embedded IPP server binds locally
  and rejects non-loopback peers unless you explicitly enable "Allow LAN" in
  Settings. Turning it on exposes the printer to your local network — only do this
  on networks you trust.
- **The local IPP endpoint is unauthenticated.** It is scoped to loopback by
  default rather than protected by a credential. Any process on your machine that
  can reach the port can submit a print job.
- **Spooled documents persist on disk.** Captured PDFs are written to the app-data
  spool directory (owner-only perms) and pruned on a retention policy (default:
  50 jobs / 7 days, configurable). Printed documents can be sensitive — keep
  retention tight and clear the spool when appropriate.
- **Header values are never written to logs** (only header *keys* are). Response
  bodies from endpoints are truncated in logs.
- **Dispatch has no retries and no queueing to third parties.** A failed send stays
  visible for a manual retry; nothing is silently re-sent.

## Scope

In scope: the desktop app (Rust core + web UI), the embedded IPP server, capture,
and dispatch. Out of scope: vulnerabilities in third-party endpoints you configure,
and issues requiring an already-compromised user account.
