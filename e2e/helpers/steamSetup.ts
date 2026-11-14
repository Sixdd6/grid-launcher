/**
 * Game stores (Steam) harness helpers (docs/stores-phase4-plan.md package
 * C): Steam's status, the key through the app's own `steam_save`, the mock
 * Steam Web API's request log and switches, the fake steam's argv file, and
 * the fake Steam root (manifests, registry.vdf).
 *
 * The key never crosses the WebDriver wire as text: wdio logs every
 * command's arguments, and the steam-secrets spec scans those logs. So
 * `steamSave` / `steamTest` send it reversed and the page turns it back
 * just before `invoke` (the answer carries no key). Use them, never
 * `invoke('steam_save', { key })` directly.
 */
import { spawnSync } from 'node:child_process';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { dataDir } from './env.js';
import { invoke } from './tv.js';

export { invoke };

/** The fixture key (mock-steam.mjs's good key). Never a real key. */
export const STEAM_KEY = 'FAKE-STEAM-KEY-not-real';
/** A key Steam revoked: 401. */
export const STEAM_REVOKED_KEY = `${STEAM_KEY}-revoked`;
/** The login name in the fake root's loginusers.vdf and registry.vdf. */
export const FAKE_ACCOUNT_NAME = 'FAKE-ACCOUNT-NAME-not-shown';
/** SamPlays (signed in now), couchguest (private game details), a manual id, no such account. */
export const SAM = '76561198000000001';
export const GUEST = '76561198000000002';
export const MANUAL = '76561198000000005';
export const NO_SUCH = '76561198000000009';
export const MAIN_WINDOW = 'main';
export const STEAM_TIMEOUT = 20_000;

/** The fixture games (e2e/fixtures-steam). */
export const GAMES = {
  moonlitFerry: 3000010,
  deepFieldRadio: 3000020,
  oldBrickRoad: 3000030,
  quietComet: 3000040,
  harborOfBells: 3000050,
  rimeAssembly: 3000060,
  signalLost: 3000070,
  deskLights: 3000080,
} as const;
/** Tools in the fake root that must never show. */
export const TOOLS = [2805730, 1628350, 228980] as const;

const here = path.dirname(fileURLToPath(import.meta.url));
/** mock-steam.mjs's request log file (one JSON line per request). */
export const STEAM_REQUEST_LOG = path.join(here, '..', 'last-run-steam-requests.log');

export type SteamCard = {
  client_path: string;
  steam_id: string;
  key: 'ok' | 'none' | 'bad' | 'private';
  key_tail: string;
  accounts: { persona: string; steam_id: string; most_recent: boolean }[];
};
export type SteamStatus = {
  source: string;
  connection: string;
  username: string;
  owned: string;
  owned_error: string;
  owned_count: number;
  installed_count: number;
  installed_bytes: number;
  in_rail: boolean;
  tool: { name: string };
  steam: SteamCard;
};
export type SteamTestResult =
  | { kind: 'ok'; owned_count: number }
  | { kind: 'bad_key'; status: number }
  | { kind: 'private' }
  | { kind: 'bad_steam_id' }
  | { kind: 'network' };
export type SteamRequest = {
  method: string;
  path: string;
  key: 'good' | 'revoked' | 'other' | null;
  referer: boolean;
  cookie: boolean;
  refused: boolean;
  ids?: number;
};

export const steamUrl = (): string => {
  const value = process.env.E2E_STEAM_URL;
  if (!value) throw new Error('E2E_STEAM_URL is not set: run the steam groups through scripts/e2e.sh');
  return value;
};

/** The fake root, its second library and registry.vdf (seed/steam-seed.mjs). */
export const steamRoot = (): string => process.env.E2E_STEAM_ROOT ?? path.join(dataDir(), 'steam', 'root');
export const steamLibrary2 = (): string => path.join(path.dirname(steamRoot()), 'library2');
export const registryVdf = (): string => path.join(path.dirname(steamRoot()), 'registry.vdf');
const argvFile = (): string => process.env.E2E_STEAM_ARGV_FILE ?? path.join(dataDir(), 'steam-argv.log');

export async function steamStatus(): Promise<SteamStatus> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await invoke<SteamStatus[]>('stores_status')).find((s) => s.source === 'steam')!;
}

/** Waits until a predicate on Steam's status holds; answers the status. */
export async function waitSteam(test: (s: SteamStatus) => boolean, timeoutMsg: string, timeout = STEAM_TIMEOUT): Promise<SteamStatus> {
  let last: SteamStatus | undefined;
  await browser.waitUntil(
    async () => {
      last = await steamStatus();
      return test(last);
    },
    { timeout, timeoutMsg },
  );
  return last!;
}

const reverse = (text: string): string => [...text].reverse().join('');

/** `steam_test` or `steam_save` with the key reversed on the wire (see the file comment). */
async function steamCall(cmd: 'steam_test' | 'steam_save', steamId: string, key: string | null): Promise<SteamTestResult> {
  await browser.switchToWindow(MAIN_WINDOW);
  return (await browser.execute(
    (c: string, id: string, reversed: string | null) => {
      const k = reversed === null ? null : [...reversed].reverse().join('');
      return (window as unknown as { __TAURI_INTERNALS__: { invoke: (c: string, a: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__.invoke(c, {
        steamId: id,
        key: k,
      });
    },
    cmd,
    steamId,
    key === null ? null : reverse(key),
  )) as SteamTestResult;
}

export const steamTest = (steamId: string, key: string | null = null): Promise<SteamTestResult> => steamCall('steam_test', steamId, key);
export const steamSave = (steamId: string, key: string | null = null): Promise<SteamTestResult> => steamCall('steam_save', steamId, key);

/**
 * For the specs that start connected (seed/steam-seed.mjs): saves the
 * fixture key through the app's own `steam_save`, then waits for the owned
 * list. The seed writes no secret.
 */
export async function ensureSteamKey(steamId = SAM): Promise<SteamTestResult> {
  const result = await steamSave(steamId, STEAM_KEY);
  await waitSteam((s) => s.owned !== 'loading', 'the owned list never loaded');
  return result;
}

/** The mock's request log (from the mock itself, so it is never stale). */
export async function steamRequests(): Promise<SteamRequest[]> {
  return (await (await fetch(`${steamUrl()}/__e2e__/requests`)).json()) as SteamRequest[];
}

/** `fail` | `up` | `redirect` | `no-redirect`. */
export async function steamControl(route: 'fail' | 'up' | 'redirect' | 'no-redirect'): Promise<void> {
  await fetch(`${steamUrl()}/__e2e__/${route}`, { method: 'POST' });
}

/** Every argv the fake steam was run with, oldest first. */
export function steamArgv(): string[][] {
  try {
    return readFileSync(argvFile(), 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line) as string[]);
  } catch {
    return [];
  }
}

/** Waits until the fake steam was run with exactly `args`. */
export async function waitSteamArgv(args: string[], timeout = STEAM_TIMEOUT): Promise<void> {
  const want = JSON.stringify(args);
  await browser.waitUntil(async () => steamArgv().some((a) => JSON.stringify(a) === want), {
    timeout,
    timeoutMsg: `the fake steam never got ${want}`,
  });
}

const quote = (text: string): string => `"${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;

/**
 * Drops an app manifest into a library of the fake root (a game installed
 * in Steam itself, or one Steam is installing), written as Steam does: a
 * new file renamed over the old one.
 */
export function writeManifest(
  appid: number,
  fields: { name: string; flags?: number; size?: number; toDownload?: number; downloaded?: number; lastPlayed?: number; library?: string },
): string {
  const library = fields.library ?? steamRoot();
  const entries: [string, string | number][] = [
    ['appid', appid],
    ['universe', 1],
    ['name', fields.name],
    ['StateFlags', fields.flags ?? 4],
    ['installdir', fields.name],
    ['LastUpdated', Math.floor(Date.now() / 1000)],
    ['LastPlayed', fields.lastPlayed ?? 0],
    ['SizeOnDisk', fields.size ?? 1_000_000_000],
    ['buildid', 17000000 + (appid % 1000)],
    ['BytesToDownload', fields.toDownload ?? 0],
    ['BytesDownloaded', fields.downloaded ?? 0],
    ['BytesToStage', 0],
    ['BytesStaged', 0],
  ];
  const text = `"AppState"\n{\n${entries.map(([k, v]) => `\t${quote(k)}\t\t${quote(String(v))}\n`).join('')}}\n`;
  const file = path.join(library, 'steamapps', `appmanifest_${appid}.acf`);
  writeFileSync(`${file}.tmp`, text);
  renameSync(`${file}.tmp`, file);
  return file;
}

/** registry.vdf's RunningAppID. */
export function runningAppId(): number {
  const text = readFileSync(registryVdf(), 'utf8');
  return Number(/"RunningAppID"\s+"(\d+)"/.exec(text)?.[1] ?? 0);
}

/**
 * Whether the run keyring's Steam slot holds exactly the fixture key
 * (`secret-tool`, when installed). Answers a boolean only, so the key never
 * reaches a log: `true` the key, `false` empty or something else, `null`
 * when secret-tool is missing.
 */
export function keyringHoldsFixtureKey(): boolean | null {
  const found = spawnSync('secret-tool', ['lookup', 'service', 'grid-launcher', 'username', 'steam-web-api-key'], {
    encoding: 'utf8',
  });
  if (found.error) return null;
  return found.status === 0 && found.stdout.replace(/\n$/, '') === STEAM_KEY;
}

/** Whether the run keyring's Steam slot is empty (`null`: no secret-tool). */
export function keyringSteamSlotEmpty(): boolean | null {
  const found = spawnSync('secret-tool', ['lookup', 'service', 'grid-launcher', 'username', 'steam-web-api-key'], {
    encoding: 'utf8',
  });
  if (found.error) return null;
  return found.status !== 0 || found.stdout.trim() === '';
}
