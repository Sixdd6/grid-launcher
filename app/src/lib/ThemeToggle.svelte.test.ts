// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import ThemeToggle from './ThemeToggle.svelte';
import { ICONS } from './icons';

const noop = () => {};

describe('ThemeToggle', () => {
  it('shows the sun in light mode and names the switch to dark', () => {
    const { body } = render(ThemeToggle, { props: { resolved: 'light', onToggle: noop } });
    expect(body).toContain('aria-label="Switch to dark mode"');
    expect(body).toContain('title="Switch to dark mode"');
    expect(body).toContain(`d="${ICONS.sun}"`);
    expect(body).not.toContain(`d="${ICONS.moon}"`);
  });

  it('shows the moon in dark mode and names the switch to light', () => {
    const { body } = render(ThemeToggle, { props: { resolved: 'dark', onToggle: noop } });
    expect(body).toContain('aria-label="Switch to light mode"');
    expect(body).toContain(`d="${ICONS.moon}"`);
  });

  it('is the shared 28×28 icon button with an 18px icon', () => {
    const { body } = render(ThemeToggle, { props: { resolved: 'light', onToggle: noop } });
    expect(body).toMatch(/class="[^"]*\bicon-btn\b/);
    expect(body).toContain('width="18"');
  });

  it('does not spin on the first paint', () => {
    const { body } = render(ThemeToggle, { props: { resolved: 'dark', onToggle: noop } });
    expect(body).not.toMatch(/class="[^"]*\bspin\b/);
  });

  it('pins to the corner only when asked, under the given test id', () => {
    const plain = render(ThemeToggle, { props: { resolved: 'light', onToggle: noop } }).body;
    expect(plain).toContain('data-testid="theme-toggle"');
    expect(plain).not.toMatch(/class="[^"]*\bcorner\b/);
    const corner = render(ThemeToggle, {
      props: { resolved: 'light', onToggle: noop, corner: true, testId: 'theme-toggle-start' },
    }).body;
    expect(corner).toContain('data-testid="theme-toggle-start"');
    expect(corner).toMatch(/class="[^"]*\bcorner\b/);
  });
});
