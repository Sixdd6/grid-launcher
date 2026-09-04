<script lang="ts">
  import type { Sample } from './ring';
  import { sparklinePaths } from './sparkline';

  // One sparkline for both the row panel (120×38) and the footer strip
  // (120×18). Network is the primary colour, disk is the §4 teal, both on
  // one shared scale so a viewer can compare them. The two paths always
  // exist — with an empty `d` before the first sample — so the structure is
  // stable for E2E and the layout never jumps.
  let {
    samples,
    width,
    height,
    label,
    testId = undefined,
  }: {
    samples: Sample[];
    width: number;
    height: number;
    label: string;
    testId?: string;
  } = $props();

  let paths = $derived(sparklinePaths(samples, { width, height }));
</script>

<svg
  data-testid={testId}
  class="spark"
  viewBox={`0 0 ${width} ${height}`}
  {width}
  {height}
  role="img"
  aria-label={label}
>
  <path class="net" d={paths.net} />
  <path class="disk" d={paths.disk} />
</svg>

<style>
  .spark {
    display: block;
    flex: none;
    border-radius: var(--r-control);
    background: var(--surface);
  }

  path {
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .net {
    stroke: var(--primary);
  }

  .disk {
    stroke: var(--graph-disk);
  }
</style>
