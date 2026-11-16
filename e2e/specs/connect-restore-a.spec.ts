import { connectRomm } from '../helpers/romm.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `connect-restore`, part A: connect once (through the welcome's RomM
 * row on this fresh profile), then let the app exit.
 *
 * Part B reruns the same binary against the SAME data directory and the same
 * mock server, so it sees the config.toml and keyring entry this spec leaves
 * behind. The pair exists because the embedded WebDriver provider cannot
 * restart the app inside a single run — the runner does it instead.
 */
describe('connect-restore (a): first launch', () => {
  it('connects and reaches the Library with the RomM platforms', async () => {
    await connectRomm();
    await $(testId('library-rail-platform-snes')).waitForExist({
      timeout: 5_000,
      timeoutMsg: 'the RomM group never rendered a platform row after a successful connect',
    });
  });
});
