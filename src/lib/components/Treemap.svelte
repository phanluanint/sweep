<script lang="ts">
  import type { DiskChild } from "$lib/api";
  import { bytes } from "$lib/format";
  import { squarify } from "$lib/treemap";

  let {
    items,
    selected = null,
    onselect,
    onopen,
  }: {
    items: DiskChild[];
    selected?: string | null;
    onselect: (c: DiskChild) => void;
    onopen: (c: DiskChild) => void;
  } = $props();

  const MAX_BLOCKS = 40;
  const PALETTE = [
    ["#dce7f8", "#1d3b6b"],
    ["#d3ebd8", "#1b4d2a"],
    ["#f6dfc2", "#5e3500"],
    ["#e6d8f3", "#43266a"],
    ["#f4d5dc", "#6b1f33"],
    ["#dce3ea", "#2b3a48"],
  ];

  let width = $state(0);
  let height = $state(0);

  let blocks = $derived.by(() => {
    const sorted = [...items].sort((a, b) => b.bytes - a.bytes);
    const shown = sorted.slice(0, MAX_BLOCKS);
    const rest = sorted.slice(MAX_BLOCKS);
    const restBytes = rest.reduce((s, c) => s + c.bytes, 0);
    const rects = squarify(
      [...shown.map((c) => c.bytes), ...(restBytes > 0 ? [restBytes] : [])],
      { x: 0, y: 0, w: width, h: height },
    );
    return rects.map((r, i) => ({
      rect: r,
      child: shown[i] as DiskChild | undefined,
      label: shown[i]?.name ?? `${rest.length} more`,
      size: shown[i]?.bytes ?? restBytes,
      colors: shown[i]?.kind === "files" || !shown[i] ? ["#ececee", "#48484a"] : PALETTE[i % PALETTE.length],
    }));
  });
</script>

<div class="map" bind:clientWidth={width} bind:clientHeight={height}>
  {#each blocks as b (b.label + b.size)}
    {#if b.rect.w > 1 && b.rect.h > 1}
      <button
        type="button"
        class="block"
        class:selected={b.child && b.child.path === selected && b.child.kind === "dir"}
        disabled={!b.child}
        style:left="{b.rect.x}px"
        style:top="{b.rect.y}px"
        style:width="{b.rect.w}px"
        style:height="{b.rect.h}px"
        style:--block-bg={b.colors[0]}
        style:--block-fg={b.colors[1]}
        title="{b.label} · {bytes(b.size)}"
        onclick={() => b.child && onselect(b.child)}
        ondblclick={() => b.child && b.child.kind === "dir" && b.child.hasChildren && onopen(b.child)}
      >
        {#if b.rect.w > 56 && b.rect.h > 34}
          <strong>{b.label}</strong>
          <span>{bytes(b.size)}</span>
        {/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .map {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 260px;
  }
  .block {
    position: absolute;
    border: 2px solid var(--bg);
    border-radius: 7px;
    background: var(--bg);
    padding: 0;
    overflow: hidden;
    text-align: left;
    font-size: 12px;
    line-height: 1.3;
    color: var(--block-fg);
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
  }
  .block::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 5px;
    background: var(--block-bg);
  }
  .block > * {
    position: relative;
    padding: 0 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .block > strong {
    padding-top: 6px;
  }
  .block:hover:not(:disabled)::before {
    filter: brightness(0.95);
  }
  .block.selected::before {
    background: var(--accent);
  }
  .block.selected {
    color: var(--on-accent);
  }
  .block:disabled {
    opacity: 1;
  }
</style>
