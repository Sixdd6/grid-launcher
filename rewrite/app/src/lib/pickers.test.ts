import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { open } = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open }));

// The "reported once per process" flag lives at module scope, so each case
// that cares about it gets a fresh module instance via `resetModules`.
async function freshPickers() {
  vi.resetModules();
  return import('./pickers');
}

beforeEach(() => {
  open.mockReset();
});

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('pickFolder/pickFile', () => {
  it('returns null and pushes no toast when the user cancels', async () => {
    const { pickFolder } = await freshPickers();
    const { toasts } = await import('./stores/toasts.svelte');
    open.mockResolvedValue(null);
    expect(await pickFolder('Select Folder')).toBeNull();
    expect(toasts.list).toHaveLength(0);
  });

  it('returns null and pushes one toast across two failures', async () => {
    const { pickFolder, pickFile } = await freshPickers();
    const { toasts } = await import('./stores/toasts.svelte');
    open.mockRejectedValue(new Error('no portal'));
    expect(await pickFolder('Select Folder')).toBeNull();
    expect(await pickFile('Select File')).toBeNull();
    expect(toasts.list).toHaveLength(1);
    expect(toasts.list[0].text).toBe('Could not open a file dialog. Enter the path by hand.');
    expect(toasts.list[0].level).toBe('error');
  });
});

describe('under the E2E build', () => {
  it('stays silent on a dialog failure', async () => {
    vi.stubEnv('VITE_E2E', '1');
    const { pickFolder } = await freshPickers();
    const { toasts } = await import('./stores/toasts.svelte');
    open.mockRejectedValue(new Error('no portal'));
    expect(await pickFolder('Select Folder')).toBeNull();
    expect(toasts.list).toHaveLength(0);
  });
});
