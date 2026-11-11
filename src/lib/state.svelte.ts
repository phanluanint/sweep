// App-wide UI state shared across screens.
import type { VolumeInfo } from "./api";

export type Screen = "cleanup" | "disk" | "health";

export const ui = $state({
  screen: "cleanup" as Screen,
  /** Total found by the last cleanup scan, shown in the sidebar. */
  cleanupFound: null as number | null,
  volume: null as VolumeInfo | null,
});
