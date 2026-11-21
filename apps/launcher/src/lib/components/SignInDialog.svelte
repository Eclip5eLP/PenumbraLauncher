<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import Modal from "./Modal.svelte";
  import { app } from "../store.svelte";
  import { api, errorMessage, on } from "../api";
  import { t } from "../i18n";
  import type { DeviceInfo } from "../types";

  // Device sign-in: the launcher shows a code, the browser opens the account page, the user confirms there.
  let info = $state<DeviceInfo | null>(null);
  let error = $state<string | null>(null);
  let copied = $state(false);

  async function start() {
    error = null;
    info = null;
    try {
      info = await api.authStart();
    } catch (e) {
      error = errorMessage(e);
    }
  }

  $effect(() => {
    void start();
    const off = on("auth:device-failed", (msg) => (error = msg));
    return () => {
      void off.then((f) => f());
    };
  });

  function close() {
    void api.authCancel();
    app.dialog = null;
  }

  async function copy() {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.userCode);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  }
</script>

<Modal title={t("account.signInTitle")} onclose={close}>
  {#if error}
    <p class="err">{error}</p>
  {:else if !info}
    <p class="muted wait"><LoaderCircle size={16} class="spin" /> {t("account.starting")}</p>
  {:else}
    <p class="muted">{t("account.instructions")}</p>
    <div class="code-row">
      <span class="code">{info.userCode}</span>
      <button class="btn ghost" onclick={copy}>
        <Copy size={16} />{copied ? t("account.copied") : t("account.copy")}
      </button>
    </div>
    <p class="muted small wait"><LoaderCircle size={14} class="spin" /> {t("account.waiting")}</p>
    <p class="muted small">
      {t("account.manual", { url: info.verificationUri })}
    </p>
  {/if}
  {#snippet actions()}
    <button class="btn" onclick={close}>{t("install.cancel")}</button>
    {#if error}
      <button class="btn primary" onclick={start}>{t("library.retry")}</button>
    {:else if info}
      <button class="btn primary" onclick={() => info && api.authReopen(info.verificationUriComplete)}>
        <ExternalLink size={16} />{t("account.openAgain")}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .code-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--bg-2);
    border: 1px solid var(--line-2);
  }
  .code {
    font-size: 2em;
    font-weight: 750;
    letter-spacing: 0.12em;
    font-variant-numeric: tabular-nums;
    user-select: text;
  }
  .wait {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .err {
    color: #f08a8a;
  }
</style>
