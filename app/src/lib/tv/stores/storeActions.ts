// What TV mode's store screens do (prototype `tvstores.js` `ACT.*`, `sPlay`,
// `sLogin`, `sOpenInstaller`, `sRestore`): play with the cloud check first,
// the install page, the login window, the offline installer, the backup page,
// and the store's Settings page. Each goes through `api` and the shared
// `gameStores` store; refusals are error cards.
import { api, type StoreGame, type StoreSource } from '../../api';
import { childPath } from '../pages/folderBrowser';
import { toolNeededNotice } from '../../gamestores/notices';
import { managedByClient, storeName, storeTool } from '../../gamestores/marks';
import { steamClientMissing } from '../../gamestores/steam';
import { gameStores, refreshLibrary } from '../../stores/gameStores.svelte';
import { pushToast, TOAST_DURATION_MS } from '../../stores/toasts.svelte';
import { toLaunchFailure } from '../../launchError';
import { compatTools } from '../../stores/compatTools.svelte';
import { openFolderBrowser } from '../gameActions';
import { closeLayer, nav, openLayer, pushPage, rememberFocus, setPane, showTab, topLayer } from '../nav.svelte';
import { openList } from '../overlays/overlays.svelte';
import { conflictPrompt, keptCard, launchingCard, PICK_INSTALLER_TITLE, READING_INSTALLER, fileName, refusalTitle, tvInspectionView } from './storeWords';
import { openStoreFold, storeRowKey, bumpDrives } from './storeState.svelte';
import { selectLibStore } from '../tabs/library/library.svelte';
import TvStoreDetails from './TvStoreDetails.svelte';
import TvStoreInstallPage from './TvStoreInstallPage.svelte';
import TvStorePlaying from './TvStorePlaying.svelte';
import TvStorePage from './TvStorePage.svelte';
import TvSteamPage from './TvSteamPage.svelte';
import TvStoreLogin from './TvStoreLogin.svelte';
import TvRefusal from './TvRefusal.svelte';
import TvOfflineInstallPage from './TvOfflineInstallPage.svelte';
import TvStoreBackupPage from './TvStoreBackupPage.svelte';

const errText = (err: unknown) => (err instanceof Error ? err.message : String(err));
export const fail = (err: unknown) => void pushToast(errText(err), 'error');

/** A card with an optional second line and action. */
export function card(text: string, level: 'success' | 'error' = 'success', sub?: string, action?: { label: string; run: () => void }): void {
  pushToast(text, level, TOAST_DURATION_MS, { ...(sub ? { sub } : {}), ...(action ? { action } : {}) });
}

export const toDownloads = { label: 'Open Downloads', run: () => void showTab('downloads') };

/** The compatibility tool a new Windows game runs under (Settings › Systems › Windows). */
export function defaultCompat(): string {
  return compatTools.defaultTool || compatTools.tools[0]?.name || '';
}

/** The compatibility tools a list offers, the default first. */
export function compatChoices(): string[] {
  const names = compatTools.tools.map((t) => t.name).filter((n) => n.trim() !== '');
  const def = defaultCompat();
  return def && !names.includes(def) ? [def, ...names] : names;
}

// ── The game page ───────────────────────────────────────────────────────

/** Opens a store game's page; `drive` when it comes from a drive's grid. */
export function openStoreGame(key: string, title: string, drive: { key: string; name: string } | null = null): void {
  void pushPage({ title, component: TvStoreDetails, props: { gameKey: key, drive } });
}

/** Install…: the install page, or why it can't open. Steam has no install page: Steam's own window opens. */
export function openInstall(game: StoreGame): void {
  if (managedByClient(game.source)) {
    void steamInstall(game);
    return;
  }
  const status = gameStores.status(game.source);
  if (!status || status.connection !== 'connected') {
    card(`Sign in to ${storeName(game.source)} again to install ${game.title}`, 'error', undefined, { label: 'Sign in again', run: () => startLogin(game.source) });
    return;
  }
  if (storeTool(game.source) !== null && status.tool.state !== 'installed') {
    const n = toolNeededNotice(game.source);
    card(n.text, 'error', undefined, { label: `Install ${storeTool(game.source)!.name}`, run: () => installTool(game.source) });
    return;
  }
  void pushPage({ title: `Install ${game.title}`, component: TvStoreInstallPage, props: { gameKey: game.key } });
}

/** X on a cover: Play, Install… or Install in Steam. */
export function quickStore(game: StoreGame, quick: 'Play' | 'Install…' | 'Install in Steam' | null): void {
  if (quick === 'Play') void playStore(game);
  else if (quick !== null) openInstall(game);
}

// ── Steam: everything goes through the Steam client ─────────────────────

/** Install in Steam (prototype `ACT.sinstall`): Steam's window opens over TV mode; Downloads shows its progress. */
export async function steamInstall(game: StoreGame): Promise<void> {
  if (steamClientMissing(gameStores.status(game.source))) {
    card('Steam isn’t installed', 'error', 'Install Steam, then try again.');
    return;
  }
  try {
    await api.steamOpen(game.key, 'install');
  } catch (err) {
    fail(err);
    return;
  }
  card(`Steam opened to install ${game.title}`, 'success', 'Choose the library folder there; GRID shows the progress. Steam’s window opens over TV mode.');
}

/** Verify in Steam (prototype `ACT.sverify`). */
export function steamVerify(game: StoreGame): void {
  api.steamOpen(game.key, 'validate').then(() => card(`Steam is verifying ${game.title}`, 'success', 'Watch it in Steam'), fail);
}

/** Uninstall in Steam… (prototype `ACT.suninstall`): Steam asks; the game leaves once Steam removes it. */
export function steamUninstall(game: StoreGame): void {
  api.steamOpen(game.key, 'uninstall').then(() => card(`Steam opened its uninstall window for ${game.title}`, 'success', 'It leaves the Library once Steam removes it.'), fail);
}

/** Check again (prototype `ACT.srecheck`): finds the Steam client again. */
export async function steamRecheck(): Promise<void> {
  try {
    await api.steamRecheckClient();
  } catch (err) {
    fail(err);
    return;
  }
  await refreshLibrary('steam');
  if (!steamClientMissing(gameStores.status('steam'))) card('Steam client found');
}

/**
 * Play (prototype `sPlay`): the backend syncs cloud saves before the game
 * starts; a conflict comes back as `store_cloud_conflict` and opens the
 * choice first (Keep this computer's save, Use the cloud save, Don't launch).
 */
export async function playStore(game: StoreGame): Promise<void> {
  try {
    await api.launchGameKey(game.key);
  } catch (err) {
    const f = toLaunchFailure(err);
    if (managedByClient(game.source)) {
      card(f.message.trim() === '' ? `Couldn’t start ${game.title}` : f.message.trim(), 'error');
      return;
    }
    if (f.code === 'store_cloud_conflict') {
      await askConflict(game);
      return;
    }
    card(f.message.trim() === '' ? `Couldn’t launch ${game.title}` : `Couldn’t launch ${game.title}: ${f.message.trim()}`, 'error');
    return;
  }
  const c = managedByClient(game.source) ? { text: `Starting ${game.title} through Steam…`, sub: undefined } : launchingCard(game);
  card(c.text, 'success', c.sub);
  void pushPage({ title: 'Playing', component: TvStorePlaying, props: { gameKey: game.key, title: game.title } });
}

async function askConflict(game: StoreGame): Promise<void> {
  const state = await api.storeCloudState(game.key).catch(() => null);
  const p = conflictPrompt(game, state?.conflict ?? null, Math.floor(Date.now() / 1000));
  void openList({
    title: p.title,
    sub: p.sub,
    items: [
      { label: 'Keep this computer’s save', sub: p.local, testId: 'tv-store-conflict-local', onPick: () => void resolveThenPlay(game, 'local') },
      { label: 'Use the cloud save', sub: p.cloud, testId: 'tv-store-conflict-cloud', onPick: () => void resolveThenPlay(game, 'cloud') },
      { label: 'Don’t launch', testId: 'tv-store-conflict-none', onPick: () => {} },
    ],
  });
}

async function resolveThenPlay(game: StoreGame, which: 'local' | 'cloud'): Promise<void> {
  if (!(await resolveCloud(game, which))) return;
  await playStore(game);
}

/** Keeps one side of a conflict; answers whether it worked. */
export async function resolveCloud(game: StoreGame, which: 'local' | 'cloud'): Promise<boolean> {
  try {
    await api.storeCloudResolve(game.key, which === 'local' ? 'upload' : 'download');
  } catch (err) {
    fail(err);
    return false;
  }
  const c = keptCard(which, game);
  card(c.text, 'success', c.sub);
  void refreshLibrary(game.source);
  return true;
}

// ── The store and its login ─────────────────────────────────────────────

/** Settings › Stores, on its pane (prototype `ACT.sgoset`). */
export function goStoreSettings(): void {
  setPane('stores');
  rememberFocus('tab:settings', 'r-stores');
  void showTab('settings', { again: true });
}

/** A store's page in Settings. */
export function openStorePage(source: StoreSource): void {
  if (managedByClient(source)) {
    void pushPage({ title: storeName(source), component: TvSteamPage, props: {} });
    return;
  }
  void pushPage({ title: storeName(source), component: TvStorePage, props: { source } });
}

/** Show in Library: pages close, the Library shows the store's Installed. */
export function showStoreInLibrary(source: StoreSource): void {
  openStoreFold(source);
  selectLibStore(source, 'installed');
  rememberFocus('tab:library', storeRowKey(source, 'installed'));
  void showTab('library', { again: true });
}

/** Open Available (the empty Installed panel). */
export function showAvailable(source: StoreSource): void {
  selectLibStore(source, 'available');
  rememberFocus('tab:library', storeRowKey(source, 'available'));
  void showTab('library', { again: true });
}

export function installTool(source: StoreSource): void {
  api.storeInstallTool(source).catch(fail);
}

export function refreshStore(source: StoreSource): void {
  // Steam's still-private answer is the backend's `store-notice` (one card,
  // and a profile made public since then refreshes).
  api.storeRefresh(source).catch(fail);
}

/** The id of the login layer open now, with its store. */
let login: { layer: number; source: StoreSource; done: boolean } | null = null;

/**
 * Connect… / Sign in again… (prototype `sLogin`): the store's login window
 * opens over TV mode (the backend's own window); the layer under it carries
 * the hints (A Select · Y Keyboard · B Cancel sign-in), Y opens the
 * on-screen keyboard, whose Done types into the store's page.
 */
export function startLogin(source: StoreSource): void {
  if (login && topLayer()?.id === login.layer) return;
  const entry = { layer: 0, source, done: false };
  login = entry;
  void openLayer({
    component: TvStoreLogin,
    props: { source, onCancel: () => void closeLayer() },
    cls: 'slogin',
    hints: [
      { button: 'A', label: 'Select' },
      { button: 'Y', label: 'Keyboard' },
      { button: 'B', label: 'Cancel sign-in' },
    ],
    binds: { Y: () => typeIntoLogin(source) },
    onClose: () => {
      if (login === entry) login = null;
      // B (or Cancel) closes it silently; the backend's window goes too.
      if (!entry.done) api.storeCancelLogin(source).catch(() => {});
    },
  }).then((id) => (entry.layer = id));
  api.storeConnect(source).catch((err) => {
    entry.done = true;
    if (topLayer()?.id === entry.layer) void closeLayer();
    fail(err);
  });
}

/** Y: the on-screen keyboard; Done types into the store's page (masked: it may be a password). */
async function typeIntoLogin(source: StoreSource): Promise<void> {
  const { openKeyboard } = await import('../overlays/overlays.svelte');
  void openKeyboard({
    title: `Typed into ${storeName(source)}’s sign-in page`,
    mask: true,
    onDone: (v) => {
      if (v !== '') api.storeLoginType(v).catch(fail);
    },
  });
}

/** The login finished (a `store-login` event): its layer closes without cancelling. */
export async function loginFinished(source: StoreSource): Promise<void> {
  const l = login;
  if (!l || l.source !== source) return;
  l.done = true;
  // The keyboard may be open over it.
  while (topLayer() && topLayer()!.id !== l.layer && nav.layers.some((x) => x.id === l.layer)) await closeLayer();
  if (topLayer()?.id === l.layer) await closeLayer();
}

// ── The offline installer ──────────────────────────────────────────────

/** Install from installer…: the folder browser in ~/Downloads, files only (prototype `sOpenInstaller`). */
export async function openInstaller(): Promise<void> {
  let start = '';
  try {
    const home = await api.listFolder('', false);
    start = childPath(home.path, 'Downloads');
  } catch {
    // The browser falls back to the home folder.
  }
  openFolderBrowser({ title: PICK_INSTALLER_TITLE, start, files: true, onPick: (p) => void pickedInstaller(p) });
}

async function pickedInstaller(path: string): Promise<void> {
  card(READING_INSTALLER, 'success', fileName(path));
  let insp;
  try {
    insp = await api.offlineInspect(path);
  } catch (err) {
    fail(err);
    return;
  }
  const gog = gameStores.status('gog');
  const view = tvInspectionView({ ...insp, path: insp.path || path }, defaultCompat(), gog?.connection === 'connected');
  if (view.kind === 'refused') {
    void openLayer({
      component: TvRefusal,
      props: { heading: view.heading, title: refusalTitle(view.title), body: view.body, another: () => void openInstaller() },
    });
    return;
  }
  void pushPage({ title: view.heading, component: TvOfflineInstallPage, props: { insp: { ...insp, path: insp.path || path } } });
}

// ── Backups and drives ──────────────────────────────────────────────────

/** The Backup page for GOG games (prototype `sBackupPage`). */
export function openStoreBackup(keys: readonly string[], title: string): void {
  void pushPage({ title, component: TvStoreBackupPage, props: { keys: [...keys], title } });
}

/** Install from drive (prototype `sRestore`): queued; the card is the notifier's. */
export async function restoreFromDrive(driveKey: string, gameKey: string): Promise<void> {
  try {
    await api.installStoreFromDrive(driveKey, gameKey);
  } catch (err) {
    fail(err);
    return;
  }
  bumpDrives();
}
