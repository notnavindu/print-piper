<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api, events, fmtBytes, fmtTime } from "$lib/api";
  import type { DispatchResult, Endpoint, Job } from "$lib/types";
  import Brand from "$lib/Brand.svelte";

  let queue = $state<Job[]>([]);
  let endpoints = $state<Endpoint[]>([]);
  let latched = $state(0);
  let phase = $state<"idle" | "sending" | "success">("idle");
  let results = $state<Record<string, DispatchResult>>({});
  // flat map "endpointId::varKey" → value, reset per job
  let varValues = $state<Record<string, string>>({});
  let railEl = $state<HTMLElement | null>(null);
  let scrollTarget: number | null = null;
  let scrollRaf = 0;

  const current = $derived(queue[0] ?? null);
  const previewSrc = $derived(
    current && current.format === "application/pdf" ? convertFileSrc(current.spool_path) : null
  );
  const latchedEp = $derived(endpoints[latched] as Endpoint | undefined);
  const latchedResult = $derived(latchedEp ? results[latchedEp.id] : undefined);
  const canSend = $derived.by(() => {
    if (!current || !latchedEp || phase !== "idle") return false;
    return latchedEp.variables.every(
      (v) => !v.required || (varValues[vkey(latchedEp!.id, v.key)] ?? "").trim() !== ""
    );
  });

  function vkey(epId: string, varKey: string): string {
    return `${epId}::${varKey}`;
  }
  // prefill defaults for the latched endpoint's variables
  $effect(() => {
    const ep = latchedEp;
    if (!ep) return;
    for (const v of ep.variables) {
      const k = vkey(ep.id, v.key);
      if (varValues[k] === undefined) varValues[k] = v.default_value ?? "";
    }
  });

  async function refreshEndpoints() {
    endpoints = await api.listEndpoints();
    latched = Math.max(0, Math.min(latched, endpoints.length - 1));
  }

  async function loadPending() {
    const jobs = await api.listJobs();
    queue = jobs.filter((j) => j.status === "pending");
  }

  onMount(() => {
    refreshEndpoints();
    loadPending();
    const unJob = events.onJobReceived((job) => {
      if (!queue.find((j) => j.id === job.id)) {
        const wasEmpty = queue.length === 0;
        queue.push(job);
        if (wasEmpty) {
          latched = 0;
          requestAnimationFrame(() => scrollRailTo(0, false));
        }
      }
    });
    const unEndpoints = events.onEndpointsChanged(() => refreshEndpoints());
    window.addEventListener("keydown", onKey);
    return () => {
      unJob.then((f) => f());
      unEndpoints.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  });

  // -- rail latching ----------------------------------------------------------

  function nearestIndex(): number {
    if (!railEl) return latched;
    const rect = railEl.getBoundingClientRect();
    const center = rect.top + rect.height / 2;
    let best = latched;
    let bestDist = Infinity;
    railEl.querySelectorAll<HTMLElement>(".ep-card").forEach((el, i) => {
      const r = el.getBoundingClientRect();
      const d = Math.abs(r.top + r.height / 2 - center);
      if (d < bestDist) {
        bestDist = d;
        best = i;
      }
    });
    return best;
  }

  function onRailScroll() {
    cancelAnimationFrame(scrollRaf);
    scrollRaf = requestAnimationFrame(() => {
      const n = nearestIndex();
      if (scrollTarget !== null) {
        if (n === scrollTarget) scrollTarget = null; // arrived
        return; // don't fight a programmatic scroll
      }
      latched = n;
    });
  }

  function scrollRailTo(i: number, smooth = true) {
    const el = railEl?.querySelectorAll<HTMLElement>(".ep-card")[i];
    if (!el) return;
    scrollTarget = i;
    el.scrollIntoView({ block: "center", behavior: smooth ? "smooth" : "auto" });
    setTimeout(() => (scrollTarget = null), 600);
  }

  function latchTo(i: number) {
    if (i < 0 || i >= endpoints.length || phase === "sending") return;
    latched = i;
    scrollRailTo(i);
  }

  // -- actions ----------------------------------------------------------------

  async function advance() {
    // hide BEFORE swapping state, or the idle screen flashes for a frame
    if (queue.length <= 1) {
      await getCurrentWindow().hide();
    }
    queue.shift();
    results = {};
    varValues = {};
    phase = "idle";
    latched = 0;
    if (queue.length > 0) {
      requestAnimationFrame(() => scrollRailTo(0, false));
    }
  }

  // cancel = discard this document entirely (spool file deleted, no retry).
  // to keep it for later, close the window instead (traffic light) — it hides.
  async function cancel() {
    if (!current || phase !== "idle") return;
    const id = current.id;
    await advance(); // same teardown/hide as a completed send
    await api.cancelJob(id);
  }

  async function send() {
    if (!canSend || !current || !latchedEp) return;
    const ep = latchedEp;
    const vars: Record<string, string> = {};
    for (const v of ep.variables) vars[v.key] = varValues[vkey(ep.id, v.key)] ?? "";
    phase = "sending";
    const result = await api.dispatchJob(current.id, ep.id, vars);
    results[ep.id] = result;
    if (result.ok) {
      phase = "success";
      setTimeout(advance, 800);
    } else {
      phase = "idle";
    }
  }

  function cardClick(i: number) {
    if (i === latched) {
      send();
    } else {
      latchTo(i);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      cancel();
      return;
    }
    // arrows move the latch even while a variable input is focused —
    // up/down do nothing useful inside a single-line field anyway
    if (e.key === "ArrowDown") {
      e.preventDefault();
      latchTo(latched + 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      latchTo(latched - 1);
      return;
    }
    if (e.key === "Enter") {
      send();
      return;
    }
  }

  function focusLater(el: HTMLInputElement) {
    const t = setTimeout(() => el.focus(), 280);
    return { destroy: () => clearTimeout(t) };
  }

  function hostOf(url: string): string {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  }
</script>

{#if current}
  <div class="picker">
    <header class="top" data-tauri-drag-region>
      <span class="brand-slot"><Brand size={15} dim /></span>
      <span class="top-right">
        {#if queue.length > 1}<span class="count">1 of {queue.length}</span>{/if}
        <button
          class="cancel-btn"
          onclick={cancel}
          disabled={phase !== "idle"}
          title="Discard this document — it won't be sent or kept"
        >
          Cancel <span class="x">✕</span>
        </button>
      </span>
    </header>

    <div class="flow-row">
      <!-- the document -->
      <div class="stage">
        {#key current.id}
          <!-- transform layers kept separate: outer = entrance/whoosh, card = idle float -->
          <div class="doc-outer" class:whoosh={phase === "success"} in:fly={{ x: -60, duration: 400 }}>
          <div class="doc-card">
            {#if previewSrc}
              <embed src={previewSrc} type="application/pdf" title="preview" />
            {:else}
              <div class="no-preview">
                <span class="np-icon">▤</span>
                <code>{current.format}</code>
                <span class="np-hint">no preview — still sendable</span>
              </div>
            {/if}
            <div class="meta">
              <strong title={current.title}>{current.title}</strong>
              <span>{fmtBytes(current.bytes)} · {fmtTime(current.received_at)}</span>
            </div>
          </div>
          </div>
        {/key}
      </div>

      <!-- the pipe -->
      <div class="pipe-zone">
        <div
          class="pipe"
          class:live={endpoints.length > 0}
          class:fast={phase === "sending"}
          class:burst={phase === "success"}
        >
          <div class="tube"></div>
          <div class="glow"></div>
          <div class="shimmer"></div>
          <div class="nozzle">
            <span class="ring r1"></span>
            <span class="ring r2"></span>
          </div>
        </div>

        <div class="under-pipe">
          {#if latchedEp && latchedEp.variables.length > 0 && phase !== "success"}
            {@const ep = latchedEp}
            <div class="varform" transition:fade={{ duration: 150 }}>
              {#each ep.variables as v, vi (v.key)}
                <label>
                  <span class="var-label">
                    {v.label || v.key}{#if v.required}<em>*</em>{/if}
                  </span>
                  {#if vi === 0}
                    <input
                      use:focusLater
                      bind:value={varValues[vkey(ep.id, v.key)]}
                      placeholder={v.default_value || v.key}
                      disabled={phase === "sending"}
                    />
                  {:else}
                    <input
                      bind:value={varValues[vkey(ep.id, v.key)]}
                      placeholder={v.default_value || v.key}
                      disabled={phase === "sending"}
                    />
                  {/if}
                </label>
              {/each}
            </div>
          {/if}
          {#if latchedResult && !latchedResult.ok && phase === "idle"}
            <div class="senderr" transition:fade={{ duration: 150 }}>
              ✗ {latchedResult.error ?? `HTTP ${latchedResult.http_status}: ${latchedResult.response_head ?? ""}`}
            </div>
          {/if}
        </div>
      </div>

      <!-- the endpoints -->
      <div class="rail-wrap">
        {#if endpoints.length === 0}
          <div class="empty">
            <p>No endpoints yet.</p>
            <button class="primary" onclick={() => api.showMainWindow()}>Open Print Piper to add one</button>
          </div>
        {:else}
          <div class="rail" bind:this={railEl} onscroll={onRailScroll}>
            {#each endpoints as e, i (e.id)}
              {@const r = results[e.id]}
              <button
                class="ep-card"
                class:latched={i === latched}
                onclick={() => cardClick(i)}
                disabled={phase === "sending" && i !== latched}
              >
                <span class="row1">
                  <span class="name" title={e.name}>{e.name}</span>
                  {#if i === latched}
                    {#if phase === "sending"}
                      <span class="pill busy"><span class="spinner"></span></span>
                    {:else if phase === "success"}
                      <span class="pill ok">✓ {r?.http_status ?? ""}</span>
                    {:else if r && !r.ok}
                      <span class="pill fail">✗ retry</span>
                    {:else}
                      <span class="pill go" class:blocked={!canSend}>Send ⏎</span>
                    {/if}
                  {:else}
                    <span class="chip">{e.method}</span>
                  {/if}
                </span>
                <span class="host">{hostOf(e.url)}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <footer class="hints">
      <span>esc cancel</span><i>·</i><span>↑↓ choose</span><i>·</i><span>⏎ send</span>
    </footer>
  </div>
{:else}
  <div class="idle" data-tauri-drag-region>
    <Brand size={22} dim />
    <p>No pending print jobs — hit ⌘P anywhere.</p>
  </div>
{/if}

<style>
  .picker {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px 0 84px; /* clears traffic lights */
    height: 36px;
    flex-shrink: 0;
  }
  .brand-slot {
    pointer-events: none;
  }
  .top-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .count {
    font-size: 11.5px;
    color: var(--warn);
    font-family: var(--mono);
  }
  .cancel-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 1px solid var(--border);
    color: var(--muted);
    padding: 3px 10px;
    font-size: 12px;
    border-radius: 99px;
  }
  .cancel-btn:hover {
    color: var(--danger);
    border-color: rgba(255, 107, 112, 0.4);
    background: var(--danger-soft);
  }
  .cancel-btn .x {
    font-size: 11px;
    opacity: 0.7;
  }

  .flow-row {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: stretch;
  }

  /* -- the document ---------------------------------------------------------- */
  /* stage and rail-wrap are the same total width so the pipe zone —
     and the variable form in it — sit dead-center in the window */
  .stage {
    width: 250px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 0 8px 16px;
    position: relative;
    z-index: 2; /* the pipe tucks in behind the document card, not over it */
  }
  .doc-outer {
    width: 100%;
    max-height: 100%;
  }
  .doc-outer.whoosh {
    transition: transform 0.55s cubic-bezier(0.55, -0.15, 0.8, 0.5), opacity 0.5s ease 0.1s;
    transform: translateX(170px) scale(0.35) rotate(3deg);
    opacity: 0;
  }
  .doc-card {
    width: 100%;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5), 0 2px 8px rgba(0, 0, 0, 0.4);
    animation: float 5.5s ease-in-out infinite;
  }
  @keyframes float {
    0%, 100% { transform: translateY(-3px); }
    50% { transform: translateY(3px); }
  }
  .doc-card embed {
    width: 100%;
    height: 258px;
    background: #101216;
    border: none;
  }
  .no-preview {
    height: 258px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--muted);
  }
  .np-icon {
    font-size: 30px;
    color: var(--faint);
  }
  .no-preview code {
    font-family: var(--mono);
    font-size: 11px;
  }
  .np-hint {
    font-size: 11px;
    color: var(--faint);
  }
  .meta {
    padding: 9px 12px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    border-top: 1px solid var(--border-soft);
  }
  .meta strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .meta span {
    color: var(--faint);
    font-size: 11px;
  }

  /* -- the pipe -------------------------------------------------------------- */
  .pipe-zone {
    flex: 1;
    min-width: 130px;
    position: relative;
  }
  .pipe {
    position: absolute;
    left: -6px;
    right: -10px;
    top: 50%;
    height: 12px;
    translate: 0 -50%;
    opacity: 0.45;
    transition: opacity 0.3s ease;
  }
  .pipe.live {
    opacity: 1;
  }
  /* exit: hold the burst flash briefly, then drain out left → right */
  .pipe.burst {
    mask-image: linear-gradient(90deg, transparent 0 40%, #000 60% 100%);
    -webkit-mask-image: linear-gradient(90deg, transparent 0 40%, #000 60% 100%);
    mask-size: 250% 100%;
    -webkit-mask-size: 250% 100%;
    mask-repeat: no-repeat;
    -webkit-mask-repeat: no-repeat;
    animation: pipeout 0.8s ease-in forwards;
  }
  @keyframes pipeout {
    0%, 28% {
      mask-position: 100% 0;
      -webkit-mask-position: 100% 0;
    }
    100% {
      mask-position: 0% 0;
      -webkit-mask-position: 0% 0;
    }
  }
  .tube {
    position: absolute;
    inset: 0;
    border-radius: 99px;
    background: linear-gradient(180deg, #20262f, #151a20);
    border: 1px solid var(--border);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.5);
  }
  .glow {
    position: absolute;
    inset: 4px 3px;
    border-radius: 99px;
    background: var(--accent);
    opacity: 0.16;
    filter: blur(1px);
    animation: glowthrob 2.8s ease-in-out infinite;
    transition: opacity 0.3s ease;
  }
  @keyframes glowthrob {
    0%, 100% { opacity: 0.1; }
    50% { opacity: 0.26; }
  }
  /* clipping window for the traveling highlight — extends a little past the
     tube's end so the light exits beyond the pipe instead of parking there */
  .shimmer {
    position: absolute;
    inset: 3px -10px 3px 3px;
    border-radius: 99px;
    overflow: hidden;
    mask-image: linear-gradient(90deg, transparent, #000 8%, #000 90%, transparent);
    -webkit-mask-image: linear-gradient(90deg, transparent, #000 8%, #000 90%, transparent);
  }
  .shimmer::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 45%;
    border-radius: 99px;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(61, 220, 151, 0.85) 40%,
      #c9ffe6 50%,
      rgba(61, 220, 151, 0.85) 60%,
      transparent
    );
    filter: blur(0.5px) drop-shadow(0 0 8px var(--accent-glow));
    transform: translateX(-120%);
    animation: shimmer 1.9s cubic-bezier(0.45, 0.05, 0.55, 0.95) infinite;
  }
  @keyframes shimmer {
    from { transform: translateX(-120%); }
    to { transform: translateX(350%); } /* % of own width → fully exits the right end */
  }
  .pipe.fast .shimmer::after {
    animation-duration: 0.5s;
    animation-timing-function: linear;
  }
  .pipe.fast .glow {
    animation: none;
    opacity: 0.35;
  }
  .pipe.burst .shimmer {
    background: var(--accent);
    filter: drop-shadow(0 0 14px var(--accent-glow));
  }
  .pipe.burst .shimmer::after {
    animation: none;
    opacity: 0;
  }
  .pipe.burst .glow {
    animation: none;
    opacity: 0.8;
  }
  .nozzle {
    position: absolute;
    right: 0;
    top: 50%;
  }
  .ring {
    position: absolute;
    top: 0;
    left: 0;
    width: 26px;
    height: 26px;
    margin: -13px 0 0 -13px;
    border: 1.5px solid var(--accent);
    border-radius: 99px;
    opacity: 0;
    animation: pulse 2.2s ease-out infinite;
  }
  .ring.r2 {
    animation-delay: 1.1s;
  }
  @keyframes pulse {
    0% { transform: scale(0.35); opacity: 0.8; }
    70% { transform: scale(1.25); opacity: 0; }
    100% { opacity: 0; }
  }
  .pipe.fast .ring {
    animation-duration: 0.7s;
  }
  .pipe.fast .ring.r2 {
    animation-delay: 0.35s;
  }
  .pipe.burst .ring {
    animation: burstring 0.7s ease-out forwards;
  }
  @keyframes burstring {
    0% { transform: scale(0.4); opacity: 1; }
    100% { transform: scale(2.6); opacity: 0; }
  }

  .under-pipe {
    position: absolute;
    top: calc(50% + 22px);
    left: 8px;
    right: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: center;
  }
  .varform {
    width: min(100%, 280px);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 8px 10px 9px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.45);
  }
  .varform label {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .var-label {
    font-size: 10.5px;
    color: var(--muted);
    letter-spacing: 0.02em;
  }
  .var-label em {
    color: var(--accent);
    font-style: normal;
  }
  .varform input {
    font-size: 12px;
    padding: 5px 8px;
  }
  .senderr {
    font-size: 11px;
    color: var(--danger);
    text-align: center;
    max-width: 280px;
    max-height: 60px;
    overflow: hidden;
    word-break: break-all;
    user-select: text;
  }

  /* -- the endpoints --------------------------------------------------------- */
  .rail-wrap {
    width: 250px;
    flex-shrink: 0;
    position: relative;
    padding-right: 12px;
  }
  .rail {
    height: 100%;
    overflow-y: auto;
    scroll-snap-type: y proximity;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 2px 0 10px;
    scrollbar-width: none;
  }
  .rail::-webkit-scrollbar {
    display: none;
  }
  .rail::before,
  .rail::after {
    content: "";
    display: block;
    flex: 0 0 auto;
    min-height: calc(50% - 34px);
  }
  .ep-card {
    position: relative;
    flex-shrink: 0;
    scroll-snap-align: center;
    display: flex;
    flex-direction: column;
    gap: 3px;
    text-align: left;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 11px 13px;
    transform: scale(0.93);
    opacity: 0.5;
    transition: transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.28s ease,
      border-color 0.28s ease, box-shadow 0.28s ease;
  }
  .ep-card:hover {
    opacity: 0.85;
    background: var(--hover);
  }
  .ep-card.latched {
    transform: scale(1);
    opacity: 1;
    border-color: var(--accent);
    background: linear-gradient(180deg, rgba(61, 220, 151, 0.07), var(--raised) 55%);
    box-shadow: 0 6px 26px rgba(0, 0, 0, 0.45), 0 0 16px rgba(61, 220, 151, 0.18);
  }
  .ep-card.latched::before {
    content: "";
    position: absolute;
    left: -9px;
    top: 50%;
    width: 10px;
    height: 10px;
    margin-top: -5px;
    border-radius: 99px;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent-glow);
  }
  .row1 {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .name {
    font-weight: 600;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .host {
    font-size: 11px;
    color: var(--faint);
    font-family: var(--mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 99px;
    padding: 1px 8px;
    font-size: 10.5px;
    color: var(--muted);
    flex-shrink: 0;
  }
  .pill {
    border-radius: 99px;
    padding: 2px 10px;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .pill.go {
    background: var(--accent);
    color: var(--accent-ink);
    animation: nudge 2s ease-in-out infinite;
  }
  .pill.go.blocked {
    background: var(--panel);
    color: var(--faint);
    border: 1px solid var(--border);
    animation: none;
  }
  @keyframes nudge {
    0%, 100% { box-shadow: 0 0 0 rgba(61, 220, 151, 0); }
    50% { box-shadow: 0 0 12px var(--accent-glow); }
  }
  .pill.busy {
    background: var(--panel);
    padding: 4px 10px;
  }
  .pill.ok {
    background: var(--accent-soft);
    color: var(--accent);
    font-family: var(--mono);
  }
  .pill.fail {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .spinner {
    width: 11px;
    height: 11px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 99px;
    display: inline-block;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
    padding: 0 10px;
    text-align: center;
  }
  .empty p {
    margin: 0;
  }

  .hints {
    flex-shrink: 0;
    display: flex;
    justify-content: center;
    gap: 8px;
    padding: 6px 0 9px 0;
    font-size: 10.5px;
    color: var(--faint);
  }
  .hints i {
    font-style: normal;
    opacity: 0.5;
  }

  .idle {
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
  }
  .idle p {
    margin: 0;
    font-size: 12px;
  }
</style>
