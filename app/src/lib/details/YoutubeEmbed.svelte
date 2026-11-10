<script lang="ts">
  // A trailer, played inside GRID through the loopback relay page
  // (youtube_relay.rs; protocol and fallback in youtubeRelay.ts). The parent
  // viewer lays it out — nothing may overlap it (YouTube's rules) — and
  // shows the poster and "Watch on YouTube" once `embed.phase` is
  // `fallback`: an error from YouTube, no relay, or no ready signal within
  // READY_TIMEOUT_MS.
  //
  // The iframe never keeps keyboard focus: a click inside it hands focus
  // straight back (`onFocusLost`), so Esc, the arrows and TV mode's keys
  // still reach the app. Controllers don't depend on it (TV mode reads them
  // natively).
  import { untrack } from 'svelte';
  import { api } from '../api';
  import { clampVolume } from './videoVolume';
  import {
    acceptRelayMessage,
    embedStep,
    initialEmbed,
    READY_TIMEOUT_MS,
    relayMessage,
    relayOrigin,
    type EmbedState,
    type RelayCommand,
  } from './youtubeRelay';

  let {
    videoId,
    volume,
    embed = $bindable(initialEmbed()),
    onVolume,
    onFocusLost,
    testid = 'youtube-embed',
  }: {
    videoId: string;
    /** The level the player opens at (`ui.video_volume`). Read once per
     *  video; later changes go through `send({ cmd: 'volume' })`. */
    volume: number;
    embed?: EmbedState;
    /** The player's own volume changed (its controls, or a command). */
    onVolume?: (value: number) => void;
    /** The iframe took keyboard focus; the parent takes it back. */
    onFocusLost?: () => void;
    testid?: string;
  } = $props();

  let iframe = $state<HTMLIFrameElement | null>(null);
  let src = $state<string | null>(null);
  let origin: string | null = null;

  $effect(() => {
    const id = videoId;
    const startVolume = clampVolume(untrack(() => volume));
    embed = initialEmbed();
    src = null;
    origin = null;
    let cancelled = false;
    const step = (input: Parameters<typeof embedStep>[1]) => {
      if (!cancelled) embed = embedStep(untrack(() => embed), input);
    };
    const timer = setTimeout(() => step({ type: 'timeout' }), READY_TIMEOUT_MS);
    api.youtubeEmbedUrl(id, startVolume).then(
      (url) => {
        if (cancelled) return;
        const o = relayOrigin(url);
        if (o === null) {
          step({ type: 'urlError' });
          return;
        }
        origin = o;
        src = url;
        step({ type: 'url' });
      },
      () => step({ type: 'urlError' }),
    );
    const onMessage = (e: MessageEvent) => {
      if (cancelled) return;
      const ev = acceptRelayMessage(e, { origin, source: iframe?.contentWindow ?? null });
      if (ev === null) return;
      if (ev.type === 'boot') {
        send({ cmd: 'hello' });
        return;
      }
      if (ev.type === 'volume') onVolume?.(ev.value);
      if (ev.type === 'ready') clearTimeout(timer);
      step(ev);
    };
    window.addEventListener('message', onMessage);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      window.removeEventListener('message', onMessage);
    };
  });

  // A click inside the player moves DOM focus into the iframe, and key
  // events stop reaching the app. Take it back once the click is through.
  $effect(() => {
    const onBlur = () => {
      setTimeout(() => {
        if (iframe !== null && document.activeElement === iframe) onFocusLost?.();
      }, 0);
    };
    window.addEventListener('blur', onBlur);
    return () => window.removeEventListener('blur', onBlur);
  });

  /** One command to the relay; dropped until the relay has loaded. */
  export function send(c: RelayCommand): void {
    const w = iframe?.contentWindow;
    if (!w || origin === null) return;
    w.postMessage(relayMessage(c), origin);
  }
</script>

{#if src !== null && embed.phase !== 'fallback'}
  <iframe
    bind:this={iframe}
    class="yt-embed"
    {src}
    title="Trailer"
    allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
    allowfullscreen
    referrerpolicy="no-referrer"
    tabindex="-1"
    data-testid={testid}
    data-phase={embed.phase}
  ></iframe>
{/if}

<style>
  .yt-embed {
    display: block;
    width: 100%;
    height: 100%;
    border: 0;
    background: #000;
  }
</style>
