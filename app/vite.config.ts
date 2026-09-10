import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// https://vite.dev/config/
// Tauri-recommended dev-server hardening: don't clear the terminal (so Rust
// build output stays visible), fail fast on port conflicts instead of
// silently hopping ports (Tauri's devUrl is pinned to 5173), ignore the
// src-tauri build output so file watching doesn't loop on it, and only
// forward VITE_/TAURI_-prefixed env vars to the frontend.
export default defineConfig({
  plugins: [svelte()],
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
