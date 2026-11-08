// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import SegmentedSwitch from './SegmentedSwitch.svelte';

const options = [
  { value: 'always', label: 'Always visible' },
  { value: 'hover', label: 'Visible on hover' },
] as const;
const noop = () => {};

describe('SegmentedSwitch', () => {
  it('is a radio group named by its label, under the given test id', () => {
    const { body } = render(SegmentedSwitch, {
      props: { options, value: 'always', onChange: noop, testId: 'card-labels', ariaLabelledby: 'lbl' },
    });
    expect(body).toContain('role="radiogroup"');
    expect(body).toContain('aria-labelledby="lbl"');
    expect(body).toContain('data-testid="card-labels"');
    expect(body).toContain('data-value="always"');
    expect(body).toContain('data-testid="card-labels-always"');
    expect(body).toContain('data-testid="card-labels-hover"');
  });

  it('checks only the chosen option and makes it the one Tab stop', () => {
    const { body } = render(SegmentedSwitch, { props: { options, value: 'hover', onChange: noop, testId: 's' } });
    const always = body.match(/<button[^>]*data-testid="s-always"[^>]*>/)![0];
    const hover = body.match(/<button[^>]*data-testid="s-hover"[^>]*>/)![0];
    expect(hover).toContain('aria-checked="true"');
    expect(hover).toContain('tabindex="0"');
    expect(hover).toMatch(/class="[^"]*\bactive\b/);
    expect(always).toContain('aria-checked="false"');
    expect(always).toContain('tabindex="-1"');
  });

  it('keeps the first option reachable when the value matches none', () => {
    const { body } = render(SegmentedSwitch, {
      props: { options, value: 'other' as 'always', onChange: noop, testId: 's' },
    });
    expect(body.match(/<button[^>]*data-testid="s-always"[^>]*>/)![0]).toContain('tabindex="0"');
  });

  it('shows every option label', () => {
    const { body } = render(SegmentedSwitch, { props: { options, value: 'always', onChange: noop } });
    expect(body).toContain('Always visible');
    expect(body).toContain('Visible on hover');
  });
});
