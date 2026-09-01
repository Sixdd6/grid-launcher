/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set to a truthy value only for E2E test builds. Gates loading the
   *  WebDriver automation bridge (@wdio/tauri-plugin) out of production
   *  bundles — see src/main.ts. */
  readonly VITE_E2E?: string
}
