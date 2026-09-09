// @vitest-environment node
import { describe, expect, it, afterEach } from 'vitest';
import { render } from 'svelte/server';
import Toast from './Toast.svelte';
import { pushToast, dismissToast } from './stores/toasts.svelte';

// Ids pushed here are dismissed after each test so the module-state store
// (shared across tests in this file) starts empty every time.
const pushed: number[] = [];
function push(text: string, level: 'success' | 'error') {
  const id = pushToast(text, level);
  if (id !== null) pushed.push(id);
  return id;
}

afterEach(() => {
  while (pushed.length > 0) dismissToast(pushed.pop()!);
});

describe('Toast', () => {
  it('wraps an error toast in role="alert" and a success toast in role="status"', () => {
    push('Something failed.', 'error');
    push('Saved successfully.', 'success');
    const { body } = render(Toast);
    expect(body).toContain('role="alert"');
    expect(body).toContain('role="status"');
    const alertIndex = body.indexOf('role="alert"');
    const errorTextIndex = body.indexOf('Something failed.');
    expect(errorTextIndex).toBeGreaterThan(alertIndex);
  });

  it('prefixes an error toast with a visually-hidden "Error:" exactly once', () => {
    push('Something failed.', 'error');
    const { body } = render(Toast);
    const matches = body.match(/Error:/g) ?? [];
    expect(matches).toHaveLength(1);
    expect(body).toContain('sr-only');
  });

  it('does not prefix a success toast with "Error:"', () => {
    push('Saved successfully.', 'success');
    const { body } = render(Toast);
    expect(body).not.toContain('Error:');
  });

  it('keeps the toast-region and toast test ids for e2e selectors', () => {
    push('Saved successfully.', 'success');
    const { body } = render(Toast);
    expect(body).toContain('data-testid="toast-region"');
    expect(body).toContain('data-testid="toast"');
  });
});
