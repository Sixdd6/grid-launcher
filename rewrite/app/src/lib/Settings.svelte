<script lang="ts">
  import { api } from './api';
  import { appUpdate, dismiss } from './stores/appUpdate.svelte';
  import {
    commitBackgroundFade,
    previewBackgroundFade,
    setTheme,
    uiSettings,
  } from './stores/uiSettings.svelte';
  import { FADE_MAX, type ThemeChoice } from './theme';
  import { LATER_STEP_TEXT, SETTINGS_PAGES, settingsPageLabel, type SettingsPage } from './settings';

  let page = $state<SettingsPage>('appearance');

  function onThemeChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as ThemeChoice;
    setTheme(value).catch(() => {
      // The attribute is already applied; a failed save is not worth a
      // blocking error in a settings pane.
    });
  }
</script>

<div class="settings">
  <nav class="rail" aria-label="Settings pages">
    {#each SETTINGS_PAGES as p (p)}
      <button
        data-testid={`settings-nav-${p}`}
        class="rail-item"
        class:active={page === p}
        aria-current={page === p ? 'page' : undefined}
        onclick={() => (page = p)}
      >
        {settingsPageLabel(p)}
      </button>
    {/each}
  </nav>

  <section class="pane" aria-label={settingsPageLabel(page)}>
    <h2>{settingsPageLabel(page)}</h2>

    {#if page === 'appearance'}
      <div class="field">
        <label for="theme-select">Theme</label>
        <select data-testid="theme-select" id="theme-select" value={uiSettings.theme} onchange={onThemeChange}>
          <option value="system">Follow system</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </div>

      <div class="field">
        <label for="background-fade">Background art fade</label>
        <!-- `oninput` previews live behind this pane (the art reads the same
             store); `onchange` is what reaches config.toml. -->
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
    {:else if page === 'updates'}
      {#if appUpdate.notice}
        <p data-testid="app-update-notice" class="update-line">
          GRID Launcher {appUpdate.notice.tag} is available
          <button data-testid="app-update-open" onclick={() => api.openReleasePage(appUpdate.notice!.url).catch(() => {})}>
            Open release
          </button>
          <button data-testid="app-update-dismiss" class="secondary" onclick={dismiss}>Dismiss</button>
        </p>
      {/if}
      <p class="placeholder">{LATER_STEP_TEXT}</p>
    {:else}
      <p class="placeholder">{LATER_STEP_TEXT}</p>
    {/if}
  </section>
</div>

<style>
  .settings {
    display: flex;
    gap: 24px;
    padding: 24px;
    box-sizing: border-box;
  }

  .rail {
    flex: 0 0 200px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rail-item {
    font: inherit;
    font-size: 13px;
    text-align: left;
    padding: 8px 12px;
    border: none;
    border-radius: var(--r-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rail-item:hover {
    background: var(--surface);
    color: var(--text-h);
  }

  .rail-item.active {
    background: var(--surface);
    color: var(--text-h);
    font-weight: 600;
  }

  .pane {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-h);
  }

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

  .field input[type='range'] {
    flex: 1 1 auto;
    max-width: 320px;
    accent-color: var(--primary);
  }

  .value {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .placeholder {
    margin: 0;
    color: var(--text-muted);
  }

  .update-line {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 13px;
    color: var(--text-h);
  }

  .update-line button {
    font: inherit;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-h);
    cursor: pointer;
  }

  .update-line button.secondary {
    border-color: transparent;
    color: var(--text-muted);
  }
</style>
