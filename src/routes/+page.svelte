<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Cleanup from "$lib/screens/Cleanup.svelte";
  import DiskMap from "$lib/screens/DiskMap.svelte";
  import Health from "$lib/screens/Health.svelte";
  import { api } from "$lib/api";
  import { ui } from "$lib/state.svelte";

  async function refreshVolume() {
    try {
      ui.volume = await api.volumeInfo();
    } catch {
      ui.volume = null;
    }
  }

  onMount(() => {
    refreshVolume();
    const timer = setInterval(refreshVolume, 30_000);
    return () => clearInterval(timer);
  });
</script>

<div class="window">
  <Sidebar />
  <main>
    <!-- Screens stay mounted so scans survive switching tabs. -->
    <div hidden={ui.screen !== "cleanup"}><Cleanup onchange={refreshVolume} /></div>
    <div hidden={ui.screen !== "disk"}><DiskMap onchange={refreshVolume} /></div>
    {#if ui.screen === "health"}<Health />{/if}
  </main>
</div>

<style>
  .window {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  main {
    flex: 1 1 auto;
    min-width: 0;
    overflow: auto;
  }
  main > div {
    min-height: 100%;
  }
</style>
