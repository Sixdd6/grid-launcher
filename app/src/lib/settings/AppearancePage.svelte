<script lang="ts">
  // Settings › Appearance (design §10): theme, card size defaults, background
  // art on/off, the fade slider 0–100% (100 = fully faded, which is off) and
  // the blur slider 0–40. The art reads
  // the same store, so the FADE slider previews on `oninput` and persists on
  // `onchange`. The BLUR slider persists on `onchange` only: each sigma is a
  // separate variant the backend builds and caches, so a live drag preview
  // would build an image per intermediate position.
  import { api } from '../api';
  import {
    commitBackgroundBlur,
    commitBackgroundFade,
    previewBackgroundFade,
    setBackgroundEnabled,
    setCardDisplay,
    setHardwareAcceleration,
    setCardLabels,
    setCardSize,
    setTheme,
    uiSettings,
  } from '../stores/uiSettings.svelte';
  import { BLUR_MAX, FADE_MAX, type ThemeChoice } from '../theme';
  import { CARD_SIZES, cardSizeLabel, normalizeCardSize } from '../cards/size';
  import { CARD_LABEL_MODES, cardLabelsLabel, normalizeCardLabels } from '../cards/labels';
  import { CARD_DISPLAYS, cardDisplayLabel, normalizeCardDisplay } from '../cards/layout';
  import { backgroundEnabled, CARD_SIZE_VIEWS } from './appearance';
  import SegmentedSwitch from '../SegmentedSwitch.svelte';
  import SettingRow from './SettingRow.svelte';

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

  function onHardwareAcceleration(e: Event) {
    setHardwareAcceleration((e.currentTarget as HTMLInputElement).checked).catch(() => {});
  }

  function onCardDisplay(value: string) {
    setCardDisplay(normalizeCardDisplay(value)).catch(() => {});
  }

  function onCardLabels(value: string) {
    setCardLabels(normalizeCardLabels(value)).catch(() => {});
  }

  // Two named options each, so a segmented switch rather than a dropdown.
  const labelOptions = CARD_LABEL_MODES.map((mode) => ({ value: mode, label: cardLabelsLabel(mode) }));
  const displayOptions = CARD_DISPLAYS.map((display) => ({ value: display, label: cardDisplayLabel(display) }));

  function onCardSize(view: 'library' | 'server', e: Event) {
    setCardSize(view, normalizeCardSize((e.currentTarget as HTMLSelectElement).value)).catch(() => {});
  }

  // Page-local rather than a `uiSettings` slot: `debug_prints` is a scalar
  // config key, not part of the `[ui]` table the store mirrors, and nothing
  // outside this page reads it.
  let debugPrints = $state(true);

  $effect(() => {
    api
      .getDebugPrints()
      .then((enabled) => {
        debugPrints = enabled;
      })
      .catch(() => {
        // The default (on) is already shown; a failed read is not worth a
        // blocking error in a settings pane.
      });
  });

  function onDebugPrints(e: Event) {
    debugPrints = (e.currentTarget as HTMLInputElement).checked;
    api.setDebugPrints(debugPrints).catch(() => {});
  }

  function sizeFor(view: 'library' | 'server') {
    return view === 'library' ? uiSettings.cardSizeLibrary : uiSettings.cardSizeServer;
  }
</script>

<!-- The design prototype's order (attachments/proposed-ui/30): theme, the
     background art, the cards, then the two switches that act on restart or
     on logging. Each row is a `SettingRow`. -->
<SettingRow label="Theme" forId="theme-select">
  <select class="g-field" data-testid="theme-select" id="theme-select" value={uiSettings.theme} onchange={onThemeChange}>
    <option value="system">Follow system</option>
    <option value="dark">Dark</option>
    <option value="light">Light</option>
  </select>
</SettingRow>

<SettingRow label="Background art" forId="background-art-toggle">
  <label class="g-check">
    <input
      data-testid="background-art-toggle"
      id="background-art-toggle"
      type="checkbox"
      checked={backgroundEnabled(uiSettings.backgroundFade)}
      onchange={onToggle}
    />
    Show the last game’s art behind the views
  </label>
</SettingRow>

<SettingRow label="Background art fade" hint="0 is full strength; 100 turns the art off" forId="background-fade">
  <input
    class="g-range"
    data-testid="background-fade"
    id="background-fade"
    aria-describedby="background-fade-hint"
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
  <span class="g-value">{uiSettings.backgroundFade}%{backgroundEnabled(uiSettings.backgroundFade) ? '' : ' · off'}</span>
</SettingRow>

<SettingRow label="Background art blur" forId="background-blur">
  <input
    class="g-range"
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
  <span class="g-value">{uiSettings.backgroundBlur}</span>
</SettingRow>

<SettingRow label="Game labels" labelId="card-labels-label">
  <SegmentedSwitch
    testId="card-labels"
    id="card-labels"
    ariaLabelledby="card-labels-label"
    options={labelOptions}
    value={uiSettings.cardLabels}
    onChange={onCardLabels}
  />
</SettingRow>

<SettingRow label="Game card display" labelId="card-display-label">
  <SegmentedSwitch
    testId="card-display"
    id="card-display"
    ariaLabelledby="card-display-label"
    options={displayOptions}
    value={uiSettings.cardDisplay}
    onChange={onCardDisplay}
  />
</SettingRow>

{#each CARD_SIZE_VIEWS as v (v.view)}
  <SettingRow label={v.label} forId={v.testId}>
    <select class="g-field" data-testid={v.testId} id={v.testId} value={sizeFor(v.view)} onchange={(e) => onCardSize(v.view, e)}>
      {#each CARD_SIZES as size (size)}
        <option value={size}>{cardSizeLabel(size)}</option>
      {/each}
    </select>
  </SettingRow>
{/each}

<SettingRow label="Hardware acceleration" hint="Applies after you restart GRID" forId="hardware-acceleration-toggle">
  <label class="g-check">
    <input
      data-testid="hardware-acceleration-toggle"
      id="hardware-acceleration-toggle"
      aria-describedby="hardware-acceleration-toggle-hint"
      type="checkbox"
      checked={uiSettings.hardwareAcceleration}
      onchange={onHardwareAcceleration}
    />
    Render with the graphics card
  </label>
</SettingRow>

<SettingRow label="Debug" forId="debug-prints-toggle">
  <label class="g-check">
    <input
      data-testid="debug-prints-toggle"
      id="debug-prints-toggle"
      type="checkbox"
      checked={debugPrints}
      onchange={onDebugPrints}
    />
    Enable debug prints
  </label>
</SettingRow>
