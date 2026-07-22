<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, fmtTime } from "./api";
  import type { LogEntry } from "./types";

  let entries = $state<LogEntry[]>([]);
  let levelFilter = $state("all");
  let areaFilter = $state("all");
  let textFilter = $state("");
  let expanded = $state<number | null>(null);

  const visible = $derived(
    entries
      .filter((e) => levelFilter === "all" || e.level === levelFilter)
      .filter((e) => areaFilter === "all" || e.area === areaFilter)
      .filter(
        (e) =>
          !textFilter ||
          e.msg.toLowerCase().includes(textFilter.toLowerCase()) ||
          JSON.stringify(e.detail).toLowerCase().includes(textFilter.toLowerCase())
      )
      .slice()
      .reverse()
  );

  onMount(() => {
    let lastSeq = 0;
    api.getLogs().then((l) => {
      entries = l;
      lastSeq = l.length ? l[l.length - 1].seq : 0;
    });
    const un = events.onLogAppended((entry) => {
      if (entry.seq > lastSeq) {
        lastSeq = entry.seq;
        entries.push(entry);
        if (entries.length > 2000) entries.shift();
      }
    });
    return () => {
      un.then((f) => f());
    };
  });

  async function copyVisible() {
    await navigator.clipboard.writeText(
      visible.map((e) => `${e.ts} [${e.level}] [${e.area}] ${e.msg} ${JSON.stringify(e.detail)}`).join("\n")
    );
  }

  async function clear() {
    await api.clearLogs();
    entries = [];
  }
</script>

<div class="tab">
  <header class="head">
    <h2>System logs</h2>
    <span class="sub">everything the printer does — this is the Windows debug instrument</span>
  </header>
  <div class="toolbar">
    <select bind:value={levelFilter}>
      <option value="all">all levels</option>
      <option value="debug">debug</option>
      <option value="info">info</option>
      <option value="warn">warn</option>
      <option value="error">error</option>
    </select>
    <select bind:value={areaFilter}>
      <option value="all">all areas</option>
      <option value="capture">capture</option>
      <option value="discovery">discovery</option>
      <option value="dispatch">dispatch</option>
      <option value="store">store</option>
      <option value="system">system</option>
    </select>
    <input placeholder="filter…" bind:value={textFilter} />
    <span class="spacer"></span>
    <button onclick={copyVisible}>Copy</button>
    <button onclick={() => api.exportLogs()}>Export…</button>
    <button onclick={clear}>Clear</button>
  </div>

  <div class="entries">
    {#each visible as e (e.seq)}
      <div
        class="entry {e.level}"
        onclick={() => (expanded = expanded === e.seq ? null : e.seq)}
        role="button"
        tabindex="0"
        onkeydown={(ev) => ev.key === "Enter" && (expanded = expanded === e.seq ? null : e.seq)}
      >
        <span class="ts">{fmtTime(e.ts)}</span>
        <span class="level">{e.level}</span>
        <span class="area">{e.area}</span>
        <span class="msg">{e.msg}</span>
      </div>
      {#if expanded === e.seq}
        <pre class="detail">{JSON.stringify(e.detail, null, 2)}</pre>
      {/if}
    {:else}
      <div class="empty">No log entries{levelFilter !== "all" || areaFilter !== "all" || textFilter ? " match the filters" : " yet"}.</div>
    {/each}
  </div>
</div>

<style>
  .tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 34px 22px 14px 22px;
    gap: 10px;
  }
  .head h2 {
    margin: 0;
    font-size: 17px;
    letter-spacing: -0.01em;
  }
  .sub {
    font-size: 11.5px;
    color: var(--faint);
  }
  .toolbar {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .toolbar input {
    width: 160px;
  }
  .spacer {
    flex: 1;
  }
  .entries {
    flex: 1;
    overflow-y: auto;
    font-family: var(--mono);
    font-size: 11.5px;
    background: var(--panel);
    border: 1px solid var(--border-soft);
    border-radius: 12px;
    padding: 8px;
  }
  .entry {
    display: flex;
    gap: 8px;
    padding: 3px 6px;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
  }
  .entry:hover {
    background: var(--raised);
  }
  .ts {
    color: var(--faint);
    flex-shrink: 0;
  }
  .level {
    width: 42px;
    flex-shrink: 0;
    color: var(--muted);
  }
  .entry.warn .level {
    color: var(--warn);
  }
  .entry.error .level {
    color: var(--danger);
  }
  .entry.debug {
    opacity: 0.55;
  }
  .area {
    color: var(--accent);
    opacity: 0.75;
    width: 70px;
    flex-shrink: 0;
  }
  .msg {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
  }
  .detail {
    background: var(--bg);
    border: 1px solid var(--border-soft);
    border-radius: 8px;
    padding: 8px;
    margin: 2px 0 6px 0;
    overflow-x: auto;
    user-select: text;
  }
  .empty {
    color: var(--muted);
    text-align: center;
    margin-top: 40px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }
</style>
