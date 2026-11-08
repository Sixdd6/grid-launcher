<script lang="ts">
  // The emulator line of an open Systems row (or an Other emulators entry):
  // name, version, which other systems it plays, Update (only when one is
  // out), Open, and the ⋯ button that opens its menu.
  import Icon from '../Icon.svelte';

  let {
    name,
    testKey,
    version,
    updateTo = null,
    note = '',
    updating = false,
    launching = false,
    menuOpen = false,
    onUpdate,
    onOpen,
    onMore,
  }: {
    name: string;
    testKey: string;
    /** "v2.0.2 · up to date", or just the version when unchecked. */
    version: string;
    /** The newer version's label, when an update is out. */
    updateTo?: string | null;
    /** "Also plays …", or blank. */
    note?: string;
    updating?: boolean;
    launching?: boolean;
    menuOpen?: boolean;
    onUpdate: () => void;
    onOpen: () => void;
    onMore: (button: HTMLElement) => void;
  } = $props();

  let moreEl = $state<HTMLElement | null>(null);
</script>

<div class="emu-line" data-testid={`sys-emu-${testKey}`}>
  <span class="emu-text">
    <b data-testid={`sys-emu-name-${testKey}`}>{name}</b>
    {#if version}<span class="muted-s" data-testid={`sys-emu-version-${testKey}`}>{version}</span>{/if}
    {#if note}<span class="muted-s" data-testid={`sys-emu-also-${testKey}`}>· {note}</span>{/if}
  </span>
  <span class="acts">
    {#if updateTo !== null}
      <button class="small-btn primary" data-testid={`sys-update-${testKey}`} disabled={updating} onclick={onUpdate}>
        {updating ? 'Updating…' : `Update to ${updateTo}`}
      </button>
    {/if}
    <button
      class="small-btn"
      data-testid={`sys-open-${testKey}`}
      title={`Opens ${name} on its own, to change its settings`}
      disabled={launching}
      onclick={onOpen}
    >
      {launching ? 'Opening…' : 'Open'}
    </button>
    <button
      bind:this={moreEl}
      class="icon-btn"
      data-testid={`sys-more-${testKey}`}
      aria-label={`More for ${name}`}
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onclick={() => moreEl && onMore(moreEl)}
    >
      <Icon name="more" size={16} />
    </button>
  </span>
</div>

<style>
  .emu-line {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .emu-text {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .emu-text b {
    color: var(--text-h);
    font-size: 13px;
  }

  .muted-s {
    font-size: 12px;
    color: var(--text-muted);
  }

  .acts {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }

  .small-btn {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-chip);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
    white-space: nowrap;
  }

  .small-btn.primary {
    border-color: var(--primary);
    background: var(--primary);
    color: var(--on-primary);
  }

  .small-btn:hover:not(:disabled) {
    background: var(--surface);
  }

  .small-btn.primary:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .small-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid transparent;
    border-radius: var(--r-control);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .icon-btn:hover,
  .icon-btn[aria-expanded='true'] {
    background: var(--surface);
    color: var(--text-h);
  }
</style>
