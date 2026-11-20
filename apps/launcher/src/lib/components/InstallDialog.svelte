<script lang="ts">
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Modal from "./Modal.svelte";
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t } from "../i18n";
  import { bytes } from "../format";

  let { id }: { id: string } = $props();
  const item = $derived(app.item(id)!);
  const channel = $derived(app.channelOf(item));
  const ch = $derived(item.channels[channel]);

  // The user picks a library folder; the item gets its own subfolder in it.
  let library = $state(app.settings.defaultLibrary ?? app.suggestedLibrary);
  let free = $state<number | null>(null);
  let busy = $state(false);

  const folderName = (name: string) =>
    name
      .replace(/[<>:"/\\|?*]/g, "_")
      .trim()
      .replace(/\.+$/, "");
  const target = $derived(`${library.replace(/[\\/]+$/, "")}\\${folderName(item.name)}`);
  const needed = $derived((ch?.size ?? 0) + 64e6);
  const enough = $derived(free == null || free >= needed);

  $effect(() => {
    const lib = library;
    const handle = setTimeout(async () => {
      free = (await app.run(api.diskSpace(lib))) ?? null;
    }, 150);
    return () => clearTimeout(handle);
  });

  const close = () => (app.dialog = null);

  async function browse() {
    const picked = await app.run(api.pickFolder(library));
    if (picked) library = picked;
  }

  async function confirm() {
    busy = true;
    const job = await app.run(api.install(item.id, channel, target));
    busy = false;
    if (job) close();
  }
</script>

<Modal title={t("install.title", { name: item.name })} onclose={close}>
  <div class="loc">
    <span class="muted small">{t("install.library")}</span>
    <div class="row">
      <input type="text" bind:value={library} spellcheck="false" />
      <button class="btn" onclick={browse}><FolderOpen size={16} />{t("install.browse")}</button>
    </div>
    <span class="muted small path">{t("install.installsTo", { path: target })}</span>
  </div>
  <div class="sizes">
    <div><span class="muted small">{t("install.download")}</span><strong>{bytes(ch?.download)}</strong></div>
    <div><span class="muted small">{t("install.required")}</span><strong>{bytes(ch?.size)}</strong></div>
    <div class:bad={!enough}>
      <span class="muted small">{t("install.free")}</span><strong>{bytes(free)}</strong>
    </div>
  </div>
  {#if !enough}<p class="warn">{t("install.notEnough")}</p>{/if}
  {#snippet actions()}
    <button class="btn" onclick={close}>{t("install.cancel")}</button>
    <button class="btn primary" disabled={!library.trim() || !enough || busy} onclick={confirm}>
      {t("install.confirm")}
    </button>
  {/snippet}
</Modal>

<style>
  .loc {
    display: grid;
    gap: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  input {
    flex: 1;
    min-width: 0;
    height: 38px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    outline: none;
    user-select: text;
  }
  input:focus {
    border-color: var(--accent);
  }
  .row .btn {
    height: 38px;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sizes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .sizes div {
    display: grid;
    gap: 2px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--bg-2);
    border: 1px solid var(--line);
  }
  .sizes div.bad {
    border-color: var(--err);
  }
  .warn {
    color: #f08a8a;
    font-size: 0.9em;
  }
</style>
