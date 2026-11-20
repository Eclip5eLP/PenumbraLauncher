<script lang="ts">
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import X from "@lucide/svelte/icons/x";
  import ArrowUpToLine from "@lucide/svelte/icons/arrow-up-to-line";
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import { app } from "../store.svelte";
  import { api } from "../api";
  import { t, tDyn } from "../i18n";
  import { bytes, duration, percent, speed } from "../format";
  import type { Job } from "../types";

  const finishedStates = ["done", "error", "cancelled"];
  const active = $derived(app.jobs.filter((j) => !finishedStates.includes(j.state)));
  const current = $derived(
    active.filter((j) => ["preparing", "downloading", "finalizing"].includes(j.state)),
  );
  const queue = $derived(active.filter((j) => !current.includes(j)));
  const done = $derived(app.jobs.filter((j) => finishedStates.includes(j.state)).reverse());

  // Speed history for the current job's sparkline (last 60 samples ≈ 6 s at 10 Hz).
  let history = $state<number[]>([]);
  $effect(() => {
    const j = current[0];
    if (!j) {
      history = [];
      return;
    }
    history = [...history.slice(-59), j.speed];
  });
  const maxSpeed = $derived(Math.max(1, ...history));
  const spark = $derived(history.map((v, i) => `${(i / 59) * 100},${30 - (v / maxSpeed) * 28}`).join(" "));

  const name = (j: Job) => app.item(j.item)?.name ?? j.item;
  const icon = (j: Job) => app.item(j.item)?.media.icon;
</script>

<div class="page">
  <h1>{t("downloads.title")}</h1>

  {#if !app.jobs.length}
    <p class="muted">{t("downloads.empty")}</p>
  {/if}

  {#each current as j (j.id)}
    {@const pct = percent(j.downloaded, j.total)}
    <section class="current" style:--item-accent={app.item(j.item)?.media.accent ?? "var(--accent)"}>
      {#if icon(j)}<img class="icon big" src={icon(j)} alt="" />{/if}
      <div class="body">
        <div class="row">
          <div>
            <div class="title">{name(j)}</div>
            <div class="muted small">{tDyn(`downloads.kinds.${j.kind}`)} · {j.version} · {j.path}</div>
          </div>
          <div class="ctl">
            <button
              class="icon-btn"
              aria-label={t("action.pause")}
              onclick={() => app.run(api.pauseJob(j.id))}
            >
              <Pause size={18} />
            </button>
            <button
              class="icon-btn"
              aria-label={t("action.cancel")}
              onclick={() => app.run(api.cancelJob(j.id))}
            >
              <X size={18} />
            </button>
          </div>
        </div>
        <div class="bar"><span style:width="{pct}%"></span></div>
        <div class="row stats">
          <span>{Math.floor(pct)}% · {t("downloads.of", { a: bytes(j.downloaded), b: bytes(j.total) })}</span>
          <span>
            {#if j.state === "downloading"}
              {speed(j.speed)}{#if j.eta != null}
                · {t("downloads.eta", { t: duration(j.eta) })}{/if}
            {:else}
              {tDyn(`action.${j.state}`)}
            {/if}
          </span>
        </div>
      </div>
      <svg class="spark" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
        <polyline points={spark} />
      </svg>
    </section>
  {/each}

  {#if queue.length}
    <h2>{t("downloads.queue")}</h2>
    <ul class="list">
      {#each queue as j, i (j.id)}
        <li>
          {#if icon(j)}<img class="icon" src={icon(j)} alt="" />{/if}
          <div class="grow">
            <div class="title-s">{name(j)}</div>
            <div class="muted small">
              {tDyn(`downloads.kinds.${j.kind}`)} · {bytes(j.total)} · {tDyn(`action.${j.state}`)}
            </div>
          </div>
          {#if i > 0 || current.length}
            <button
              class="icon-btn"
              aria-label={t("downloads.moveUp")}
              title={t("downloads.moveUp")}
              onclick={() => app.run(api.prioritizeJob(j.id))}
            >
              <ArrowUpToLine size={17} />
            </button>
          {/if}
          {#if j.state === "paused"}
            <button
              class="icon-btn"
              aria-label={t("action.resume")}
              onclick={() => app.run(api.resumeJob(j.id))}
            >
              <Play size={17} />
            </button>
          {:else}
            <button
              class="icon-btn"
              aria-label={t("action.pause")}
              onclick={() => app.run(api.pauseJob(j.id))}
            >
              <Pause size={17} />
            </button>
          {/if}
          <button
            class="icon-btn"
            aria-label={t("action.cancel")}
            onclick={() => app.run(api.cancelJob(j.id))}
          >
            <X size={17} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if done.length}
    <div class="h2row">
      <h2>{t("downloads.completed")}</h2>
      <button class="btn ghost small" onclick={() => app.run(api.clearFinishedJobs())}>
        {t("downloads.clear")}
      </button>
    </div>
    <ul class="list">
      {#each done as j (j.id)}
        <li class="finished">
          {#if icon(j)}<img class="icon" src={icon(j)} alt="" />{/if}
          <div class="grow">
            <div class="title-s">{name(j)}</div>
            <div class="muted small">
              {tDyn(`downloads.kinds.${j.kind}`)} · {j.version} · {bytes(j.total)}
            </div>
          </div>
          <span class="result {j.state}">
            {#if j.state === "done"}<CircleCheck size={15} />{/if}
            {tDyn(`downloads.states.${j.state}`)}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  h2 {
    font-size: 1em;
    font-weight: 650;
    color: var(--text-2);
    margin: 28px 0 10px;
  }
  .h2row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .h2row h2 {
    margin-bottom: 10px;
  }
  .current {
    position: relative;
    display: flex;
    gap: 18px;
    padding: 18px 20px;
    border-radius: var(--radius-lg);
    background: var(--panel);
    border: 1px solid var(--line);
    overflow: hidden;
    margin-bottom: 12px;
  }
  .current .body {
    flex: 1;
    display: grid;
    gap: 10px;
    z-index: 1;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .title {
    font-size: 1.2em;
    font-weight: 650;
  }
  .stats {
    color: var(--text-2);
    font-size: 0.9em;
    font-variant-numeric: tabular-nums;
  }
  .ctl {
    display: flex;
    gap: 4px;
  }
  .bar > span {
    background: linear-gradient(
      90deg,
      var(--item-accent),
      color-mix(in oklab, var(--item-accent) 65%, white)
    );
  }
  .spark {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    width: 100%;
    height: 70px;
    opacity: 0.22;
    pointer-events: none;
  }
  .spark polyline {
    fill: none;
    stroke: var(--item-accent);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }
  .icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    object-fit: cover;
  }
  .icon.big {
    width: 64px;
    height: 64px;
    border-radius: 14px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--line);
    animation: rise-in var(--fast) var(--ease);
  }
  .list li.finished {
    background: transparent;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title-s {
    font-weight: 600;
  }
  .result {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.88em;
    color: var(--muted);
  }
  .result.done {
    color: #7fd3a1;
  }
  .result.error {
    color: #f08a8a;
  }
</style>
