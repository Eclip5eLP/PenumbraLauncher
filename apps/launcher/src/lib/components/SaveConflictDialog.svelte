<script lang="ts">
  import Cloud from "@lucide/svelte/icons/cloud";
  import Monitor from "@lucide/svelte/icons/monitor";
  import Modal from "./Modal.svelte";
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t } from "../i18n";
  import type { CloudMeta } from "../types";

  let { item, cloud }: { item: string; cloud: CloudMeta } = $props();
  let busy = $state(false);
  const when = $derived(cloud.updated ? new Date(cloud.updated).toLocaleString() : "?");

  async function keep(which: "cloud" | "local") {
    // Copy the prop first: closing the dialog unmounts this component, and its props go away with it.
    const id = item;
    busy = true;
    const ok = await app.run(api.resolveSaves(id, which).then(() => true));
    busy = false;
    if (ok) {
      app.dialog = null;
      void app.play(id);
    }
  }
</script>

<Modal title={t("saves.title")} onclose={() => (app.dialog = null)}>
  <p class="muted">{t("saves.body")}</p>
  <div class="choices">
    <button class="choice" disabled={busy} onclick={() => keep("cloud")}>
      <Cloud size={22} />
      <span>
        <strong>{t("saves.keepCloud")}</strong>
        <small>{t("saves.cloud", { device: cloud.device || "?", date: when })}</small>
      </span>
    </button>
    <button class="choice" disabled={busy} onclick={() => keep("local")}>
      <Monitor size={22} />
      <span><strong>{t("saves.keepLocal")}</strong></span>
    </button>
  </div>
  <p class="muted small">{t("saves.backupNote")}</p>
  {#snippet actions()}
    <button class="btn" onclick={() => (app.dialog = null)}>{t("install.cancel")}</button>
  {/snippet}
</Modal>

<style>
  .choices {
    display: grid;
    gap: 8px;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    text-align: left;
    transition:
      border-color var(--fast),
      background var(--fast);
  }
  .choice:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--panel-2);
  }
  .choice span {
    display: grid;
  }
  small {
    color: var(--muted);
  }
</style>
