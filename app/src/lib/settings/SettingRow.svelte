<script lang="ts">
  // One Settings row (design prototype `.setting`): the name in a 200px
  // column, an optional hint under it, the control beside it, and a hairline
  // between rows. `forId` makes the name the control's <label>; a control
  // that is a group (a segmented switch) names itself with
  // `aria-labelledby={labelId}` instead. The hint's id is `<forId>-hint`, for
  // the control's `aria-describedby`.
  import type { Snippet } from 'svelte';

  let {
    label,
    hint,
    forId,
    labelId,
    children,
  }: {
    label: string;
    hint?: string;
    forId?: string;
    labelId?: string;
    children: Snippet;
  } = $props();
</script>

<div class="setting">
  <div class="name">
    {#if forId}
      <label for={forId} id={labelId}>{label}</label>
    {:else}
      <span id={labelId}>{label}</span>
    {/if}
    {#if hint}<small id={forId ? `${forId}-hint` : undefined}>{hint}</small>{/if}
  </div>
  <div class="control">
    {@render children()}
  </div>
</div>

<style>
  .setting {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }

  .setting:last-child {
    border-bottom: none;
  }

  .name {
    min-width: 0;
    color: var(--text-h);
  }

  .name small {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
  }

  .control {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    justify-self: start;
  }
</style>
