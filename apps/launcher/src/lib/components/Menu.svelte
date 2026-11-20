<script lang="ts" module>
  import type { Component } from "svelte";
  export interface MenuEntry {
    label: string;
    icon?: Component<{ size?: number }>;
    danger?: boolean;
    disabled?: boolean;
    checked?: boolean;
    hint?: string;
    onclick?: () => void;
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  // Small dropdown: trigger snippet + entries. Closes on outside click / Escape.
  let {
    entries,
    trigger,
    placement = "top-end",
  }: {
    entries: (MenuEntry | "sep")[];
    trigger: Snippet<[() => void, boolean]>;
    placement?: "top-end" | "bottom-end";
  } = $props();

  let open = $state(false);
  let root: HTMLElement;
  const toggle = () => (open = !open);

  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => !root.contains(e.target as Node) && (open = false);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && (open = false);
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="menu-root" bind:this={root}>
  {@render trigger(toggle, open)}
  {#if open}
    <div class="menu {placement}" role="menu">
      {#each entries as e, i (i)}
        {#if e === "sep"}
          <div class="sep"></div>
        {:else}
          <button
            role="menuitem"
            class:danger={e.danger}
            disabled={e.disabled}
            onclick={() => {
              open = false;
              e.onclick?.();
            }}
          >
            <span class="ic">
              {#if e.icon}<e.icon size={16} />{:else if e.checked}<span class="check"></span>{/if}
            </span>
            <span class="label">{e.label}</span>
            {#if e.hint}<span class="hint">{e.hint}</span>{/if}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu-root {
    position: relative;
  }
  .menu {
    position: absolute;
    min-width: 230px;
    width: max-content;
    max-width: 380px;
    padding: 6px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow);
    z-index: 40;
    animation: rise-in var(--fast) var(--ease);
  }
  .top-end {
    right: 0;
    bottom: calc(100% + 8px);
  }
  .bottom-end {
    right: 0;
    top: calc(100% + 8px);
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    text-align: left;
    color: var(--text-2);
  }
  button:hover:not(:disabled) {
    background: var(--panel-3);
    color: var(--text);
  }
  button:disabled {
    opacity: 0.4;
  }
  button.danger {
    color: #f08a8a;
  }
  .ic {
    width: 16px;
    display: grid;
    place-items: center;
  }
  .check {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-2);
  }
  .label {
    flex: 1;
    white-space: nowrap;
  }
  .hint {
    white-space: nowrap;
    margin-left: 12px;
    color: var(--faint);
    font-size: 0.85em;
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: 5px 4px;
  }
</style>
