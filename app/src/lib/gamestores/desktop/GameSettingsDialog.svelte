<script lang="ts">
  // Game Settings for a store game (GameStores README, "Game Settings";
  // prototype `stSettingsHTML`): install folder (read-only), executable,
  // launch parameters, compatibility tool and Wine prefix (Windows) or
  // "Linux games run directly.", and the per-game cloud saves switch.
  import { api, type StoreGame, type StoreGameSettings } from '../../api';
  import { pickFile } from '../../pickers';
  import { compatTools } from '../../stores/compatTools.svelte';
  import { cloudSwitchLabel, executableHelp, LINUX_RUNS_DIRECTLY } from '../details';
  import { gameSettingsSavedNotice, refusedNotice } from '../notices';
  import { hasStoreCloud } from '../marks';
  import StoreDialog from './StoreDialog.svelte';
  import { showCard } from './ui.svelte';

  let { game, onClose }: { game: StoreGame; onClose: () => void } = $props();

  let s = $state<StoreGameSettings | null>(null);
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let exeBefore = '';

  $effect(() => {
    const key = game.key;
    api
      .storeGameSettings(key)
      .then((got) => {
        s = got;
        exeBefore = got.executable;
      })
      .catch((err) => (loadError = err instanceof Error ? err.message : String(err)));
  });

  let windows = $derived(game.installed_platform === 'windows');
  let compatChoices = $derived.by(() => {
    const names = compatTools.tools.map((t) => t.name);
    const current = s?.compat_tool ?? '';
    return current !== '' && !names.includes(current) ? [current, ...names] : names;
  });

  async function chooseExe() {
    const picked = await pickFile(`Executable for ${game.title}`);
    if (picked && s) s = { ...s, executable: picked };
  }

  async function save() {
    if (!s) return;
    saving = true;
    // Picking another executable makes it the user's choice (store updates leave it).
    const next = { ...s, exe_user_pinned: s.exe_user_pinned || s.executable !== exeBefore };
    try {
      await api.storeSetGameSettings(game.key, next);
      showCard(gameSettingsSavedNotice());
      onClose();
    } catch (err) {
      showCard(refusedNotice('Couldn’t save Game Settings', err instanceof Error ? err.message : String(err)));
    } finally {
      saving = false;
    }
  }
</script>

<StoreDialog testId="store-game-settings" label={`Game Settings · ${game.title}`} {onClose}>
  <h3>Game Settings</h3>
  {#if loadError}
    <p class="error" role="alert">{loadError}</p>
  {:else if s}
    <div class="st-field"><span>Install folder</span><code data-testid="store-settings-folder">{s.install_dir}</code></div>
    <label class="st-field">
      <span>Executable</span>
      <span class="inline">
        <input class="g-field" data-testid="store-settings-exe" bind:value={s.executable} />
        <button class="small-btn" type="button" onclick={chooseExe}>Choose…</button>
      </span>
    </label>
    <p class="help">{executableHelp(game.source)}</p>
    <label class="st-field">
      <span>Launch parameters</span>
      <input class="g-field" data-testid="store-settings-args" placeholder="Optional" bind:value={s.launch_parameters} />
    </label>
    {#if windows}
      <label class="st-field">
        <span>Compatibility tool</span>
        <select class="g-field" data-testid="store-settings-compat" bind:value={s.compat_tool}>
          {#each compatChoices as t (t)}<option value={t}>{t}</option>{/each}
        </select>
      </label>
      <div class="st-field"><span>Wine prefix (read-only)</span><code>{s.wineprefix}</code></div>
    {:else}
      <p class="help">{LINUX_RUNS_DIRECTLY}</p>
    {/if}
    {#if hasStoreCloud(game.source) && game.cloud !== 'none'}
      <label class="check">
        <input type="checkbox" data-testid="store-settings-cloud" bind:checked={s.cloud_saves} />
        <span>{cloudSwitchLabel(game.source)}</span>
      </label>
    {/if}
  {:else}
    <p class="note">Loading…</p>
  {/if}
  <div class="foot">
    <span></span>
    <button class="g-btn secondary" onclick={onClose}>Cancel</button>
    <button class="g-btn" data-testid="store-settings-save" disabled={!s || saving} onclick={save}>Save</button>
  </div>
</StoreDialog>
