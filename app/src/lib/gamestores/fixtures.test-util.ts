// Builders for the gamestores tests: a status, a game and a Downloads row with
// sensible defaults (the fake's own data underneath). Fake markers only.
import type { DownloadEntry, StoreGame, StoreStatus } from '../api';
import { baseAmazonStatus, baseEpicStatus, baseStatus, FakeStores } from './fakeApi';

export function status(over: Partial<StoreStatus> = {}): StoreStatus {
  return { ...baseStatus(), ...over, tool: { ...baseStatus().tool, ...(over.tool ?? {}) } };
}

/** An Epic Games status: signed in as SamPlays, legendary installed. */
export function epicStatus(over: Partial<StoreStatus> = {}): StoreStatus {
  return { ...baseEpicStatus(), ...over, tool: { ...baseEpicStatus().tool, ...(over.tool ?? {}) } };
}

/** One of the fake's Epic Games games by app name (`Starlight`, `GlimmerDeep`, `IronOrchard`, …). */
export function epicGame(appName: string, over: Partial<StoreGame> = {}): StoreGame {
  const base = new FakeStores().games.find((g) => g.key === `epic:${appName}`)!;
  return { ...base, ...over };
}

/** An Amazon Games status: signed in as sam.plays, nile 1.2.0 installed, no cloud saves. */
export function amazonStatus(over: Partial<StoreStatus> = {}): StoreStatus {
  return { ...baseAmazonStatus(), ...over, tool: { ...baseAmazonStatus().tool, ...(over.tool ?? {}) } };
}

/** One of the fake's Amazon Games games by title (`Fogline`, `Kettle Drum`, `Lowsun`). */
export function amazonGame(title: string, over: Partial<StoreGame> = {}): StoreGame {
  const base = new FakeStores().games.find((g) => g.source === 'amazon' && g.title === title)!;
  return { ...base, ...over };
}

export function storeGame(over: Partial<StoreGame> = {}): StoreGame {
  const base = new FakeStores().games.find((g) => g.store_id === '1004')!;
  return { ...base, ...over };
}

export function installedGame(over: Partial<StoreGame> = {}): StoreGame {
  const base = new FakeStores().games.find((g) => g.store_id === '1001')!;
  return { ...base, ...over };
}

export function row(over: Partial<DownloadEntry> = {}): DownloadEntry {
  return {
    id: 1,
    job: 'store',
    kind: 'store_install',
    rom_id: 0,
    source_id: 'gog:1004',
    title: 'Copper Saints',
    platform: 'GOG',
    status: 'downloading',
    downloaded_bytes: 45,
    total_bytes: 100,
    speed_bps: 41.2 * 1024 * 1024,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    detail: '',
    drive_id: '',
    drive_name: '',
    drive_label: '',
    batch: 0,
    phase: 'downloading',
    failure: null,
    ...over,
  };
}
