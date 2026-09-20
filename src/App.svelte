<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  import * as api from "./lib/api";
  import { chatOpenKey, withChatOpenKey } from "./lib/types";
  import type { Config, Status, WindowRef } from "./lib/types";

  // Only used to pick the platform's base metrics; everything else comes from
  // the system colours.
  const platform = navigator.userAgent.includes("Mac") ? "mac" : "win";

  let config = $state<Config | null>(null);
  let status = $state<Status | null>(null);
  let windows = $state<WindowRef[]>([]);
  /// True while the chat-key button is waiting for the user to press a key.
  let capturing = $state(false);
  /// Set when the captured key is one Zaapy cannot play, so the button can say so.
  let captureRefused = $state(false);

  /// One entry per application, not per window: the source is an app.
  const sourceOptions = $derived([
    ...new Map(windows.map((window) => [window.process, window])).values(),
  ]);

  const selectedSource = $derived(config?.source_processes[0] ?? "");
  const sourceIsRunning = $derived(
    selectedSource === "" || sourceOptions.some((w) => w.process === selectedSource),
  );

  const chatKeyLabel = $derived(keyLabel(config ? chatOpenKey(config) : null));

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

  /// The core's key names, as the user reads them: `enter` → `Enter`, `v` → `V`.
  function keyLabel(key: string | null): string {
    if (!key) return "—";
    return key.length === 1 ? key.toUpperCase() : key[0].toUpperCase() + key.slice(1);
  }

  /// A browser key event as the core spells it, or `null` for a key Zaapy has no
  /// code for on both platforms.
  function coreKey(event: KeyboardEvent): string | null {
    // A chord would need modifiers in the step; the chat bind is a single key.
    if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return null;
    switch (event.key) {
      case "Enter":
        return "enter";
      case "Tab":
        return "tab";
      case " ":
        return "space";
      case "Backspace":
        return "backspace";
      case "Delete":
        return "delete";
      default:
        return /^[a-z0-9]$/i.test(event.key) ? event.key.toLowerCase() : null;
    }
  }

  function capture(event: KeyboardEvent) {
    if (!capturing || !config) return;
    // Let go of a held modifier without ending the capture: the user is still
    // reaching for the key.
    if (["Shift", "Control", "Alt", "Meta"].includes(event.key)) return;

    // Tab would move the focus and Space would press the button again, so every
    // key the capture sees is ours, refused ones included.
    event.preventDefault();
    // Escape is the way out rather than a binding: it opens the game menu in
    // Dofus, so it is never the chat key, and a capture with no exit is a trap.
    if (event.key === "Escape") {
      stopCapture();
      return;
    }

    const key = coreKey(event);
    if (!key) {
      captureRefused = true;
      return;
    }
    stopCapture();
    void save(withChatOpenKey(config, key));
  }

  function stopCapture() {
    capturing = false;
    captureRefused = false;
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

<svelte:window onkeydown={capture} />

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

    <!-- The key Dofus opens its chat with. Rebindable in game, so it has to be
         rebindable here; the rest of the send sequence stays in the file.
         The label alone cannot say which of the two Enters this is, nor that it
         has to match a keybind the user set elsewhere, so the explanation goes
         in a native tooltip — the same affordance the /zaap row already uses. -->
    <div class="row">
      <span
        class="label"
        title="The key that opens the Dofus chat. Zaapy presses it before pasting the command, so it has to be the one bound in the game's controls — Enter unless you moved it."
      >
        Dofus chat key <span class="info" aria-hidden="true">i</span>
      </span>
      <button onclick={() => (capturing = true)} onblur={stopCapture}>
        {capturing ? "Press a key…" : chatKeyLabel}
      </button>
      <!-- One hint at a time: a refusal answers the press the user just made,
           which matters more than the way out they have not asked for yet.
           Both are short enough to sit beside the button on one line, at either
           platform's base size — the row is 130px of label plus a button wide
           inside a 420px window, and what is left is not much. -->
      {#if captureRefused}
        <span class="hint" title="Zaapy can send Enter, Escape, Tab, Space, Backspace, Delete, and any letter or digit. It has no key code for the rest on both Windows and macOS.">
          Unsupported key
        </span>
      {:else if capturing}
        <span class="hint">Escape to cancel</span>
      {/if}
    </div>

    <hr />

    <div class="row end">
      <button onclick={() => api.openLogFolder()}>Open log folder</button>
    </div>
  {/if}
</main>
