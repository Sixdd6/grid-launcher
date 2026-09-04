<script lang="ts">
  import type { InstalledGame } from './api';
  import { installed } from './stores/installed.svelte';
  import { updates } from './stores/updates.svelte';
  import { visibleLibraryGames } from './library';
  import { fromInstalled, type DetailsSubject } from './details/subject';
  import Image from './Image.svelte';
  import Details from './Details.svelte';
  import { moveFocus, type NavDirection } from './focus/grid';
  import { createHoverViewed } from './lastViewedHover';
  import { noteViewed } from './stores/lastViewed.svelte';

  let { active }: { active: boolean } = $props();

  const COLUMNS = 6;
  let rows = $derived(visibleLibraryGames(installed.list));
  let focusIndex = $state(0);
  let gridEl = $state<HTMLElement | null>(null);
  let subject = $state<DetailsSubject | null>(null);

  function openDetails(row: InstalledGame) {
    subject = fromInstalled(row);
    noteViewed(row.cover_large_path);
  }

  function closeDetails() {
    subject = null;
  }

  // Design §3: a card becomes the background only after the pointer has
  // rested on it for more than half a second.
  const hover = createHoverViewed();

  export function handleNav(action: NavDirection | 'accept' | 'back') {
    if (action === 'back') {
      if (subject) closeDetails();
      return;
    }
    if (action === 'accept') {
      if (!subject) {
        const row = rows[focusIndex];
        if (row) openDetails(row);
      }
      return;
    }
    if (subject) return; // grid navigation is suspended while the overlay is open
    focusIndex = moveFocus(focusIndex, action, COLUMNS, rows.length);
    gridEl?.children[focusIndex]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function onKey(e: KeyboardEvent) {
    if (!active) return;
    const map: Record<string, NavDirection> = {
      ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    };
    const action = map[e.key];
    if (action) {
      e.preventDefault();
      handleNav(action);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<section data-testid="library-section">
  {#if rows.length === 0}
    <p data-testid="library-empty" class="empty">No games installed</p>
  {:else}
    <div class="grid" bind:this={gridEl} style="--columns: {COLUMNS}">
      {#each rows as row, i (row.rom_id ?? `x-${i}`)}
        <div
          data-testid={`library-card-${row.rom_id ?? `x-${i}`}`}
          class="card"
          class:focused={i === focusIndex}
          onclick={() => openDetails(row)}
          onmouseenter={() => hover.start(row.cover_large_path)}
          onmouseleave={hover.end}
          role="presentation"
        >
          <Image url={row.cover_small_path || null} alt={row.title} placeholder="No cover" />
          <div class="caption">
            <span class="title">{row.title}</span>
            <span class="platform">{row.platform}</span>
            {#if updates.labelFor(row.rom_id) !== null}
              <span data-testid={`library-update-badge-${row.rom_id}`} class="update-badge">Update Available</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>

{#if subject}
  {#key subject.romId}
    <Details {subject} onClose={closeDetails} onLibraryPathUnset={() => {}} />
  {/key}
{/if}

<style>
  .empty {
    padding: 40px 24px;
    color: var(--text);
    font-size: 14px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(var(--columns), 1fr);
    gap: 16px;
    /* Extra bottom padding keeps the last row clear of the fixed downloads footer. */
    padding: 24px;
  }
  .card {
    position: relative;
    aspect-ratio: 3 / 4;
    /* Off-screen cards skip layout/paint; the intrinsic size keeps the
       scrollbar stable at the 3:4 cover ratio. */
    content-visibility: auto;
    contain-intrinsic-size: auto 200px 267px;
    border-radius: 8px;
    overflow: hidden;
    transform: scale(1);
    transition: transform 160ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
    will-change: transform;
    cursor: pointer;
  }
  .card.focused {
    transform: scale(1.08);
    outline: 3px solid #7aa2ff;
    z-index: 1;
  }
  .card :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .caption {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.75), rgba(0, 0, 0, 0));
    color: #fff;
  }

  .caption .title {
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .caption .platform {
    font-size: 10px;
    opacity: 0.75;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Warning tone (the app's amber, as in CloudPanel's `.warn`) rather than
     the accent: an available update is a notice, not the card's identity. */
  .caption .update-badge {
    font-size: 12px;
    font-weight: 600;
    color: #e5a53a;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
