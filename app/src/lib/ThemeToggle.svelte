<script lang="ts">
  // The light / dark toggle (ProposedApp README, "Light / dark toggle"): a
  // 28×28 icon button showing the current mode, its label naming the switch.
  // `corner` pins it to the window's top-right, 16px in, for the startup
  // screens; without it, it sits in the flow (the top bar's session cluster).
  // Props in, callback out, so the component renders without the store; the
  // callers wire `uiSettings.resolved` and `toggleTheme`.
  import Icon from './Icon.svelte';
  import { themeToggleIcon, themeToggleLabel, type ResolvedTheme } from './theme';

  let {
    resolved,
    onToggle,
    corner = false,
    testId = 'theme-toggle',
  }: {
    resolved: ResolvedTheme;
    onToggle: () => void;
    corner?: boolean;
    testId?: string;
  } = $props();

  let label = $derived(themeToggleLabel(resolved));

  // The icon spins in on a change of mode, not on the first paint: the
  // first mode seen is remembered and `spin` turns on once it differs.
  // `{#key}` below remounts the icon on every change so the animation
  // replays each time.
  let spin = $state(false);
  let seen: ResolvedTheme | null = null;
  $effect.pre(() => {
    const now = resolved;
    if (seen !== null && seen !== now) spin = true;
    seen = now;
  });
</script>

<button
  type="button"
  data-testid={testId}
  data-mode={resolved}
  class="icon-btn theme-toggle"
  class:corner
  aria-label={label}
  title={label}
  onclick={onToggle}
>
  {#key resolved}
    <span class="tt-icon" class:spin><Icon name={themeToggleIcon(resolved)} size={18} /></span>
  {/key}
</button>

<style>
  .theme-toggle {
    color: var(--text-h);
  }

  .corner {
    position: fixed;
    top: 16px;
    right: 16px;
    z-index: 101;
  }

  .tt-icon {
    display: grid;
    place-items: center;
  }

  .tt-icon.spin {
    animation: tt-spin var(--m-spin) cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  @keyframes tt-spin {
    from {
      opacity: 0;
      transform: rotate(-90deg) scale(0.6);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .tt-icon.spin {
      animation: none;
    }
  }
</style>
