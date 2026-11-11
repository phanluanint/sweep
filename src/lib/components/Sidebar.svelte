<script lang="ts">
  import Icon from "./Icon.svelte";
  import SizeBar from "./SizeBar.svelte";
  import { ui, type Screen } from "$lib/state.svelte";
  import { bytes, pct } from "$lib/format";

  const items: { id: Screen; label: string; icon: string }[] = [
    { id: "cleanup", label: "Cleanup", icon: "sparkle" },
    { id: "disk", label: "Disk Map", icon: "map" },
    { id: "health", label: "Health", icon: "pulse" },
  ];
</script>

<nav aria-label="Sections">
  <div class="drag" data-tauri-drag-region></div>
  <div class="brand">Sweep</div>
  {#each items as item (item.id)}
    <button
      type="button"
      class="item"
      class:active={ui.screen === item.id}
      aria-current={ui.screen === item.id ? "page" : undefined}
      onclick={() => (ui.screen = item.id)}
    >
      <Icon name={item.icon} />
      <span class="label">{item.label}</span>
      {#if item.id === "cleanup" && ui.cleanupFound}
        <span class="count">{bytes(ui.cleanupFound)}</span>
      {/if}
    </button>
  {/each}

  <div class="spacer"></div>

  {#if ui.volume}
    {@const used = ui.volume.total - ui.volume.free}
    <div class="disk">
      <div class="row small"><strong>Startup disk</strong><span class="muted">{bytes(ui.volume.free)} free</span></div>
      <SizeBar value={pct(used, ui.volume.total)} />
      <div class="small muted">{bytes(used)} of {bytes(ui.volume.total)} used</div>
    </div>
  {/if}
</nav>

<style>
  nav {
    width: 220px;
    flex: 0 0 220px;
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 0 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .drag {
    height: 44px;
    flex: 0 0 44px;
  }
  .brand {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-2);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 0 10px 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
  }
  .item:hover:not(.active) {
    background: rgba(127, 127, 127, 0.12);
  }
  .item.active {
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 500;
  }
  .label {
    flex-grow: 1;
  }
  .count {
    font-size: 12px;
    opacity: 0.85;
  }
  .spacer {
    flex-grow: 1;
  }
  .disk {
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    justify-content: space-between;
  }
</style>
