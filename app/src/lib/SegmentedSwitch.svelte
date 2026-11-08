<script lang="ts" generics="T extends string">
  // A segmented switch: a choice between a few named options, all visible
  // (design system "g-switch": Saves | States, Always visible | Visible on
  // hover). Radio-group semantics: one Tab stop on the chosen option, the
  // arrow keys (and Home / End) move the choice, Space or a click picks.
  // `testId` names the group; each option is `<testId>-<value>`, and the
  // group carries the current value as `data-value`.
  import { segmentKeyTarget, segmentTabIndex } from './segmented';

  type Option = { value: T; label: string };

  let {
    options,
    value,
    onChange,
    testId,
    id,
    ariaLabel,
    ariaLabelledby,
    disabled = false,
  }: {
    options: readonly Option[];
    value: T;
    onChange: (value: T) => void;
    testId?: string;
    id?: string;
    ariaLabel?: string;
    ariaLabelledby?: string;
    disabled?: boolean;
  } = $props();

  let buttons = $state<HTMLButtonElement[]>([]);
  let tabStop = $derived(
    segmentTabIndex(
      options.map((o) => o.value),
      value,
    ),
  );

  function choose(index: number) {
    const next = options[index]?.value;
    if (next !== undefined && next !== value) onChange(next);
  }

  function onKeydown(e: KeyboardEvent, index: number) {
    const target = segmentKeyTarget(e.key, index, options.length);
    if (target === null) return;
    e.preventDefault();
    choose(target);
    buttons[target]?.focus();
  }
</script>

<div
  class="g-switch"
  role="radiogroup"
  {id}
  aria-label={ariaLabel}
  aria-labelledby={ariaLabelledby}
  aria-disabled={disabled || undefined}
  data-testid={testId}
  data-value={value}
>
  {#each options as option, i (option.value)}
    <button
      type="button"
      role="radio"
      aria-checked={option.value === value}
      class:active={option.value === value}
      tabindex={i === tabStop ? 0 : -1}
      data-testid={testId ? `${testId}-${option.value}` : undefined}
      {disabled}
      bind:this={buttons[i]}
      onclick={() => choose(i)}
      onkeydown={(e) => onKeydown(e, i)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .g-switch {
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    background: var(--surface);
  }

  button {
    font: inherit;
    font-size: 13px;
    padding: 5px 16px;
    border: none;
    border-radius: var(--r-pill);
    background: transparent;
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  button:not(.active):hover:not(:disabled) {
    background: var(--surface);
  }

  button.active {
    background: var(--primary);
    color: var(--on-primary);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  @media (prefers-reduced-motion: reduce) {
    button {
      transition: none;
    }
  }
</style>
