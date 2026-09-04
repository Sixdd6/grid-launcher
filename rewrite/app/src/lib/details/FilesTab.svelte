<script lang="ts">
  import type { RomFile } from '../api';
  import { contentRows, fileRows } from './files';

  let {
    files,
    installedVersion,
    serverVersion,
    installedNow,
  }: {
    files: RomFile[];
    installedVersion: string;
    serverVersion: string;
    installedNow: boolean;
  } = $props();

  let rows = $derived(fileRows(files));
  let content = $derived(contentRows(files));

  // D-UI-10's comparison line. Only meaningful once the game is installed:
  // for a server-only game there is no installed side to compare against,
  // and the left column's Install button is the whole story.
  let versionLine = $derived(
    installedNow && (installedVersion !== '' || serverVersion !== '')
      ? `Installed ${installedVersion || 'unknown'} · Server ${serverVersion || 'unknown'}`
      : ''
  );
</script>

<div class="files">
  {#if versionLine}
    <p class="version-line" data-testid="details-files-version">{versionLine}</p>
  {/if}

  {#if rows.length}
    <ul class="rows">
      {#each rows as row (row.id)}
        <li class="row" data-testid={`details-file-${row.id}`}>
          <span class="name">{row.name}</span>
          <span class="size">{row.sizeText}</span>
          <span class="version" data-testid={`details-file-version-${row.id}`}>{row.version}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="empty" data-testid="details-files-empty">The server lists no files for this game</p>
  {/if}

  {#if content.length}
    <h3>Extra content</h3>
    <ul class="rows">
      {#each content as row (row.id)}
        <li class="row" data-testid={`details-content-${row.id}`}>
          <span class="name">{row.name}</span>
          <span class="size">{row.sizeText}</span>
          <span class="version">{row.category === 'update' ? 'Update' : 'DLC'}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .version-line {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  h3 {
    margin: 4px 0 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 12px;
    align-items: baseline;
    padding: 8px 10px;
    border-radius: var(--r-row);
    background: var(--surface);
  }

  .name {
    color: var(--text);
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .size,
  .version {
    color: var(--text-muted);
    font-size: 12px;
    white-space: nowrap;
  }

  .empty {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
