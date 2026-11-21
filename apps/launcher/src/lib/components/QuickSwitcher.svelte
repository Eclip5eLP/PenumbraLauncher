<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import { app, type Route } from "../store.svelte";
  import { t, tDyn } from "../i18n";

  // Ctrl+K: jump to any item or page. Typing filters, arrows move, Enter opens.
  type Entry = { key: string; label: string; sub: string; icon?: string | null; route: Route };

  let query = $state("");
  let index = $state(0);
  let input = $state<HTMLInputElement>();

  const entries = $derived.by<Entry[]>(() => {
    const items: Entry[] = app.orderedItems.map((i) => ({
      key: "item:" + i.id,
      label: i.name,
      sub:
        tDyn(`rail.groups.${i.group}`, i.group) +
        (app.library[i.id] ? " · " + app.library[i.id].version : ""),
      icon: i.media.icon,
      route: { page: "item", id: i.id },
    }));
    const pages: Entry[] = [
      { key: "p:library", label: t("rail.library"), sub: t("switcher.page"), route: { page: "library" } },
      {
        key: "p:downloads",
        label: t("rail.downloads"),
        sub: t("switcher.page"),
        route: { page: "downloads" },
      },
      ...["general", "downloads", "appearance", "account", "advanced", "about"].map((s) => ({
        key: "s:" + s,
        label: `${t("settings.title")} › ${tDyn(`settings.sections.${s}`)}`,
        sub: t("switcher.page"),
        route: { page: "settings", section: s } as Route,
      })),
    ];
    const q = query.trim().toLowerCase();
    const all = [...items, ...pages];
    if (!q) return all;
    // Simple scoring: prefix match beats substring match.
    return all
      .map((e) => {
        const l = e.label.toLowerCase();
        const score = l.startsWith(q)
          ? 0
          : l.split(/\s+/).some((w) => w.startsWith(q))
            ? 1
            : l.includes(q)
              ? 2
              : 9;
        return { e, score };
      })
      .filter((x) => x.score < 9)
      .sort((a, b) => a.score - b.score)
      .map((x) => x.e);
  });

  $effect(() => {
    void query;
    index = 0;
  });
  $effect(() => {
    input?.focus();
  });

  function open(e: Entry | undefined) {
    if (!e) return;
    app.switcherOpen = false;
    app.go(e.route);
  }

  function onkey(ev: KeyboardEvent) {
    if (ev.key === "ArrowDown") {
      index = Math.min(entries.length - 1, index + 1);
      ev.preventDefault();
    } else if (ev.key === "ArrowUp") {
      index = Math.max(0, index - 1);
      ev.preventDefault();
    } else if (ev.key === "Enter") {
      open(entries[index]);
    } else if (ev.key === "Escape") {
      app.switcherOpen = false;
    }
  }
</script>

<div
  class="backdrop"
  role="presentation"
  onpointerdown={(e) => e.target === e.currentTarget && (app.switcherOpen = false)}
>
  <div class="box" role="dialog" aria-label="Quick switcher">
    <div class="search">
      <Search size={18} />
      <input
        bind:this={input}
        bind:value={query}
        placeholder={t("switcher.placeholder")}
        onkeydown={onkey}
        spellcheck="false"
      />
      <kbd>Esc</kbd>
    </div>
    <ul>
      {#each entries as e, i (e.key)}
        <li>
          <button class:on={i === index} onpointermove={() => (index = i)} onclick={() => open(e)}>
            {#if e.icon}<img src={e.icon} alt="" />{:else}<span class="ph">{e.label[0]}</span>{/if}
            <span class="label">{e.label}</span>
            <span class="sub">{e.sub}</span>
          </button>
        </li>
      {:else}
        <li class="empty">{t("switcher.empty")}</li>
      {/each}
    </ul>
    <div class="hint">{t("switcher.hint")}</div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 150;
    background: rgb(0 0 0 / 0.45);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 14vh;
    animation: fade-in var(--fast);
  }
  .box {
    width: min(560px, calc(100vw - 32px));
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-lg);
    background: var(--panel);
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow);
    overflow: hidden;
    animation: rise-in var(--med) var(--ease);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }
  input {
    flex: 1;
    height: 50px;
    background: none;
    border: 0;
    outline: none;
    font-size: 1.05em;
    color: var(--text);
    user-select: text;
  }
  kbd {
    font: inherit;
    font-size: 0.75em;
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid var(--line-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 42px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    text-align: left;
  }
  li button.on {
    background: var(--panel-3);
  }
  img,
  .ph {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    object-fit: cover;
    flex: none;
  }
  .ph {
    display: grid;
    place-items: center;
    background: var(--panel-2);
    color: var(--muted);
    font-weight: 700;
    font-size: 12px;
  }
  .label {
    flex: 1;
    font-weight: 550;
  }
  .sub {
    color: var(--faint);
    font-size: 0.85em;
  }
  .empty {
    padding: 14px;
    color: var(--muted);
  }
  .hint {
    padding: 8px 14px;
    border-top: 1px solid var(--line);
    color: var(--faint);
    font-size: 0.8em;
  }
</style>
