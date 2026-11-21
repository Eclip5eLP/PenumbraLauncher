<script lang="ts">
  import { untrack } from "svelte";

  // Crossfading full-bleed background. The next image is decoded before it's shown, so switching items never
  // flashes a half-loaded picture. At most two layers exist at any time.
  let { src, video }: { src?: string | null; video?: string | null } = $props();
  let videoEl = $state<HTMLVideoElement>();

  // Background videos stop while the launcher is hidden or minimized (no CPU/GPU use in the background).
  $effect(() => {
    if (!videoEl) return;
    const v = videoEl;
    const sync = () => (document.hidden ? v.pause() : void v.play().catch(() => {}));
    document.addEventListener("visibilitychange", sync);
    return () => document.removeEventListener("visibilitychange", sync);
  });

  let layers = $state<{ key: number; src: string }[]>([]);
  let nextKey = 0;

  $effect(() => {
    const s = src;
    if (!s) {
      layers = [];
      return;
    }
    if (untrack(() => layers.at(-1)?.src) === s) return;
    let cancelled = false;
    const img = new Image();
    img.src = s;
    img
      .decode()
      .catch(() => {})
      .finally(() => {
        if (!cancelled) layers = [...untrack(() => layers).slice(-1), { key: ++nextKey, src: s }];
      });
    return () => (cancelled = true);
  });
</script>

<div class="bg" aria-hidden="true">
  {#each layers as l (l.key)}
    <img src={l.src} alt="" />
  {/each}
  {#key video}
    {#if video}
      <video bind:this={videoEl} src={video} poster={src ?? undefined} autoplay muted loop playsinline
      ></video>
    {/if}
  {/key}
  <div class="scrim"></div>
</div>

<style>
  .bg {
    position: absolute;
    inset: 0;
    overflow: hidden;
    background:
      radial-gradient(
        ellipse at 70% 30%,
        color-mix(in oklab, var(--accent) 35%, transparent),
        transparent 60%
      ),
      var(--bg-2);
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fade-in 320ms var(--ease) both;
  }
  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: fade-in 600ms var(--ease) both;
  }
  .scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgb(10 10 14 / 0.72) 0%, rgb(10 10 14 / 0.25) 38%, transparent 60%),
      linear-gradient(0deg, rgb(10 10 14 / 0.85) 0%, rgb(10 10 14 / 0.2) 35%, transparent 55%);
  }
</style>
