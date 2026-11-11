<script lang="ts">
  type Tone = "ok" | "warn" | "bad" | "muted";
  let { tone = "ok", text, dot = false }: { tone?: Tone; text: string; dot?: boolean } = $props();
</script>

<span class="chip {tone}" class:pill={dot}>
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
  {text}
</span>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 6px;
    padding: 2px 8px;
    white-space: nowrap;
  }
  .pill {
    border-radius: 14px;
    padding: 5px 12px;
    font-size: 13px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: currentColor;
  }
  .ok {
    background: var(--ok-bg);
    color: var(--ok-text);
  }
  .warn {
    background: var(--warn-bg);
    color: var(--warn-text);
  }
  .bad {
    background: var(--bad-bg);
    color: var(--bad-text);
  }
  .muted {
    background: var(--bg-sunken);
    color: var(--text-2);
  }
</style>
