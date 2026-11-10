<script lang="ts">
  // Downloads (P5 placeholder: the three sections, empty). P10 builds the
  // rows, their actions and Clear completed (items 209–224).
  const SECTIONS = ['Active', 'Queued', 'Completed'] as const;
</script>

<div class="list" data-scroll="list" data-testid="tv-downloads">
  <div class="list-in">
    {#each SECTIONS as name (name)}
      <section class="panel">
        <h2>{name}<span>0</span></h2>
        <div data-nav="v"><p class="note">Nothing {name.toLowerCase()}.</p></div>
      </section>
    {/each}
  </div>
</div>
