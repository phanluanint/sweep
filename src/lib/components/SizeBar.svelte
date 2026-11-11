<script lang="ts">
  // A thin horizontal bar; value is 0–100.
  let {
    value,
    height = 6,
    color = "var(--accent)",
    track = "var(--track)",
  }: { value: number; height?: number; color?: string; track?: string } = $props();
</script>

<span class="bar" aria-hidden="true" style:height="{height}px" style:background={track}>
  <span style:width="{Math.max(0, Math.min(100, value))}%" style:background={color}></span>
</span>

<style>
  .bar {
    display: block;
    flex-grow: 1;
    border-radius: 999px;
    overflow: hidden;
  }
  .bar > span {
    display: block;
    height: 100%;
    border-radius: 999px;
    transition: width 0.3s ease;
  }
</style>
