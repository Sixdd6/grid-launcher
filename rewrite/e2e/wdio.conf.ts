import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';

const here = path.dirname(fileURLToPath(import.meta.url));

function required(name: string): string {
  const value = process.env[name];
  if (!value || value.trim() === '') {
    throw new Error(
      `${name} is not set. Run the E2E suite through rewrite/scripts/e2e.sh, ` +
        'which builds the app and exports the per-stage environment.',
    );
  }
  return value;
}

// One stage = one spec file, one wdio run. The embedded WebDriver provider
// shares a single app process across every spec of a run and offers no
// restart, so per-app-instance isolation has to happen at the runner level.
const specFile = required('E2E_SPEC');
const appBinaryPath = required('E2E_APP_BINARY');
const dataDir = required('E2E_DATA_DIR');
const mockUrl = required('E2E_MOCK_URL');

const appEnv: Record<string, string> = {
  // All app state (config.toml, grid-launcher.db, covers/) goes to the
  // stage's temp directory — never ~/.config/grid-launcher.
  GRID_LAUNCHER_DATA_DIR: dataDir,
  // WebKitGTK's DMABUF renderer cannot allocate GBM buffers under Xvfb.
  WEBKIT_DISABLE_DMABUF_RENDERER: '1',
  // Belt and braces with e2e.sh's own `unset WAYLAND_DISPLAY`: without this,
  // GTK prefers the session's real Wayland compositor over Xvfb's DISPLAY.
  GDK_BACKEND: 'x11',
  E2E_MOCK_URL: mockUrl,
  RUST_LOG: process.env.E2E_RUST_LOG ?? 'info',
  // Only the stage groups that run a mock forge set these (e2e.sh's
  // group_needs_forge). GRID_LAUNCHER_E2E_FORGE_BASE is what the app's `e2e`
  // build redirects forge requests to (grid-core launch/forge.rs); the app's
  // spawned emulators inherit GRID_E2E_ARGV_FILE and record their argv there.
  ...(process.env.E2E_FORGE_URL
    ? { GRID_LAUNCHER_E2E_FORGE_BASE: process.env.E2E_FORGE_URL }
    : {}),
  ...(process.env.GRID_E2E_ARGV_FILE
    ? { GRID_E2E_ARGV_FILE: process.env.GRID_E2E_ARGV_FILE }
    : {}),
};

export const config: WebdriverIO.Config = {
  runner: 'local',
  tsConfigPath: path.join(here, 'tsconfig.json'),

  specs: [path.resolve(here, specFile)],

  // Embedded mode drives one app process; never run two at once.
  maxInstances: 1,
  maxInstancesPerCapability: 1,

  capabilities: [
    {
      browserName: 'tauri',
      // BiDi negotiation breaks the embedded server's session handshake.
      'wdio:enforceWebDriverClassic': true,
      'tauri:options': {
        application: appBinaryPath,
        args: [],
      },
      'wdio:tauriServiceOptions': {
        appBinaryPath,
        driverProvider: 'embedded',
        env: appEnv,
        captureBackendLogs: true,
        captureFrontendLogs: true,
        startTimeout: 60_000,
      },
    } as WebdriverIO.Capabilities,
  ],

  services: [['@wdio/tauri-service', { driverProvider: 'embedded' }]],

  // e2e.sh wraps the whole run in `xvfb-run -a` (inside `dbus-run-session`),
  // so wdio must not start a second Xvfb of its own.
  autoXvfb: false,

  // 'info', not 'warn': the tauri service forwards the app's backend and
  // frontend logs into this stream, and at 'warn' none of them appear — which
  // makes a failure dump that promises app logs deliver nothing.
  logLevel: (process.env.E2E_LOG_LEVEL as Options.Testrunner['logLevel']) ?? 'info',
  // e2e.sh points this at a per-stage directory and dumps its *.log files when
  // a stage fails, so the driver/service side of a failure is recoverable too.
  outputDir: process.env.E2E_WDIO_LOG_DIR,
  bail: 0,
  waitforTimeout: 5_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 3,

  // No specFileRetries on purpose. A spec leaves the app and the data
  // directory mutated, so retrying just the spec re-runs it against state the
  // failed attempt already changed and reports a misleading second error
  // (typically "the connect form never appeared", because the app is already
  // connected). e2e.sh retries at the stage-group level instead, from a fresh
  // data directory and a fresh mock server.

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 120_000,
  },

  reporters: ['spec'],
};
