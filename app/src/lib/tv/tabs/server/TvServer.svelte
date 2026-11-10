<script lang="ts">
  // Server (P5 placeholder: Discover, the platforms heading and an empty
  // main area). P7 builds Discover, the platforms and the offline panels
  // (items 157–175, 196–208, 518–519).
  import TvRail from '../../ui/TvRail.svelte';
  import TvRailRow from '../../ui/TvRailRow.svelte';
</script>

<div class="split" data-testid="tv-server">
  <TvRail label="Server">
    <TvRailRow k="r-discover" selected testId="tv-srv-rail-discover">Discover</TvRailRow>
    <hr />
    <h3>PLATFORMS</h3>
  </TvRail>
  <div class="main" id="srv-main" data-rail-target></div>
</div>
