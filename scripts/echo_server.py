#!/usr/bin/env python3
"""Tiny echo endpoint for Print Piper testing: logs every request's shape."""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer


class Echo(BaseHTTPRequestHandler):
    def _handle(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b""
        ctype = self.headers.get("Content-Type", "")
        summary = {
            "method": self.command,
            "path": self.path,
            "content_type": ctype,
            "body_bytes": len(body),
            "looks_like_pdf": b"%PDF" in body[:4096],
            "multipart": ctype.startswith("multipart/form-data"),
            "headers": {k: (v if k.lower() != "authorization" else "***") for k, v in self.headers.items()},
        }
        print(json.dumps(summary), flush=True)
        resp = json.dumps({"ok": True, "received_bytes": len(body)}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)

    do_POST = _handle
    do_GET = _handle

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    print("echo server on http://localhost:9999", flush=True)
    HTTPServer(("127.0.0.1", 9999), Echo).serve_forever()
