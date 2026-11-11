<script lang="ts">
  import { onMount } from "svelte";
  import { api, events, type HealthSnapshot } from "$lib/api";
  import { bytes, pct, rpm, uptime } from "$lib/format";
  import Sparkline from "$lib/components/Sparkline.svelte";
  import SizeBar from "$lib/components/SizeBar.svelte";
  import StatusChip from "$lib/components/StatusChip.svelte";
  import Icon from "$lib/components/Icon.svelte";

  const HISTORY = 30;
  let snap = $state<HealthSnapshot | null>(null);
  let history = $state<number[]>([]);
  let interval = $state(2000);
  let sortBy = $state<"cpu" | "memory">("cpu");

  let mem = $derived(snap?.memory ?? null);
  let hot = $derived((snap?.cpuTempC ?? 0) >= 95);
  let overall = $derived.by((): { tone: "ok" | "warn" | "bad"; text: string } => {
    if (!snap) return { tone: "ok", text: "Reading…" };
    if (mem?.pressureLevel === "critical") return { tone: "bad", text: "Memory pressure is critical" };
    if (mem?.pressureLevel === "warning") return { tone: "warn", text: "Memory pressure is high" };
    if (hot) return { tone: "warn", text: "CPU is running hot" };
    return { tone: "ok", text: "Everything looks normal" };
  });
  let procs = $derived(snap ? (sortBy === "cpu" ? snap.topCpu : snap.topMemory) : []);
  let maxProcCpu = $derived(Math.max(100, ...procs.map((p) => p.cpu)));

  const MEMORY_PARTS = [
    { key: "app", label: "App", color: "var(--accent)" },
    { key: "wired", label: "Wired", color: "var(--accent-mid)" },
    { key: "compressed", label: "Compressed", color: "var(--accent-light)" },
    { key: "cached", label: "Cached files", color: "var(--line-strong)" },
    { key: "free", label: "Free", color: "var(--track)" },
  ] as const;

  function applyInterval(ms: number) {
    const visible = document.visibilityState === "visible";
    api.setHealthInterval(visible ? ms : 0);
  }

  $effect(() => applyInterval(interval));

  onMount(() => {
    const off = events.healthTick((s) => {
      snap = s;
      history = [...history, s.cpu.total].slice(-HISTORY);
    });
    const onVisibility = () => applyInterval(interval);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      off.then((f) => f());
      document.removeEventListener("visibilitychange", onVisibility);
      api.setHealthInterval(0);
    };
  });
</script>

<div class="screen">
  <header>
    <div>
      <h1>Health</h1>
      <p class="muted">
        {#if snap}
          {snap.cpu.brand} · {snap.cpu.perCore.length} cores{mem ? ` · ${bytes(mem.total)}` : ""} · up {uptime(snap.cpu.uptimeSecs)}
        {:else}
          Reading sensors…
        {/if}
      </p>
    </div>
    <div class="actions">
      <StatusChip tone={overall.tone} text={overall.text} dot />
      <label class="select">
        <span class="muted">Refresh</span>
        <select bind:value={interval}>
          <option value={2000}>Every 2 s</option>
          <option value={5000}>Every 5 s</option>
          <option value={0}>Paused</option>
        </select>
      </label>
    </div>
  </header>

  <div class="cards">
    <section class="card" aria-labelledby="cpu-h">
      <div class="chead">
        <h2 id="cpu-h">CPU</h2>
        {#if snap}<StatusChip tone={hot ? "warn" : "ok"} text={hot ? "Hot" : "Normal"} />{/if}
      </div>
      <div class="bignum">
        <span class="num">{snap ? Math.round(snap.cpu.total) : "–"}%</span>
        <span class="muted small">average across cores</span>
      </div>
      <Sparkline values={history} label="CPU load over the last {HISTORY} readings" />
      {#if snap}
        <div class="cores">
          <div class="small muted split">
            <span>Per core</span>
            {#if snap.cpu.performanceCores}
              <span>{snap.cpu.performanceCores} performance · {snap.cpu.efficiencyCores} efficiency</span>
            {/if}
          </div>
          <div class="corebars" aria-hidden="true">
            {#each snap.cpu.perCore as load, i (i)}
              <span class="core"><span style:height="{Math.max(2, load)}%"></span></span>
            {/each}
          </div>
        </div>
        <dl class="facts">
          <div><dt>Temperature</dt><dd>{snap.cpuTempC != null ? `${Math.round(snap.cpuTempC)} °C` : "Unavailable"}</dd></div>
          <div><dt>Load average</dt><dd class="num">{snap.cpu.loadAverage[0].toFixed(2)}</dd></div>
          <div><dt>5 / 15 min</dt><dd class="num">{snap.cpu.loadAverage[1].toFixed(2)} / {snap.cpu.loadAverage[2].toFixed(2)}</dd></div>
        </dl>
      {/if}
    </section>

    <section class="card" aria-labelledby="mem-h">
      <div class="chead">
        <h2 id="mem-h">Memory</h2>
        {#if mem}
          <StatusChip
            tone={mem.pressureLevel === "normal" ? "ok" : mem.pressureLevel === "warning" ? "warn" : "bad"}
            text="Pressure {mem.pressureLevel}"
          />
        {/if}
      </div>
      {#if mem}
        <div class="bignum">
          <span class="num">{bytes(mem.used)}</span>
          <span class="muted small">used of {bytes(mem.total)}</span>
        </div>
        <div class="col">
          <div class="small muted split"><span>Memory pressure</span><span class="num">{mem.pressurePct}%</span></div>
          <SizeBar
            value={mem.pressurePct}
            height={8}
            color={mem.pressureLevel === "normal" ? "var(--ok-dot)" : mem.pressureLevel === "warning" ? "#e8a33d" : "var(--bad-text)"}
          />
        </div>
        <div class="memstack" aria-hidden="true">
          {#each MEMORY_PARTS as part (part.key)}
            <span style:flex-grow={mem[part.key]} style:background={part.color}></span>
          {/each}
        </div>
        <dl class="legend">
          {#each MEMORY_PARTS as part (part.key)}
            <div>
              <i style:background={part.color}></i>
              <dt>{part.label}</dt>
              <dd class="num">{bytes(mem[part.key])}</dd>
            </div>
          {/each}
          <div><i></i><dt>Swap used</dt><dd class="num">{bytes(mem.swapUsed)}</dd></div>
        </dl>
      {:else if snap}
        <p class="muted">Memory details are unavailable.</p>
      {/if}
    </section>

    <section class="card" aria-labelledby="fan-h">
      <div class="chead">
        <h2 id="fan-h">Fans</h2>
        {#if snap?.fans?.length}
          {@const busiest = Math.max(...snap.fans.map((f) => pct(f.rpm - f.min, f.max - f.min)))}
          <StatusChip tone={busiest > 70 ? "warn" : "ok"} text={busiest > 70 ? "Working hard" : busiest > 20 ? "Active" : "Quiet"} />
        {/if}
      </div>
      {#if snap?.fans === null}
        <p class="muted">Fan speeds can't be read on this Mac.</p>
      {:else if snap?.fans?.length === 0}
        <p class="muted">No fans: this Mac is cooled without them.</p>
      {:else if snap?.fans}
        <div class="fans">
          {#each snap.fans as fan (fan.label)}
            <div class="col">
              <div class="split">
                <span class="muted">{fan.label}</span>
                <span><strong class="rpm num">{rpm(fan.rpm)}</strong> <span class="small muted">rpm</span></span>
              </div>
              <SizeBar value={pct(fan.rpm - fan.min, fan.max - fan.min)} height={8} />
              <div class="small muted split"><span>min {rpm(fan.min)}</span><span>max {rpm(fan.max)}</span></div>
            </div>
          {/each}
        </div>
      {/if}
      <p class="readonly small muted">
        <Icon name="info" size={16} />
        <span>Read-only. Sweep never changes fan speeds.</span>
      </p>
    </section>
  </div>

  <section class="card procs" aria-labelledby="proc-h">
    <div class="chead">
      <h2 id="proc-h">Top processes</h2>
      <div role="group" aria-label="Sort by" class="seg">
        <button type="button" aria-pressed={sortBy === "cpu"} onclick={() => (sortBy = "cpu")}>CPU</button>
        <button type="button" aria-pressed={sortBy === "memory"} onclick={() => (sortBy = "memory")}>Memory</button>
      </div>
    </div>
    <table>
      <thead>
        <tr>
          <th scope="col">Process</th>
          <th scope="col" class="cpucol">CPU</th>
          <th scope="col" class="r">Memory</th>
          <th scope="col" class="r">PID</th>
        </tr>
      </thead>
      <tbody>
        {#each procs as p (p.pid)}
          <tr>
            <td class="pname">{p.name}</td>
            <td>
              <span class="share">
                <SizeBar value={pct(p.cpu, maxProcCpu)} />
                <span class="num pc">{p.cpu.toFixed(1)}%</span>
              </span>
            </td>
            <td class="r num">{bytes(p.memory)}</td>
            <td class="r muted num">{p.pid}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

<style>
  .screen {
    padding: 28px 32px 32px;
    display: flex;
    flex-direction: column;
    gap: 18px;
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
    font-size: 15px;
    font-weight: 600;
  }
  p {
    margin: 0;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
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
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 16px;
  }
  .card {
    border: 1px solid var(--line);
    border-radius: 14px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .chead {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .bignum {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .bignum > span:first-child {
    font-size: 34px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .split {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .cores {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .corebars {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 4px;
    height: 44px;
  }
  .core {
    background: var(--track);
    border-radius: 3px;
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }
  .core > span {
    width: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: height 0.3s ease;
  }
  .facts {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .facts dt {
    font-size: 12px;
    color: var(--text-2);
  }
  .facts dd {
    margin: 2px 0 0;
    font-size: 15px;
    font-weight: 600;
  }
  .memstack {
    display: flex;
    gap: 2px;
    height: 14px;
    border-radius: 4px;
    overflow: hidden;
  }
  .memstack > span {
    flex-basis: 0;
  }
  .legend {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px 16px;
    font-size: 13px;
  }
  .legend > div {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    flex: 0 0 10px;
  }
  .legend dt {
    color: var(--text-2);
    flex-grow: 1;
  }
  .legend dd {
    margin: 0;
    font-weight: 600;
  }
  .fans {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .rpm {
    font-size: 22px;
  }
  .readonly {
    display: flex;
    gap: 8px;
    align-items: center;
    background: var(--bg-sunken);
    border-radius: 8px;
    padding: 8px 12px;
    margin-top: auto;
  }
  .procs {
    padding: 14px 0 6px;
  }
  .procs .chead {
    padding: 0 18px;
  }
  .seg {
    display: flex;
    background: var(--sidebar);
    border-radius: 8px;
    padding: 2px;
  }
  .seg button {
    min-height: 26px;
    padding: 0 14px;
    border-radius: 6px;
    border: 0;
    background: transparent;
    font-size: 13px;
  }
  .seg button[aria-pressed="true"] {
    background: var(--bg);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    text-align: left;
    font-weight: 500;
    font-size: 12px;
    color: var(--text-2);
    padding: 6px 18px;
    background: var(--bg-header);
  }
  td {
    padding: 8px 18px;
    border-top: 1px solid var(--line);
  }
  .pname {
    font-weight: 500;
    max-width: 320px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cpucol {
    width: 240px;
  }
  .share {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pc {
    width: 52px;
    text-align: right;
  }
  .r {
    text-align: right;
  }
</style>
