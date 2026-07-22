<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { api, events } from "$lib/api";
  import type { CaptureStatus, Endpoint, UpdateInfo } from "$lib/types";
  import Brand from "$lib/Brand.svelte";
  import EndpointEditor from "$lib/EndpointEditor.svelte";
  import EndpointActivity from "$lib/EndpointActivity.svelte";
  import LogsTab from "$lib/LogsTab.svelte";
  import SettingsTab from "$lib/SettingsTab.svelte";

  // selected: endpoint id, or "new" | "logs" | "settings", or null → hero
  let selected = $state<string | null>(null);
  let detailTab = $state<"configure" | "activity">("configure");
  let endpoints = $state<Endpoint[]>([]);
  let status = $state<CaptureStatus | null>(null);
  let booted = $state(false);
  let update = $state<UpdateInfo | null>(null);

  const selectedEndpoint = $derived(endpoints.find((e) => e.id === selected) ?? null);
  const viewKey = $derived(`${selected ?? "home"}:${detailTab}`);

  async function refresh() {
    endpoints = await api.listEndpoints();
    // selection can go stale after a delete elsewhere
    if (selected && !["new", "logs", "settings"].includes(selected) && !selectedEndpoint) {
      selected = endpoints[0]?.id ?? null;
    }
  }

  onMount(() => {
    refresh().then(() => {
      if (!selected && endpoints.length > 0) selected = endpoints[0].id;
      booted = true;
    });
    api.getCaptureStatus().then((s) => (status = s));
    // best-effort: silently ignored if offline / rate-limited
    api.checkUpdate().then((u) => (update = u.update_available ? u : null)).catch(() => {});
    const unStatus = events.onCaptureStatus((s) => (status = s));
    const unEndpoints = events.onEndpointsChanged(() => refresh());
    return () => {
      unStatus.then((f) => f());
      unEndpoints.then((f) => f());
    };
  });

  function select(id: string) {
    selected = id;
    detailTab = "configure";
  }

  function hostOf(url: string): string {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  }
</script>

<div class="shell">
  <aside>
    <div class="brand-row" data-tauri-drag-region>
      <Brand size={24} img />
    </div>

    <div class="side-label">Endpoints</div>
    <nav class="eps">
      {#each endpoints as e (e.id)}
        <button class="ep" class:active={selected === e.id} onclick={() => select(e.id)}>
          <span class="ep-name">{e.name}</span>
          <span class="ep-meta">{e.method} · {hostOf(e.url)}</span>
        </button>
      {/each}
      <button class="new-ep" class:active={selected === "new"} onclick={() => (selected = "new")}>
        + New endpoint
      </button>
    </nav>

    <div class="side-foot">
      <button class="nav" class:active={selected === "logs"} onclick={() => (selected = "logs")}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="M2.5 3.5h11M2.5 8h7M2.5 12.5h9.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
        System logs
      </button>
      <button class="nav" class:active={selected === "settings"} onclick={() => (selected = "settings")}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true"><circle cx="8" cy="8" r="2.2" stroke="currentColor" stroke-width="1.6"/><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M12.4 3.6L11 5M5 11l-1.4 1.4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>
        Settings
      </button>
      {#if update}
        <button
          class="update"
          onclick={() => api.openExternal(update!.url)}
          title="Print Piper {update.latest} is available — click to view the release"
        >
          <span class="up-dot"></span>
          Update available{update.latest ? ` · v${update.latest}` : ""}
        </button>
      {/if}
      {#if status}
        <div class="status">
          {#if status.error}
            <span class="dot err"></span> capture error
          {:else if status.paused}
            <span class="dot warn"></span> paused
          {:else if status.running}
            <span class="dot ok"></span> ready · :{status.port}
          {:else}
            <span class="dot warn"></span> starting…
          {/if}
        </div>
      {/if}
    </div>
  </aside>

  <main>
    <div class="drag-strip" data-tauri-drag-region></div>
    {#key viewKey}
      <div class="view" in:fly={{ y: 8, duration: 200 }}>
        {#if selected === "logs"}
          <LogsTab />
        {:else if selected === "settings"}
          <SettingsTab />
        {:else if selected === "new"}
          <header class="detail-head">
            <div class="titles"><h2>New endpoint</h2></div>
          </header>
          <EndpointEditor
            onsaved={(e) => {
              refresh().then(() => (selected = e.id));
            }}
            ondeleted={() => (selected = endpoints[0]?.id ?? null)}
          />
        {:else if selectedEndpoint}
          <header class="detail-head">
            <div class="titles">
              <h2>{selectedEndpoint.name}</h2>
              <span class="sub">{selectedEndpoint.method} · {hostOf(selectedEndpoint.url)}</span>
            </div>
            <div class="seg">
              <button class:active={detailTab === "configure"} onclick={() => (detailTab = "configure")}>Configure</button>
              <button class:active={detailTab === "activity"} onclick={() => (detailTab = "activity")}>Activity</button>
            </div>
          </header>
          {#if detailTab === "configure"}
            <EndpointEditor
              endpoint={selectedEndpoint}
              onsaved={() => refresh()}
              ondeleted={() => {
                refresh().then(() => (selected = endpoints[0]?.id ?? null));
              }}
            />
          {:else}
            <EndpointActivity endpointId={selectedEndpoint.id} />
          {/if}
        {:else if booted}
          <div class="hero">
            <Brand size={72} wordmark={false} img />
            <h1>⌘P <span class="arrow">→</span> pipe to anywhere</h1>
            <div class="flowline"></div>
            <p>
              Print Piper shows up as a printer in every app. Hit print, and the document
              lands in any API you point it at — as a true vector PDF.
            </p>
            <button class="primary big" onclick={() => (selected = "new")}>Create your first endpoint</button>
            {#if status?.running && !status.paused}
              <span class="hero-hint">"{status.printer_name}" is already live — try ⌘P in any app</span>
            {/if}
          </div>
        {/if}
      </div>
    {/key}
  </main>
</div>

<style>
  .shell {
    display: flex;
    height: 100vh;
  }
  aside {
    width: 224px;
    flex-shrink: 0;
    background: var(--panel);
    border-right: 1px solid var(--border-soft);
    display: flex;
    flex-direction: column;
  }
  .brand-row {
    padding: 40px 16px 14px 16px; /* clears the macOS traffic lights */
  }
  .brand-row :global(.brand) {
    pointer-events: none; /* keep the strip draggable */
  }
  .side-label {
    padding: 4px 16px 6px 16px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .eps {
    flex: 1;
    overflow-y: auto;
    padding: 0 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ep,
  .new-ep,
  .nav {
    background: none;
    border: none;
    border-radius: 8px;
    text-align: left;
    padding: 7px 10px;
    color: var(--muted);
  }
  .ep {
    display: flex;
    flex-direction: column;
    gap: 1px;
    position: relative;
  }
  .ep::before {
    content: "";
    position: absolute;
    left: 0;
    top: 50%;
    translate: 0 -50%;
    width: 3px;
    height: 0;
    border-radius: 99px;
    background: var(--accent);
    transition: height 0.18s ease;
  }
  .ep:hover,
  .new-ep:hover,
  .nav:hover {
    background: var(--hover);
    color: var(--text);
  }
  .ep.active {
    background: var(--raised);
    color: var(--text);
  }
  .ep.active::before {
    height: 60%;
  }
  .ep-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ep-meta {
    font-size: 11px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .new-ep {
    color: var(--faint);
    margin-top: 2px;
  }
  .new-ep.active {
    background: var(--raised);
    color: var(--text);
  }
  .side-foot {
    padding: 8px;
    border-top: 1px solid var(--border-soft);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
  }
  .nav.active {
    background: var(--raised);
    color: var(--text);
  }
  .update {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    margin-top: 2px;
    padding: 7px 10px;
    border: none;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
    text-align: left;
  }
  .update:hover {
    background: rgba(61, 220, 151, 0.18);
  }
  .up-dot {
    width: 7px;
    height: 7px;
    border-radius: 99px;
    flex-shrink: 0;
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent-glow);
    animation: breathe 2.8s ease-in-out infinite;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px 4px 10px;
    font-size: 11.5px;
    color: var(--faint);
    font-family: var(--mono);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 99px;
    flex-shrink: 0;
  }
  .dot.ok {
    background: var(--accent);
    box-shadow: 0 0 6px var(--accent-glow);
    animation: breathe 2.8s ease-in-out infinite;
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.err {
    background: var(--danger);
  }
  @keyframes breathe {
    0%, 100% { box-shadow: 0 0 3px var(--accent-glow); }
    50% { box-shadow: 0 0 9px var(--accent-glow); }
  }

  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .drag-strip {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 26px;
    z-index: 5;
  }
  .view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 34px 22px 12px 22px;
    border-bottom: 1px solid var(--border-soft);
  }
  .titles {
    min-width: 0;
  }
  .titles h2 {
    margin: 0;
    font-size: 17px;
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11.5px;
    color: var(--faint);
    font-family: var(--mono);
  }
  .seg {
    display: flex;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 2px;
    flex-shrink: 0;
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

  .hero {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    text-align: center;
    padding: 24px;
  }
  .hero h1 {
    margin: 4px 0 0 0;
    font-size: 26px;
    letter-spacing: -0.02em;
  }
  .hero .arrow {
    color: var(--accent);
  }
  .flowline {
    width: 200px;
    height: 3px;
    border-radius: 99px;
    background: repeating-linear-gradient(
      90deg,
      transparent 0 8px,
      var(--accent) 8px 20px
    );
    background-size: 28px 100%;
    animation: flow 1.4s linear infinite;
    opacity: 0.6;
    mask-image: linear-gradient(90deg, transparent, #000 20%, #000 80%, transparent);
    -webkit-mask-image: linear-gradient(90deg, transparent, #000 20%, #000 80%, transparent);
  }
  @keyframes flow {
    to {
      background-position-x: 28px;
    }
  }
  .hero p {
    max-width: 400px;
    margin: 0;
    color: var(--muted);
    line-height: 1.55;
  }
  .big {
    padding: 8px 18px;
    font-size: 13.5px;
  }
  .hero-hint {
    font-size: 11.5px;
    color: var(--faint);
  }
</style>
