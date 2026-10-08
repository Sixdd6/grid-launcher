import { createReadStream, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { extname, join } from 'node:path'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vite'

// The image cache directory the app uses (lib.rs `cache_dir`): the
// GRID_LAUNCHER_DATA_DIR override, else directories' ProjectDirs cache dir
// for ("io.github", "Sixdd6", "grid-launcher").
function coversDir(): string {
  const override = process.env.GRID_LAUNCHER_DATA_DIR?.trim()
  if (override) return join(override, 'covers')
  const home = homedir()
  if (process.platform === 'win32') {
    const local = process.env.LOCALAPPDATA ?? join(home, 'AppData', 'Local')
    return join(local, 'Sixdd6', 'grid-launcher', 'cache', 'covers')
  }
  if (process.platform === 'darwin') {
    return join(home, 'Library', 'Caches', 'io.github.Sixdd6.grid-launcher', 'covers')
  }
  const cache = process.env.XDG_CACHE_HOME?.trim() || join(home, '.cache')
  return join(cache, 'grid-launcher', 'covers')
}

const CONTENT_TYPES: Record<string, string> = {
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.avif': 'image/avif',
}

// Cache file names: a SHA-256 hex key plus one or more extensions
// (`<key>.webp`, `<key>.bg0.jpg`). Anything else is refused, so the route
// can never reach outside the cache directory.
const CACHE_FILE_NAME = /^[0-9a-f]{64}(\.[a-z0-9]+)+$/

// Dev server only (`apply: 'serve'`). `tauri dev` serves the page from
// http://localhost:5173 and WebKitGTK 2.54 refuses `asset://` images from
// that origin, so src/lib/fileSrc.ts points dev builds at this route
// instead. Bundled builds load from tauri://localhost and use the asset
// protocol as before.
function devCovers(): Plugin {
  return {
    name: 'grid-dev-covers',
    apply: 'serve',
    configureServer(server) {
      const dir = coversDir()
      server.middlewares.use('/__grid_covers__/', (req, res) => {
        let name = ''
        try {
          name = decodeURIComponent((req.url ?? '').replace(/^\//, '').split('?')[0])
        } catch {
          // a malformed escape falls through to the 404 below
        }
        const type = CONTENT_TYPES[extname(name).toLowerCase()]
        const path = join(dir, name)
        if (!CACHE_FILE_NAME.test(name) || !type || !statSync(path, { throwIfNoEntry: false })?.isFile()) {
          res.statusCode = 404
          res.end()
          return
        }
        res.setHeader('Content-Type', type)
        res.setHeader('Cache-Control', 'no-cache')
        createReadStream(path).pipe(res)
      })
    },
  }
}

// https://vite.dev/config/
// Tauri-recommended dev-server hardening: don't clear the terminal (so Rust
// build output stays visible), fail fast on port conflicts instead of
// silently hopping ports (Tauri's devUrl is pinned to 5173), ignore the
// src-tauri build output so file watching doesn't loop on it, and only
// forward VITE_/TAURI_-prefixed env vars to the frontend.
export default defineConfig({
  plugins: [svelte(), devCovers()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
})
