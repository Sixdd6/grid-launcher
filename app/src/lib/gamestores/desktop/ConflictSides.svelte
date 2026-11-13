<script lang="ts">
  // The two sides of a cloud save conflict (prototype `.st-conflict`,
  // `.st-side`): This computer and the store's cloud, the newer one ringed.
  import type { ConflictSide } from '../details';

  let { sides, onChoose }: { sides: ConflictSide[]; onChoose: (choice: 'upload' | 'download') => void } = $props();
</script>

<div class="st-conflict" data-testid="store-conflict-sides">
  {#each sides as side (side.which)}
    <div class="st-side" class:newer={side.newer}>
      <b>{side.label}</b>
      <span class="note">{side.note}</span>
      <button
        class="g-btn"
        class:secondary={side.which === 'cloud'}
        data-testid={`store-conflict-${side.which}`}
        onclick={() => onChoose(side.which === 'local' ? 'upload' : 'download')}>{side.button}</button
      >
      {#if side.after}<span class="note">{side.after}</span>{/if}
    </div>
  {/each}
</div>

<style>
  .st-conflict {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .st-side {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: var(--r-row);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  .st-side b {
    color: var(--text-h);
  }

  .st-side .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .st-side .g-btn {
    align-self: flex-start;
    padding: 6px 14px;
    margin-top: 4px;
  }

  .st-side.newer {
    border-color: var(--primary);
  }
</style>
