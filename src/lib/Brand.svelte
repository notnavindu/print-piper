<script lang="ts">
  let {
    size = 20,
    wordmark = true,
    dim = false,
    img = false,
  }: { size?: number; wordmark?: boolean; dim?: boolean; img?: boolean } = $props();
</script>

<span class="brand" class:dim>
  {#if img}
    <img class="glyph-img" src="/logo.png" width={size} height={size} alt="" style="border-radius: {size * 0.22}px" />
  {:else}
  <svg
    class="glyph"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="pipe-g" x1="4" y1="5" x2="19" y2="20" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="#8b93a1" />
        <stop offset="1" stop-color="#3ddc97" />
      </linearGradient>
    </defs>
    <!-- inlet ticks: the "document" going in -->
    <path d="M3 3.2 H7" stroke="#59616d" stroke-width="1.6" stroke-linecap="round" />
    <path d="M3 6.8 H5.4" stroke="#59616d" stroke-width="1.6" stroke-linecap="round" />
    <!-- the pipe: elbow from the print dialog to anywhere -->
    <path
      d="M5.2 11 H12 A6.4 6.4 0 0 1 18.4 17.4 V20.4"
      stroke="url(#pipe-g)"
      stroke-width="3.4"
      stroke-linecap="round"
    />
    <!-- outlet drop -->
    <circle class="drop" cx="18.4" cy="21.4" r="1.7" fill="#3ddc97" />
  </svg>
  {/if}
  {#if wordmark}
    <span class="word">Print<em>Piper</em></span>
  {/if}
</span>

<style>
  .brand {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    line-height: 1;
  }
  .brand.dim {
    opacity: 0.55;
  }
  .glyph {
    flex-shrink: 0;
  }
  .glyph-img {
    flex-shrink: 0;
    display: block;
    object-fit: contain;
  }
  .drop {
    animation: drip 2.6s ease-in-out infinite;
    transform-origin: 18.4px 21.4px;
  }
  @keyframes drip {
    0%,
    55%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    70% {
      opacity: 0.35;
      transform: scale(0.6);
    }
    85% {
      opacity: 0.9;
      transform: scale(1.15);
    }
  }
  .word {
    font-weight: 700;
    font-size: 14px;
    letter-spacing: -0.01em;
  }
  .word em {
    font-style: normal;
    color: var(--accent);
  }
</style>
