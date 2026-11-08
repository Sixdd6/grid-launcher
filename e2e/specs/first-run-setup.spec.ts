import { mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { APP_START_TIMEOUT, configPath, dataDir, FIXTURE_TOKEN, mockUrl, TRANSITION_TIMEOUT } from '../helpers/env.js';
import { groups, openSystems, selectValue } from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Three installs from the mock forge (two emulators and GE-Proton). */
const SETUP_TIMEOUT = 40_000;

/** With E2E_SCREENSHOTS=1, the picker and the progress card are saved under
 *  ctr-out/first-run-setup/ (git-ignored) for a look at the real screens. */
async function screenshot(name: string): Promise<void> {
  if (!process.env.E2E_SCREENSHOTS) return;
  const dir = path.resolve(process.cwd(), '..', 'ctr-out', 'first-run-setup');
  mkdirSync(dir, { recursive: true });
  await browser.saveScreenshot(path.join(dir, `${name}.png`));
}

/**
 * Stage `first-run-setup`: a fresh profile connects, Connect asks for a
 * library folder, and Choose your systems follows: the same rows as
 * Settings › Systems (`sys-row-<slug>`), each with a tick box
 * (`sys-tick-<slug>`), plus Windows, which picks the default Proton version.
 *
 * Fixtures (fixtures-first-run-setup): PS2 and Dreamcast, whose standalone
 * emulators the mock forge serves (PCSX2, Redream), and an Emulators platform
 * whose `ps2` folder holds PCSX2 builds packaged to the standard
 * (docs/server-emulator-packages.md: 1.6.0 and 2.0.2 for Linux, 2.0.2 for
 * Windows), which PS2 also offers; Zeebo, which nothing
 * installable serves; and SNES, which only RetroArch serves (with a core).
 * RetroArch's two-archive install has no mock forge route, so SNES is shown
 * but not ticked: its rules are covered by grid-core's launch::setup tests.
 * The mock forge serves a GE-Proton release, so the Windows row's
 * recommended download installs too. "Continue" (an installed Proton
 * version only) needs a Proton on the computer, which the container has
 * not: vitest covers it (setup/choices.test.ts).
 */
const SERVER_PCSX2 = 'romm:ps2/pcsx2 (playstation 2)';
const GE_PROTON = 'GloriousEggroll/proton-ge-custom';
const REDREAM_SOURCE = 'inolen/redream';
const segment = (source: string) => testId(`setup-seg-${source}`);

describe('first-run setup: choose systems, install their emulators', () => {
  const library = path.join(dataDir(), 'library');

  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-library-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('connect-library-path')).setValue(library);
    await $(testId('connect-submit')).click();
  });

  it('shows Choose your systems after connecting, with what each system offers', async () => {
    await $(testId('setup-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'Choose your systems never appeared after connecting',
    });
    await expect($(`${testId('setup-view')} h1`)).toHaveText('Choose your systems');
    await $(testId('sys-row-ps2')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    // Alphabetical by name, Windows last (Linux).
    const order = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid="setup-view"] [data-testid^="sys-row-"]')).map((r) =>
        (r as HTMLElement).dataset.testid!.replace('sys-row-', ''),
      ),
    );
    // The Emulators platform holds emulators, not games: never a row.
    expect(order).toEqual(['dc', 'ps2', 'snes', 'zeebo', 'windows']);
    await expect($(testId('sys-note-zeebo'))).toHaveText('No emulator available');
    await expect($(testId('sys-tick-zeebo'))).toBeDisabled();
    // Unticked rows show no picker.
    await expect($(testId('sys-pick-ps2'))).not.toExist();
    // Nothing ticked yet, so nothing to install.
    await expect($(testId('setup-install'))).toBeDisabled();
    await expect($(testId('setup-install'))).toHaveText('Install emulators');
    await screenshot('1-choose');
  });

  it('pre-selects the recommended emulator for a ticked system', async () => {
    await $(testId('sys-tick-ps2')).click();
    await $(testId('sys-tick-dc')).click();
    // The catalog download stays recommended; the server's newest Linux
    // PCSX2 build is the alternative, and is what this run installs.
    await expect($(testId('sys-pick-ps2'))).toHaveValue('PCSX2/pcsx2');
    const ps2 = await groups('ps2');
    expect(ps2.Installed).toBeUndefined();
    expect(ps2.Download.slice(0, 2)).toEqual([
      'PCSX2/pcsx2=PCSX2 (recommended)',
      `${SERVER_PCSX2}=PCSX2 2.0.2 · from your server`,
    ]);
    await selectValue('sys-pick-ps2', SERVER_PCSX2);
    await expect($(testId('sys-pick-ps2'))).toHaveValue(SERVER_PCSX2);
    // SNES offers RetroArch only (with its core). Ticked to read the picker,
    // then unticked: the mock forge cannot serve RetroArch.
    await $(testId('sys-tick-snes')).click();
    await $(testId('sys-pick-snes')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    const snes = (await groups('snes')).Download ?? [];
    await $(testId('sys-tick-snes')).click();
    await $(testId('sys-pick-snes')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    expect(snes).toHaveLength(1);
    expect(snes[0]).toMatch(/=RetroArch · \S+ core \(recommended\)$/);
    await expect($(testId('setup-install'))).toHaveText('Install 2 emulators');
  });

  it('pre-selects GE-Proton for Windows and counts it with the emulators', async () => {
    // The Proton catalog loads beside the rows.
    await browser.waitUntil(async () => $(testId('sys-tick-windows')).isEnabled(), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Windows row never offered a Proton version',
    });
    await $(testId('sys-tick-windows')).click();
    await expect($(testId('sys-pick-windows'))).toHaveValue(`install:${GE_PROTON}`);
    const windows = await groups('windows');
    expect(windows.Download[0]).toBe(`install:${GE_PROTON}=Install GE-Proton (latest, recommended)`);
    await expect($(testId('setup-install'))).toHaveText('Install 3 emulators');
    await screenshot('2-picked');
  });

  it('lands in the app while everything installs, one bar segment per install', async () => {
    await $(testId('setup-install')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the shell never opened after starting setup',
    });
    await $(testId('setup-progress')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT });
    // One segment per install, Proton included, each named by its title.
    const segs = await browser.execute(() =>
      Array.from(document.querySelectorAll('[data-testid^="setup-seg-"]')).map((s) => ({
        id: (s as HTMLElement).dataset.testid!.replace('setup-seg-', ''),
        state: (s as HTMLElement).dataset.state ?? '',
        title: s.getAttribute('title') ?? '',
      })),
    );
    expect(segs.map((s) => s.id).sort()).toEqual(['GloriousEggroll/proton-ge-custom', REDREAM_SOURCE, SERVER_PCSX2].sort());
    for (const s of segs) {
      expect(['waiting', 'running', 'done']).toContain(s.state);
      expect(s.title).toMatch(/^(PCSX2|Redream|GE-Proton) · (Waiting|Installing|Installed)$/);
    }
    await expect($(segment(GE_PROTON))).toHaveAttribute('title', expect.stringMatching(/^GE-Proton · /));
    await screenshot('3-installing');

    let title = '';
    await browser
      .waitUntil(
        async () => {
          title = await $(testId('setup-progress-title')).getText();
          return title === '3 installs done';
        },
        { timeout: SETUP_TIMEOUT },
      )
      .catch(() => undefined);
    if (title !== '3 installs done') {
      // What the card and the Downloads rows said, for the failure report.
      const rows = await browser.execute(() =>
        Array.from(document.querySelectorAll('[data-testid^="download-detail-"]')).map(
          (d) => `${(d as HTMLElement).dataset.testid}: ${d.textContent?.trim()}`,
        ),
      );
      console.log(`setup card: ${title}\ndownload rows:\n${rows.join('\n')}`);
    }
    expect(title).toBe('3 installs done');
    for (const source of [SERVER_PCSX2, REDREAM_SOURCE, GE_PROTON]) {
      await expect($(segment(source))).toHaveAttribute('data-state', 'done');
    }
    await expect($(segment(GE_PROTON))).toHaveAttribute('title', 'GE-Proton · Installed');
    await browser.waitUntil(
      async () => (await $(testId('setup-progress-ready')).getText()) === '3 systems are ready to play.',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the card never said three systems are ready',
      },
    );
    await screenshot('4-done');
    // Done, the card counts down (it would close itself after 10 s); Dismiss
    // closes it at once. Done here, before the slower checks below, so the
    // countdown cannot beat the click.
    await $(testId('setup-progress-dismiss')).click();
    await expect($(testId('setup-progress'))).not.toExist();

    // The chosen defaults land once each entry exists (the install hook).
    await browser.waitUntil(
      () => {
        const config = readFileSync(configPath(), 'utf-8');
        return (
          config.includes('"Sony PlayStation 2" = "PCSX2 (Playstation 2)"') &&
          config.includes('"Sega Dreamcast" = "Redream (Sega Dreamcast)"')
        );
      },
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the chosen defaults never reached config.toml' },
    );
    // The Server view stayed in front the whole time: its header must pick
    // the new default up without being left and re-entered. Selecting a
    // platform in the rail re-reads nothing, so Dreamcast's chip can only show
    // Redream if the view refreshed its defaults while it stayed open.
    await $(testId('platform-btn-2')).click();
    await browser.waitUntil(
      async () => (await $(testId('server-emulator-chip')).getText()).includes('Redream'),
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: "the Server header never showed Dreamcast's new default" },
    );
    const config = readFileSync(configPath(), 'utf-8');
    // PCSX2 came from the server's build, not from the catalog download,
    // and registered with its folder, emulator and version.
    expect(config).toMatch(/path = ".*\/emulators\/PCSX2 \(Playstation 2\)\/[^"]*pcsx2[^"]*"/i);
    expect(config).toContain(`source_id = "${SERVER_PCSX2}"`);
    expect(config).toContain('source_installed_tag = "2.0.2"');
    // Only the newest Linux build was downloaded: not the older one, not the
    // Windows one.
    const requests = (await (await fetch(`${mockUrl()}/__e2e__/requests`)).json()) as { path: string }[];
    const fetched = requests.filter((r) => r.path.startsWith('/api/roms/505/content/')).map((r) => r.path);
    expect(fetched.length).toBeGreaterThan(0);
    expect(fetched.every((p) => p.includes('file_ids=5052'))).toBe(true);
    expect(config).toMatch(/\[setup\]\s*\ncompleted = true/);
    expect(config).not.toContain('[setup.pending');
    // GE-Proton is the default Proton version, and nothing waits any more.
    expect(config).toMatch(/default_compat_tool = ".*GE-Proton[^"]*"/);
    expect(config).not.toMatch(/pending_compat = "\S/);

  });

  it('carries on in Settings › Systems, where Install recommended replaces the chooser', async () => {
    await openSystems();
    await expect($(testId('sys-status-ps2'))).toHaveAttribute('data-status', 'ready');
    await expect($(testId('sys-status-dc'))).toHaveAttribute('data-status', 'ready');
    await expect($(testId('sys-status-windows'))).toHaveAttribute('data-status', 'ready');
    // The server's PCSX2 build is installed: the picker lists it under Installed.
    expect(((await groups('ps2')).Installed ?? []).some((o) => o.startsWith('use:PCSX2 (Playstation 2)'))).toBe(true);
    // SNES (RetroArch) still has no emulator; Zeebo has nothing installable.
    await expect($(testId('systems-install-recommended'))).toHaveText('Install recommended (1)');
    await expect($(testId('setup-view'))).not.toExist();
  });
});
