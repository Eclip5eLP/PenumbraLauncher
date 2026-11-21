<script lang="ts">
  import { t, tDyn } from "../i18n";
  import { date } from "../format";
  import type { Item } from "../types";
  import { app } from "../store.svelte";
  import { api } from "../api";

  let { item }: { item: Item } = $props();

  let slide = $state(0);
  let hovering = $state(false);
  let tab = $state<string>("all");

  const banners = $derived(item.banners);
  const categories = $derived(["all", ...new Set(item.news.map((n) => n.category))]);
  const posts = $derived(
    (tab === "all" ? item.news : item.news.filter((n) => n.category === tab)).slice(0, 5),
  );

  // Reset when switching items.
  $effect(() => {
    void item.id;
    slide = 0;
    tab = "all";
  });

  // Auto-advance every 6 s, paused while hovered.
  $effect(() => {
    if (banners.length < 2 || hovering) return;
    const id = setInterval(() => (slide = (slide + 1) % banners.length), 6000);
    return () => clearInterval(id);
  });
</script>

{#if banners.length || item.news.length}
  <section class="news glass">
    {#if banners.length}
      <div
        class="carousel"
        role="region"
        aria-label="banners"
        onpointerenter={() => (hovering = true)}
        onpointerleave={() => (hovering = false)}
      >
        <div class="track" style:transform="translateX(-{slide * 100}%)">
          {#each banners as b (b.id)}
            <div class="slide" class:link={!!b.url}>
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
              <img
                src={b.image}
                alt={b.title ?? ""}
                loading="lazy"
                onclick={() => b.url && app.run(api.openNews(item.id, b.id))}
              />
              {#if b.title}<span class="caption">{b.title}</span>{/if}
            </div>
          {/each}
        </div>
        {#if banners.length > 1}
          <div class="dots">
            {#each banners as b, i (b.id)}
              <button class:on={i === slide} aria-label="banner {i + 1}" onclick={() => (slide = i)}></button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <div class="tabs" role="tablist">
      {#each categories as c (c)}
        <button role="tab" class:on={tab === c} aria-selected={tab === c} onclick={() => (tab = c)}>
          {tDyn(`news.${c}`, c)}
        </button>
      {/each}
    </div>
    <ul>
      {#each posts as p (p.id)}
        <li class:link={!!p.url}>
          <button disabled={!p.url} onclick={() => app.run(api.openNews(item.id, p.id))}>
            <span class="cat {p.category}"></span>
            <span class="title">{p.title}</span>
            <span class="date">{date(p.date)}</span>
          </button>
        </li>
      {:else}
        <li class="muted">{t("news.empty")}</li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .news {
    width: 420px;
    border-radius: var(--radius-lg);
    overflow: hidden;
    animation: rise-in var(--med) var(--ease);
  }
  .carousel {
    position: relative;
    aspect-ratio: 1280 / 600;
    overflow: hidden;
  }
  .track {
    display: flex;
    height: 100%;
    transition: transform 420ms var(--ease);
  }
  .slide {
    position: relative;
    flex: 0 0 100%;
    height: 100%;
  }
  .slide img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .caption {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 28px 14px 12px;
    background: linear-gradient(transparent, rgb(0 0 0 / 0.7));
    font-weight: 600;
  }
  .dots {
    position: absolute;
    right: 12px;
    bottom: 14px;
    display: flex;
    gap: 6px;
  }
  .dots button {
    width: 7px;
    height: 7px;
    border-radius: 4px;
    background: rgb(255 255 255 / 0.45);
    transition:
      width var(--med) var(--ease),
      background var(--fast);
  }
  .dots button.on {
    width: 18px;
    background: #fff;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 10px 12px 4px;
  }
  .tabs button {
    padding: 4px 10px;
    border-radius: 14px;
    color: var(--muted);
    font-size: 0.9em;
    font-weight: 550;
    transition:
      background var(--fast),
      color var(--fast);
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.on {
    background: rgb(255 255 255 / 0.1);
    color: var(--text);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 12px 12px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 4px;
    border-radius: var(--radius-sm);
    font-size: 0.93em;
    color: var(--text-2);
  }
  li button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 100%;
    text-align: left;
    cursor: default;
  }
  li.link button {
    cursor: pointer;
  }
  .slide.link img {
    cursor: pointer;
  }
  li:hover {
    background: rgb(255 255 255 / 0.05);
    color: var(--text);
  }
  .cat {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--muted);
  }
  .cat.event {
    background: #e7a84a;
  }
  .cat.notice {
    background: var(--accent-2);
  }
  .title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .date {
    color: var(--faint);
    font-size: 0.88em;
    font-variant-numeric: tabular-nums;
  }
</style>
