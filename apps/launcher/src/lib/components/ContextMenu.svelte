<script lang="ts">
  import { app } from "../store.svelte";

  // Right-click menu at the cursor; entries come from app.context (see store.openContext).
  const ctx = $derived(app.context);
  let el = $state<HTMLElement>();
  let pos = $state({ x: 0, y: 0 });

  // Keep the menu inside the window.
  $effect(() => {
    if (!ctx || !el) return;
    const r = el.getBoundingClientRect();
    pos = {
      x: Math.min(ctx.x, window.innerWidth - r.width - 8),
      y: Math.min(ctx.y, window.innerHeight - r.height - 8),
    };
  });

  $effect(() => {
    if (!ctx) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent && e.key !== "Escape") return;
      if (e.type === "pointerdown" && el?.contains(e.target as Node)) return;
      app.context = null;
    };
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", close, true);
      window.removeEventListener("keydown", close);
      window.removeEventListener("blur", close);
    };
  });
</script>

{#if ctx}
  <div class="ctx" role="menu" bind:this={el} style:left="{pos.x || ctx.x}px" style:top="{pos.y || ctx.y}px">
    {#each ctx.entries as e, i (i)}
      {#if e === "sep"}
        <div class="sep"></div>
      {:else}
        <button
          role="menuitem"
          class:danger={e.danger}
          disabled={e.disabled}
          onclick={() => {
            app.context = null;
            e.onclick?.();
          }}
        >
          <span class="ic">
            {#if e.icon}<e.icon size={15} />{/if}
          </span>
          {e.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .ctx {
    position: fixed;
    z-index: 300;
    min-width: 210px;
    padding: 5px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow);
    animation: fade-in var(--fast);
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    text-align: left;
    white-space: nowrap;
    color: var(--text-2);
  }
  button:hover:not(:disabled) {
    background: var(--panel-3);
    color: var(--text);
  }
  button:disabled {
    opacity: 0.4;
  }
  .danger {
    color: #f08a8a;
  }
  .ic {
    width: 15px;
    display: grid;
    place-items: center;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 4px;
  }
</style>
