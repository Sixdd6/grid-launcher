import { convertFileSrc } from '@tauri-apps/api/core';

/**
 * The dev server's route for the image cache (vite.config.ts `devCovers`).
 *
 * `tauri dev` serves the page from http://localhost:5173, and WebKitGTK 2.54
 * refuses `asset://` images from that origin ("Unsafe attempt to load URL
 * asset://localhost/... Domains, protocols and ports must match"). A bundled
 * build loads from `tauri://localhost` and keeps using the asset protocol.
 */
export const DEV_COVERS_ROUTE = '/__grid_covers__/';

/**
 * The `src` for a cached image file. In dev the dev server serves the file
 * by name from the cache directory; everywhere else it is the asset-protocol
 * URL. Every path `ensure_image` returns sits directly in that directory.
 */
export function fileSrcFor(path: string, devServer: boolean): string {
  if (devServer) {
    const name = path.split(/[\\/]/).pop() ?? '';
    if (name) return DEV_COVERS_ROUTE + encodeURIComponent(name);
  }
  return convertFileSrc(path);
}

export function fileSrc(path: string): string {
  return fileSrcFor(path, import.meta.env.DEV && import.meta.env.MODE !== 'test');
}
