<script lang="ts">
  // Game settings for a native game (items 342–345; tvmode.js
  // `gameSettings`): the title with "Game settings" under it, then Launch
  // file (a list of the game's launch files; "Set by the game" when it came
  // from there, else "Your choice"), Launch parameters (the keyboard),
  // the compatibility tool where one applies (the desktop dialog's), Reload
  // launch info from server, and Save, which goes back. Loading, saving and
  // reloading are the shared nativeSettingsStore's.
  import type { CompatTool, NativeGameSettings } from '../../api';
  import { candidateLabel, compatToolNotice, installDirOf, isWindowsHost } from '../../details/actions';
  import { loadNativeSettings, reloadFromServer, saveNativeSettings } from '../../details/nativeSettingsStore';
  import { refresh as refreshInstalled } from '../../stores/installed.svelte';
  import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
  import { popPage } from '../nav.svelte';
  import { openKeyboard, openList } from '../overlays/overlays.svelte';
  import type { Row } from '../rows';
  import TvPage from '../ui/TvPage.svelte';
  import TvRows from '../ui/TvRows.svelte';
  import { GAME_SETTINGS_SAVED, LAUNCH_INFO_RELOADED, launchFileSub } from './details';

  let { romId, title, platform, platformSlug }: { romId: number; title: string; platform: string; platformSlug: string } = $props();

  const windowsHost = isWindowsHost(typeof navigator === 'undefined' ? '' : navigator.platform);
  // svelte-ignore state_referenced_locally
  const wantsCompat = !windowsHost && compatToolNotice(platformSlug, platform) === '';

  let settings = $state<NativeGameSettings | null>(null);
  let loadError = $state<string | null>(null);
  let exe = $state('');
  let params = $state('');
  let compat = $state('');
  let tools = $state<CompatTool[]>([]);
  let toolsError = $state('');
  let busy = $state(false);

  async function load() {
    try {
      const r = await loadNativeSettings(romId, wantsCompat);
      settings = r.settings;
      exe = r.executable;
      params = r.parameters;
      if (r.compat && 'error' in r.compat) toolsError = r.compat.error;
      else if (r.compat) {
        tools = r.compat.tools;
        compat = r.compat.selected;
      }
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err);
    }
  }
  void load();

  let installDir = $derived(settings?.install_dir || installDirOf(settings?.candidates ?? []));

  async function reload() {
    if (busy) return;
    busy = true;
    const answer = await reloadFromServer(romId);
    if (answer.failed) pushToast(answer.message, 'error');
    else pushToast(LAUNCH_INFO_RELOADED, 'success', TOAST_DURATION_MS, { sub: answer.message });
    if (answer.answered) await load();
    busy = false;
  }

  async function save() {
    if (busy || !settings) return;
    busy = true;
    try {
      await saveNativeSettings(romId, exe, params, compat);
      pushToast(GAME_SETTINGS_SAVED, 'success');
      void refreshInstalled();
      void popPage();
    } catch (err) {
      pushToast(err instanceof Error ? err.message : String(err), 'error');
    } finally {
      busy = false;
    }
  }

  let rows = $derived.by((): Row[] => {
    if (!settings) return [];
    const s = settings;
    const out: Row[] = [];
    if (s.candidates.length === 0) {
      out.push({ key: 'exe', type: 'info', label: 'Launch file', value: '', sub: "No game file to launch was found." });
    } else {
      out.push({
        key: 'exe',
        type: 'choice',
        label: 'Launch file',
        sub: launchFileSub(s.launch_source, exe, s.executable),
        value: exe,
        options: s.candidates.map((c) => [c, candidateLabel(c, installDir)] as const),
        choose: () =>
          void openList({
            title: 'Launch file',
            items: s.candidates.map((c) => ({ label: candidateLabel(c, installDir), checked: c === exe, onPick: () => (exe = c) })),
          }),
      });
    }
    out.push({
      key: 'params',
      type: 'action',
      label: 'Launch parameters',
      sub: 'Optional',
      value: params || 'None',
      run: () => void openKeyboard({ title: 'Launch parameters', value: params, onDone: (v) => (params = v) }),
    });
    if (wantsCompat) {
      const options = [['', 'None'] as const, ...tools.map((t) => [t.path, t.name] as const)];
      out.push({
        key: 'compat',
        type: 'choice',
        label: 'Compatibility tool',
        ...(toolsError ? { sub: toolsError } : {}),
        value: compat,
        options,
        choose: () =>
          void openList({
            title: 'Compatibility tool',
            items: options.map(([v, l]) => ({ label: l, checked: v === compat, onPick: () => (compat = v) })),
          }),
      });
    }
    out.push({ key: 'reload', type: 'action', label: 'Refresh from server', run: () => void reload() });
    out.push({
      key: 'save',
      type: 'action',
      label: 'Save',
      run: () => void save(),
      ...(s.candidates.length === 0 ? { disabled: true, why: "No game file to launch was found." } : {}),
    });
    return out;
  });
</script>

<div data-testid="tv-game-settings">
  <TvPage {title} sub="Game settings">
    {#if loadError}<p class="note err">{loadError}</p>{/if}
    <TvRows {rows} />
  </TvPage>
</div>
