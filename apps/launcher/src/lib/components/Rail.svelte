<script lang="ts">
  import Download from "@lucide/svelte/icons/download";
  import Settings from "@lucide/svelte/icons/settings";
  import LibraryBig from "@lucide/svelte/icons/library-big";
  import logo from "../../assets/logo-96.png";
  import { t } from "../i18n";
  import { app } from "../store.svelte";
  import { percent } from "../format";
  import type { Item } from "../types";

  const groups = $derived(
    app.catalog.groups
      .map((g) => ({ ...g, items: app.visibleItems.filter((i) => i.group === g.id) }))
      .filter((g) => g.items.length > 0),
  );
  const route = $derived(app.route);
  let dragId = $state<string | null>(null);
  let dropTarget = $state<string | null>(null);

  // Warm the image cache on hover so switching feels instant.
  const prefetched = new Set<string>();
  function prefetch(item: Item) {
    const src = item.media.bg;
    if (!src || prefetched.has(src)) return;
    prefetched.add(src);
    const img = new Image();
    img.src = src;
  }

  function status(item: Item) {
    const job = app.activeJob(item.id);
    if (job)
      return {
        kind: "job" as const,
        pct: percent(job.downloaded, job.total),
        paused: job.state === "paused",
      };
    if (app.running.includes(item.id)) return { kind: "running" as const };
    if (app.action(item) === "update") return { kind: "update" as const };
    return null;
  }
</script>

<nav>
  <button
    class="logo"
    aria-label={t("rail.library")}
    onclick={() => app.go({ page: "library" })}
    data-tip={t("rail.library")}
  >
    <img src={logo} alt="" width="40" height="40" />
  </button>

  <div class="items">
    {#each groups as g, gi (g.id)}
      {#if gi > 0}<div class="divider"></div>{/if}
      {#each g.items as item (item.id)}
        {@const s = status(item)}
        {@const active = route.page === "item" && route.id === item.id}
        <button
          class="item"
          class:active
          style:--item-accent={item.media.accent ?? "var(--accent)"}
          data-tip={item.name}
          aria-label={item.name}
          class:drop-target={dropTarget === item.id}
          draggable="true"
          onpointerenter={() => prefetch(item)}
          onclick={() => app.go({ page: "item", id: item.id })}
          oncontextmenu={(e) => app.openContext(e, app.itemMenu(item))}
          ondragstart={(e) => {
            dragId = item.id;
            e.dataTransfer?.setData("text/plain", item.id);
          }}
          ondragover={(e) => {
            if (dragId && dragId !== item.id) {
              e.preventDefault();
              dropTarget = item.id;
            }
          }}
          ondragleave={() => dropTarget === item.id && (dropTarget = null)}
          ondrop={(e) => {
            e.preventDefault();
            if (dragId) app.moveBefore(dragId, item.id);
            dragId = dropTarget = null;
          }}
          ondragend={() => (dragId = dropTarget = null)}
        >
          {#if item.media.icon}
            <img src={item.media.icon} alt="" width="44" height="44" />
          {:else}
            <span class="letter">{item.name[0]}</span>
          {/if}
          {#if s?.kind === "job"}
            <svg class="ring" viewBox="0 0 52 52" aria-hidden="true">
              <circle cx="26" cy="26" r="24" pathLength="100" class="track" />
              <circle
                cx="26"
                cy="26"
                r="24"
                pathLength="100"
                class="fill"
                class:paused={s.paused}
                stroke-dasharray="{s.pct} 100"
              />
            </svg>
          {:else if s?.kind === "running"}
            <span class="dot running"></span>
          {:else if s?.kind === "update"}
            <span class="dot update"></span>
          {/if}
        </button>
      {/each}
    {/each}
  </div>

  <div class="bottom">
    <button
      class="tool"
      class:active={route.page === "library"}
      data-tip={t("rail.library")}
      aria-label={t("rail.library")}
      onclick={() => app.go({ page: "library" })}
    >
      <LibraryBig size={20} />
    </button>
    <button
      class="tool"
      class:active={route.page === "downloads"}
      data-tip={t("rail.downloads")}
      aria-label={t("rail.downloads")}
      onclick={() => app.go({ page: "downloads" })}
    >
      <Download size={20} />
      {#if app.activeJobCount > 0}<span class="badge">{app.activeJobCount}</span>{/if}
    </button>
    <button
      class="tool"
      class:active={route.page === "settings"}
      data-tip={t("rail.settings")}
      aria-label={t("rail.settings")}
      onclick={() => app.go({ page: "settings" })}
    >
      <Settings size={20} />
    </button>
  </div>
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--bg);
    border-right: 1px solid rgb(255 255 255 / 0.04);
    z-index: 30;
    padding: 10px 0 12px;
  }
  .logo {
    width: 48px;
    height: 48px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    margin-bottom: 10px;
    transition: transform var(--med) var(--ease);
  }
  .logo:hover {
    transform: rotate(-12deg) scale(1.05);
  }
  .items {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding-top: 6px;
    width: 100%;
  }
  .divider {
    width: 28px;
    height: 1px;
    background: var(--line);
    margin: 2px 0;
  }
  .item {
    position: relative;
    width: 48px;
    height: 48px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    transition:
      border-radius var(--med) var(--ease),
      transform var(--fast) var(--ease);
  }
  .item img,
  .item .letter {
    width: 44px;
    height: 44px;
    border-radius: 12px;
    object-fit: cover;
    opacity: 0.72;
    filter: saturate(0.8);
    transition:
      opacity var(--fast),
      filter var(--fast),
      border-radius var(--med) var(--ease);
  }
  .item .letter {
    display: grid;
    place-items: center;
    background: var(--panel-2);
    font-weight: 700;
    font-size: 18px;
  }
  .item:hover img,
  .item:hover .letter,
  .item.active img,
  .item.active .letter {
    opacity: 1;
    filter: none;
  }
  .item:active {
    transform: scale(0.95);
  }
  .item::before {
    content: "";
    position: absolute;
    left: -12px;
    top: 50%;
    width: 4px;
    height: 0;
    border-radius: 0 4px 4px 0;
    background: var(--item-accent);
    transform: translateY(-50%);
    transition: height var(--med) var(--ease);
  }
  .item:hover::before {
    height: 16px;
  }
  .item.active::before {
    height: 32px;
  }
  .item.drop-target {
    box-shadow: 0 -3px 0 0 var(--accent-2);
  }
  .item.active {
    box-shadow: 0 0 0 2px var(--item-accent);
  }
  .ring {
    position: absolute;
    inset: -2px;
    width: 52px;
    height: 52px;
    transform: rotate(-90deg);
    pointer-events: none;
  }
  .ring circle {
    fill: none;
    stroke-width: 3;
  }
  .ring .track {
    stroke: rgb(0 0 0 / 0.5);
  }
  .ring .fill {
    stroke: var(--item-accent);
    stroke-linecap: round;
    transition: stroke-dasharray 120ms linear;
  }
  .ring .fill.paused {
    stroke: var(--faint);
  }
  .dot {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 3px solid var(--bg);
  }
  .dot.running {
    background: var(--ok);
  }
  .dot.update {
    background: var(--accent);
  }
  .bottom {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .tool {
    position: relative;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border-radius: 12px;
    color: var(--muted);
    transition:
      background var(--fast),
      color var(--fast);
  }
  .tool:hover,
  .tool.active {
    background: var(--panel);
    color: var(--text);
  }
  .badge {
    position: absolute;
    top: 4px;
    right: 4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    display: grid;
    place-items: center;
  }
  /* Tooltips */
  [data-tip]::after {
    content: attr(data-tip);
    position: absolute;
    left: calc(100% + 14px);
    top: 50%;
    transform: translate(-4px, -50%);
    white-space: nowrap;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    color: var(--text);
    font-size: 13px;
    font-weight: 550;
    pointer-events: none;
    opacity: 0;
    transition:
      opacity var(--fast),
      transform var(--fast) var(--ease);
    z-index: 50;
  }
  [data-tip]:hover::after {
    opacity: 1;
    transform: translate(0, -50%);
    transition-delay: 250ms;
  }
  .logo,
  .tool {
    position: relative;
  }
</style>
