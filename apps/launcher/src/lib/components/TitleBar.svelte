<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import X from "@lucide/svelte/icons/x";
  import User from "@lucide/svelte/icons/user";
  import { t } from "../i18n";
  import { app } from "../store.svelte";

  import LogOut from "@lucide/svelte/icons/log-out";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Menu, { type MenuEntry } from "./Menu.svelte";
  import { api, inTauri } from "../api";

  const accountMenu = $derived<(MenuEntry | "sep")[]>([
    { label: t("account.signedInAs", { name: app.auth.user?.username ?? "" }), disabled: true },
    "sep",
    { label: t("account.manage"), icon: ExternalLink, onclick: () => app.run(api.openAccountPage()) },
    { label: t("account.signOut"), icon: LogOut, danger: true, onclick: () => app.run(api.authSignOut()) },
  ]);

  let { overlay = false }: { overlay?: boolean } = $props();
  // Browser dev preview has no native window: buttons become no-ops.
  const win = inTauri
    ? getCurrentWindow()
    : { minimize: async () => {}, toggleMaximize: async () => {}, close: async () => {} };
</script>

<header class:overlay data-tauri-drag-region>
  <div class="left" data-tauri-drag-region>
    {#if app.mock}<span class="mock" title={t("settings.mock")}>MOCK</span>{/if}
  </div>
  <div class="right">
    {#if app.updater.state === "ready" || app.updater.state === "available" || app.updater.state === "installing"}
      <button
        class="pill"
        disabled={app.updater.state === "installing"}
        onclick={() => app.run(api.launcherUpdateInstall())}
        title={app.updater.version ?? ""}
      >
        <RefreshCw size={13} class={app.updater.state === "installing" ? "spin" : ""} />
        {app.updater.state === "ready"
          ? t("updater.restart")
          : app.updater.state === "installing"
            ? t("updater.installing")
            : t("updater.available", { v: app.updater.version ?? "" })}
      </button>
    {:else if app.updater.state === "downloading"}
      <span class="pill quiet">{t("updater.downloading", { p: app.updater.progress ?? 0 })}</span>
    {/if}
    {#if app.auth.signedIn}
      <Menu entries={accountMenu} placement="bottom-end">
        {#snippet trigger(toggle, open)}
          <button class="account" class:open onclick={toggle}>
            <span class="avatar initial">{(app.auth.user?.username ?? "?")[0].toUpperCase()}</span>
            <span>{app.auth.user?.username}</span>
          </button>
        {/snippet}
      </Menu>
    {:else}
      <button class="account" onclick={() => (app.dialog = { kind: "signin" })}>
        <span class="avatar"><User size={14} /></span>
        <span>{t("titlebar.signIn")}</span>
      </button>
    {/if}
    <div class="sep"></div>
    <button class="win" aria-label={t("titlebar.minimize")} onclick={() => win.minimize()}>
      <Minus size={16} />
    </button>
    <button class="win" aria-label={t("titlebar.maximize")} onclick={() => win.toggleMaximize()}>
      <Square size={13} />
    </button>
    <button class="win close" aria-label={t("titlebar.close")} onclick={() => win.close()}>
      <X size={17} />
    </button>
  </div>
</header>

<style>
  header {
    position: absolute;
    inset: 0 0 auto 0;
    height: var(--title-h);
    display: flex;
    align-items: center;
    justify-content: space-between;
    z-index: 20;
    background: var(--bg-2);
  }
  header.overlay {
    background: linear-gradient(rgb(0 0 0 / 0.45), transparent);
  }
  .left {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: center;
    padding-left: 16px;
  }
  .mock {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgb(194 138 46 / 0.25);
    color: #f0c27a;
  }
  .right {
    display: flex;
    align-items: center;
    height: 100%;
  }
  .account {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 12px 0 4px;
    margin-right: 6px;
    border-radius: 14px;
    color: var(--text-2);
    font-size: 0.9em;
    transition: background var(--fast);
  }
  .avatar.initial {
    font-size: 12px;
    font-weight: 700;
  }
  .account.open,
  .account:hover {
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }
  .avatar {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--brand-1), var(--brand-2));
    color: #fff;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 12px;
    margin-right: 8px;
    border-radius: 13px;
    background: var(--ok);
    color: #fff;
    font-size: 0.85em;
    font-weight: 600;
  }
  .pill:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .pill.quiet {
    background: rgb(255 255 255 / 0.1);
    color: var(--text-2);
  }
  .sep {
    width: 1px;
    height: 16px;
    background: rgb(255 255 255 / 0.15);
    margin: 0 4px;
  }
  .win {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--text-2);
    transition: background var(--fast);
  }
  .win:hover {
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }
  .win.close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
