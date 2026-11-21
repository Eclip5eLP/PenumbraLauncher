<script lang="ts">
  import { app } from "./lib/store.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import Rail from "./lib/components/Rail.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import InstallDialog from "./lib/components/InstallDialog.svelte";
  import UninstallDialog from "./lib/components/UninstallDialog.svelte";
  import SignInDialog from "./lib/components/SignInDialog.svelte";
  import SaveConflictDialog from "./lib/components/SaveConflictDialog.svelte";
  import ItemPage from "./lib/pages/ItemPage.svelte";
  import LibraryPage from "./lib/pages/LibraryPage.svelte";
  import DownloadsPage from "./lib/pages/DownloadsPage.svelte";
  import SettingsPage from "./lib/pages/SettingsPage.svelte";

  import QuickSwitcher from "./lib/components/QuickSwitcher.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Megaphone from "@lucide/svelte/icons/megaphone";
  import X from "@lucide/svelte/icons/x";
  import { api } from "./lib/api";

  const route = $derived(app.route);
  const announcement = $derived(app.announcements[0]);

  function onkey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      app.switcherOpen = !app.switcherOpen;
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="shell">
  <Rail />
  <main>
    {#if route.page === "item"}
      <ItemPage id={route.id} />
    {:else if route.page === "library"}
      <LibraryPage />
    {:else if route.page === "downloads"}
      <DownloadsPage />
    {:else if route.page === "settings"}
      <SettingsPage section={route.section} />
    {/if}
    <TitleBar overlay={route.page === "item"} />
    {#if announcement}
      <div class="announce glass">
        <Megaphone size={15} />
        {#if announcement.url}
          <button class="a-title" onclick={() => app.run(api.openAnnouncement(announcement.id))}>
            {announcement.title}
          </button>
        {:else}
          <span class="a-title">{announcement.title}</span>
        {/if}
        <button class="a-x" aria-label="dismiss" onclick={() => app.dismiss(announcement.id)}>
          <X size={14} />
        </button>
      </div>
    {/if}
  </main>
</div>

{#if app.dialog?.kind === "install"}
  <InstallDialog id={app.dialog.item} />
{:else if app.dialog?.kind === "uninstall"}
  <UninstallDialog id={app.dialog.item} />
{:else if app.dialog?.kind === "signin"}
  <SignInDialog />
{:else if app.dialog?.kind === "saveConflict"}
  <SaveConflictDialog item={app.dialog.item} cloud={app.dialog.cloud} />
{/if}
{#if app.switcherOpen}<QuickSwitcher />{/if}
<ContextMenu />
<Toasts />

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--rail-w) 1fr;
    height: 100%;
  }
  main {
    position: relative;
    min-width: 0;
    overflow: hidden;
    background: var(--bg-2);
  }
  .announce {
    position: absolute;
    top: calc(var(--title-h) + 6px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 25;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(640px, 80%);
    padding: 6px 8px 6px 14px;
    border-radius: 18px;
    font-size: 0.9em;
    animation: rise-in var(--med) var(--ease);
  }
  .a-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button.a-title:hover {
    text-decoration: underline;
  }
  .a-x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 11px;
    color: var(--muted);
  }
  .a-x:hover {
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }
  main > :global(.page) {
    padding-top: calc(var(--title-h) + 16px);
  }
</style>
