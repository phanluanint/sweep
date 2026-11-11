<script lang="ts">
  // Stroke icons from the design. Decorative: hidden from assistive tech.
  let { name, size = 18 }: { name: string; size?: number } = $props();
</script>

<svg
  aria-hidden="true"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  {#if name === "sparkle"}
    <path d="M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7-4.7-1.8 4.7-1.8z" />
    <path d="M18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z" />
  {:else if name === "map"}
    <rect x="3" y="3" width="8" height="18" rx="1.5" />
    <rect x="13" y="3" width="8" height="8" rx="1.5" />
    <rect x="13" y="13" width="8" height="8" rx="1.5" />
  {:else if name === "pulse"}
    <path d="M3 12h4l2.5-6 4 12 2.5-6H21" />
  {:else if name === "folder"}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  {:else if name === "doc"}
    <path d="M6 3h9l4 4v14H6z" /><path d="M9 11h7M9 15h7M9 19h4" />
  {:else if name === "code"}
    <path d="M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3c3 3.5 3 14.5 0 18M12 3c-3 3.5-3 14.5 0 18" />
  {:else if name === "trash"}
    <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />
  {:else if name === "download"}
    <path d="M12 3v12M7 10l5 5 5-5M4 20h16" />
  {:else if name === "box"}
    <path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" /><path d="M4 7.5l8 4.5 8-4.5M12 12v9" />
  {:else if name === "chevron-right"}
    <path d="M9 6l6 6-6 6" />
  {:else if name === "chevron-down"}
    <path d="M6 9l6 6 6-6" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" /><path d="M12 8v.5M12 11v5" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" />
  {/if}
</svg>
