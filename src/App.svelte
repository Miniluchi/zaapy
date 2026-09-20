<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  import * as api from "./lib/api";
  import type { Config, Status, WindowRef } from "./lib/types";

  // Only used to pick the platform's base metrics; everything else comes from
  // the system colours.
  const platform = navigator.userAgent.includes("Mac") ? "mac" : "win";

  let config = $state<Config | null>(null);
  let status = $state<Status | null>(null);
  let windows = $state<WindowRef[]>([]);

  /// One entry per application, not per window: the source is an app.
  const sourceOptions = $derived([
    ...new Map(windows.map((window) => [window.process, window])).values(),
  ]);

  const selectedSource = $derived(config?.source_processes[0] ?? "");
  const sourceIsRunning = $derived(
    selectedSource === "" || sourceOptions.some((w) => w.process === selectedSource),
  );

  const targetLabel = $derived(
    config?.target.title_pattern
      ? `${config.target.process} — ${config.target.title_pattern}`
      : (config?.target.process ?? ""),
  );

  onMount(() => {
    void load();
    // Windows open and close without telling us; the panel stays truthful by
    // re-reading the OS rather than caching a snapshot.
    const timer = setInterval(() => void refresh(), 2000);
    // The tray menu writes the same configuration this panel does. Without this
    // the bridge switch here would keep showing whatever it was told on mount.
    const unlisten = listen<Config>("config-changed", (event) => {
      config = event.payload;
      void refresh();
    });
    return () => {
      clearInterval(timer);
      void unlisten.then((stop) => stop());
    };
  });

  async function load() {
    config = await api.getConfig();
    await refresh();
  }

  async function refresh() {
    [status, windows] = await Promise.all([api.getStatus(), api.listWindows()]);
  }

  async function save(next: Config) {
    config = await api.setConfig(next);
    await refresh();
  }

  function pickSource(process: string) {
    if (!config) return;
    void save({ ...config, source_processes: process ? [process] : [] });
  }

  function pickTarget(handle: string) {
    if (!config) return;
    const window = windows.find((candidate) => String(candidate.handle) === handle);
    if (!window) return;
    // Dofus titles read "Character - Dofus 3.x". The character name is what
    // identifies one client among several and survives a patch.
    const [characterName] = window.title.split(" - ");
    void save({
      ...config,
      target: { process: window.process, title_pattern: characterName ?? "" },
    });
  }
</script>

<main data-platform={platform}>
  {#if config && status}
    <label class="check">
      <input
        type="checkbox"
        checked={config.enabled}
        onchange={(event) => save({ ...config!, enabled: event.currentTarget.checked })}
      />
      Bridge enabled
    </label>

    {#if !status.configured}
      <p class="note">Select the Ganymède and Dofus windows below.</p>
    {:else if status.target}
      <p class="note ok">Sending to {status.target.title}</p>
    {:else}
      <p class="note warn">{status.targetError}</p>
    {/if}

    {#if status.host.privilegeWarning}
      <p class="note warn">{status.host.privilegeWarning}</p>
    {/if}
    {#if status.host.canRequestPermission}
      <button onclick={() => api.requestPermissions()}>Grant permission…</button>
    {/if}

    <hr />

    <div class="row">
      <span class="label">Ganymède window</span>
      <select value={selectedSource} onchange={(e) => pickSource(e.currentTarget.value)}>
        <option value="">Not selected</option>
        {#if selectedSource && !sourceIsRunning}
          <option value={selectedSource}>{selectedSource} (not running)</option>
        {/if}
        {#each sourceOptions as window (window.process)}
          <option value={window.process}>{window.process}</option>
        {/each}
      </select>
    </div>

    <div class="row">
      <span class="label">Dofus window</span>
      <select
        value={status.target ? String(status.target.handle) : "current"}
        onchange={(e) => pickTarget(e.currentTarget.value)}
      >
        {#if !status.target}
          <option value="current">
            {targetLabel ? `${targetLabel} (not running)` : "Not selected"}
          </option>
        {/if}
        {#each windows as window (window.handle)}
          <option value={String(window.handle)}>{window.title}</option>
        {/each}
      </select>
    </div>

    <hr />

    <div class="row">
      <span class="label">Commands</span>
      <label class="check">
        <input
          type="checkbox"
          checked={config.commands.travel}
          onchange={(event) =>
            save({
              ...config!,
              commands: { ...config!.commands, travel: event.currentTarget.checked },
            })}
        />
        /travel
      </label>
      <!-- Left visible rather than hidden: the command is coming, and a missing
           row reads as a missing feature. -->
      <label class="check" title="Announced by Ankama, not live in game yet">
        <input type="checkbox" checked={config.commands.zaap} disabled />
        /zaap <span class="hint">soon</span>
      </label>
    </div>

    <hr />

    <div class="row end">
      <button onclick={() => api.openLogFolder()}>Open log folder</button>
    </div>
  {/if}
</main>
