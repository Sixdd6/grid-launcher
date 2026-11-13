import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// The WebDriver bridge is test-only automation surface: it must never load
// in a production build. The dynamic import keeps it out of the bundle
// entirely (Rollup drops an `if (false)`-guarded dynamic import) whenever
// VITE_E2E isn't set, which is the case unless e2e's build injects
// it. See app/src-tauri's `e2e` cargo feature for the matching
// Rust-side gate.
if (import.meta.env.VITE_E2E) {
  await import('@wdio/tauri-plugin')
}

// DEV ONLY: VITE_FAKE_STORES=1 runs the UI in a plain browser against the
// store fake (gamestores/devMock.ts). `import.meta.env.DEV` is false in every
// build, so the import is dropped from production bundles.
if (import.meta.env.DEV && import.meta.env.VITE_FAKE_STORES) {
  const { installDevMock } = await import('./lib/gamestores/devMock')
  installDevMock()
}

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
