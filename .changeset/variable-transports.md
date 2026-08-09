---
"print-piper": minor
---

Variables can now pick their transport: **auto**, **query**, or **header**.

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
