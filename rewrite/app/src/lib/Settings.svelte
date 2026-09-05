<script lang="ts">
  // The Settings view (design §10, D-UI-5): a 220px category rail and one
  // pane per page. All five panes stay mounted and switch with `hidden`,
  // the same rule the shell applies to views, so an in-flight save or a
  // typed-but-unsaved field survives a rail click. Each pane's column caps
  // at 1100px (D-UI-7).
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import {
    SETTINGS_PAGES,
    settingsPageLabel,
    settingsRailEntries,
    type SettingsPage,
  } from './settings/pages';
  import ConnectionPage from './settings/ConnectionPage.svelte';
  import CloudSavesPage from './settings/CloudSavesPage.svelte';
  import RetroAchievementsPage from './settings/RetroAchievementsPage.svelte';
  import UpdatesPage from './settings/UpdatesPage.svelte';
  import AppearancePage from './settings/AppearancePage.svelte';

  let { active = true }: { active?: boolean } = $props();

  let page = $state<SettingsPage>('connection');

  /**
   * Programmatic page selection, for callers that route straight to a page
   * — the top bar's update badge opens Settings on `updates` (design §3).
   */
  export function show(next: SettingsPage) {
    page = next;
  }

  let railRows = $derived(
    settingsRailEntries(page).map(
      (e): RailPaneEntry<SettingsPage> => ({
        key: e.key,
        testId: e.testId,
        label: e.label,
        selected: e.selected,
        heading: e.heading,
      }),
    ),
  );
</script>

<section class="settings" aria-label="Settings">
  <RailPane entries={railRows} testId="settings-rail" ariaLabel="Settings pages" onSelect={(k) => (page = k)} />

  <div class="panes">
    {#each SETTINGS_PAGES as p (p)}
      <section data-testid={`settings-page-${p}`} class="pane" hidden={page !== p} aria-label={settingsPageLabel(p)}>
        <div class="view-content pane-inner">
          <h2>{settingsPageLabel(p)}</h2>
          {#if p === 'connection'}
            <!-- Page-level activation, unlike the other panes: the edit form
                 holds a plain secret, so leaving this page must close it. -->
            <ConnectionPage active={active && page === 'connection'} />
          {:else if p === 'cloud-saves'}
            <CloudSavesPage {active} />
          {:else if p === 'retroachievements'}
            <!-- Page-level activation, like the connection page: the login
                 form holds a plain-typed password, so leaving this page must
                 clear it. -->
            <RetroAchievementsPage active={active && page === 'retroachievements'} />
          {:else if p === 'updates'}
            <UpdatesPage {active} />
          {:else}
            <AppearancePage />
          {/if}
        </div>
      </section>
    {/each}
  </div>
</section>

<style>
  .settings {
    display: flex;
    align-items: stretch;
    height: 100%;
    min-height: 0;
  }

  /* Definite-height flex column so the pane below can scroll itself — see
     the same note in `Emulators.svelte`. */
  .panes {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  /* No `display` on `.pane` itself: the `hidden` attribute's UA rule must
     win, and an author `display: flex` here would override it. Sizing comes
     from `flex`, which needs no `display` on the item. */
  .pane {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .pane[hidden] {
    display: none;
  }

  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-h);
  }
</style>
