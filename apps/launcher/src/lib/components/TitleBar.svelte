<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import X from "@lucide/svelte/icons/x";
  import User from "@lucide/svelte/icons/user";
  import { t } from "../i18n";
  import { app } from "../store.svelte";

  import { inTauri } from "../api";

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
    <button class="account" onclick={() => app.go({ page: "settings", section: "account" })}>
      <span class="avatar"><User size={14} /></span>
      <span>{t("titlebar.guest")}</span>
    </button>
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
