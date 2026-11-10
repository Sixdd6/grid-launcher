import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

// The commands are mocked at the `invoke` boundary, so these run the real
// api wrappers, notifier and cards: the outcome each screen reacts to, and
// the cards and registry refreshes each action owns.

type Answer = (args: Record<string, unknown> | undefined) => unknown;

describe('games/run', () => {
  let answers: Record<string, Answer>;
  let invoked: string[];

  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    answers = { list_installed: () => [] };
    invoked = [];
    vi.doMock('@tauri-apps/api/core', () => ({
      invoke: async (cmd: string, args?: Record<string, unknown>) => {
        invoked.push(cmd);
        const answer = answers[cmd];
        if (answer === undefined) return undefined;
        return answer(args);
      },
      convertFileSrc: (p: string) => p,
    }));
    vi.doMock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.doUnmock('@tauri-apps/api/core');
    vi.doUnmock('@tauri-apps/api/event');
  });

  async function load() {
    const run = await import('./run');
    const { toasts } = await import('../stores/toasts.svelte');
    const cards = () => {
      vi.advanceTimersByTime(3000);
      return toasts.list.map((t) => [t.text, t.level]);
    };
    return { run, cards };
  }

  function refuse(reason: unknown): Answer {
    return () => {
      throw reason;
    };
  }

  it('plays an installed game and announces it', async () => {
    const { run, cards } = await load();
    expect(await run.playInstalled(7, 'Zelda')).toEqual({ kind: 'ok' });
    expect(invoked).toContain('launch_game');
    expect(cards()).toEqual([['Launching Zelda…', 'success']]);
  });

  it('reads a launch with no umu-run as needs-umu', async () => {
    answers.launch_game = refuse({ code: 'umu_missing', message: 'umu-run was not found' });
    const { run } = await load();
    expect(await run.playInstalled(7, 'Halo')).toEqual({ kind: 'needs-umu', reason: 'umu-run was not found' });
  });

  it('reads any other launch failure as refused, with a card', async () => {
    answers.launch_game = refuse({ code: '', message: 'No emulator' });
    const { run, cards } = await load();
    expect(await run.playInstalled(7, 'Zelda')).toEqual({ kind: 'refused', reason: 'No emulator' });
    expect(cards().map(([text]) => text)).toContain("Couldn't launch Zelda: No emulator");
  });

  it('queues an install, and reads an unset library folder', async () => {
    const { run } = await load();
    expect(await run.installServerGame(7, 'Zelda')).toEqual({ kind: 'ok' });
    answers.install_game = refuse('Set a library folder in Settings › Library before installing games');
    expect(await run.installServerGame(7, 'Zelda')).toEqual({
      kind: 'needs-library-folder',
      reason: 'Set a library folder in Settings › Library before installing games',
    });
  });

  it('reads another install refusal as refused', async () => {
    answers.install_game = refuse('Already queued');
    const { run } = await load();
    expect(await run.installServerGame(7, 'Zelda')).toEqual({ kind: 'refused', reason: 'Already queued' });
  });

  it('installs from a drive and reports a refusal', async () => {
    answers.install_from_drive = () => ({ notices: [] });
    const { run } = await load();
    const game = { rom_id: 7, name: 'Zelda', server: 'https://romm' };
    expect(await run.installFromDrive('drv', game)).toEqual({ kind: 'ok' });
    answers.install_from_drive = refuse('Drive is busy');
    expect((await run.installFromDrive('drv', game)).kind).toBe('refused');
  });

  it('uninstalls with a card, a kept-folders card, and a registry refresh', async () => {
    answers.uninstall_game = () => ({ skipped: [{ path: '/games/zelda', reason: 'not empty' }] });
    const { run, cards } = await load();
    expect(await run.uninstallGame(7, 'Zelda')).toEqual({ kind: 'ok' });
    expect(invoked).toEqual(['uninstall_game', 'list_installed']);
    const shown = cards();
    expect(shown[0][1]).toBe('error');
    expect(shown).toContainEqual(['Zelda uninstalled', 'success']);
  });

  it('reports a refused uninstall without refreshing', async () => {
    answers.uninstall_game = refuse('Game is running');
    const { run } = await load();
    expect(await run.uninstallGame(7, 'Zelda')).toEqual({ kind: 'refused', reason: 'Game is running' });
    expect(invoked).toEqual(['uninstall_game']);
  });

  it('removes from the library with a card and a refresh', async () => {
    const { run, cards } = await load();
    expect(await run.removeFromLibrary('Zelda', 'SNES')).toEqual({ kind: 'ok' });
    expect(invoked).toEqual(['remove_from_library', 'list_installed']);
    expect(cards()).toEqual([['Zelda removed from the library', 'success']]);
  });

  it('locates and links with a refresh, and passes a refusal through', async () => {
    const { run } = await load();
    expect(await run.locateGame('Zelda', 'SNES', ' /games/zelda ')).toEqual({ kind: 'ok' });
    expect(await run.linkGame('Zelda', 'SNES', 7)).toEqual({ kind: 'ok' });
    expect(invoked).toEqual(['locate_installed_row', 'list_installed', 'link_installed_row', 'list_installed']);
    answers.link_installed_row = refuse('Already linked');
    expect(await run.linkGame('Zelda', 'SNES', 7)).toEqual({ kind: 'refused', reason: 'Already linked' });
  });

  it('trims the located folder', async () => {
    let folder: unknown = null;
    answers.locate_installed_row = (args) => {
      folder = args?.folder;
    };
    const { run } = await load();
    await run.locateGame('Zelda', 'SNES', ' /games/zelda ');
    expect(folder).toBe('/games/zelda');
  });

  it('cancels, updates, installs content and stops, each with its refusal', async () => {
    const { run } = await load();
    expect(await run.cancelInstall(7)).toEqual({ kind: 'ok' });
    expect(await run.updateGame(7)).toEqual({ kind: 'ok' });
    expect(await run.installContent(7, 'dlc')).toEqual({ kind: 'ok' });
    expect(await run.stopGame(3)).toEqual({ kind: 'ok' });
    expect(invoked).toEqual(['cancel_download_for_rom', 'update_game', 'install_content', 'stop_game']);
    answers.stop_game = refuse(new Error('No such session'));
    expect(await run.stopGame(3)).toEqual({ kind: 'refused', reason: 'No such session' });
  });
});
