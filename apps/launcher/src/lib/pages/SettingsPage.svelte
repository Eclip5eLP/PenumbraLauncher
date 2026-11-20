<script lang="ts">
  import { app } from "../store.svelte";
  import { t, tDyn, type Key } from "../i18n";
  import type { Settings } from "../types";

  let { section = "general" }: { section?: string } = $props();
  let current = $state("general");
  $effect(() => {
    current = section;
  });

  type Field =
    | { kind: "toggle"; key: keyof Settings; hint?: Key }
    | { kind: "select"; key: keyof Settings; options: string[]; hint?: Key }
    | { kind: "number"; key: keyof Settings; min: number; max: number; step?: number; hint?: Key }
    | { kind: "text"; key: keyof Settings; hint?: Key; disabled?: boolean };

  const sections: { id: string; fields: Field[]; devOnly?: boolean }[] = [
    {
      id: "general",
      fields: [
        { kind: "select", key: "language", options: ["en"] },
        { kind: "select", key: "closeButton", options: ["tray", "exit"] },
        { kind: "select", key: "onGameLaunch", options: ["minimize", "tray", "stay", "close"] },
        { kind: "toggle", key: "startWithWindows" },
        { kind: "toggle", key: "startMinimized" },
        { kind: "toggle", key: "notifications" },
        { kind: "select", key: "launcherChannel", options: ["stable", "beta"] },
        { kind: "toggle", key: "autoUpdateLauncher" },
      ],
    },
    {
      id: "downloads",
      fields: [
        { kind: "toggle", key: "autoUpdateGames" },
        { kind: "number", key: "maxConcurrentJobs", min: 1, max: 3 },
        { kind: "number", key: "connectionsPerJob", min: 1, max: 32 },
        { kind: "number", key: "bandwidthLimit", min: 0, max: 1_000_000, step: 100 },
        { kind: "toggle", key: "pauseWhileRunning" },
        { kind: "toggle", key: "resumeOnStart" },
        { kind: "toggle", key: "verifyAfterInstall" },
        { kind: "select", key: "createDesktopShortcut", options: ["ask", "always", "never"] },
      ],
    },
    {
      id: "appearance",
      fields: [
        { kind: "toggle", key: "useGameAccent" },
        { kind: "toggle", key: "backgroundVideo" },
        { kind: "select", key: "reduceMotion", options: ["system", "on", "off"] },
        { kind: "number", key: "uiScale", min: 0.8, max: 1.25, step: 0.05 },
      ],
    },
    { id: "account", fields: [] },
    {
      id: "advanced",
      fields: [
        { kind: "select", key: "logLevel", options: ["error", "warn", "info", "debug", "trace"] },
        { kind: "toggle", key: "offlineMode" },
        { kind: "text", key: "apiBase", hint: "settings.hintRestart" },
        { kind: "toggle", key: "devMode" },
      ],
    },
    { id: "about", fields: [] },
  ];

  const label = (key: string) => tDyn(`settings.${key}`, key);
  const optionLabel = (key: string, opt: string) =>
    tDyn(`settings.${key}Opts.${opt}`, opt === "en" ? "English" : opt);
  const active = $derived(sections.find((s) => s.id === current) ?? sections[0]);

  // Text/number inputs save on change (blur/enter), toggles/selects immediately.
  function set<K extends keyof Settings>(key: K, value: Settings[K]) {
    void app.setSetting(key, value);
  }
</script>

<div class="settings">
  <aside>
    <h1>{t("settings.title")}</h1>
    {#each sections as s (s.id)}
      <button class:on={s.id === current} onclick={() => (current = s.id)}>
        {tDyn(`settings.sections.${s.id}`)}
      </button>
    {/each}
  </aside>

  <div class="content">
    <h2>{tDyn(`settings.sections.${active.id}`)}</h2>

    {#if active.id === "account"}
      <p class="note">{t("settings.accountSoon")}</p>
      <div class="field">
        <span>{t("settings.syncSettings")}</span>
        <input
          type="checkbox"
          class="switch"
          checked={app.settings.syncSettings}
          onchange={(e) => set("syncSettings", e.currentTarget.checked)}
        />
      </div>
      <div class="field">
        <span>{t("settings.cloudSaves")}</span>
        <input
          type="checkbox"
          class="switch"
          checked={app.settings.cloudSaves}
          onchange={(e) => set("cloudSaves", e.currentTarget.checked)}
        />
      </div>
    {:else if active.id === "about"}
      <div class="about">
        <div class="kv">
          <span>{t("app.name")}</span><span>{t("settings.version", { v: app.appVersion })}</span>
        </div>
        <div class="kv"><span>Startup</span><span>{t("settings.startup", { ms: app.startupMs })}</span></div>
        {#if app.mock}<p class="note">{t("settings.mock")}</p>{/if}
      </div>
    {/if}

    {#each active.fields as f (f.key)}
      <label class="field">
        <span>
          {label(f.key)}
          {#if f.hint}<small>{t(f.hint)}</small>{/if}
        </span>
        {#if f.kind === "toggle"}
          <input
            type="checkbox"
            class="switch"
            checked={app.settings[f.key] as boolean}
            onchange={(e) => set(f.key, e.currentTarget.checked as never)}
          />
        {:else if f.kind === "select"}
          <select value={app.settings[f.key]} onchange={(e) => set(f.key, e.currentTarget.value as never)}>
            {#each f.options as o (o)}<option value={o}>{optionLabel(f.key, o)}</option>{/each}
          </select>
        {:else if f.kind === "number"}
          <input
            type="number"
            min={f.min}
            max={f.max}
            step={f.step ?? 1}
            value={app.settings[f.key]}
            onchange={(e) => {
              const v = Number(e.currentTarget.value);
              if (Number.isFinite(v)) set(f.key, Math.min(f.max, Math.max(f.min, v)) as never);
            }}
          />
        {:else}
          <input
            type="text"
            class="wide"
            value={app.settings[f.key] as string}
            disabled={f.disabled}
            onchange={(e) => set(f.key, e.currentTarget.value as never)}
          />
        {/if}
      </label>
    {/each}
  </div>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100%;
    padding-top: var(--title-h);
  }
  aside {
    padding: 16px 12px;
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  aside h1 {
    font-size: 1.4em;
    font-weight: 650;
    padding: 8px 12px 14px;
  }
  aside button {
    text-align: left;
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    color: var(--muted);
    font-weight: 550;
    transition:
      background var(--fast),
      color var(--fast);
  }
  aside button:hover {
    color: var(--text);
    background: rgb(255 255 255 / 0.04);
  }
  aside button.on {
    color: var(--text);
    background: var(--panel-2);
  }
  .content {
    overflow-y: auto;
    padding: 24px 40px 40px;
    max-width: 820px;
  }
  h2 {
    font-size: 1.2em;
    font-weight: 650;
    margin-bottom: 14px;
  }
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 54px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .field > span {
    display: flex;
    flex-direction: column;
  }
  small {
    color: var(--faint);
    font-size: 0.82em;
  }
  select,
  input[type="number"],
  input[type="text"] {
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    background: var(--panel);
    border: 1px solid var(--line-2);
    min-width: 200px;
    outline: none;
  }
  input[type="number"] {
    min-width: 120px;
    width: 120px;
  }
  input.wide {
    min-width: 320px;
  }
  select:focus,
  input:focus {
    border-color: var(--accent);
  }
  .switch {
    appearance: none;
    position: relative;
    width: 40px;
    height: 22px;
    border-radius: 11px;
    background: var(--panel-3);
    cursor: pointer;
    transition: background var(--fast);
    flex: none;
  }
  .switch::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform var(--fast) var(--ease);
  }
  .switch:checked {
    background: var(--accent);
  }
  .switch:checked::after {
    transform: translateX(18px);
  }
  .note {
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--text-2);
    margin-bottom: 10px;
  }
  .about {
    display: grid;
    gap: 8px;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .kv span:last-child {
    color: var(--muted);
  }
</style>
