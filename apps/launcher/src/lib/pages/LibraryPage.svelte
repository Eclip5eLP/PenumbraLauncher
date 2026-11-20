<script lang="ts">
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t, tDyn } from "../i18n";

  let filter = $state<"all" | "installed">("all");
  const items = $derived(
    app.visibleItems.filter((i) => filter === "all" || app.library[i.id] || app.activeJob(i.id)),
  );
</script>

<div class="page">
  <div class="head">
    <h1>{t("library.title")}</h1>
    <div class="chips">
      <button class:on={filter === "all"} onclick={() => (filter = "all")}>{t("library.all")}</button>
      <button class:on={filter === "installed"} onclick={() => (filter = "installed")}>
        {t("library.installed")}
      </button>
    </div>
  </div>
  <div class="grid">
    {#each items as item (item.id)}
      {@const a = app.action(item)}
      <button
        class="tile"
        style:--item-accent={item.media.accent ?? "var(--accent)"}
        onclick={() => app.go({ page: "item", id: item.id })}
      >
        <div class="art">
          {#if item.media.cover}
            <img src={item.media.cover} alt="" loading="lazy" />
          {:else}
            <span class="letter">{item.name[0]}</span>
          {/if}
          {#if a === "update" || a === "running" || a === "comingSoon" || app.activeJob(item.id)}
            <span class="tag tag-{a}">{tDyn(`action.${a}`)}</span>
          {/if}
        </div>
        <div class="name">{item.name}</div>
        <div class="sub">
          {app.library[item.id]
            ? app.library[item.id].version
            : tDyn(`rail.groups.${item.group}`, item.group)}
        </div>
      </button>
    {:else}
      <div class="empty">
        {#if app.catalog.items.length === 0 && app.catalogStatus.state === "loading"}
          <p class="muted">{t("library.loading")}</p>
        {:else if app.catalog.items.length === 0 && app.catalogStatus.state === "offline"}
          <p class="muted">{t("library.offline")}</p>
        {:else if app.catalog.items.length === 0 && app.catalogStatus.state === "error"}
          <p class="muted">{app.catalogStatus.message}</p>
          <button class="btn" onclick={() => app.run(api.refreshCatalog())}>{t("library.retry")}</button>
        {:else}
          <p class="muted">{t("library.empty")}</p>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .empty {
    grid-column: 1 / -1;
    display: grid;
    justify-items: start;
    gap: 12px;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 20px;
  }
  .chips {
    display: flex;
    gap: 6px;
  }
  .chips button {
    padding: 5px 12px;
    border-radius: 16px;
    color: var(--muted);
    font-weight: 550;
    background: var(--panel);
    border: 1px solid var(--line);
  }
  .chips button.on {
    color: var(--text);
    border-color: var(--line-2);
    background: var(--panel-2);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 22px;
  }
  .tile {
    text-align: left;
    animation: rise-in var(--med) var(--ease) both;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--panel);
    box-shadow: 0 0 0 1px var(--line);
    transition:
      transform var(--med) var(--ease),
      box-shadow var(--med);
  }
  .tile:hover .art {
    transform: translateY(-4px);
    box-shadow:
      0 0 0 2px var(--item-accent),
      0 14px 30px rgb(0 0 0 / 0.5);
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .letter {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 48px;
    font-weight: 800;
    color: var(--muted);
  }
  .tag {
    position: absolute;
    left: 8px;
    top: 8px;
    padding: 2px 8px;
    border-radius: 6px;
    font-size: 0.8em;
    font-weight: 650;
    background: rgb(0 0 0 / 0.65);
  }
  .tag-update {
    background: var(--accent);
  }
  .tag-running {
    background: var(--ok);
  }
  .name {
    margin-top: 10px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 0.88em;
  }
</style>
