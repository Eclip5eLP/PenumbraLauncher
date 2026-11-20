<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onclose,
    children,
    actions,
  }: { title: string; onclose: () => void; children: Snippet; actions: Snippet } = $props();

  $effect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onclose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title}>
    <h2>{title}</h2>
    <div class="body">{@render children()}</div>
    <div class="actions">{@render actions()}</div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.55);
    animation: fade-in var(--fast) both;
  }
  .modal {
    width: min(520px, calc(100vw - 48px));
    padding: 22px 24px 20px;
    border-radius: var(--radius-lg);
    background: var(--panel);
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow);
    animation: rise-in var(--med) var(--ease) both;
  }
  h2 {
    font-size: 1.25em;
    font-weight: 650;
    margin-bottom: 16px;
  }
  .body {
    display: grid;
    gap: 14px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 22px;
  }
</style>
