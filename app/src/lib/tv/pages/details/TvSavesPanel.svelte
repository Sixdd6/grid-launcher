<script lang="ts">
  // One kind's cloud panel in Details › Saves (items 306–313): the Saves |
  // States switch, "Cloud Saves" / "Cloud States" and Upload latest, then
  // the records. A on a record opens Restore to this computer / Delete from
  // the server, each confirmed; a deleted record leaves the list and focus
  // moves to the record now in its place (`#saves-list`, item 311); an
  // upload reloads the list, so it shows at the top as LATEST (item 312).
  // The records and actions are the shared CloudStore's.
  import type { CloudPanelInfo, CloudRecord, SaveType } from '../../../api';
  import { CloudStore } from '../../../details/cloudStore.svelte';
  import type { DetailsStore } from '../../../details/detailsStore.svelte';
  import { restoreAllConfirmText } from '../../../details/cloud';
  import { pushToast } from '../../../stores/toasts.svelte';
  import { tvf } from '../../focus/actions';
  import { confirmBox, openList } from '../../overlays/overlays.svelte';
  import TvButton from '../../ui/TvButton.svelte';
  import TvIcon from '../../ui/TvIcon.svelte';
  import {
    deleteConfirm,
    deletedNotice,
    restoreConfirm,
    restoredNotice,
    SAVE_KINDS,
    saveLine,
    saveListTitle,
    saveName,
    savesEmpty,
    savesHeading,
    uploadingNotice,
    uploadLabel,
  } from '../details';
  import { det } from '../detailsState.svelte';

  let { d, info, kind }: { d: DetailsStore; info: CloudPanelInfo | null; kind: SaveType } = $props();

  // The panel is keyed by kind and by whether its info has arrived.
  // svelte-ignore state_referenced_locally
  const store = info
    ? new CloudStore(() => ({ game: d.cloudGame, gameTitle: d.subject.name, saveType: kind, panelInfo: info!, isNative: d.isNative }))
    : null;

  $effect(() => {
    store?.load();
  });

  let title = $derived(d.subject.name);
  let scope = $derived(info?.scope ?? 'per_game');

  async function upload() {
    if (!store) return;
    pushToast(uploadingNotice(kind), 'success');
    // The backend's upload-finished card follows with the desktop's words.
    await store.upload();
    if (store.uploadError) pushToast(store.uploadError, 'error');
  }

  /** After a confirmed restore: the result, and the restore-all offer when
   *  folders were skipped (the desktop's G1-Q step). */
  async function runRestore(s: CloudStore) {
    await s.confirmAction();
    if (s.actionError) {
      pushToast(s.actionError, 'error');
      return;
    }
    if (s.actionNotice) pushToast(s.actionNotice, 'success');
    else pushToast(restoredNotice(kind, title), 'success');
    const c = s.confirmState;
    if (c?.kind === 'restore-all') {
      const text = restoreAllConfirmText(title, c.folders ?? []);
      void confirmBox({
        title: text.title,
        body: text.message,
        ok: 'Restore all',
        danger: true,
        onOk: () => void runRestore(s),
      });
      // B or Cancel there leaves the skipped folders alone.
    }
  }

  function restore(r: CloudRecord) {
    if (!store) return;
    if (!r.restorable) {
      if (r.restore_tooltip) pushToast(r.restore_tooltip, 'error');
      return;
    }
    const words = restoreConfirm(kind, title, r.restore_tooltip ?? '');
    void confirmBox({
      ...words,
      onOk: () => {
        store.requestRestore(r);
        void runRestore(store);
      },
    });
  }

  function remove(r: CloudRecord) {
    if (!store) return;
    const words = deleteConfirm(kind, store.sharedNote);
    void confirmBox({
      ...words,
      danger: true,
      onOk: async () => {
        store.requestDelete(r);
        await store.confirmAction();
        if (store.actionError) pushToast(store.actionError, 'error');
        else pushToast(deletedNotice(kind), 'success');
      },
    });
  }

  function rowActions(r: CloudRecord) {
    void openList({
      title: saveListTitle(kind, r),
      items: [
        { label: 'Restore to this computer', testId: 'tv-save-restore', onPick: () => restore(r) },
        { label: 'Delete from the server', danger: true, testId: 'tv-save-delete', onPick: () => remove(r) },
      ],
    });
  }
</script>

<div class="bar-h">
  <div class="seg" data-nav="h">
    {#each SAVE_KINDS as [k, label] (k)}
      <button class:on={kind === k} data-k={`sk-${k}`} data-testid={`tv-saves-kind-${k}`} use:tvf={{ a: () => (det.saveKind = k) }}>{label}</button>
    {/each}
  </div>
  <h3 class="saves-h" data-testid="tv-saves-heading">{savesHeading(kind, scope)}</h3>
  {#if store}
    <TvButton k="up" testId="tv-saves-upload" disabled={!store.uploadEnabled} why={store.uploadTooltip || undefined} onA={() => void upload()}>
      <TvIcon name="cloud" px={18} />{uploadLabel(kind, scope)}
    </TvButton>
  {/if}
</div>
{#if store && info}
  {#if store.recordsBlocked}
    <p class="note" data-testid="tv-saves-blocked">{info.block_reason || 'Cloud saves are not configured for this game.'}</p>
  {:else if store.recordsError}
    <p class="note err" data-testid="tv-saves-error">{store.recordsError}</p>
  {:else if !store.recordsLoading || store.records.length}
    {#if store.sharedNote}<p class="note warn">{store.sharedNote}</p>{/if}
    <div data-nav="v" id="saves-list" data-testid="tv-saves-list">
      {#each store.records as r, i (r.id)}
        <div class="lrow" data-k={`sv${r.id}`} data-al="Restore or delete" data-testid={`tv-save-${r.id}`} use:tvf={{ a: () => rowActions(r) }}>
          <div class="grow">
            <div class="t">{saveName(r)}{#if i === 0}{' '}<span class="latest" data-testid="tv-save-latest">LATEST</span>{/if}</div>
            <div class="s">{saveLine(r)}</div>
          </div>
        </div>
      {:else}
        <p class="note" data-testid="tv-saves-empty">{savesEmpty(kind)}</p>
      {/each}
    </div>
  {/if}
{/if}
