import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  configPath,
  dataDir,
  FIXTURE_TOKEN,
  mockUrl,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';
import { waitForToast } from '../helpers/toasts.js';
import {
  addManual,
  configEmulatorNames,
  editEmulator,
  findLine,
  groups,
  hasLine,
  lineKey,
  openRow,
  openSystems,
  pick,
  removeEmulator,
  rowOrder,
  saveSheet,
  selectValue,
  startManualAdd,
  tipsText,
} from '../helpers/systems.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/**
 * Stage `emulators`: Settings › Systems' emulator flows on the default
 * fixtures (SNES, Arcade, and Windows 9x, which runs natively and so has no
 * system row): manual add with autofill, the settings sheet (edit, rename
 * in place, duplicate rejection), two-click Remove, the setup tips, and each
 * system's picker and core select, asserted against the real config.toml on
 * disk.
 *
 * Only the Open case spawns the emulator; every other flow
 * here just needs the stub to exist on disk with its executable bit set,
 * because `match_profile` (grid-core's `profile_for_entry`) matches purely
 * on the path string, not on file contents. The stub therefore touches a
 * marker file, which is how the launch case proves the spawn happened.
 */
describe('emulators', () => {
  let stubPath: string;
  let launchMarker: string;

  before(async () => {
    const stubsDir = path.join(dataDir(), 'stubs');
    mkdirSync(stubsDir, { recursive: true });
    // Basename "retroarch" is a literal `emulator-autoprofiles.json`
    // match_tokens entry (root/emulator-autoprofiles.json), matching the
    // "RetroArch (Multi-System)" profile: name "RetroArch (Multi-System)",
    // args `-L "%core%" "%rom%"`.
    stubPath = path.join(stubsDir, 'retroarch');
    launchMarker = path.join(stubsDir, 'retroarch.launched');
    // The Open case below is the only thing in this group that runs the stub; the marker is how it proves the spawn happened.
    writeFileSync(stubPath, `#!/bin/sh\ntouch '${launchMarker}'\nexit 0\n`);
    chmodSync(stubPath, 0o755);

    // Design D-RC-1: RetroArch's platform support is now decided by the
    // core files installed beside its executable, so the stub needs a
    // `cores/` sibling. Two SNES cores in the bundled slug map's curated
    // order (romm-platform-cores.json maps "snes" to
    // ["snes9x", "snes9x2010", "bsnes"]) and no Arcade core at all, so
    // platform 1 offers RetroArch and platform 2 (Arcade) does not.
    const coresDir = path.join(stubsDir, 'cores');
    mkdirSync(coresDir, { recursive: true });
    for (const core of ['snes9x', 'bsnes']) {
      writeFileSync(path.join(coresDir, `${core}_libretro.so`), '');
    }

    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });

    await openSystems();
  });

  const SNES = 'Super Nintendo Entertainment System';
  const SNES_LINE = lineKey(SNES);
  const config = () => readFileSync(configPath(), 'utf-8');

  /** Waits until config.toml contains `line`, or fails with a useful message. */
  async function waitForConfigLine(line: string) {
    await browser.waitUntil(
      () => {
        try {
          return config().includes(line);
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: `config.toml never contained ${line}`,
      },
    );
  }

  it('lists one row per system that needs an emulator, then Windows', async () => {
    await browser.waitUntil(async () => (await rowOrder()).length === 3, {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the system rows never appeared',
    });
    // Nothing is configured yet in this group: both systems lack an
    // emulator, so both come first, alphabetically, ahead of Windows (a
    // Linux host: Proton is set up there).
    expect(await rowOrder()).toEqual(['arcade', 'snes', 'windows']);
    // Platform 3 (Windows 9x) runs natively (design §9): no system row and
    // no emulator picker.
    await expect($(testId('sys-row-win'))).not.toExist();
    await expect($(testId('sys-pick-win'))).not.toExist();
    await expect($(testId('systems-add-manual'))).toBeDisplayed();
    await expect($(testId('systems-other-empty'))).toBeDisplayed();
  });

  it('auto-fills name and args from a profile-matching path, then saves the entry', async () => {
    await startManualAdd({ path: stubPath });
    await expect($(testId('emu-form-name'))).toHaveValue('');
    await expect($(testId('emu-form-args'))).toHaveValue('');
    // Blur the path field (autoFillFromPath runs onblur) by moving focus to
    // the name field.
    await $(testId('emu-form-name')).click();

    await browser.waitUntil(async () => (await $(testId('emu-form-name')).getValue()) !== '', {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the name field never auto-filled from the matched profile',
    });
    await expect($(testId('emu-form-name'))).toHaveValue('RetroArch (Multi-System)');
    await expect($(testId('emu-form-args'))).toHaveValue('-L "%core%" "%rom%"');

    // The five per-emulator cloud fields (parity gap 1), read back from
    // config.toml and the settings sheet by the case below.
    await $(testId('emu-form-advanced')).click();
    await selectValue('emu-form-save-strategy', 'folder');
    await $(testId('emu-form-ignore-files')).setValue('skip.bin;other.bin');
    await $(testId('emu-form-ignore-extensions')).setValue('.tmp;.log');
    await $(testId('emu-form-save-paths')).setValue('saves');
    await $(testId('emu-form-state-paths')).setValue('states');

    await saveSheet();
    // The global toast surface (parity gap 5) with the reference's text
    // (emulator_ui_mixin.py:1591); any card in the stack may hold it.
    await waitForToast("Added emulator 'RetroArch (Multi-System)'.", {
      timeoutMsg: 'no toast appeared after adding an emulator',
    });
    // Two SNES cores are installed beside it, so RetroArch plays SNES: it is
    // the emulator a SNES launch would use (the first compatible one), and
    // the picker lists it under Installed. A manual add sets no default.
    expect(await findLine('RetroArch (Multi-System)')).toBe(SNES_LINE);
    // RetroArch is SNES's recommendation (no standalone emulator serves it).
    expect((await groups('snes')).Installed).toEqual([
      'use:RetroArch (Multi-System)=RetroArch (Multi-System) (recommended)',
    ]);
    expect(config()).not.toContain(`"${SNES}" = "RetroArch (Multi-System)"`);
  });

  it('writes the five per-emulator cloud fields to config.toml and reloads them into the settings sheet', async () => {
    await waitForConfigLine('save_strategy = "folder"');
    await waitForConfigLine('ignore_files = "skip.bin;other.bin"');
    await waitForConfigLine('ignore_extensions = ".tmp;.log"');
    await waitForConfigLine('save_paths = "saves"');
    await waitForConfigLine('state_paths = "states"');

    await openRow('snes');
    await editEmulator(SNES_LINE);
    await expect($(testId('emu-form-path'))).toHaveValue(stubPath);
    await expect($(testId('emu-form-args'))).toHaveValue('-L "%core%" "%rom%"');
    await expect($(testId('emu-form-save-strategy'))).toHaveValue('folder');
    await expect($(testId('emu-form-ignore-files'))).toHaveValue('skip.bin;other.bin');
    await expect($(testId('emu-form-ignore-extensions'))).toHaveValue('.tmp;.log');
    await expect($(testId('emu-form-save-paths'))).toHaveValue('saves');
    await expect($(testId('emu-form-state-paths'))).toHaveValue('states');
    await $(testId('emu-form-cancel')).click();
    await $(testId('systems-sheet')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('opens an installed emulator with no ROM and syncs its settings first', async () => {
    // The autoconfig sync also runs at ADD time, so the cfg the add wrote is
    // removed here: only the pre-launch sync can put it back.
    // `retroarch::config_path_candidates` targets `<dir>/retroarch.cfg` for a
    // bare executable with no portable `.home` sibling.
    const raConfig = path.join(path.dirname(stubPath), 'retroarch.cfg');
    rmSync(raConfig, { force: true });

    await openRow('snes');
    const open = $(testId(`sys-open-${SNES_LINE}`));
    await expect(open).toBeDisplayed();
    await open.click();
    await browser.waitUntil(() => existsSync(launchMarker), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the standalone launch never ran the emulator stub',
    });
    await browser.waitUntil(() => existsSync(raConfig), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the standalone launch never re-wrote retroarch.cfg',
    });
  });

  it('adds a second emulator and keeps entry order when renaming the first', async () => {
    // Something else: no profile, so the name is typed; this path matches
    // no profile either.
    await addManual({ name: 'AAA Manual Emulator', path: '/nonexistent/aaa-emulator' });
    await waitForConfigLine('name = "AAA Manual Emulator"');
    await findLine('AAA Manual Emulator');

    expect(configEmulatorNames(config())).toEqual(['RetroArch (Multi-System)', 'AAA Manual Emulator']);

    // Rename the FIRST entry and confirm it keeps its position —
    // apply_save_emulator (commands.rs) re-inserts an edited entry at its
    // original index rather than appending it.
    await editEmulator(await findLine('RetroArch (Multi-System)'));
    await expect($(testId('emu-form-name'))).toHaveValue('RetroArch (Multi-System)');
    await $(testId('emu-form-name')).setValue('RetroArch Renamed');
    await saveSheet();
    await waitForConfigLine('name = "RetroArch Renamed"');
    await findLine('RetroArch Renamed');
    expect(await hasLine('RetroArch (Multi-System)')).toBe(false);

    expect(configEmulatorNames(config())).toEqual(['RetroArch Renamed', 'AAA Manual Emulator']);
  });

  it('rejects saving a duplicate name with the verbatim error', async () => {
    await startManualAdd({ name: 'AAA Manual Emulator', path: '/nonexistent/dup' });
    await $(testId('emu-form-save')).click();

    await expect($(testId('emu-form-error'))).toHaveText("An emulator named 'AAA Manual Emulator' already exists.");
    await $(testId('emu-form-cancel')).click();
    await $(testId('systems-sheet')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
  });

  it('removes an emulator with a two-click confirm', async () => {
    await removeEmulator(await findLine('AAA Manual Emulator'), 'AAA Manual Emulator');
    await browser.waitUntil(() => !config().includes('name = "AAA Manual Emulator"'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the removed emulator was still in config.toml',
    });
  });

  it('shows the DuckStation tip and none of another emulator’s, and seeds its Vulkan renderer', async () => {
    // A real stub in its OWN directory: a manual add is a fresh install, so
    // the add-time autoconfig sync writes `settings.ini` beside the
    // executable and the renderer seed
    // (docs/superpowers/specs/2026-09-17-vulkan-renderer-seed-design.md)
    // must land in it. Its own directory keeps that file away from the
    // RetroArch stub. `delete_emulator` leaves hand-configured paths on
    // disk, so the cleanup below removes only the entry.
    const duckDir = path.join(dataDir(), 'stubs', 'duckstation');
    mkdirSync(duckDir, { recursive: true });
    const duckPath = path.join(duckDir, 'duckstation');
    writeFileSync(duckPath, '#!/bin/sh\nexit 0\n');
    chmodSync(duckPath, 0o755);

    await addManual({ name: 'DuckStation', path: duckPath });
    await findLine('DuckStation');

    const tips = await tipsText('DuckStation');
    expect(tips).toContain(
      'RetroAchievements: Configure login via Emulator Settings → Achievements (tokens are machine-encrypted)',
    );
    // Azahar's controller tip belongs to Azahar alone.
    expect(tips).not.toContain('Auto Map');

    // The sync runs after the config save, on the blocking pool; poll for
    // the file rather than reading it right away.
    const settingsIni = path.join(duckDir, 'settings.ini');
    await browser.waitUntil(() => existsSync(settingsIni), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: `the add-time autoconfig never wrote ${settingsIni}`,
    });
    const ini = readFileSync(settingsIni, 'utf-8');
    expect(ini).toContain('[GPU]');
    expect(ini).toContain('Renderer = Vulkan');

    // Clean up so the picker cases below still see RetroArch alone.
    await removeEmulator(await findLine('DuckStation'), 'DuckStation');
  });

  it('shows both Eden setup tips when keys and firmware are missing', async () => {
    await addManual({ name: 'Eden', path: '/nonexistent/eden/eden.exe' });
    await findLine('Eden');

    // The probes run in the backend, so the tips arrive a tick after the line.
    await browser.waitUntil(async () => (await tipsText('Eden')).includes('prod.keys'), {
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the Eden keys tip never appeared',
    });
    const tips = await tipsText('Eden');
    expect(tips).toContain('Switch keys (prod.keys) must be placed in user/keys/ before playing games.');
    expect(tips).toContain('Switch firmware must be installed via Emulation → Install Firmware before playing games.');

    await removeEmulator(await findLine('Eden'), 'Eden');
  });

  it('picks a system’s emulator and records a core in config.toml', async () => {
    await pick('snes', 'use:RetroArch Renamed');
    await waitForConfigLine(`"${SNES}" = "RetroArch Renamed"`);
    // D-RC-4: picking RetroArch also records the first installed compatible
    // core, which the slug map orders snes9x before bsnes.
    await waitForConfigLine(`"${SNES}" = "snes9x"`);
  });

  it('lists the installed cores for the RetroArch system, in slug-map order', async () => {
    await openRow('snes');
    await $(testId('sys-core-snes')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the core select never appeared for RetroArch on SNES',
    });
    const cores = await browser.execute(() =>
      Array.from((document.querySelector('[data-testid="sys-core-snes"]') as HTMLSelectElement).options).map((o) => o.value),
    );
    expect(cores).toEqual(['snes9x', 'bsnes']);
  });

  it('changing the core rewrites the retroarch_cores line', async () => {
    await selectValue('sys-core-snes', 'bsnes');
    await waitForConfigLine(`"${SNES}" = "bsnes"`);
  });

  it('None records the <none> marker and clears the saved core', async () => {
    await pick('snes', '');
    await browser.waitUntil(
      () => {
        try {
          const contents = config();
          return (
            contents.includes(`"${SNES}" = "<none>"`) &&
            !contents.includes(`"${SNES}" = "bsnes"`) &&
            !contents.includes(`"${SNES}" = "RetroArch Renamed"`)
          );
        } catch {
          return false;
        }
      },
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg:
          'config.toml did not hold the <none> marker, or still had a retroarch_cores line for the platform, after picking None',
      },
    );
  });

  it('the None choice survives leaving and re-entering the page', async () => {
    // Coming forward re-runs list_platforms, which is where the autoconfig
    // backfill used to re-assign RetroArch over a cleared default.
    await $(testId('nav-server')).click();
    await $(testId('settings-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the settings view never went away',
    });
    await openSystems();

    await expect($(testId('sys-pick-snes'))).toHaveValue('');
    await expect($(testId('sys-status-snes'))).toHaveAttribute('data-status', 'none');
    await expect($(testId('sys-core-snes'))).not.toExist();
    expect(config()).toContain(`"${SNES}" = "<none>"`);
  });

  it('does not offer RetroArch for a system with no installed core', async () => {
    // Arcade needs fbneo/mame2003_plus/mame; only SNES cores are installed,
    // so D-RC-1's gate keeps RetroArch out of its picker even though its
    // autoprofile sets all_platforms: true.
    const arcade = await groups('arcade');
    expect(arcade.Installed ?? []).not.toContain('use:RetroArch Renamed=RetroArch Renamed');
    expect(JSON.stringify(arcade)).not.toContain('RetroArch Renamed');
  });
});
