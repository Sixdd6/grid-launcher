<script lang="ts">
  // An emulator's ⋯ menu (ProposedApp README, "Opening a row"): Settings…,
  // Show program folder, Remove (two clicks). Fixed to the window and placed
  // by `placeMenu` (below, right-aligned, when it fits; beside the button
  // near the bottom edge). A fixed menu would drift from its button as the
  // list scrolls, so any scroll closes it, as do Escape and a click outside.
  import { tick } from 'svelte';
  import { placeMenu, type MenuPlacement } from './menuPlacement';

  let {
    anchor,
    name,
    removing = false,
    onSettings,
    onShowFolder,
    onRemove,
    onClose,
  }: {
    /** The ⋯ button the menu belongs to. */
    anchor: HTMLElement;
    name: string;
    removing?: boolean;
    onSettings: () => void;
    onShowFolder: () => void;
    onRemove: () => void;
    onClose: () => void;
  } = $props();

  let menuEl = $state<HTMLElement | null>(null);
  let placement = $state<MenuPlacement | null>(null);
  let armed = $state(false);

  function place() {
    if (!menuEl) return;
    const b = anchor.getBoundingClientRect();
    placement = placeMenu(
      { top: b.top, left: b.left, right: b.right, bottom: b.bottom },
      { width: menuEl.offsetWidth, height: menuEl.offsetHeight },
      { top: 0, left: 0, right: window.innerWidth, bottom: window.innerHeight },
    );
  }

  $effect(() => {
    void tick().then(place);
  });

  function onScroll(e: Event) {
    // Scrolling inside the menu itself (it never does today) is not the list.
    if (menuEl && e.target instanceof Node && menuEl.contains(e.target)) return;
    onClose();
  }

  function onPointerDown(e: PointerEvent) {
    const target = e.target as Node | null;
    if (target === null) return;
    if (menuEl?.contains(target) || anchor.contains(target)) return;
    onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
      anchor.focus();
    }
  }

  function remove() {
    if (!armed) {
      armed = true;
      return;
    }
    onRemove();
  }

  $effect(() => {
    // Capture: a scroll on any scroller (the Settings pane) reaches here.
    window.addEventListener('scroll', onScroll, true);
    return () => window.removeEventListener('scroll', onScroll, true);
  });
</script>

<svelte:window onresize={place} onpointerdown={onPointerDown} onkeydown={onKeydown} />

<div
  bind:this={menuEl}
  class="row-menu"
  role="menu"
  aria-label={`More for ${name}`}
  data-testid="sys-menu"
  data-side={placement?.side ?? ''}
  style:top={placement ? `${placement.top}px` : '0px'}
  style:left={placement ? `${placement.left}px` : '0px'}
  style:visibility={placement ? 'visible' : 'hidden'}
>
  <button role="menuitem" data-testid="sys-menu-settings" onclick={onSettings}>Settings…</button>
  <button role="menuitem" data-testid="sys-menu-folder" onclick={onShowFolder}>Show program folder</button>
  <button
    role="menuitem"
    class="danger"
    class:armed
    data-testid="sys-menu-remove"
    disabled={removing}
    onclick={remove}
  >
    {#if removing}
      Removing…
    {:else if armed}
      Remove {name}? Games and saves stay
    {:else}
      Remove {name}…
    {/if}
  </button>
</div>

<style>
  .row-menu {
    position: fixed;
    z-index: 25;
    min-width: 190px;
    max-width: 320px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border-radius: var(--r-control);
    border: 1px solid var(--line-strong);
    background: var(--overlay);
    box-shadow: var(--shadow-overlay);
  }

  button {
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: var(--r-chip);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--surface);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .danger {
    color: var(--danger);
  }

  .danger.armed {
    background: var(--danger);
    color: var(--on-danger);
  }
</style>
