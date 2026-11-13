<script lang="ts">
  // "Which save should Lanternfall use?" (GameStores README, "Launch";
  // prototype `stConflictHTML`): before a launch over a cloud conflict, or
  // from Details. A launch never goes ahead without a choice.
  import type { StoreCloudState, StoreGame } from '../../api';
  import { conflictNote, conflictPromptTitle, conflictSides } from '../details';
  import { storeName } from '../marks';
  import { storeCoverUrl } from './coverUrl';
  import StoreDialog from './StoreDialog.svelte';
  import ConflictSides from './ConflictSides.svelte';

  let {
    game,
    state,
    launch,
    onChoose,
    onClose,
  }: {
    game: StoreGame;
    state: StoreCloudState;
    launch: boolean;
    onChoose: (choice: 'upload' | 'download') => void;
    onClose: () => void;
  } = $props();

  let now = Math.floor(Date.now() / 1000);
  let sides = $derived(state.conflict ? conflictSides(game.source, state.conflict, now, true) : []);
</script>

<StoreDialog testId="store-conflict-dialog" label={conflictPromptTitle(game.title)} {onClose}>
  <div class="st-dlg-head">
    {#if storeCoverUrl(game.cover)}<img class="st-cover-sm" src={storeCoverUrl(game.cover)} alt="" />{/if}
    <span>
      <h3>{conflictPromptTitle(game.title)}</h3>
      <span class="note">{storeName(game.source)} cloud saves</span>
    </span>
  </div>
  <p class="note">{conflictNote(game.source)}</p>
  <ConflictSides {sides} {onChoose} />
  <div class="foot">
    <span class="note">{launch ? 'The game starts after you pick.' : ''}</span>
    <button class="g-btn secondary" data-testid="store-conflict-later" onclick={onClose}>{launch ? 'Don’t launch' : 'Decide later'}</button>
  </div>
</StoreDialog>
