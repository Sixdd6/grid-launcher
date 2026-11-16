import { expectStartView, openAppearance, setServerOffline } from '../helpers/openOn.js';
import { connectRomm } from '../helpers/romm.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `open-on`, part A: Where GRID opens (TV mode README § Decided), with
 * Open GRID on at its default, Automatic. A connect on the welcome lands on
 * Library › RomM › Discover, and Settings › Appearance shows Open GRID on as its first row.
 * Parts B–D relaunch against the same data directory.
 */
describe('open-on (a): Automatic and connected opens Library › RomM › Discover', () => {
  before(async () => {
    await connectRomm();
  });

  it('opens on Library › RomM › Discover', async () => {
    await expectStartView('romm:discover');
    await expect($(testId('discover'))).toBeDisplayed();
  });

  it('shows Open GRID on first in Settings › Appearance', async () => {
    await openAppearance();
    const pane = await browser.execute(() => {
      const select = document.querySelector('[data-testid="open-on-select"]') as HTMLSelectElement;
      const row = select.closest('.setting')!;
      const first = row.parentElement!.querySelector('.setting');
      return {
        first: first === row,
        label: row.querySelector('.name label')?.textContent?.trim() ?? '',
        hint: row.querySelector('.name small')?.textContent?.trim() ?? '',
        value: select.value,
        options: [...select.options].map((o) => [o.value, o.textContent?.trim() ?? '']),
      };
    });
    expect(pane).toEqual({
      first: true,
      label: 'Open GRID on',
      hint: 'Here and in TV mode',
      value: 'automatic',
      options: [
        ['automatic', 'Automatic: Discover when RomM is connected, else Library'],
        ['discover', 'Discover (when RomM is connected)'],
        ['library', 'Library'],
      ],
    });
    // Connected: no "opens on All games" note.
    await expect($(testId('open-on-romm-off'))).not.toExist();
  });

  // Last: part B starts against an unreachable server.
  it('takes the mock offline for part B', async () => {
    await setServerOffline(true);
  });
});
