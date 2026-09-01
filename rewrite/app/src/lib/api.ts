import { invoke } from '@tauri-apps/api/core';

// Field names below mirror the Rust structs' actual serde output exactly
// (see rewrite/crates/grid-core/src/session.rs and romm/mod.rs) — no
// cosmetic renames.
export type SessionState = { connected: boolean; username: string; server_url: string };
export type Platform = { id: number; name: string; slug: string; rom_count: number };
export type GameSummary = { id: number; name: string; platform_id: number; path_cover_small: string | null };

export const api = {
  connect: (serverUrl: string, username: string, secret: string, useToken: boolean) =>
    invoke<SessionState>('connect', { serverUrl, username, secret, useToken }),
  restoreSession: () => invoke<SessionState | null>('restore_session'),
  disconnect: () => invoke<void>('disconnect'),
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  listGames: (platformId: number) => invoke<GameSummary[]>('list_games', { platformId }),
  ensureCover: (gameId: number, coverPath: string) =>
    invoke<string>('ensure_cover', { gameId, coverPath }),
};
