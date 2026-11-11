<script lang="ts">
  // Line chart of recent values, 0–100 on the y axis.
  let { values, label, height = 60 }: { values: number[]; label: string; height?: number } =
    $props();

  const W = 300;
  let points = $derived.by(() => {
    if (values.length < 2) return "";
    const step = W / (values.length - 1);
    return values
      .map((v, i) => `${(i * step).toFixed(1)},${(height - 2 - (Math.min(100, v) / 100) * (height - 4)).toFixed(1)}`)
      .join(" ");
  });
</script>

<svg role="img" aria-label={label} viewBox="0 0 {W} {height}" preserveAspectRatio="none" style:height="{height}px">
  <line x1="0" y1={height - 0.5} x2={W} y2={height - 0.5} stroke="var(--line)" />
  {#if points}
    <polyline fill="none" stroke="var(--accent)" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke" {points} />
  {/if}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
  }
</style>
