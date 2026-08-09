---
"print-piper": patch
---

Fixed: the config window came up with an empty endpoint list on launch, and only
showed saved endpoints once something else triggered a re-fetch.

Tauri creates windows declared in `tauri.conf.json` *before* the setup hook runs, so
the webview could call `list_endpoints` before the app state was managed and get back
an error that nothing surfaced. Both windows now declare `"create": false` and are
built inside setup, after the state exists. A failed load shows a retry instead of a
blank pane, and the settings file is no longer rewritten (with an fsync) on every
launch — that write now sits before the first window is created, so it was stalling
first paint to produce a byte-identical file.
