<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, fmtBytes } from "./api";
  import type { Job, JobSend } from "./types";

  let { endpointId }: { endpointId: string } = $props();

  let jobs = $state<Job[]>([]);
  let expanded = $state<string | null>(null);

  interface Row extends JobSend {
    jobTitle: string;
    jobBytes: number;
    rowKey: string;
  }

  const rows = $derived(
    jobs
      .flatMap((j) =>
        j.sends
          .filter((s) => s.endpoint_id === endpointId)
          .map((s, i) => ({
            ...s,
            jobTitle: j.title,
            jobBytes: j.bytes,
            rowKey: `${j.id}:${i}`,
          }))
      )
      .sort((a, b) => b.at.localeCompare(a.at)) as Row[]
  );

  async function load() {
    jobs = await api.listJobs();
  }

  onMount(() => {
    load();
    const un = events.onDispatchResult(() => load());
    return () => {
      un.then((f) => f());
    };
  });

  function fmtWhen(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleDateString(undefined, { month: "short", day: "numeric" }) +
        " · " + d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    } catch {
      return iso;
    }
  }
</script>

<div class="activity">
  {#if rows.length === 0}
    <div class="empty">
      <p>Nothing piped here yet.</p>
      <p class="sub">Every send to this endpoint — from the pipe window or a Test — shows up here.</p>
    </div>
  {:else}
    <ul>
      {#each rows as r (r.rowKey)}
        <li>
          <button
            class="row"
            onclick={() => (expanded = expanded === r.rowKey ? null : r.rowKey)}
          >
            <span class="mark {r.ok ? 'ok' : 'fail'}"></span>
            <span class="title" title={r.jobTitle}>{r.jobTitle}</span>
            <span class="meta">{fmtBytes(r.jobBytes)}</span>
            <span class="meta mono">
              {r.ok ? r.http_status : (r.http_status ?? "error")} · {r.duration_ms}ms
            </span>
            <span class="when">{fmtWhen(r.at)}</span>
          </button>
          {#if expanded === r.rowKey}
            <div class="detail">
              {#if r.error}<p class="err-text">{r.error}</p>{/if}
              {#if r.response_head}<pre>{r.response_head}</pre>{/if}
              {#if !r.error && !r.response_head}<p class="sub">Empty response body.</p>{/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .activity {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px 22px;
    max-width: 760px;
  }
  .empty {
    text-align: center;
    margin-top: 70px;
    color: var(--muted);
  }
  .empty p {
    margin: 4px 0;
  }
  .sub {
    color: var(--faint);
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--panel);
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    padding: 9px 12px;
    text-align: left;
  }
  .row:hover {
    background: var(--raised);
  }
  .mark {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    flex-shrink: 0;
  }
  .mark.ok {
    background: var(--accent);
    box-shadow: 0 0 5px var(--accent-glow);
  }
  .mark.fail {
    background: var(--danger);
  }
  .title {
    font-weight: 600;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--muted);
    font-size: 11.5px;
    flex-shrink: 0;
  }
  .mono {
    font-family: var(--mono);
  }
  .when {
    color: var(--faint);
    font-size: 11.5px;
    flex-shrink: 0;
  }
  .detail {
    margin: 2px 0 6px 18px;
    padding: 8px 12px;
    background: var(--panel);
    border-radius: 8px;
    border: 1px solid var(--border-soft);
  }
  .detail pre {
    margin: 0;
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--muted);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
    max-height: 160px;
    overflow-y: auto;
  }
  .err-text {
    color: var(--danger);
    font-size: 12px;
    margin: 0 0 4px 0;
    user-select: text;
  }
</style>
