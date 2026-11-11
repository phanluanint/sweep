<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open = $bindable(false),
    title,
    confirmLabel,
    busy = false,
    onconfirm,
    children,
  }: {
    open?: boolean;
    title: string;
    confirmLabel: string;
    busy?: boolean;
    onconfirm: () => void;
    children: Snippet;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog bind:this={dialog} onclose={() => (open = false)} aria-labelledby="confirm-title">
  <h2 id="confirm-title">{title}</h2>
  <div class="body">{@render children()}</div>
  <div class="actions">
    <button type="button" class="btn" onclick={() => (open = false)} disabled={busy}>Cancel</button>
    <button type="button" class="btn primary" onclick={onconfirm} disabled={busy}>
      {busy ? "Moving…" : confirmLabel}
    </button>
  </div>
</dialog>

<style>
  dialog {
    width: min(460px, calc(100vw - 32px));
    border: 1px solid var(--line);
    border-radius: 14px;
    padding: 22px 24px 18px;
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.25);
  }
  dialog::backdrop {
    background: rgba(0, 0, 0, 0.25);
  }
  h2 {
    margin: 0 0 10px;
    font-size: 16px;
  }
  .body {
    color: var(--text-2);
    line-height: 1.45;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
</style>
