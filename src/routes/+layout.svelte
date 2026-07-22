<script lang="ts">
  let { children } = $props();
</script>

{@render children()}

<style>
  :global(:root) {
    --bg: #0b0d10;
    --panel: #12151a;
    --raised: #191e25;
    --hover: #20262f;
    --border: #262c35;
    --border-soft: #1c222a;
    --text: #e8eaed;
    --muted: #8b93a1;
    --faint: #59616d;
    --accent: #3ddc97;
    --accent-strong: #52f0ab;
    --accent-ink: #062b1c;
    --accent-soft: rgba(61, 220, 151, 0.13);
    --accent-glow: rgba(61, 220, 151, 0.4);
    --danger: #ff6b70;
    --danger-soft: rgba(255, 107, 112, 0.12);
    --warn: #ffd479;
    --mono: ui-monospace, "SF Mono", Menlo, monospace;
  }
  :global(*) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-size: 13px;
    color: var(--text);
    background: var(--bg);
    -webkit-font-smoothing: antialiased;
    user-select: none;
    overflow: hidden;
  }
  :global(button) {
    font: inherit;
    color: inherit;
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 5px 12px;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease, transform 0.1s ease,
      box-shadow 0.2s ease, opacity 0.15s ease;
  }
  :global(button:hover) {
    background: var(--hover);
    border-color: #313947;
  }
  :global(button:active) {
    transform: scale(0.97);
  }
  :global(button:disabled) {
    opacity: 0.4;
    cursor: default;
    transform: none;
  }
  :global(button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }
  :global(button.primary:hover) {
    background: var(--accent-strong);
    border-color: var(--accent-strong);
    box-shadow: 0 2px 14px var(--accent-glow);
  }
  :global(button.danger) {
    color: var(--danger);
    border-color: rgba(255, 107, 112, 0.35);
    background: transparent;
  }
  :global(button.danger:hover) {
    background: var(--danger-soft);
    border-color: var(--danger);
  }
  :global(input),
  :global(select),
  :global(textarea) {
    font: inherit;
    color: inherit;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 9px;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }
  :global(input:focus),
  :global(select:focus),
  :global(textarea:focus) {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  :global(input::placeholder) {
    color: var(--faint);
  }
  :global(::-webkit-scrollbar) {
    width: 8px;
    height: 8px;
  }
  :global(::-webkit-scrollbar-thumb) {
    background: #2a313c;
    border-radius: 99px;
  }
  :global(::-webkit-scrollbar-thumb:hover) {
    background: #364050;
  }
  :global(::-webkit-scrollbar-corner) {
    background: transparent;
  }
</style>
