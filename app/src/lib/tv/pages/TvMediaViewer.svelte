<script lang="ts">
  // The media viewer's face (items 303–305; tvmode.css `.viewer`): black,
  // fullscreen down to the hint bar, the picture at 70% of the screen
  // height, and "(LB) Screenshot <i> of <n> · <title> (RB)" ("Trailer ·
  // <title>" for a video) under it. A trailer can't play inside GRID (the
  // desktop's reason: YouTube refuses embeds from the app), so A on it opens
  // it in the browser; a server video plays in place, A pausing and playing
  // it and LT / RT seeking 10 s (ruling E6; the controls stay for a mouse).
  import { api } from '../../api';
  import Image from '../../Image.svelte';
  import { trailerPoster, videoLoadMessage } from '../../details/media';
  import { pushToast } from '../../stores/toasts.svelte';
  import { tvf } from '../focus/actions';
  import Glyph from '../glyphs/Glyph.svelte';
  import TvIcon from '../ui/TvIcon.svelte';
  import { viewerCaption } from './details';
  import { toggleVideo, type ViewerModel } from './mediaViewer.svelte';

  let { model }: { model: ViewerModel } = $props();

  let item = $derived(model.items[model.index] ?? null);
  let caption = $derived(viewerCaption(model.items, model.index, model.title));
  let ratio = $state<Record<string, number>>({});
  let thumbFailed = $state(false);

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

  /** A screenshot that fails leaves the sequence, as in the desktop viewer. */
  function failed(url: string) {
    model.onFail(url);
    const i = model.items.findIndex((m) => m.kind === 'screenshot' && m.url === url);
    if (i < 0) return;
    model.items = model.items.filter((_, n) => n !== i);
    if (model.index >= model.items.length) model.index = Math.max(0, model.items.length - 1);
  }

  function activate() {
    // A server video: A pauses and plays it (ruling E6).
    if (item?.kind === 'video') toggleVideo(model);
    else if (item?.kind === 'youtube') api.openYoutubeVideo(item.videoId).catch((e: unknown) => pushToast(e instanceof Error ? e.message : String(e), 'error'));
  }
</script>

<div class="viewer" data-testid="tv-viewer">
  {#if item}
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
      {@const poster = trailerPoster(item.videoId, model.cover, thumbFailed)}
      <div class="shot" style="aspect-ratio:16/9" data-testid="tv-viewer-trailer">
        {#if poster.kind === 'youtube'}
          <img src={poster.url} alt="" referrerpolicy="no-referrer" draggable="false" onerror={() => (thumbFailed = true)} />
        {:else}
          <Image url={poster.url} alt="" placeholder="Trailer" />
        {/if}
        <span class="play"><TvIcon name="play" px={64} /></span>
      </div>
      <div class="muted">Trailers open in your browser.</div>
    {:else}
      <div class="shot" style="aspect-ratio:16/9" data-testid="tv-viewer-video">
        {#if videoSrc}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video src={videoSrc} autoplay controls bind:this={model.video} bind:paused={model.paused} data-testid="tv-viewer-videoel"></video>
        {:else if videoError}
          <span class="verr">{videoError}</span>
        {/if}
      </div>
    {/if}
    <div class="cap"><Glyph button="LB" /><span data-testid="tv-viewer-caption">{caption}</span><Glyph button="RB" /></div>
  {/if}
  <!-- The one focusable item, so focus stays inside the viewer; A on a
       trailer opens it in the browser. -->
  <button class="sr" data-k="viewer" data-al={item?.kind === 'youtube' ? 'Watch on YouTube' : item?.kind === 'video' ? (model.paused ? 'Play' : 'Pause') : 'Select'} use:tvf={{ a: activate }}>media</button>
</div>
