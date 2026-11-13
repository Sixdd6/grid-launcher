<script lang="ts">
  // The desktop's store dialogs, mounted once by Shell.svelte over every
  // view: store Details, Install, Game Settings, the cloud conflict prompt,
  // the offline installer and the GOG backup dialog. It also shows the
  // `store-notice` and sign-in cards for the desktop's lifetime.
  import StoreDetails from './StoreDetails.svelte';
  import InstallDialog from './InstallDialog.svelte';
  import GameSettingsDialog from './GameSettingsDialog.svelte';
  import ConflictDialog from './ConflictDialog.svelte';
  import InstallerDialog from './InstallerDialog.svelte';
  import StoreBackupDialog from './StoreBackupDialog.svelte';
  import {
    closeConflict,
    closeGameSettings,
    closeInstall,
    closeInstaller,
    closeStoreBackup,
    closeStoreDetails,
    initStoreCards,
    resolveConflict,
    storeUi,
  } from './ui.svelte';

  let { onSetUpDrive }: { onSetUpDrive: () => void } = $props();

  $effect(() => initStoreCards());
</script>

{#if storeUi.details}
  {#key storeUi.details.key + (storeUi.details.drive?.key ?? '')}
    <StoreDetails open={storeUi.details} onClose={closeStoreDetails} />
  {/key}
{/if}

{#if storeUi.install}
  {#key storeUi.install.key}
    <InstallDialog game={storeUi.install} onClose={closeInstall} />
  {/key}
{/if}

{#if storeUi.settings}
  <GameSettingsDialog game={storeUi.settings} onClose={closeGameSettings} />
{/if}

{#if storeUi.conflict}
  <ConflictDialog
    game={storeUi.conflict.game}
    state={storeUi.conflict.state}
    launch={storeUi.conflict.launch}
    onChoose={(c) => resolveConflict(c)}
    onClose={closeConflict}
  />
{/if}

{#if storeUi.installer}
  <InstallerDialog onClose={closeInstaller} />
{/if}

{#if storeUi.backup}
  {@const b = storeUi.backup}
  <StoreBackupDialog
    keys={b.keys}
    title={b.title}
    onQueued={b.onQueued}
    onClose={closeStoreBackup}
    onSetUp={() => {
      closeStoreBackup();
      closeStoreDetails();
      onSetUpDrive();
    }}
  />
{/if}
