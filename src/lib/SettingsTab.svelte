<script lang="ts">
  import { api } from "./api";
  import type { Settings } from "./types";

  let settings = $state<Settings | null>(null);
  let message = $state("");
  let error = $state("");

  api.getSettings().then((s) => (settings = s));

  async function save() {
    if (!settings) return;
    message = "";
    error = "";
    try {
      settings = await api.saveSettings($state.snapshot(settings) as Settings);
      message = "Saved. Printer name & port changes apply after restarting the app.";
    } catch (e) {
      error = String(e);
    }
  }

  async function clearSpool() {
    if (!confirm("Delete all spooled documents and job history?")) return;
    await api.clearSpool();
    message = "Spool cleared.";
  }
</script>

<div class="tab">
  <header class="head">
    <h2>Settings</h2>
  </header>
  {#if settings}
    <section class="card">
      <h3>The printer</h3>
      <label>Printer name <input bind:value={settings.printer_name} /></label>
      <label>Port <input type="number" bind:value={settings.port} min="1024" max="65535" /></label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.allow_lan} />
        Allow other devices on the network to print <span class="hint">(off = this Mac only)</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={settings.autostart} />
        Launch at login <span class="hint">(a printer that isn't running isn't a printer)</span>
      </label>
    </section>

    <section class="card">
      <h3>Spool retention</h3>
      <div class="row">
        <label class="inline">Keep last <input type="number" bind:value={settings.retention_max_jobs} min="1" /> jobs</label>
        <label class="inline">or <input type="number" bind:value={settings.retention_max_days} min="1" /> days</label>
        <button onclick={clearSpool}>Clear spool now</button>
      </div>
      <p class="hint">
        Spooled documents live in the app data folder and are pruned automatically.
        Printed documents can be sensitive — keep retention tight.
      </p>
    </section>

    <div class="actions">
      <button class="primary" onclick={save}>Save settings</button>
      {#if message}<span class="ok">{message}</span>{/if}
      {#if error}<span class="error">{error}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .tab {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 34px 22px 14px 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 620px;
  }
  .head h2 {
    margin: 0;
    font-size: 17px;
    letter-spacing: -0.01em;
  }
  .card {
    background: var(--panel);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
    padding: 13px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card h3 {
    margin: 0;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--faint);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
    max-width: 320px;
  }
  label input {
    color: var(--text);
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    max-width: none;
    color: var(--text);
  }
  label.check input {
    accent-color: var(--accent);
  }
  .row {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
  }
  label.inline {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    color: var(--text);
  }
  label.inline input {
    width: 64px;
  }
  .actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .ok {
    color: var(--accent);
    font-size: 12px;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
  }
  .hint {
    color: var(--faint);
    font-size: 11.5px;
    margin: 0;
  }
</style>
