<script lang="ts">
  // The media viewer's face (items 303–305; tvmode.css `.viewer`): black,
  // fullscreen down to the hint bar. A screenshot sits at 70% of the screen
  // height with "(LB) Screenshot <i> of <n> · <title> (RB)" under it.
  //
  // A video — a server video, or a trailer in the YouTube player (the
  // loopback relay, youtube_relay.rs) — plays in place (user decision
  // 2026-11-10): A plays and pauses, LB / RB seek 10 s, LT / RT step its own
  // volume, Left / Right page. It is centred and as large as the screen
  // allows at 16:9, its width or its height filling the space (user
  // decision 2026-11-11). Nothing may be drawn over YouTube's player, so
  // notification cards wait while a video shows (`holdNotices`), and the
  // "Volume N%" readout sits under it, in the caption line. "(X) Watch on
  // YouTube" stays under a trailer's player. When the
  // player can't play the trailer (an error, or no ready signal in 10 s) the
  // poster comes back and A opens the trailer in the browser.
  import { untrack } from 'svelte';
  import Image from '../../Image.svelte';
  import { trailerPoster, videoLoadMessage } from '../../details/media';
  import { fromElementVolume, toElementVolume, volumeLabel } from '../../details/videoVolume';
  import YoutubeEmbed from '../../details/YoutubeEmbed.svelte';
  import { initialEmbed } from '../../details/youtubeRelay';
  import { api } from '../../api';
  import { setVideoVolume, uiSettings } from '../../stores/uiSettings.svelte';
  import { tvf } from '../focus/actions';
  import { holdNotices } from '../notices.svelte';
  import Glyph from '../glyphs/Glyph.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { viewerCaption } from './details';
  import { pausedOf, showOf, toggleVideo, type ViewerModel } from './mediaViewer.svelte';

  let { model }: { model: ViewerModel } = $props();

  let item = $derived(model.items[model.index] ?? null);
  let caption = $derived(viewerCaption(model.items, model.index, model.title));
  let show = $derived(showOf(model));
  let ratio = $state<Record<string, number>>({});
  let thumbFailed = $state(false);
  let srEl = $state<HTMLButtonElement | null>(null);

  // A server video's playable URL (the desktop's `video_url`).
  let videoSrc = $state<string | null>(null);
  let videoError = $state<string | null>(null);
  $effect(() => {
    const it = item;
    videoSrc = null;
    videoError = null;
    thumbFailed = false;
    if (!it || it.kind !== 'video') return;
    let cancelled = false;
    api.videoUrl(it.url).then(
      (src) => {
        if (!cancelled) videoSrc = src;
      },
      (e: unknown) => {
        if (!cancelled) videoError = videoLoadMessage(e instanceof Error ? e.message : String(e));
      },
    );
    return () => {
      cancelled = true;
    };
  });

  // Each item starts its own player: a trailer after one that fell back
  // gets its own try.
  let itemKey = $derived(item === null ? '' : item.kind === 'youtube' ? `yt:${item.videoId}` : `${item.kind}:${item.url}`);
  $effect(() => {
    void itemKey;
    model.embed = initialEmbed();
  });

  // A server video opens at the stored level.
  $effect(() => {
    const v = model.video;
    if (v) v.volume = toElementVolume(untrack(() => uiSettings.videoVolume));
  });

  /** A screenshot that fails leaves the sequence, as in the desktop viewer. */
  function failed(url: string) {
    model.onFail(url);
    const i = model.items.findIndex((m) => m.kind === 'screenshot' && m.url === url);
    if (i < 0) return;
    model.items = model.items.filter((_, n) => n !== i);
    if (model.index >= model.items.length) model.index = Math.max(0, model.items.length - 1);
  }

  function activate() {
    if (item?.kind === 'video' || item?.kind === 'youtube') toggleVideo(model);
  }

  // Cards wait while a video shows: nothing may cover the player.
  // `untrack`: holdNotices reads the held flag it writes, and tracked, that
  // read made this effect re-run itself without end.
  $effect(() => {
    if (show !== 'video') return;
    untrack(() => holdNotices(true));
    return () => untrack(() => holdNotices(false));
  });

  let aLabel = $derived(show === 'video' ? (pausedOf(model) ? 'Play' : 'Pause') : show === 'poster' ? 'Watch on YouTube' : 'Select');
</script>

<div class="viewer" class:vid={show === 'video'} class:yt={item?.kind === 'youtube'} data-testid="tv-viewer" data-show={show}>
  {#if item}
    <div class="col">
      {#if item.kind === 'screenshot'}
        <div class="shot" style={`aspect-ratio:${ratio[item.url] ?? 16 / 9}`} data-testid="tv-viewer-shot">
          <Image
            url={item.url}
            alt={item.caption}
            placeholder="Screenshot"
            onmeasure={(w, h) => (ratio = { ...ratio, [item.url]: w / h })}
            onerror={() => failed(item.url)}
          />
        </div>
      {:else if item.kind === 'youtube'}
        {#if show === 'video'}
          <div
            class="shot player"
            data-testid="tv-viewer-trailer"
            data-phase={model.embed?.phase}
            data-linked={model.embed?.linked ? 'true' : 'false'}
          >
            {#key item.videoId}
              <YoutubeEmbed
                bind:this={model.yt}
                bind:embed={model.embed}
                videoId={item.videoId}
                volume={uiSettings.videoVolume}
                onVolume={(v) => setVideoVolume(v)}
                onFocusLost={() => srEl?.focus()}
                testid="tv-viewer-embed"
              />
            {/key}
          </div>
        {:else}
          {@const poster = trailerPoster(item.videoId, model.cover, thumbFailed)}
          <div
            class="shot"
            style="aspect-ratio:16/9"
            data-testid="tv-viewer-trailer"
            data-phase={model.embed?.phase}
            data-reason={model.embed?.reason}
            data-linked={model.embed?.linked ? 'true' : 'false'}
          >
            {#if poster.kind === 'youtube'}
              <img src={poster.url} alt="" referrerpolicy="no-referrer" draggable="false" onerror={() => (thumbFailed = true)} />
            {:else}
              <Image url={poster.url} alt="" placeholder="Trailer" />
            {/if}
            <span class="play"><TvIcon name="play" px={64} /></span>
          </div>
          <div class="muted" data-testid="tv-viewer-trailer-note">This trailer can't play in GRID.</div>
        {/if}
        <!-- Always under the player or the poster, never over it. -->
        <button class="watch" data-btn="X" data-testid="tv-viewer-watch" tabindex="-1"><Glyph button="X" />Watch on YouTube</button>
      {:else}
        <div class="shot player" data-testid="tv-viewer-video">
          {#if videoSrc}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video
              src={videoSrc}
              autoplay
              controls
              tabindex="-1"
              bind:this={model.video}
              onplay={() => (model.paused = false)}
              onpause={() => (model.paused = true)}
              onvolumechange={(e) => setVideoVolume(fromElementVolume((e.currentTarget as HTMLVideoElement).volume))}
              data-testid="tv-viewer-videoel"
            ></video>
          {:else if videoError}
            <span class="verr">{videoError}</span>
          {/if}
        </div>
      {/if}
      <div class="cap">
        {#if show === 'video'}<Glyph button="DPAD" />{:else}<Glyph button="LB" />{/if}<span data-testid="tv-viewer-caption">{caption}</span>{#if show !== 'video'}<Glyph button="RB" />{/if}
        {#if show === 'video'}
          <!-- Under the player, never over it. -->
          <span class="vol" class:on={model.readout} aria-live="polite" data-testid={model.readout ? 'tv-viewer-volume' : undefined}>{model.readout ? volumeLabel(uiSettings.videoVolume) : ''}</span>
        {/if}
      </div>
    </div>
  {/if}
  <!-- The one focusable item, so focus stays inside the viewer. -->
  <button class="sr" data-k="viewer" data-al={aLabel} bind:this={srEl} use:tvf={{ a: activate }}>media</button>
</div>
