<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { api, type GroupScan, type TrashReport } from "$lib/api";
  import { bytes, count, pct } from "$lib/format";
  import { ui } from "$lib/state.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";

  let { onchange }: { onchange?: () => void } = $props();

  const ITEMS_SHOWN = 50;

  let groups = $state<GroupScan[] | null>(null);
  let scanning = $state(false);
  let scannedAt = $state<Date | null>(null);
  let error = $state<string | null>(null);
  let report = $state<TrashReport | null>(null);
  let confirmOpen = $state(false);
  let busy = $state(false);
  const selected = new SvelteSet<string>();
  const expanded = new SvelteSet<string>();

  let sizeOf = $derived(new Map((groups ?? []).flatMap((g) => g.items.map((i) => [i.path, i.bytes] as const))));
  let selectedBytes = $derived([...selected].reduce((s, p) => s + (sizeOf.get(p) ?? 0), 0));
  let selectedGroups = $derived((groups ?? []).filter((g) => g.items.some((i) => selected.has(i.path))).length);
  let cleanable = $derived((groups ?? []).filter((g) => g.cleanable));
  let found = $derived(cleanable.reduce((s, g) => s + g.bytes, 0));
  let reviewBytes = $derived(
    cleanable.filter((g) => g.risk === "review").reduce((s, g) => s + g.bytes, 0),
  );
  let unreadable = $derived((groups ?? []).some((g) => g.errors > 0));
  let totalFiles = $derived((groups ?? []).reduce((s, g) => s + g.files, 0));

  async function scan() {
    scanning = true;
    error = null;
    try {
      const result = await api.cleanupScan();
      groups = result;
      scannedAt = new Date();
      selected.clear();
      for (const g of result) {
        if (g.cleanable && g.defaultSelected) g.items.forEach((i) => selected.add(i.path));
      }
      ui.cleanupFound = result.filter((g) => g.cleanable).reduce((s, g) => s + g.bytes, 0);
    } catch (e) {
      error = String(e);
    } finally {
      scanning = false;
    }
  }

  function groupState(g: GroupScan): "all" | "some" | "none" {
    const n = g.items.filter((i) => selected.has(i.path)).length;
    return n === 0 ? "none" : n === g.items.length ? "all" : "some";
  }

  function toggleGroup(g: GroupScan) {
    const all = groupState(g) === "all";
    g.items.forEach((i) => (all ? selected.delete(i.path) : selected.add(i.path)));
  }

  function toggleItem(path: string) {
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
  }

  function toggleExpanded(id: string) {
    if (expanded.has(id)) expanded.delete(id);
    else expanded.add(id);
  }

  function indeterminate(node: HTMLInputElement, value: boolean) {
    node.indeterminate = value;
    return { update: (v: boolean) => (node.indeterminate = v) };
  }

  async function clean() {
    busy = true;
    try {
      report = await api.cleanupClean([...selected]);
      confirmOpen = false;
      onchange?.();
      await scan();
    } catch (e) {
      error = String(e);
      confirmOpen = false;
    } finally {
      busy = false;
    }
  }

  onMount(scan);
</script>

<div class="screen">
  <header>
    <div>
      <h1>Cleanup</h1>
      <p class="muted">
        {#if scanning}
          Scanning caches and leftovers…
        {:else if scannedAt}
          Scanned {scannedAt.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} · {count(totalFiles)} files checked
        {:else}
          Find caches, logs and leftovers you can safely remove.
        {/if}
      </p>
    </div>
    <button type="button" class="btn" onclick={scan} disabled={scanning || busy}>
      {scanning ? "Scanning…" : "Rescan"}
    </button>
  </header>

  {#if error}
    <p class="note bad" role="alert">{error}</p>
  {/if}

  {#if report}
    <p class="note ok" role="status">
      Moved {count(report.moved.length)} item{report.moved.length === 1 ? "" : "s"} ({bytes(report.bytesMoved)}) to the Trash.
      {#if report.skipped.length + report.failed.length > 0}
        {report.skipped.length + report.failed.length} left in place.
      {/if}
      <button type="button" class="link" onclick={() => (report = null)}>Dismiss</button>
    </p>
  {/if}

  <section class="summary" aria-label="Scan summary">
    <div class="big">
      <span class="amount num">{groups ? bytes(found) : "—"}</span>
      <span class="muted">found · {bytes(selectedBytes)} selected</span>
    </div>
    {#if ui.volume}
      {@const used = ui.volume.total - ui.volume.free}
      {@const other = Math.max(0, used - found)}
      <div class="stack">
        <div class="stackbar" aria-hidden="true">
          <span style:width="{pct(other, ui.volume.total)}%" style:background="var(--text-3)"></span>
          <span style:width="{pct(selectedBytes, ui.volume.total)}%" style:background="var(--accent)"></span>
          <span style:width="{pct(found - selectedBytes, ui.volume.total)}%" style:background="#e8a33d"></span>
        </div>
        <div class="legend small muted">
          <span><i style:background="var(--text-3)"></i>Other data {bytes(other)}</span>
          <span><i style:background="var(--accent)"></i>Selected {bytes(selectedBytes)}</span>
          <span><i style:background="#e8a33d"></i>Not selected {bytes(found - selectedBytes)}</span>
          <span><i class="free"></i>Free {bytes(ui.volume.free)}</span>
        </div>
      </div>
    {/if}
  </section>

  {#if unreadable}
    <p class="note info small">
      <Icon name="info" size={16} />
      Some folders couldn't be read. To include them, add Sweep under System Settings › Privacy &amp;
      Security › Full Disk Access.
    </p>
  {/if}

  {#if groups}
    <section class="groups" aria-label="Cleanup groups">
      <div class="row head small muted">
        <span></span><span></span><span>Group</span><span class="r">Files</span><span class="r">Size</span><span></span>
      </div>
      {#each groups as g (g.id)}
        {@const gs = groupState(g)}
        {@const open = expanded.has(g.id)}
        <div class="row group" class:open class:disabled={!g.available}>
          {#if g.cleanable && g.items.length > 0}
            <input
              id="g-{g.id}"
              type="checkbox"
              checked={gs === "all"}
              use:indeterminate={gs === "some"}
              onchange={() => toggleGroup(g)}
            />
          {:else}
            <span></span>
          {/if}
          <span class="tile" class:warn={g.risk === "review"} aria-hidden="true"><Icon name={g.icon} /></span>
          <label for="g-{g.id}" class="name">
            <span class="title">
              {g.label}
              {#if !g.available}<span class="badge muted">Coming later</span>
              {:else if g.risk === "review"}<span class="badge warn">Review</span>
              {:else if g.risk === "info"}<span class="badge info">Info only</span>{/if}
            </span>
            <span class="desc small muted">
              {g.description}
              {#if g.skippedInUse.length}· {g.skippedInUse.length} skipped while their app is open{/if}
            </span>
          </label>
          <span class="r muted num">{g.available ? count(g.files) : ""}</span>
          <span class="r num strong">{g.available ? bytes(g.bytes) : ""}</span>
          {#if g.items.length > 0}
            <button
              type="button"
              class="chev"
              aria-expanded={open}
              aria-label="{open ? 'Hide' : 'Show'} items in {g.label}"
              onclick={() => toggleExpanded(g.id)}
            >
              <Icon name={open ? "chevron-down" : "chevron-right"} size={14} />
            </button>
          {:else}
            <span></span>
          {/if}
        </div>
        {#if open}
          <div class="items">
            {#each g.items.slice(0, ITEMS_SHOWN) as item (item.path)}
              <div class="row item">
                <span></span>
                {#if g.cleanable}
                  <input
                    id="i-{item.path}"
                    type="checkbox"
                    checked={selected.has(item.path)}
                    onchange={() => toggleItem(item.path)}
                  />
                {:else}
                  <span></span>
                {/if}
                <label for="i-{item.path}" class="iname" title={item.path}>{item.name}</label>
                <span class="r muted num small">{count(item.files)}</span>
                <span class="r num">{bytes(item.bytes)}</span>
                <span></span>
              </div>
            {/each}
            {#if g.items.length > ITEMS_SHOWN}
              <div class="row item small muted">
                <span></span><span></span>
                <span>and {g.items.length - ITEMS_SHOWN} smaller items</span>
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </section>
  {:else if scanning}
    <p class="muted">Measuring folders, this takes a few seconds…</p>
  {/if}

  <footer>
    <div>
      <strong>{bytes(selectedBytes)} selected in {selectedGroups} group{selectedGroups === 1 ? "" : "s"}</strong>
      <div class="small muted">Items go to the Trash, so you can put them back until you empty it.</div>
    </div>
    <button
      type="button"
      class="btn primary big-btn"
      disabled={selected.size === 0 || busy || scanning}
      onclick={() => (confirmOpen = true)}
    >
      Move {bytes(selectedBytes)} to Trash
    </button>
  </footer>
</div>

<ConfirmDialog bind:open={confirmOpen} title="Move to Trash?" confirmLabel="Move to Trash" {busy} onconfirm={clean}>
  {count(selected.size)} item{selected.size === 1 ? "" : "s"} ({bytes(selectedBytes)}) will go to the Trash. Apps
  rebuild their caches when needed. You can put items back from the Trash until you empty it.
</ConfirmDialog>

<style>
  .screen {
    padding: 28px 32px 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-height: 100vh;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 12px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  p {
    margin: 0;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 10px;
    padding: 10px 14px;
  }
  .note.ok {
    background: var(--ok-bg);
    color: var(--ok-text);
  }
  .note.bad {
    background: var(--bad-bg);
    color: var(--bad-text);
  }
  .note.info {
    background: var(--bg-sunken);
    color: var(--text-2);
  }
  .link {
    margin-left: auto;
    border: 0;
    background: none;
    color: inherit;
    text-decoration: underline;
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    align-items: center;
    background: var(--bg-sunken);
    border-radius: 14px;
    padding: 18px 22px;
  }
  .big {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .amount {
    font-size: 36px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .stack {
    flex: 1 1 320px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .stackbar {
    display: flex;
    height: 12px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--track);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
  .legend i.free {
    background: var(--track);
    border: 1px solid var(--line-strong);
  }
  .groups {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 24px 40px minmax(0, 1fr) 90px 90px 28px;
    align-items: center;
    gap: 12px;
    padding: 0 14px;
  }
  .row.head {
    height: 32px;
    background: var(--bg-header);
    border-bottom: 1px solid var(--line);
    font-weight: 500;
  }
  .row.group {
    min-height: 60px;
    border-bottom: 1px solid var(--line);
  }
  .row.group.open {
    background: var(--bg-row-selected);
    border-bottom: 0;
  }
  .row.group.disabled {
    opacity: 0.6;
  }
  .items {
    background: var(--bg-row-selected);
    border-bottom: 1px solid var(--line);
    padding-bottom: 6px;
    max-height: 360px;
    overflow: auto;
  }
  .row.item {
    min-height: 32px;
  }
  .row.item input {
    justify-self: end;
  }
  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }
  .tile {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    background: var(--accent-soft);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .tile.warn {
    background: var(--warn-bg);
    color: var(--warn-text);
  }
  .name {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .desc,
  .iname {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .r {
    text-align: right;
  }
  .strong {
    font-weight: 600;
  }
  .chev {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .chev:hover {
    background: rgba(127, 127, 127, 0.15);
  }
  footer {
    position: sticky;
    bottom: 0;
    margin-top: auto;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 14px 0 20px;
    background: var(--bg);
    border-top: 1px solid var(--line);
  }
  .big-btn {
    min-height: 38px;
    padding: 0 20px;
  }
</style>
