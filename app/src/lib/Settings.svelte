<script lang="ts">
  // The Settings view (design §10, D-UI-5): a 220px category rail and one
  // pane per page. All eight panes stay mounted and switch with `hidden`,
  // the same rule the shell applies to views, so an in-flight save or a
  // typed-but-unsaved field survives a rail click. Each pane's column caps
  // at 1100px (D-UI-7).
  import RailPane, { type RailPaneEntry } from './RailPane.svelte';
  import {
    DEFAULT_SETTINGS_PAGE,
    SETTINGS_PAGES,
    settingsPageLabel,
    settingsRailEntries,
    type SettingsPage,
  } from './settings/pages';
  import ConnectionPage from './settings/ConnectionPage.svelte';
  import LibraryPage from './settings/LibraryPage.svelte';
  import CloudSavesPage from './settings/CloudSavesPage.svelte';
  import RetroAchievementsPage from './settings/RetroAchievementsPage.svelte';
  import UpdatesPage from './settings/UpdatesPage.svelte';
  import AppearancePage from './settings/AppearancePage.svelte';
  import BackupDrivesPage from './settings/BackupDrivesPage.svelte';
  import SystemsPage from './systems/SystemsPage.svelte';

  let {
    active = true,
    onShowDriveInLibrary = undefined,
  }: {
    active?: boolean;
    /**
     * Opens Library on a backup drive (Backup drives' "Show in Library").
     * TODO(Task 20): Shell passes this once Library › Backup drives exists;
     * until then the button is left out.
     */
    onShowDriveInLibrary?: (key: string) => void;
  } = $props();

  let page = $state<SettingsPage>(DEFAULT_SETTINGS_PAGE);

  let systemsPage = $state<ReturnType<typeof SystemsPage> | null>(null);

  /**
   * Programmatic page selection, for callers that route straight to a page
   * — the top bar's update badge opens Settings on `updates` (design §3),
   * the Server header's chips open `systems` on one system's row.
   */
  export function show(next: SettingsPage, opts: { system?: string } = {}) {
    page = next;
    if (next === 'systems' && opts.system) systemsPage?.reveal(opts.system);
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
          <!-- Systems titles itself: its header holds the title, the lede
               and the actions in one row (design prototype 31). -->
          {#if p !== 'systems'}<h2 class="g-pane-title">{settingsPageLabel(p)}</h2>{/if}
          {#if p === 'appearance'}
            <AppearancePage />
          {:else if p === 'backup-drives'}
            <!-- Page-level activation: the backup default and the "last
                 seen" clock are re-read each time the page comes forward. -->
            <BackupDrivesPage active={active && page === 'backup-drives'} onShowInLibrary={onShowDriveInLibrary} />
          {:else if p === 'connection'}
            <!-- Page-level activation, unlike the other panes: the edit form
                 holds a plain secret, so leaving this page must close it. -->
            <ConnectionPage active={active && page === 'connection'} />
          {:else if p === 'library'}
            <!-- Page-level activation: the path is re-read each time the page
                 comes forward. -->
            <LibraryPage active={active && page === 'library'} />
          {:else if p === 'cloud-saves'}
            <CloudSavesPage {active} />
          {:else if p === 'retroachievements'}
            <!-- Page-level activation, like the connection page: the login
                 form holds a plain-typed password, so leaving this page must
                 clear it. -->
            <RetroAchievementsPage active={active && page === 'retroachievements'} />
          {:else if p === 'systems'}
            <!-- Page-level activation: its fetches (and the once-per-session
                 update check) run only while it is in front. -->
            <SystemsPage active={active && page === 'systems'} bind:this={systemsPage} />
          {:else if p === 'updates'}
            <UpdatesPage {active} />
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
    gap: var(--gutter);
    height: 100%;
    min-height: 0;
  }

  /* A definite-height flex column, not a plain block: the pane below scrolls
     itself, and it can only do that if this box stops growing with its
     content. Left auto-height, the pane's height resolved to its own content,
     the section grew past the window and the document scrolled — taking the
     rail with it. */
  .panes {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    /* The pages are a panel: forms and lists never sit on the art. */
    background: var(--panel);
    -webkit-backdrop-filter: var(--panel-blur);
    backdrop-filter: var(--panel-blur);
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    box-sizing: border-box;
    overflow: hidden;
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

  /* The design prototype's `.pane`: 24px in, 14px between blocks. */
  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 24px;
  }
</style>
