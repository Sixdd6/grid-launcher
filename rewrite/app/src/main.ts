import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// The WebDriver bridge is test-only automation surface: it must never load
// in a production build. The dynamic import keeps it out of the bundle
// entirely (Rollup drops an `if (false)`-guarded dynamic import) whenever
// VITE_E2E isn't set, which is the case unless rewrite/e2e's build injects
// it. See rewrite/app/src-tauri's `e2e` cargo feature for the matching
// Rust-side gate.
if (import.meta.env.VITE_E2E) {
  await import('@wdio/tauri-plugin')
}

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
