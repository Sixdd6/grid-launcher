<script lang="ts">
  // Q4 "Link to server game": a searchable list of the server ROMs on the
  // platform an unlinked installed row names. Choosing one links the row.
  // Same modal shape as NativeSettings (backdrop + `role="dialog"`,
  // Escape/backdrop/close-button dismissal), stacked over Details.
  //
  // Keyboard: the search box has focus on open. ArrowDown / Enter in it moves
  // into the list; ArrowUp / ArrowDown walk the rows (`moveFocus`, the same
  // clamped rule the grids use, one column wide); ArrowUp on the first row
  // returns to the search box; Enter or Space on a row links it. Tab is
  // trapped inside the dialog and focus returns to the opener on close.
  // Gamepad: `Shell.handleNav` offers every `nav` event to the open dialogs
  // first (focus/dialogNav.ts). Up / down walk the search box and the rows,
  // accept enters the list from the search box or links the focused row, back
  // closes the picker. Nothing reaches the views underneath.
  import { api, type GameSummary } from '../api';
  import Icon from '../Icon.svelte';
  import { moveFocus } from '../focus/grid';
  import { activateFocused, moveDialogFocus, registerDialog, type NavAction } from '../focus/dialogNav';
  import { linkCandidates } from './unlinked';

  let {
    title,
    platform,
    heldRomIds,
    onPick,
    onClose,
  }: {
    title: string;
    platform: string;
    /** Rom ids other installed rows hold; the list leaves them out. */
    heldRomIds: ReadonlySet<number>;
    /** Links the row. A rejection is shown in the picker. */
    onPick: (rom: GameSummary) => Promise<void>;
    onClose: () => void;
  } = $props();

  let games = $state<GameSummary[] | null>(null);
  let loadError = $state<string | null>(null);
  let query = $state('');
  let pickingId = $state<number | null>(null);
  let pickError = $state<string | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);
  let panelEl = $state<HTMLElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);
  // Roving tab stop: the list is one Tab stop (the row last focused, else the
  // first), so Tab from the search box reaches Cancel without walking every row.
  let stopId = $state<number | null>(null);

  let shown = $derived(games === null ? [] : linkCandidates(games, query, heldRomIds));
  let trimmedQuery = $derived(query.trim());
  let tabStopId = $derived(shown.some((g) => g.id === stopId) ? stopId : (shown[0]?.id ?? null));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  $effect(() => {
    let cancelled = false;
    api
      .listServerRomsForPlatform(platform)
      .then((list) => {
        if (!cancelled) games = list;
      })
      .catch((err) => {
        if (!cancelled) loadError = errorMessage(err);
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    searchEl?.focus();
  });

  // Return focus to whatever opened the dialog once it unmounts. The opener
  // can be gone by then (a successful link removes the unlinked actions), in
  // which case there is nothing to restore.
  $effect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      if (opener?.isConnected) opener.focus();
    };
  });

  async function pick(rom: GameSummary) {
    // `aria-disabled`, not `disabled`: a disabled button drops focus to the
    // page body and takes the keyboard out of the dialog mid-request.
    if (pickingId !== null) return;
    pickingId = rom.id;
    pickError = null;
    try {
      await onPick(rom);
    } catch (err) {
      pickError = errorMessage(err);
    } finally {
      pickingId = null;
    }
  }

  function rows(): HTMLButtonElement[] {
    return listEl ? Array.from(listEl.querySelectorAll<HTMLButtonElement>('button.item')) : [];
  }

  function trapTab(e: KeyboardEvent) {
    if (!panelEl) return;
    const focusable = Array.from(
      panelEl.querySelectorAll<HTMLElement>('button:not([disabled]):not([tabindex="-1"]), input:not([disabled])'),
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && (active === first || active === panelEl)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === 'Tab') {
      trapTab(e);
      return;
    }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp' && !(e.key === 'Enter' && e.target === searchEl)) return;

    const items = rows();
    if (items.length === 0) return;
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    if (current === -1) {
      // From the search box (or the panel): into the list on the way down.
      if (e.key === 'ArrowUp') return;
      e.preventDefault();
      items[0].focus();
      return;
    }
    if (e.key === 'Enter') return; // a row's own click handles Enter
    e.preventDefault();
    if (e.key === 'ArrowUp' && current === 0) {
      searchEl?.focus();
      return;
    }
    items[moveFocus(current, e.key === 'ArrowDown' ? 'down' : 'up', 1, items.length)].focus();
  }

  /** A gamepad `nav` action, routed here while the picker is open. */
  export function handleNav(action: NavAction): boolean {
    if (action === 'back') {
      onClose();
      return true;
    }
    if (action === 'accept') {
      if (document.activeElement === searchEl) {
        rows()[0]?.focus();
        return true;
      }
      return activateFocused(panelEl);
    }
    // The search box first, then the rows: the same walk the arrow keys make.
    return moveDialogFocus([...(searchEl ? [searchEl] : []), ...rows()], action);
  }

  $effect(() => registerDialog(handleNav));

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div class="backdrop" onclick={onBackdropClick} role="presentation">
  <div
    data-testid="link-picker"
    class="panel"
    bind:this={panelEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby="link-picker-title"
    aria-describedby="link-picker-hint"
    tabindex="-1"
    onkeydown={onKey}
  >
    <button data-testid="link-picker-close" class="close icon-btn" onclick={onClose} aria-label="Close">
      <Icon name="close" size={20} />
    </button>
    <h3 id="link-picker-title">Link to server game</h3>
    <p id="link-picker-hint" class="hint">
      Choose the server game that <strong>“{title}”</strong> ({platform}) is.
    </p>

    <input
      data-testid="link-picker-search"
      type="search"
      placeholder="Search title, platform or genre"
      aria-label="Search server games"
      aria-controls="link-picker-results"
      autocomplete="off"
      bind:this={searchEl}
      bind:value={query}
    />

    <div id="link-picker-results" class="results" aria-busy={games === null && loadError === null}>
      {#if loadError}
        <p data-testid="link-picker-error" class="state error" role="alert">{loadError}</p>
      {:else if games === null}
        <p class="state" role="status">Loading server games…</p>
      {:else if shown.length === 0}
        <p data-testid="link-picker-empty" class="state" role="status">
          {#if games.length === 0}
            The server has no games for this platform.
          {:else if trimmedQuery !== ''}
            No games match “{trimmedQuery}”.
          {:else}
            Every game on this platform is already linked to another row.
          {/if}
        </p>
      {:else}
        <p class="sr-only" role="status">{shown.length} {shown.length === 1 ? 'game' : 'games'}</p>
        <ul class="list" aria-label="Server games" bind:this={listEl}>
          {#each shown as rom (rom.id)}
            <li>
              <button
                data-testid="link-picker-item"
                data-rom-id={rom.id}
                class="item"
                aria-disabled={pickingId !== null}
                tabindex={rom.id === tabStopId ? 0 : -1}
                onfocus={() => (stopId = rom.id)}
                onclick={() => pick(rom)}
              >
                <span class="name">{rom.name}</span>
                {#if pickingId === rom.id}
                  <span class="meta linking">Linking…</span>
                {:else if rom.platform_display_name}
                  <span class="meta">{rom.platform_display_name}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    {#if pickError}
      <p data-testid="link-picker-pick-error" class="error" role="alert">{pickError}</p>
    {/if}

    <div class="actions">
      <button data-testid="link-picker-cancel" class="btn secondary" onclick={onClose}>Cancel</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: grid;
    place-items: center;
    z-index: 25;
  }

  /* A fixed height (capped by the window) so the dialog does not jump in size
     as the search narrows the list. The results region takes the slack. */
  .panel {
    position: relative;
    width: min(480px, calc(100vw - 48px));
    height: min(560px, calc(100vh - 48px));
    box-sizing: border-box;
    padding: 24px;
    border-radius: 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .panel:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--text);
  }

  h3 {
    margin: 0;
    padding-right: 28px;
    color: var(--text-h);
    font-size: 16px;
  }

  input {
    flex: none;
    font: inherit;
    padding: 9px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-h);
  }

  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .results {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--r-row);
    background: var(--surface-2);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .list li + li {
    border-top: 1px solid var(--border);
  }

  .item {
    width: 100%;
    min-height: 44px;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    text-align: left;
    font: inherit;
    padding: 8px 12px;
    border: none;
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .item:hover,
  .item:focus-visible {
    background: var(--surface);
  }

  .item:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .item[aria-disabled='true'] {
    opacity: 0.6;
    cursor: default;
  }

  .name {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .meta {
    flex: none;
    max-width: 45%;
    font-size: 12px;
    color: var(--text-muted);
    text-align: right;
  }

  .meta.linking {
    color: var(--primary);
    font-weight: 600;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .hint strong {
    color: var(--text-h);
    font-weight: 600;
  }

  /* Loading / empty / load-error line: centred in the results box. */
  .state {
    margin: auto;
    padding: 16px;
    font-size: 13px;
    color: var(--text-muted);
    text-align: center;
    overflow-wrap: anywhere;
  }

  .error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .state.error {
    color: var(--danger);
  }

  .actions {
    flex: none;
    display: flex;
    justify-content: flex-end;
  }

  .btn {
    font: inherit;
    padding: 8px 16px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    transition: background var(--m-fast) ease;
  }

  .btn:hover {
    background: var(--surface);
  }

  .btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
