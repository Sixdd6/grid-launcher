/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set to a truthy value only for E2E test builds. Gates loading the
   *  WebDriver automation bridge (@wdio/tauri-plugin) out of production
   *  bundles — see src/main.ts. */
  readonly VITE_E2E?: string
  /** DEV ONLY: runs the UI against the store fake (lib/gamestores/devMock.ts). */
  readonly VITE_FAKE_STORES?: string
}
