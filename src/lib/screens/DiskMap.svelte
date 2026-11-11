<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    api,
    events,
    type DiskChild,
    type FileInfo,
    type ScanProgress,
    type ScanStats,
    type TrashReport,
  } from "$lib/api";
  import { ago, bytes, count, pct } from "$lib/format";
  import Icon from "$lib/components/Icon.svelte";
  import SizeBar from "$lib/components/SizeBar.svelte";
  import Treemap from "$lib/components/Treemap.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";

  let { onchange }: { onchange?: () => void } = $props();

  type Filter = "all" | "big" | "old" | "video" | "archive";
  const FILTERS: { id: Filter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "big", label: "Over 1 GB" },
    { id: "old", label: "Not used in a year" },
    { id: "video", label: "Videos" },
    { id: "archive", label: "Installers & archives" },
  ];
  const FILES_SHOWN = 40;

  let home = $state("");
  let rootChoice = $state<"home" | "disk">("home");
  let scanning = $state(false);
  let progress = $state<ScanProgress | null>(null);
  let stats = $state<ScanStats | null>(null);
  let error = $state<string | null>(null);
  let report = $state<TrashReport | null>(null);

  let current = $state<DiskChild | null>(null);
  let children = $state<DiskChild[]>([]);
  const expanded = new SvelteMap<string, DiskChild[]>();
  let focus = $state<string | null>(null);

  let largest = $state<FileInfo[]>([]);
  let filter = $state<Filter>("all");

  /** Checked items: path -> bytes. */
  const checked = new SvelteMap<string, number>();
  let confirmOpen = $state(false);
  let busy = $state(false);

  let crumbs = $derived.by(() => {
    if (!stats || !current) return [];
    const root = stats.root;
    const rel = current.path.slice(root.length).split("/").filter(Boolean);
    const out = [{ name: root === home ? "Home" : root === "/" ? "Startup disk" : root, path: root }];
    let acc = root;
    for (const part of rel) {
      acc = acc === "/" ? `/${part}` : `${acc}/${part}`;
      out.push({ name: part, path: acc });
    }
    return out;
  });

  let shownFiles = $derived.by(() => {
    const yearAgo = Date.now() / 1000 - 365 * 86400;
    return largest
      .filter((f) => {
        switch (filter) {
          case "big":
            return f.bytes >= 1e9;
          case "old":
            return f.lastUsedSecs < yearAgo;
          case "video":
            return f.kind === "video";
          case "archive":
            return f.kind === "archive" || f.kind === "installer" || f.kind === "disk-image";
          default:
            return true;
        }
      })
      .slice(0, FILES_SHOWN);
  });

  /** Selected size, not counting items inside a selected folder twice. */
  let checkedBytes = $derived.by(() => {
    const paths = [...checked.keys()];
    return paths
      .filter((p) => !paths.some((q) => q !== p && p.startsWith(q + "/")))
      .reduce((s, p) => s + (checked.get(p) ?? 0), 0);
  });

  async function scan() {
    scanning = true;
    error = null;
    report = null;
    progress = null;
    checked.clear();
    expanded.clear();
    try {
      stats = await api.diskScan(rootChoice === "home" ? home : "/");
      await open(stats.root);
      largest = await api.diskLargest();
      onchange?.();
    } catch (e) {
      if (!String(e).includes("cancelled")) error = String(e);
    } finally {
      scanning = false;
    }
  }

  async function open(path: string) {
    const [node, kids] = await api.diskChildren(path);
    current = node;
    children = kids;
    focus = null;
    expanded.clear();
  }

  async function toggleExpand(c: DiskChild) {
    if (expanded.has(c.path)) {
      expanded.delete(c.path);
      return;
    }
    const [, kids] = await api.diskChildren(c.path);
    expanded.set(c.path, kids);
  }

  function toggleCheck(path: string, size: number) {
    if (checked.has(path)) checked.delete(path);
    else checked.set(path, size);
  }

  async function trash() {
    busy = true;
    try {
      report = await api.diskTrash([...checked.keys()]);
      confirmOpen = false;
      checked.clear();
      largest = await api.diskLargest();
      // The open folder may have been trashed; walk up until one is still in the tree.
      let path = current?.path ?? stats?.root ?? "";
      for (;;) {
        try {
          await open(path);
          break;
        } catch {
          const up = path.slice(0, path.lastIndexOf("/")) || "/";
          if (up === path) break;
          path = up;
        }
      }
      onchange?.();
    } catch (e) {
      error = String(e);
      confirmOpen = false;
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    api.homeDir().then((h) => (home = h));
    const off = events.diskProgress((p) => (progress = p));
    return () => {
      off.then((f) => f());
    };
  });
</script>

{#snippet rows(list: DiskChild[], parentBytes: number, depth: number)}
  {#each list as c (c.path + c.kind)}
    {@const isDir = c.kind === "dir"}
    {@const isOpen = expanded.has(c.path)}
    <div class="trow" class:focus={focus === c.path && isDir} style:--depth={depth}>
      <span class="namecell">
        {#if isDir && c.hasChildren}
          <button
            type="button"
            class="chev"
            aria-expanded={isOpen}
            aria-label="{isOpen ? 'Collapse' : 'Expand'} {c.name}"
            onclick={() => toggleExpand(c)}
          >
            <Icon name={isOpen ? "chevron-down" : "chevron-right"} size={12} />
          </button>
        {:else}
          <span class="chev"></span>
        {/if}
        {#if isDir}
          <input
            type="checkbox"
            aria-label="Select {c.name}"
            checked={checked.has(c.path)}
            onchange={() => toggleCheck(c.path, c.bytes)}
          />
          <span class="folder" aria-hidden="true"><Icon name="folder" size={16} /></span>
          <button
            type="button"
            class="fname"
            onclick={() => (focus = c.path)}
            ondblclick={() => c.hasChildren && open(c.path)}
            title="Double-click to open">{c.name}</button
          >
        {:else}
          <span class="loose muted">{c.name}</span>
        {/if}
      </span>
      <span class="share">
        <SizeBar value={pct(c.bytes, parentBytes)} />
        <span class="small muted num pctlabel">{Math.round(pct(c.bytes, parentBytes))}%</span>
      </span>
      <span class="r num" class:strong={depth === 0}>{bytes(c.bytes)}</span>
    </div>
    {#if isOpen}
      {@render rows(expanded.get(c.path) ?? [], c.bytes, depth + 1)}
    {/if}
  {/each}
{/snippet}

<div class="screen">
  <header>
    <div>
      <h1>Disk Map</h1>
      <p class="muted">
        {#if scanning}
          Scanning… {progress ? `${count(progress.files)} files, ${bytes(progress.bytes)}` : ""}
        {:else if stats}
          {bytes(stats.bytes)} in {count(stats.files)} files · scanned in {(stats.elapsedMs / 1000).toFixed(1)} s
        {:else}
          See which folders and files take the most space.
        {/if}
      </p>
    </div>
    <div class="actions">
      <label class="select">
        <span class="muted">Scan</span>
        <select bind:value={rootChoice} disabled={scanning}>
          <option value="home">Home folder</option>
          <option value="disk">Startup disk</option>
        </select>
      </label>
      {#if scanning}
        <button type="button" class="btn" onclick={() => api.diskCancel()}>Stop</button>
      {:else}
        <button type="button" class="btn primary" onclick={scan}>{stats ? "Rescan" : "Scan"}</button>
      {/if}
    </div>
  </header>

  {#if scanning && progress}
    <p class="small muted current" aria-live="polite">{progress.current}</p>
  {/if}
  {#if error}<p class="note bad" role="alert">{error}</p>{/if}
  {#if report}
    <p class="note ok" role="status">
      Moved {count(report.moved.length)} item{report.moved.length === 1 ? "" : "s"} ({bytes(report.bytesMoved)}) to the Trash.
      {#each [...report.skipped, ...report.failed] as s (s.path)}
        <span class="small">{s.path.split("/").pop()}: {s.reason}.</span>
      {/each}
      <button type="button" class="link" onclick={() => (report = null)}>Dismiss</button>
    </p>
  {/if}
  {#if stats && stats.errors > 0}
    <p class="note info small">
      <Icon name="info" size={16} />
      {count(stats.errors)} folders couldn't be read and aren't counted. Add Sweep under System Settings ›
      Privacy &amp; Security › Full Disk Access to include them.
    </p>
  {/if}

  {#if !stats && !scanning}
    <div class="empty">
      <Icon name="map" size={32} />
      <p>Scan your home folder to see where the space goes.</p>
      <button type="button" class="btn primary" onclick={scan}>Scan home folder</button>
    </div>
  {/if}

  {#if stats && current}
    <nav aria-label="Folder path" class="crumbs">
      {#each crumbs as crumb, i (crumb.path)}
        {#if i > 0}<span class="sep" aria-hidden="true">›</span>{/if}
        {#if i === crumbs.length - 1}
          <span aria-current="location" class="crumb here">{crumb.name}</span>
        {:else}
          <button type="button" class="crumb" onclick={() => open(crumb.path)}>{crumb.name}</button>
        {/if}
      {/each}
      <span class="muted small total">{bytes(current.bytes)}</span>
    </nav>

    <div class="split">
      <section class="tree" aria-label="Folder tree">
        <div class="trow head small muted">
          <span>Name</span><span>Share of parent</span><span class="r">Size</span>
        </div>
        <div class="tbody">
          {@render rows(children, current.bytes, 0)}
        </div>
      </section>
      <section class="mapbox" aria-label="Treemap">
        <div class="small muted maphint">
          Blocks are sized by disk use. Click to select, double-click to open.
        </div>
        <div class="mapwrap">
          <Treemap
            items={children}
            selected={focus}
            onselect={(c) => (focus = c.path)}
            onopen={(c) => open(c.path)}
          />
        </div>
      </section>
    </div>

    <section class="files" aria-labelledby="files-h">
      <div class="fhead">
        <h2 id="files-h">Largest files</h2>
        <div role="group" aria-label="Filter" class="chips">
          {#each FILTERS as f (f.id)}
            <button type="button" class="chipbtn" aria-pressed={filter === f.id} onclick={() => (filter = f.id)}>
              {f.label}
            </button>
          {/each}
        </div>
      </div>
      <div class="tablewrap">
        <table>
          <thead>
            <tr>
              <th scope="col" class="cb"><span class="sr-only">Select</span></th>
              <th scope="col">File</th>
              <th scope="col">Last used</th>
              <th scope="col" class="r">Size</th>
              <th scope="col" class="r"><span class="sr-only">Actions</span></th>
            </tr>
          </thead>
          <tbody>
            {#each shownFiles as f (f.path)}
              <tr class:checked={checked.has(f.path)}>
                <td class="cb">
                  <input
                    type="checkbox"
                    aria-label="Select {f.name}"
                    checked={checked.has(f.path)}
                    onchange={() => toggleCheck(f.path, f.bytes)}
                  />
                </td>
                <td>
                  <div class="fn">{f.name}</div>
                  <div class="small muted path" title={f.parent}>{f.parent.replace(home, "~")}</div>
                </td>
                <td class="muted">{ago(f.lastUsedSecs)}</td>
                <td class="r num strong">{bytes(f.bytes)}</td>
                <td class="r">
                  <button type="button" class="link-btn" onclick={() => api.revealInFinder(f.path)}>
                    Show in Finder
                  </button>
                </td>
              </tr>
            {:else}
              <tr><td colspan="5" class="muted">No files match this filter.</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {/if}

  {#if stats}
    <footer>
      <div>
        <strong>{checked.size === 0 ? "Nothing selected" : `${count(checked.size)} selected · ${bytes(checkedBytes)}`}</strong>
        <div class="small muted">Only items inside your home folder can be moved to the Trash.</div>
      </div>
      <div class="actions">
        <button type="button" class="btn" disabled={checked.size === 0} onclick={() => checked.clear()}>Clear</button>
        <button
          type="button"
          class="btn primary big-btn"
          disabled={checked.size === 0 || busy}
          onclick={() => (confirmOpen = true)}
        >
          Move {bytes(checkedBytes)} to Trash
        </button>
      </div>
    </footer>
  {/if}
</div>

<ConfirmDialog bind:open={confirmOpen} title="Move to Trash?" confirmLabel="Move to Trash" {busy} onconfirm={trash}>
  {count(checked.size)} item{checked.size === 1 ? "" : "s"} ({bytes(checkedBytes)}) will go to the Trash:
  <ul class="small">
    {#each [...checked.keys()].slice(0, 6) as p (p)}<li>{p.replace(home, "~")}</li>{/each}
    {#if checked.size > 6}<li>and {checked.size - 6} more</li>{/if}
  </ul>
  You can put them back from the Trash until you empty it.
</ConfirmDialog>

<style>
  .screen {
    padding: 28px 32px 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100vh;
  }
  header {
    display: flex;
    flex-wrap: wrap;
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
  h2 {
    margin: 0;
    font-size: 16px;
  }
  p {
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .select {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 10px;
    border: 1px solid var(--line-strong);
    border-radius: 7px;
  }
  .select select {
    border: 0;
    background: transparent;
  }
  .current {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note {
    display: flex;
    flex-wrap: wrap;
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
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 80px 0;
    color: var(--text-2);
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
  }
  .crumb {
    border: 0;
    background: none;
    color: var(--accent);
    padding: 4px 6px;
    border-radius: 6px;
  }
  .crumb:hover {
    background: var(--bg-sunken);
  }
  .crumb.here {
    color: var(--text);
    font-weight: 600;
    background: var(--bg-sunken);
  }
  .sep {
    color: var(--text-3);
  }
  .total {
    margin-left: 8px;
  }
  .split {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
  }
  .tree {
    flex: 1 1 420px;
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    max-height: 460px;
  }
  .tbody {
    overflow: auto;
  }
  .trow {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 130px 80px;
    gap: 12px;
    align-items: center;
    min-height: 34px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
  }
  .trow.head {
    min-height: 32px;
    background: var(--bg-header);
    font-weight: 500;
  }
  .trow.focus {
    background: var(--accent-soft);
  }
  .namecell {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-left: calc(var(--depth) * 18px);
  }
  .chev {
    width: 20px;
    height: 20px;
    flex: 0 0 20px;
    border: 0;
    background: none;
    color: var(--text-2);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }
  .folder {
    color: var(--accent-mid);
    display: flex;
  }
  .fname {
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .loose {
    padding-left: 22px;
  }
  .share {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pctlabel {
    width: 32px;
    text-align: right;
  }
  .mapbox {
    flex: 1 1 380px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .mapwrap {
    flex: 1;
    min-height: 300px;
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .fhead {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chipbtn {
    min-height: 28px;
    padding: 0 12px;
    border-radius: 14px;
    border: 1px solid var(--line-strong);
    background: var(--bg);
    font-size: 13px;
  }
  .chipbtn[aria-pressed="true"] {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg);
  }
  .tablewrap {
    overflow-x: auto;
  }
  table {
    width: 100%;
    min-width: 620px;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    text-align: left;
    font-weight: 500;
    font-size: 12px;
    color: var(--text-2);
    padding: 6px 10px;
  }
  td {
    padding: 7px 10px;
    border-top: 1px solid var(--line);
  }
  tr.checked td {
    background: var(--bg-row-selected);
  }
  .cb {
    width: 28px;
  }
  .fn {
    font-weight: 500;
  }
  .path {
    max-width: 420px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .link-btn {
    border: 0;
    background: none;
    color: var(--accent);
    padding: 4px;
  }
  input[type="checkbox"] {
    width: 15px;
    height: 15px;
    margin: 0;
    accent-color: var(--accent);
  }
  .r {
    text-align: right;
  }
  .strong {
    font-weight: 600;
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
  ul {
    margin: 8px 0;
    padding-left: 18px;
    word-break: break-all;
  }
</style>
