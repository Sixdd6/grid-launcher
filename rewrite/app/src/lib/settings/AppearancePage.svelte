<script lang="ts">
  // Settings › Appearance (design §10): theme, card size defaults, background
  // art on/off, the fade slider 0–60% and the blur slider 0–40. The art reads
  // the same store, so the FADE slider previews on `oninput` and persists on
  // `onchange`. The BLUR slider persists on `onchange` only: each sigma is a
  // separate variant the backend builds and caches, so a live drag preview
  // would build an image per intermediate position.
  import {
    commitBackgroundBlur,
    commitBackgroundFade,
    previewBackgroundFade,
    setBackgroundEnabled,
    setCardSize,
    setTheme,
    uiSettings,
  } from '../stores/uiSettings.svelte';
  import { BLUR_MAX, FADE_MAX, type ThemeChoice } from '../theme';
  import { CARD_SIZES, cardSizeLabel, normalizeCardSize } from '../cards/size';
  import { backgroundEnabled, CARD_SIZE_VIEWS } from './appearance';

  function onThemeChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as ThemeChoice;
    setTheme(value).catch(() => {
      // The attribute is already applied; a failed save is not worth a
      // blocking error in a settings pane.
    });
  }

  function onToggle(e: Event) {
    setBackgroundEnabled((e.currentTarget as HTMLInputElement).checked).catch(() => {});
  }

  function onCardSize(view: 'library' | 'server', e: Event) {
    setCardSize(view, normalizeCardSize((e.currentTarget as HTMLSelectElement).value)).catch(() => {});
  }

  function sizeFor(view: 'library' | 'server') {
    return view === 'library' ? uiSettings.cardSizeLibrary : uiSettings.cardSizeServer;
  }
</script>

<div class="field">
  <label for="theme-select">Theme</label>
  <select data-testid="theme-select" id="theme-select" value={uiSettings.theme} onchange={onThemeChange}>
    <option value="system">Follow system</option>
    <option value="dark">Dark</option>
    <option value="light">Light</option>
  </select>
</div>

<div class="field">
  <label for="background-art-toggle">Background art</label>
  <input
    data-testid="background-art-toggle"
    id="background-art-toggle"
    type="checkbox"
    checked={backgroundEnabled(uiSettings.backgroundFade)}
    onchange={onToggle}
  />
</div>

<div class="field">
  <label for="background-fade">Background art fade</label>
  <input
    data-testid="background-fade"
    id="background-fade"
    type="range"
    min="0"
    max={FADE_MAX}
    step="1"
    value={uiSettings.backgroundFade}
    oninput={(e) => previewBackgroundFade(Number((e.currentTarget as HTMLInputElement).value))}
    onchange={(e) => {
      commitBackgroundFade(Number((e.currentTarget as HTMLInputElement).value)).catch(() => {});
    }}
  />
  <span class="value">{uiSettings.backgroundFade}%</span>
</div>

<div class="field">
  <label for="background-blur">Background art blur</label>
  <input
    data-testid="background-blur"
    id="background-blur"
    type="range"
    min="0"
    max={BLUR_MAX}
    step="1"
    value={uiSettings.backgroundBlur}
    onchange={(e) => {
      commitBackgroundBlur(Number((e.currentTarget as HTMLInputElement).value)).catch(() => {});
    }}
  />
  <span class="value">{uiSettings.backgroundBlur}</span>
</div>

{#each CARD_SIZE_VIEWS as v (v.view)}
  <div class="field">
    <label for={v.testId}>{v.label}</label>
    <select data-testid={v.testId} id={v.testId} value={sizeFor(v.view)} onchange={(e) => onCardSize(v.view, e)}>
      {#each CARD_SIZES as size (size)}
        <option value={size}>{cardSizeLabel(size)}</option>
      {/each}
    </select>
  </div>
{/each}

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
    color: var(--text-h);
  }

  .field label {
    flex: 0 0 180px;
  }

  .field select {
    font: inherit;
    font-size: 13px;
    padding: 6px 8px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-h);
  }

  .field input[type='range'] {
    flex: 1 1 auto;
    max-width: 320px;
    accent-color: var(--primary);
  }

  .field input[type='checkbox'] {
    accent-color: var(--primary);
  }

  .value {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
</style>
