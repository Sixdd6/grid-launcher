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
  E2E_MOCK_URL: mockUrl,
  RUST_LOG: process.env.E2E_RUST_LOG ?? 'info',
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

  logLevel: (process.env.E2E_LOG_LEVEL as Options.Testrunner['logLevel']) ?? 'warn',
  outputDir: process.env.E2E_WDIO_LOG_DIR,
  bail: 0,
  waitforTimeout: 5_000,
  connectionRetryTimeout: 90_000,
  connectionRetryCount: 3,

  // The app start is the flaky part (webview + embedded server under Xvfb).
  // One retry turns a cold-start hiccup into a warning instead of a red run.
  specFileRetries: 1,
  specFileRetriesDeferred: false,

  framework: 'mocha',
  mochaOpts: {
    ui: 'bdd',
    timeout: 120_000,
  },

  reporters: ['spec'],
};
