<script lang="ts">
  import Play from "@lucide/svelte/icons/play";
  import Pause from "@lucide/svelte/icons/pause";
  import Download from "@lucide/svelte/icons/download";
  import ArrowUpToLine from "@lucide/svelte/icons/arrow-up-to-line";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Hourglass from "@lucide/svelte/icons/hourglass";
  import Lock from "@lucide/svelte/icons/lock";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Globe from "@lucide/svelte/icons/globe";
  import X from "@lucide/svelte/icons/x";
  import Menu, { type MenuEntry } from "./Menu.svelte";
  import { t, tDyn } from "../i18n";
  import { app, type Action } from "../store.svelte";
  import { api } from "../api";
  import { bytes, duration, percent, speed } from "../format";
  import type { Item } from "../types";

  let { item }: { item: Item } = $props();

  const action = $derived(app.action(item));
  const job = $derived(app.activeJob(item.id));
  const installed = $derived(app.library[item.id]);
  const channel = $derived(app.channelOf(item));
  const ch = $derived(item.channels[channel]);
  const channelNames = $derived(Object.keys(item.channels));
  const pct = $derived(job ? percent(job.downloaded, job.total) : 0);

  const icons: Partial<Record<Action, typeof Play>> = {
    open: ExternalLink,
    comingSoon: Hourglass,
    locked: Lock,
    install: Download,
    update: ArrowUpToLine,
    play: Play,
    queued: Pause,
    downloading: Pause,
    verifying: Pause,
    paused: Play,
  };
  const spinning: Action[] = ["preparing", "finalizing"];
  const disabled: Action[] = ["comingSoon", "unavailable", "running", "preparing", "finalizing", "locked"];

  const label = $derived.by(() => {
    switch (action) {
      case "play":
        return t(item.type === "software" ? "action.launch" : "action.play");
      case "downloading":
      case "verifying":
      case "queued":
        return t("action.pause");
      case "paused":
        return t("action.resume");
      default:
        return tDyn(`action.${action}`);
    }
  });

  const statusLine = $derived.by(() => {
    if (job) {
      const parts = [`${Math.floor(pct)}%`];
      if (job.state === "downloading") {
        if (job.speed > 0) parts.push(speed(job.speed));
        else if (app.settings.pauseWhileRunning && app.running.length)
          parts.push(t("downloads.pausedWhileRunning"));
        if (job.eta != null) parts.push(t("downloads.eta", { t: duration(job.eta) }));
      } else parts.push(tDyn(`action.${job.state}`));
      return parts.join(" · ");
    }
    return "";
  });

  function channelLabel(name: string) {
    const c = item.channels[name];
    const base = tDyn(`channels.${name}`, name);
    return c?.mode ? `${base} · ${tDyn(`item.modes.${c.mode}`)}` : base;
  }

  const menu = $derived.by<(MenuEntry | "sep")[]>(() => {
    const e: (MenuEntry | "sep")[] = [];
    if (installed) {
      e.push(
        {
          label: t("item.menu.openFolder"),
          icon: FolderOpen,
          onclick: () => app.run(api.openFolder(item.id)),
        },
        {
          label: t("item.menu.verify"),
          icon: ShieldCheck,
          disabled: !!job || app.running.includes(item.id),
          onclick: () => app.run(api.verify(item.id)),
        },
        { label: t("item.menu.move"), icon: HardDrive, disabled: true, hint: t("item.menu.moveLater") },
      );
    }
    const links = Object.keys(item.links);
    if (links.length) {
      if (e.length) e.push("sep");
      for (const k of links)
        e.push({
          label: tDyn(`item.menu.${k}`, k),
          icon: k === "website" ? Globe : ExternalLink,
          onclick: () => app.run(api.openItemLink(item.id, k)),
        });
    }
    if (installed) {
      e.push("sep", {
        label: t("item.menu.uninstall"),
        icon: Trash2,
        danger: true,
        disabled: !!job || app.running.includes(item.id),
        onclick: () => (app.dialog = { kind: "uninstall", item: item.id }),
      });
    }
    return e;
  });

  const channelMenu = $derived<MenuEntry[]>(
    channelNames.map((n) => ({
      label: channelLabel(n),
      hint: item.channels[n].version ?? "–",
      checked: n === channel,
      onclick: () => (app.channelPref[item.id] = n),
    })),
  );

  const info = $derived.by(() => {
    if (job) return job.path;
    if (item.type === "link") return item.url?.replace(/^https:\/\//, "") ?? "";
    if (item.status === "coming-soon") return "";
    if (installed && action === "update") return t("item.updateTo", { v: ch?.version ?? "" });
    if (installed) return t("item.installed", { v: installed.version });
    if (ch?.size) return `${t("item.notInstalled")} · ${bytes(ch.size)}`;
    return t("item.notInstalled");
  });
</script>

<div class="panel">
  {#if job}
    <div class="progress glass">
      <div class="row">
        <span class="state">{tDyn(`downloads.kinds.${job.kind}`)} · {job.version}</span>
        <span class="nums">{bytes(job.downloaded)} / {bytes(job.total)}</span>
        <button
          class="icon-btn"
          aria-label={t("action.cancel")}
          onclick={() => app.run(api.cancelJob(job.id))}
        >
          <X size={15} />
        </button>
      </div>
      <div class="bar" class:paused={job.state === "paused"}><span style:width="{pct}%"></span></div>
      <div class="status">{statusLine}</div>
    </div>
  {/if}

  <div class="meta">
    {#if channelNames.length > 1}
      <Menu entries={channelMenu} placement="top-end">
        {#snippet trigger(toggle, open)}
          <button class="channel" class:open onclick={toggle}>
            {channelLabel(channel)}
            <ChevronDown size={14} />
          </button>
        {/snippet}
      </Menu>
    {/if}
    <span class="info">{info}</span>
  </div>

  <div class="actions">
    <button
      class="primary action-{action}"
      disabled={disabled.includes(action)}
      onclick={() => app.primary(item)}
    >
      {#if spinning.includes(action)}
        <LoaderCircle size={22} class="spin" />
      {:else if icons[action]}
        {@const Icon = icons[action]}
        <Icon size={22} fill={action === "play" || action === "paused" ? "currentColor" : "none"} />
      {/if}
      <span>{label}</span>
    </button>
    {#if menu.length}
      <Menu entries={menu}>
        {#snippet trigger(toggle, open)}
          <button class="more" class:open aria-label="more" onclick={toggle}><Ellipsis size={20} /></button>
        {/snippet}
      </Menu>
    {/if}
  </div>
  {#if installed && installed.playtimeSec > 0}
    <div class="foot">{t("item.playtime", { t: duration(installed.playtimeSec) })}</div>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
    width: 330px;
    animation: rise-in var(--med) var(--ease);
  }
  .progress {
    align-self: stretch;
    padding: 12px 14px;
    border-radius: var(--radius);
    display: grid;
    gap: 8px;
  }
  .progress .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: -4px -6px -4px 0;
  }
  .state {
    flex: 1;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nums,
  .status {
    color: var(--text-2);
    font-size: 0.88em;
    font-variant-numeric: tabular-nums;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 28px;
  }
  .info {
    max-width: 330px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
    font-size: 0.9em;
    text-shadow: 0 1px 4px rgb(0 0 0 / 0.6);
  }
  .channel {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border-radius: 14px;
    background: rgb(0 0 0 / 0.4);
    border: 1px solid var(--glass-line);
    font-size: 0.88em;
    font-weight: 550;
    transition: background var(--fast);
  }
  .channel:hover,
  .channel.open {
    background: rgb(0 0 0 / 0.6);
  }
  .actions {
    display: flex;
    gap: 10px;
    align-self: stretch;
  }
  .primary {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 64px;
    padding: 0 28px;
    border-radius: 14px;
    font-size: 1.3em;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--accent-ink);
    background: linear-gradient(180deg, var(--accent-2), var(--accent));
    box-shadow:
      0 8px 24px color-mix(in oklab, var(--accent) 45%, transparent),
      inset 0 1px 0 rgb(255 255 255 / 0.25);
    transition:
      transform var(--fast) var(--ease),
      filter var(--fast),
      box-shadow var(--med);
  }
  .primary:hover:not(:disabled) {
    filter: brightness(1.08);
    transform: translateY(-1px);
  }
  .primary:active:not(:disabled) {
    transform: scale(0.98);
  }
  .primary:disabled {
    background: rgb(40 42 48 / 0.85);
    color: var(--muted);
    box-shadow: none;
  }
  .primary.action-running:disabled {
    background: color-mix(in oklab, var(--ok) 55%, #1d1e22);
    color: #dff5e7;
  }
  .primary.action-downloading,
  .primary.action-queued,
  .primary.action-paused {
    background: rgb(30 31 36 / 0.85);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--glass-line);
  }
  .more {
    width: 64px;
    height: 64px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: rgb(20 20 24 / 0.65);
    border: 1px solid var(--glass-line);
    color: var(--text-2);
    backdrop-filter: blur(14px);
    transition: background var(--fast);
  }
  .more:hover,
  .more.open {
    background: rgb(40 40 46 / 0.8);
    color: var(--text);
  }
  .foot {
    color: var(--muted);
    font-size: 0.85em;
    text-shadow: 0 1px 4px rgb(0 0 0 / 0.6);
  }
</style>
