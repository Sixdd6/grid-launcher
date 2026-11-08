<script lang="ts" module>
  import { api } from '../api';

  /** Shades already asked for, by screenshot url: one backend call per
   *  image per session, however often a game comes round. */
  const shades = new Map<string, Promise<[number, number, number]>>();

  function shadeOf(url: string): Promise<[number, number, number]> {
    let pending = shades.get(url);
    if (!pending) {
      pending = api.imageDominantShade(url);
      // A failure is not remembered: the next showing asks again.
      pending.catch(() => shades.delete(url));
      shades.set(url, pending);
    }
    return pending;
  }
</script>

<script lang="ts">
  // Logo, never plain text (ProposedApp README): a game's server clear logo
  // when it has one; otherwise its title drawn in one of three logo styles,
  // coloured from its screenshot's dominant shade.
  import Image from '../Image.svelte';
  import { logoColours, logoStyle } from './feed';

  let {
    id,
    title,
    logoUrl,
    shotUrl,
  }: {
    id: number;
    title: string;
    /** The server's clear logo; `''` when none. */
    logoUrl: string;
    /** The picture the logo sits on, sampled for the generated colours. */
    shotUrl: string | null;
  } = $props();

  /** The server logo failed to load: draw the title instead. */
  let logoFailed = $state(false);
  let colours = $state<{ letters: string; shadow: string } | null>(null);

  let generated = $derived(logoUrl === '' || logoFailed);

  $effect(() => {
    void logoUrl;
    logoFailed = false;
  });

  $effect(() => {
    const url = shotUrl;
    colours = null;
    if (!generated || !url) return;
    let cancelled = false;
    shadeOf(url)
      .then((shade) => {
        if (!cancelled) colours = logoColours(shade);
      })
      .catch(() => {
        // No shade: the style's own default colours stay.
      });
    return () => {
      cancelled = true;
    };
  });
</script>

{#if generated}
  <div
    data-testid="discover-logo"
    data-generated="true"
    class="logo logo-{logoStyle(id)}"
    role="img"
    aria-label={title}
    style:--lf={colours?.letters}
    style:--ls={colours?.shadow}
  >
    {title}
  </div>
{:else}
  <div data-testid="discover-logo" data-generated="false" class="server-logo" role="img" aria-label={title}>
    <Image url={logoUrl} alt={title} class="server-logo-img" onerror={() => (logoFailed = true)} />
  </div>
{/if}

<style>
  .server-logo {
    max-width: 55%;
    /* cqw: the frame (Shot.svelte) is the size container, so a small window
       gets a smaller logo instead of a wrapped one. */
    height: clamp(40px, 15cqw, 96px);
    display: flex;
    align-items: flex-end;
  }

  .server-logo :global(.server-logo-img) {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 2px 8px rgba(0, 0, 0, 0.5));
  }

  .server-logo :global(.skeleton) {
    width: 160px;
    height: 48px;
    border-radius: var(--r-row);
    opacity: 0.4;
  }

  .logo {
    max-width: 55%;
    line-height: 1;
    color: #fff;
    overflow-wrap: break-word;
  }

  /* --lf: letters, --ls: outline and shadow, both from the screenshot's
     dominant shade once it is known; each style's own colours until then. */
  .logo-a {
    font: italic 700 clamp(18px, 5.2cqw, 34px)/1 Georgia, 'Times New Roman', serif;
    color: var(--lf, #f6d77a);
    text-shadow: 0 2px 0 var(--ls, #6b4a0e), 0 4px 12px rgba(0, 0, 0, 0.6);
  }

  .logo-b {
    font: 900 clamp(16px, 4.6cqw, 30px)/0.95 system-ui, sans-serif;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--lf, #fff);
    -webkit-text-stroke: 1px rgba(0, 0, 0, 0.6);
    text-shadow: 3px 3px 0 var(--ls, #553e98), 0 4px 14px rgba(0, 0, 0, 0.6);
  }

  .logo-c {
    font: 800 clamp(17px, 4.9cqw, 32px)/1 system-ui, sans-serif;
    letter-spacing: -0.01em;
    color: var(--lf, #eafcff);
    text-shadow: 0 0 1px var(--ls, #0b3b46), 0 3px 0 var(--ls, #0b3b46), 0 5px 14px rgba(0, 0, 0, 0.55);
  }
</style>
