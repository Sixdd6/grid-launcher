/**
 * UI helpers shared by the `backups` specs: Settings › Backup drives, the
 * Downloads rows, and a notification recorder.
 */
import { TRANSITION_TIMEOUT } from './env.js';
import { mountOf } from './drives.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Settings › Backup drives' card for the drive `label` (keyed by its mount path). */
export const driveCard = (label: string) => `[data-testid="backup-drive"][data-drive-key="${mountOf(label)}"]`;

/** Opens Settings › Backup drives and waits for the drive `label`'s card. */
export async function openBackupDrives(label: string): Promise<void> {
  await $(testId('nav-settings')).click();
  await $(testId('settings-view')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the settings view never opened',
  });
  await $(testId('settings-nav-backup-drives')).click();
  await $(testId('settings-backup-drives-pane')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Settings › Backup drives never opened',
  });
  // The drive service polls every 2 s.
  await $(driveCard(label)).waitForExist({
    timeout: 10_000,
    timeoutMsg: `Settings › Backup drives never listed ${label}`,
  });
}

/** The status pill of the drive `label`'s card. */
export async function driveStatus(label: string): Promise<string> {
  return (await $(`${driveCard(label)} [data-testid="backup-drive-status"]`).getText()).trim();
}

/** Switches to one of the top-level views. */
export async function show(view: 'server' | 'library' | 'downloads' | 'settings'): Promise<void> {
  await $(testId(`nav-${view}`)).click();
  await $(testId(`${view}-view`)).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the ${view} view never opened`,
  });
}

/** One Downloads row as the page shows it. */
export type Row = { id: string; title: string; kind: string; detail: string };

/** Every Downloads row, top to bottom. The Downloads view need not be on screen. */
export async function downloadRows(): Promise<Row[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid^="download-row-"]')).map((row) => {
      const id = (row as HTMLElement).dataset.testid!.replace('download-row-', '');
      const text = (sel: string) => (row.querySelector(sel)?.textContent ?? '').trim();
      return {
        id,
        title: text('.title'),
        kind: text(`[data-testid="download-kind-${id}"]`),
        detail: text(`[data-testid="download-detail-${id}"]`),
      };
    }),
  );
}

/** The rows with the Backup kind badge. */
export async function backupRows(): Promise<Row[]> {
  return (await downloadRows()).filter((r) => r.kind === 'Backup');
}

/**
 * Starts recording every notification card the page adds, and every text a
 * Downloads detail line or the strip shows, from now on. A card lasts ten
 * seconds and a fast copy's status may show for a moment only, so specs
 * count and match against these logs instead of what is on screen.
 */
export async function startRecording(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as { __e2eToasts?: string[]; __e2eDetails?: string[]; __e2eObserver?: MutationObserver };
    w.__e2eToasts = [];
    w.__e2eDetails = [];
    w.__e2eObserver?.disconnect();
    // A card whose text changes in place counts again: that is a second
    // announcement as far as the user is concerned.
    const seenToasts = new WeakMap<Element, string>();
    const scan = () => {
      for (const el of Array.from(document.querySelectorAll('[data-testid="toast"]'))) {
        const text = (el.textContent ?? '').trim();
        if (text === '' || seenToasts.get(el) === text) continue;
        seenToasts.set(el, text);
        w.__e2eToasts!.push(text.replace(/^Error:\s*/, ''));
      }
      for (const el of Array.from(
        document.querySelectorAll('[data-testid^="download-detail-"], [data-testid="downloads-aggregate"]'),
      )) {
        const text = (el.textContent ?? '').trim();
        const log = w.__e2eDetails!;
        if (text !== '' && log[log.length - 1] !== text && !log.includes(text)) log.push(text);
      }
    };
    w.__e2eObserver = new MutationObserver(scan);
    w.__e2eObserver.observe(document.body, { childList: true, subtree: true, characterData: true });
    scan();
  });
}

/** Every notification text recorded since `startRecording`. */
export async function recordedToasts(): Promise<string[]> {
  return browser.execute(() => (window as unknown as { __e2eToasts?: string[] }).__e2eToasts ?? []);
}

/** Every Downloads line text recorded since `startRecording`. */
export async function recordedDetails(): Promise<string[]> {
  return browser.execute(() => (window as unknown as { __e2eDetails?: string[] }).__e2eDetails ?? []);
}

/** Waits until a recorded notification matches `pattern`, and returns it. */
export async function waitForRecordedToast(pattern: RegExp, timeout = 30_000): Promise<string> {
  let found: string | undefined;
  let seen: string[] = [];
  await browser
    .waitUntil(
      async () => {
        seen = await recordedToasts();
        found = seen.find((t) => pattern.test(t));
        return found !== undefined;
      },
      { timeout, interval: 200 },
    )
    .catch(() => undefined);
  if (found === undefined) {
    throw new Error(`no notification matched ${String(pattern)} (recorded: ${JSON.stringify(seen)})`);
  }
  return found;
}
