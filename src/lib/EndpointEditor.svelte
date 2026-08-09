<script lang="ts">
  import { slide } from "svelte/transition";
  import { api } from "./api";
  import { emptyEndpoint, emptyVariable, type DispatchResult, type Endpoint } from "./types";

  let {
    endpoint = emptyEndpoint(),
    onsaved,
    ondeleted,
  }: {
    endpoint?: Endpoint;
    onsaved: (e: Endpoint) => void;
    ondeleted: () => void;
  } = $props();

  // one-time clone by design: the parent re-keys this component per selection
  // svelte-ignore state_referenced_locally
  let draft = $state<Endpoint>(structuredClone($state.snapshot(endpoint)) as Endpoint);
  // svelte-ignore state_referenced_locally
  let original = $state(JSON.stringify($state.snapshot(endpoint)));
  let error = $state("");
  let savedFlash = $state(false);
  let testResult = $state<DispatchResult | "pending" | null>(null);

  const dirty = $derived(JSON.stringify($state.snapshot(draft)) !== original);
  const autoTransportLabel = $derived(
    draft.method === "GET" || draft.body_mode === "raw" ? "query param" : "form field",
  );

  async function save() {
    error = "";
    try {
      const saved = await api.saveEndpoint($state.snapshot(draft) as Endpoint);
      draft = structuredClone(saved);
      original = JSON.stringify(saved);
      savedFlash = true;
      setTimeout(() => (savedFlash = false), 1600);
      onsaved(saved);
    } catch (e) {
      error = String(e);
    }
  }

  async function remove() {
    if (!draft.id) return;
    if (!confirm(`Delete endpoint "${draft.name}"?`)) return;
    await api.deleteEndpoint(draft.id);
    ondeleted();
  }

  async function test() {
    if (!draft.id) return;
    testResult = "pending";
    testResult = await api.testEndpoint(draft.id);
  }
</script>

<div class="editor">
  <section class="card">
    <h3>Where it goes</h3>
    <label>
      Name
      <input bind:value={draft.name} placeholder="Invoice inbox (n8n)" />
    </label>
    <label>
      URL
      <input class="mono" bind:value={draft.url} placeholder="https://example.com/webhook/abc" />
    </label>
    <div class="row">
      <div class="field">
        <span class="field-label">Method</span>
        <div class="seg">
          <button class:active={draft.method === "POST"} onclick={() => (draft.method = "POST")}>POST</button>
          <button class:active={draft.method === "GET"} onclick={() => (draft.method = "GET")}>GET</button>
        </div>
      </div>
      {#if draft.method === "POST"}
        <div class="field" transition:slide={{ duration: 150, axis: "x" }}>
          <span class="field-label">Body</span>
          <div class="seg">
            <button
              class:active={draft.body_mode !== "raw"}
              onclick={() => (draft.body_mode = "multipart")}>multipart</button
            >
            <button class:active={draft.body_mode === "raw"} onclick={() => (draft.body_mode = "raw")}>raw PDF</button>
          </div>
        </div>
        {#if draft.body_mode !== "raw"}
          <label class="grow" transition:slide={{ duration: 150, axis: "x" }}>
            File field
            <input bind:value={draft.file_field} placeholder="file" />
          </label>
        {/if}
      {:else}
        <span class="hint self-end">GET sends metadata as query params — no document body.</span>
      {/if}
    </div>
  </section>

  <section class="card">
    <h3>Variables <span class="h-hint">asked for in the pipe window, every send</span></h3>
    {#each draft.variables as v, i (i)}
      <div class="var-row" transition:slide={{ duration: 140 }}>
        <input
          class="mono"
          bind:value={v.key}
          placeholder={v.transport === "header" ? "X-Patient-Name" : "key (e.g. name)"}
        />
        <input bind:value={v.label} placeholder="Label (optional)" />
        <input bind:value={v.default_value} placeholder="Default (optional)" />
        <select bind:value={v.transport} title="Where this value rides in the request">
          <option value="auto">auto</option>
          <option value="query">query</option>
          <option value="header">header</option>
        </select>
        <label class="req" title="Send is blocked until this is filled">
          <input type="checkbox" bind:checked={v.required} />
          req
        </label>
        <button class="icon" onclick={() => draft.variables.splice(i, 1)} aria-label="Remove variable">✕</button>
      </div>
    {/each}
    <button class="ghost" onclick={() => draft.variables.push(emptyVariable())}>+ variable</button>
    {#if draft.variables.length > 0}
      <p class="hint">
        <b>auto</b> sends each value as a {autoTransportLabel} under its key. Pick
        <b>query</b> or <b>header</b> to override — a header key is the header name and must
        be ASCII, and anything in the URL shows up in the server's access logs.
      </p>
    {/if}
  </section>

  <section class="card">
    <h3>Headers <span class="h-hint">stored locally · values never logged</span></h3>
    {#each draft.headers as h, i (i)}
      <div class="kv" transition:slide={{ duration: 140 }}>
        <input class="mono" bind:value={h.key} placeholder="Authorization" />
        <input class="mono" bind:value={h.value} placeholder="Bearer …" />
        <button class="icon" onclick={() => draft.headers.splice(i, 1)} aria-label="Remove header">✕</button>
      </div>
    {/each}
    <button class="ghost" onclick={() => draft.headers.push({ key: "", value: "" })}>+ header</button>
  </section>

  {#if draft.method === "POST" && draft.body_mode !== "raw"}
    <section class="card" transition:slide={{ duration: 150 }}>
      <h3>Extra form fields <span class="h-hint">fixed values, sent with every job</span></h3>
      {#each draft.extra_fields as f, i (i)}
        <div class="kv" transition:slide={{ duration: 140 }}>
          <input class="mono" bind:value={f.key} placeholder="source" />
          <input bind:value={f.value} placeholder="print-piper" />
          <button class="icon" onclick={() => draft.extra_fields.splice(i, 1)} aria-label="Remove field">✕</button>
        </div>
      {/each}
      <button class="ghost" onclick={() => draft.extra_fields.push({ key: "", value: "" })}>+ field</button>
    </section>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}

  <div class="actions">
    <button class="primary" onclick={save} disabled={!dirty && !!draft.id}>
      {draft.id ? "Save changes" : "Create endpoint"}
    </button>
    {#if savedFlash}<span class="flash">Saved ✓</span>{/if}
    {#if draft.id}
      <button onclick={test} disabled={testResult === "pending"} title="Sends a tiny sample PDF using the saved config">
        {testResult === "pending" ? "Testing…" : "Test"}
      </button>
      {#if testResult && testResult !== "pending"}
        <span class="chip {testResult.ok ? 'ok' : 'fail'}">
          {testResult.ok ? "✓" : "✗"}
          {testResult.http_status ?? testResult.error} · {testResult.duration_ms}ms
        </span>
      {/if}
      {#if dirty}<span class="hint">unsaved changes</span>{/if}
      <span class="spacer"></span>
      <button class="danger" onclick={remove}>Delete</button>
    {/if}
  </div>
</div>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 22px 8px 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 720px;
    width: 100%;
  }
  .card {
    background: var(--panel);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
    padding: 13px 14px;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .card h3 {
    margin: 0;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--faint);
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .h-hint {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    font-size: 11px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  label input {
    color: var(--text);
  }
  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }
  .row {
    display: flex;
    gap: 12px;
    align-items: end;
    flex-wrap: wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .field-label {
    font-size: 12px;
  }
  .grow {
    flex: 1;
    min-width: 120px;
  }
  .seg {
    display: flex;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 2px;
    width: fit-content;
  }
  .seg button {
    background: none;
    border: none;
    border-radius: 7px;
    padding: 4px 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .seg button.active {
    background: var(--raised);
    color: var(--text);
  }
  .kv {
    display: flex;
    gap: 6px;
  }
  .kv input {
    flex: 1;
    min-width: 0;
  }
  .var-row {
    display: grid;
    grid-template-columns: 1.1fr 1fr 1fr auto auto auto;
    gap: 6px;
    align-items: center;
  }
  .var-row input {
    min-width: 0;
  }
  .var-row select {
    padding: 6px 4px;
    font-size: 11.5px;
    color: var(--muted);
    font-family: var(--mono);
  }
  .req {
    flex-direction: row;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .req input {
    accent-color: var(--accent);
  }
  .icon {
    padding: 4px 8px;
    color: var(--faint);
    background: none;
    border-color: transparent;
  }
  .icon:hover {
    color: var(--danger);
    background: var(--danger-soft);
    border-color: transparent;
  }
  .ghost {
    background: none;
    border: 1px dashed var(--border);
    color: var(--faint);
    width: fit-content;
    padding: 4px 10px;
    font-size: 12px;
  }
  .ghost:hover {
    color: var(--text);
    border-color: var(--muted);
    background: none;
  }
  .hint {
    color: var(--faint);
    font-size: 11.5px;
    margin: 0;
  }
  .self-end {
    align-self: end;
    padding-bottom: 7px;
  }
  .error {
    color: var(--danger);
    margin: 0;
    font-size: 12.5px;
  }
  .actions {
    position: sticky;
    bottom: 0;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 12px 0 14px 0;
    background: linear-gradient(transparent, var(--bg) 35%);
  }
  .spacer {
    flex: 1;
  }
  .flash {
    color: var(--accent);
    font-size: 12px;
    animation: fadeout 1.6s ease forwards;
  }
  @keyframes fadeout {
    0%, 60% { opacity: 1; }
    100% { opacity: 0; }
  }
  .chip {
    border-radius: 99px;
    padding: 2px 9px;
    font-size: 11px;
    font-family: var(--mono);
  }
  .chip.ok {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .chip.fail {
    background: var(--danger-soft);
    color: var(--danger);
  }
</style>
