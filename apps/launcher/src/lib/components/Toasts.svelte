<script lang="ts">
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Info from "@lucide/svelte/icons/info";
  import { app } from "../store.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast {toast.kind}">
      {#if toast.kind === "ok"}<CircleCheck size={18} />{:else if toast.kind === "err"}<TriangleAlert
          size={18}
        />{:else}<Info size={18} />{/if}
      <span>{toast.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 200;
    display: grid;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 380px;
    padding: 12px 16px;
    border-radius: var(--radius);
    background: var(--panel-2);
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow);
    animation: rise-in var(--med) var(--ease);
  }
  .ok :global(svg) {
    color: #7fd3a1;
  }
  .err :global(svg) {
    color: #f08a8a;
  }
  .info :global(svg) {
    color: var(--accent-2);
  }
</style>
