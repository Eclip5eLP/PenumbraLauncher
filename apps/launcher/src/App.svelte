<script lang="ts">
  import { app } from "./lib/store.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import Rail from "./lib/components/Rail.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import InstallDialog from "./lib/components/InstallDialog.svelte";
  import UninstallDialog from "./lib/components/UninstallDialog.svelte";
  import ItemPage from "./lib/pages/ItemPage.svelte";
  import LibraryPage from "./lib/pages/LibraryPage.svelte";
  import DownloadsPage from "./lib/pages/DownloadsPage.svelte";
  import SettingsPage from "./lib/pages/SettingsPage.svelte";

  const route = $derived(app.route);
</script>

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
  </main>
</div>

{#if app.dialog?.kind === "install"}
  <InstallDialog id={app.dialog.item} />
{:else if app.dialog?.kind === "uninstall"}
  <UninstallDialog id={app.dialog.item} />
{/if}
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
  main > :global(.page) {
    padding-top: calc(var(--title-h) + 16px);
  }
</style>
