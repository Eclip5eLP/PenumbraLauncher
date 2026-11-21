<script lang="ts">
  import Background from "../components/Background.svelte";
  import NewsPanel from "../components/NewsPanel.svelte";
  import ActionPanel from "../components/ActionPanel.svelte";
  import { app } from "../store.svelte";
  import { t } from "../i18n";

  let { id }: { id: string } = $props();
  const item = $derived(app.item(id));
  const accent = $derived(app.settings.useGameAccent ? (item?.media.accent ?? null) : null);
</script>

{#if item}
  <section class="item-page" style:--accent={accent}>
    <Background
      src={item.media.bg}
      video={app.settings.backgroundVideo && !document.documentElement.hasAttribute("data-reduce-motion")
        ? item.media.bgVideo
        : null}
    />
    <div class="content">
      {#key item.id}
        <div class="hero">
          {#if item.media.logo}
            <img class="logo" src={item.media.logo} alt={item.name} />
          {:else}
            <h1>{item.name}</h1>
          {/if}
          {#if item.tagline}<p class="tagline">{item.tagline}</p>{/if}
          {#if item.source?.kind === "github"}
            <p class="source">{t("item.sourceGithub", { repo: item.source.repo })}</p>
          {/if}
          {#if item.description}<p class="desc">{item.description}</p>{/if}
        </div>
        <div class="bottom">
          <NewsPanel {item} />
          <div class="spacer"></div>
          <ActionPanel {item} />
        </div>
      {/key}
    </div>
  </section>
{/if}

<style>
  .item-page {
    position: absolute;
    inset: 0;
    --accent-2: color-mix(in oklab, var(--accent) 72%, white);
  }
  .content {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: calc(var(--title-h) + 28px) 40px 36px;
  }
  .hero {
    max-width: 560px;
    animation: rise-in var(--med) var(--ease);
  }
  h1 {
    font-size: 2.8em;
    line-height: 1.05;
    font-weight: 800;
    letter-spacing: -0.02em;
    text-shadow: 0 2px 18px rgb(0 0 0 / 0.55);
  }
  .logo {
    max-width: 420px;
    max-height: 160px;
    object-fit: contain;
    filter: drop-shadow(0 4px 18px rgb(0 0 0 / 0.5));
  }
  .tagline {
    margin-top: 10px;
    font-size: 1.1em;
    color: var(--text);
    text-shadow: 0 1px 8px rgb(0 0 0 / 0.7);
  }
  .source {
    margin-top: 6px;
    font-size: 0.88em;
    color: var(--text-2);
    text-shadow: 0 1px 6px rgb(0 0 0 / 0.7);
  }
  .desc {
    margin-top: 12px;
    color: var(--text-2);
    max-width: 460px;
    text-shadow: 0 1px 6px rgb(0 0 0 / 0.8);
  }
  .bottom {
    display: flex;
    align-items: flex-end;
    gap: 24px;
  }
  .spacer {
    flex: 1;
  }
  @media (max-width: 1150px) {
    .bottom :global(.news) {
      width: 360px;
    }
  }
</style>
