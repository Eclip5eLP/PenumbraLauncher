<script lang="ts">
  import Modal from "./Modal.svelte";
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t } from "../i18n";

  let { id }: { id: string } = $props();
  const item = $derived(app.item(id)!);
  const inst = $derived(app.library[id]);
  const close = () => (app.dialog = null);

  async function confirm() {
    await app.run(api.uninstall(id));
    close();
  }
</script>

<Modal title={t("uninstall.title", { name: item.name })} onclose={close}>
  <p class="muted">{t("uninstall.body", { path: inst?.path ?? "" })}</p>
  {#snippet actions()}
    <button class="btn" onclick={close}>{t("install.cancel")}</button>
    <button class="btn danger" onclick={confirm}>{t("uninstall.confirm")}</button>
  {/snippet}
</Modal>
