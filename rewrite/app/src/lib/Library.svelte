<script lang="ts">
  import type { NavDirection } from './focus/grid';

  // `active` is unused until Task 9 fills this section in — accepted so the
  // Shell can pass it uniformly to both Library and Server.
  let { active: _active }: { active: boolean } = $props();

  // No-op: this section has no navigable content yet. Task 9 replaces it.
  export function handleNav(action: NavDirection | 'accept' | 'back') {
    void action;
  }
</script>

<section data-testid="library-section">
  <p>No games installed</p>
</section>
