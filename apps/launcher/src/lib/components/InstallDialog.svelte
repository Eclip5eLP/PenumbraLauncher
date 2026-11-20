<script lang="ts">
  import Modal from "./Modal.svelte";
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t } from "../i18n";
  import { bytes } from "../format";

  let { id }: { id: string } = $props();
  const item = $derived(app.item(id)!);
  const channel = $derived(app.channelOf(item));
  const ch = $derived(item.channels[channel]);
  const library = $derived(app.settings.defaultLibrary ?? app.settings.libraries[0] ?? "D:\\Penumbra Games");
  let path = $state("");
  $effect(() => {
    path = `${library}\\${item.name}`;
  });

  const close = () => (app.dialog = null);
  async function confirm() {
    const job = await app.run(api.install(item.id, channel, path.trim()));
    if (job) close();
  }
</script>

<Modal title={t("install.title", { name: item.name })} onclose={close}>
  <label class="loc">
    <span class="muted small">{t("install.location")}</span>
    <input type="text" bind:value={path} spellcheck="false" />
    <span class="muted small">{t("install.pathHint")}</span>
  </label>
  <div class="sizes">
    <div><span class="muted small">{t("install.download")}</span><strong>{bytes(ch?.download)}</strong></div>
    <div><span class="muted small">{t("install.required")}</span><strong>{bytes(ch?.size)}</strong></div>
    <div><span class="muted small">{t("install.version")}</span><strong>{ch?.version ?? "–"}</strong></div>
  </div>
  {#snippet actions()}
    <button class="btn" onclick={close}>{t("install.cancel")}</button>
    <button class="btn primary" disabled={!path.trim()} onclick={confirm}>{t("install.confirm")}</button>
  {/snippet}
</Modal>

<style>
  .loc {
    display: grid;
    gap: 6px;
  }
  input {
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
</style>
