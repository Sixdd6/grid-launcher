<script lang="ts">
  // A cover (items 15–18, 20, 130): 3:4 at 10rem (17rem on Details), the
  // title under it in grids, and the desktop badges: installed dot, UPDATE,
  // MISSING (which replaces UPDATE), a platform tag in mixed lists, and
  // "Queued" / "<n>%" while installing. Focused, it grows to 107% with a lift
  // and a ring, and the background art follows it (item 11). A opens
  // Details; X is its quick action, which `quick` names (none: no X hint).
  import Image from '../../Image.svelte';
  import { noteViewed } from '../../stores/lastViewed.svelte';
  import type { BackgroundSubject } from '../../background';
  import { tvf } from '../focus/actions';
  import TvIcon from './TvIcon.svelte';

  let {
    k,
    title,
    cover,
    installed = false,
    update = false,
    missing = false,
    platform = null,
    queue = null,
    picked = false,
    sel = undefined,
    label = true,
    width = undefined,
    art = null,
    quick = null,
    testId = undefined,
    onA,
    onX = undefined,
    onFocus = undefined,
  }: {
    k: string;
    title: string;
    cover: string | null;
    installed?: boolean;
    update?: boolean;
    missing?: boolean;
    /** The platform tag, in mixed lists. */
    platform?: string | null;
    /** "Queued" or "<n>%" while installing. */
    queue?: string | null;
    /** Picked in select mode (item 441). */
    picked?: boolean;
    /** The id select mode picks it by, on a grid that has select mode. */
    sel?: string;
    /** The title under the cover (grids); rows and Details show none. */
    label?: boolean;
    /** A width other than the 10rem cover (Details: 17rem). */
    width?: string;
    /** The art the background shows while it is focused. */
    art?: BackgroundSubject | null;
    /** X's label (Play, Install, Install from drive), or null for none. */
    quick?: string | null;
    testId?: string;
    onA: () => void;
    onX?: () => void;
    /** It took focus (Discover's hero follows it). */
    onFocus?: () => void;
  } = $props();
</script>

<button
  class="cv"
  class:pk={picked}
  data-k={k}
  data-al="Details"
  data-xl={quick ?? undefined}
  data-cover
  data-sel={sel}
  data-testid={testId}
  style={width ? `--cw:${width}` : undefined}
  use:tvf={{ a: onA, x: quick ? onX : undefined, focus: () => {
      if (art) noteViewed(art);
      onFocus?.();
    } }}
>
  <span class="cv-art">
    <span class="cv-img"><Image url={cover} alt={title} placeholder={title} /></span>
    {#if missing}<span class="tag miss">MISSING</span>{:else if update}<span class="tag upd">UPDATE</span>{/if}
    {#if installed}<i class="idot" aria-label="Installed"></i>{/if}
    {#if platform}<span class="tag plat">{platform}</span>{/if}
    {#if queue}<span class="tag q">{queue}</span>{/if}
    {#if picked}<span class="pick" aria-label="Selected"><TvIcon name="check" px={16} /></span>{/if}
  </span>
  {#if label}<span class="cv-l">{title}</span>{/if}
</button>
