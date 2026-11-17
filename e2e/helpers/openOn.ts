import { mockUrl } from './env.js';

// The Open GRID on helpers (expectStartView, openAppearance, chooseOpenOn)
// went with the setting, hidden for now with Discover (2026-11-17). What is
// left serves the specs that take the mock RomM offline.

/** Switches the mock RomM between answering and dropping every API request. */
export async function setServerOffline(offline: boolean): Promise<void> {
  const res = await fetch(`${mockUrl()}/__e2e__/offline`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ offline }),
  });
  expect(res.ok).toBe(true);
}
