// Helpers the `tv-backups` specs share (docs/tv-mode/plan.md P13).
import { TRANSITION_TIMEOUT } from './env.js';
import { mountOf } from './drives.js';
import { focused, invoke, key, waitFocused } from './tv.js';
import { testId, text } from './tvDetails.js';

/** The TV item with `data-k` `k`. */
export const byK = (k: string) => `[data-tv-root] [data-k="${k}"]`;

/** Focuses the TV item and presses A (Enter). TV mode ignores the mouse, so
 *  the jump goes through the e2e hook (`__tvE2E.focus`), as if a controller
 *  had moved there. */
export async function tap(selector: string): Promise<void> {
  // A top-bar tab isn't an item: switch to it as LB / RB would.
  // While a page is open the top bar shows its breadcrumb instead of the
  // tabs, so a tab's test id names the tab even when it isn't on screen.
  const named = /^\[data-testid="tv-tab-(\w+)"\]$/.exec(selector)?.[1] ?? null;
  const tab =
    named ??
    ((await browser.execute((sel: string) => {
      const el = document.querySelector(sel);
      return el && !el.hasAttribute('data-f') ? el.getAttribute('data-tab') : null;
    }, selector)) as string | null);
  if (tab) {
    await browser.execute((t: string) => (window as unknown as { __tvE2E: { tab: (t: string) => void } }).__tvE2E.tab(t), tab);
    await browser.waitUntil(
      async () => (await browser.execute((t: string) => document.querySelector(`[data-tab="${t}"]`)?.classList.contains('on') ?? false, tab)) as boolean,
      { timeout: TRANSITION_TIMEOUT, timeoutMsg: `the ${tab} tab never showed` },
    );
    return;
  }
  await focusOn(selector);
  await key('Enter');
}

/** Focuses the TV item through the e2e hook, as a controller moving there
 *  would, without pressing anything. */
export async function focusOn(selector: string): Promise<void> {
  const el = $(selector);
  await el.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `${selector} never appeared` });
  await browser.execute((sel: string) => {
    const w = window as unknown as { __tvE2E: { focus: (el: Element | null) => void } };
    w.__tvE2E.focus(document.querySelector(sel));
  }, selector);
  await browser
    .waitUntil(
      async () => (await browser.execute((sel: string) => document.querySelector(sel)?.classList.contains('f') ?? false, selector)) as boolean,
      { timeout: TRANSITION_TIMEOUT },
    )
    .catch(async () => {
      // Say where focus is and which layers are open, so a failure is readable.
      const where = await browser.execute(() => ({
        focused: document.querySelector('[data-tv-root] .f')?.getAttribute('data-k') ?? null,
        layers: Array.from(document.querySelectorAll('[data-layer]')).map((l) => l.getAttribute('data-layer')),
        notices: (window as unknown as { __tvE2E?: { notices: () => string[] } }).__tvE2E?.notices() ?? [],
      }));
      throw new Error(`${selector} never took focus (${JSON.stringify(where)})`);
    });
}

/** Focuses the TV item `k` and presses A. */
export async function tapK(k: string): Promise<void> {
  await tap(byK(k));
}

/** Every notice of the session, oldest first, "title" or "title | sub". */
export async function notices(): Promise<string[]> {
  return (await browser.execute(() => (window as unknown as { __tvE2E?: { notices: () => string[] } }).__tvE2E?.notices() ?? [])) as string[];
}

export async function waitNotice(want: string | RegExp, timeout = TRANSITION_TIMEOUT): Promise<string> {
  let found = '';
  await browser
    .waitUntil(
      async () => {
        found = (await notices()).find((n) => (typeof want === 'string' ? n === want : want.test(n))) ?? '';
        return found !== '';
      },
      { timeout, interval: 150 },
    )
    .catch(async () => {
      throw new Error(`no notice ${String(want)} (notices: ${JSON.stringify(await notices())})`);
    });
  return found;
}

/** Settings › Backup drives' row for the drive `label`. */
export const driveRow = (label: string) => `[data-testid="tv-drive-row"][data-drive-key="${mountOf(label)}"]`;

export async function driveRowStatus(label: string): Promise<string> {
  return ((await browser.execute((s: string) => document.querySelector(`${s} [data-testid="tv-drive-status"]`)?.textContent ?? '', driveRow(label))) as string).trim();
}

export async function waitDriveStatus(label: string, want: string, timeout = 10_000): Promise<void> {
  await browser
    .waitUntil(async () => (await driveRowStatus(label)) === want, { timeout, interval: 200 })
    .catch(async () => {
      throw new Error(`${label} never read "${want}" (it reads "${await driveRowStatus(label)}"; ${await screenState()})`);
    });
}

/** Where TV mode is, for a failure message. */
export async function screenState(): Promise<string> {
  return (await browser.execute(() => {
    const q = (s: string) => document.querySelector(s)?.textContent?.replace(/\s+/g, ' ').trim() ?? '-';
    return `tab ${document.querySelector('[data-testid="tv-tabs"] .tab.on')?.getAttribute('data-tab') ?? '-'}, crumb ${q('[data-testid="tv-crumb"]')}, pane ${q('[data-testid="tv-pane-title"]')}, focused ${document.querySelector('[data-tv-root] .f')?.getAttribute('data-k') ?? '-'}, rows ${document.querySelectorAll('[data-testid="tv-drive-row"]').length}, memory ${JSON.stringify((window as unknown as { __tvE2E?: { memory: () => unknown } }).__tvE2E?.memory())}, log ${JSON.stringify((window as unknown as { __navLog?: string[] }).__navLog ?? [])}`;
  })) as string;
}

/** Opens Settings › Backup drives. */
export async function openDrivesPane(): Promise<void> {
  await tap(testId('tv-tab-settings'));
  await tap(testId('tv-set-rail-backup-drives'));
  await browser.waitUntil(async () => (await text('tv-pane-title')) === 'Backup drives', {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Settings › Backup drives never showed',
  });
}

/** A rows page row's main text, its sub line and its value. */
export async function pageRow(key: string): Promise<{ label: string; sub: string; value: string; on: boolean | null; radio: boolean | null }> {
  return (await browser.execute((k: string) => {
    const el = document.querySelector(`[data-testid="tv-pr-${k}"]`);
    const clean = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
    const sw = el?.querySelector('.sw');
    const radio = el?.querySelector('.radio');
    return {
      label: clean(el?.querySelector('.lab .t')?.textContent),
      sub: clean(el?.querySelector('.lab .s')?.textContent),
      value: clean(el?.querySelector('.val')?.textContent),
      on: sw ? sw.classList.contains('on') : null,
      radio: radio ? radio.classList.contains('on') : null,
    };
  }, key)) as { label: string; sub: string; value: string; on: boolean | null; radio: boolean | null };
}

/** The open list's title and its options. */
export async function listOptions(): Promise<{ title: string; options: string[] }> {
  return (await browser.execute(() => {
    const sheet = document.querySelector('[data-testid="tv-list"]');
    const clean = (s: string | null | undefined) => (s ?? '').replace(/\s+/g, ' ').trim();
    return { title: clean(sheet?.querySelector('h2')?.textContent), options: Array.from(sheet?.querySelectorAll('.opt') ?? []).map((o) => clean(o.textContent)) };
  })) as { title: string; options: string[] };
}

/** Confirms the open dialog (Cancel has focus; Right reaches the action) and
 *  answers its title and body. */
export async function confirmDialog(): Promise<{ title: string; body: string; ok: string; danger: boolean }> {
  await $(testId('tv-confirm')).waitForExist({ timeout: TRANSITION_TIMEOUT });
  const words = (await browser.execute(() => {
    const all = document.querySelectorAll('[data-testid="tv-confirm"]');
    const d = all[all.length - 1]!;
    const ok = d.querySelector('[data-testid="tv-confirm-ok"]')!;
    return {
      title: d.querySelector('h2')?.textContent?.trim() ?? '',
      body: d.querySelector('p')?.textContent?.trim() ?? '',
      ok: ok.textContent?.trim() ?? '',
      danger: ok.classList.contains('dng'),
      layer: d.closest('[data-layer]')?.getAttribute('data-layer') ?? '',
    };
  })) as { title: string; body: string; ok: string; danger: boolean; layer: string };
  await waitFocused('dlg-cancel');
  await key('ArrowRight');
  await waitFocused('dlg-ok');
  await key('Enter');
  // A second question may open at once (Remove GRID files asks twice): wait
  // for this dialog's own layer to go.
  await browser.waitUntil(
    async () => !(await browser.execute((id: string) => !!document.querySelector(`[data-layer="${id}"]`), words.layer)),
    { timeout: TRANSITION_TIMEOUT, timeoutMsg: `the dialog "${words.title}" never closed` },
  );
  return { title: words.title, body: words.body, ok: words.ok, danger: words.danger };
}

/** The Downloads queue as the backend has it. */
export type Entry = { id: number; title: string; status: string; kind: string; job: string; error: string };

export async function entries(): Promise<Entry[]> {
  return (await invoke<{ entries: Entry[] }>('list_downloads')).entries;
}

/** Every Downloads row's line on the TV Downloads tab, by title. */
export async function dlLines(): Promise<{ title: string; kind: string; line: string }[]> {
  return (await browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid^="tv-dl-row-"]')).map((row) => {
      const t = row.querySelector('.t');
      const kind = t?.querySelector('.kind')?.textContent?.trim() ?? '';
      const title = (t?.textContent ?? '').trim().slice(0, Math.max(0, (t?.textContent ?? '').trim().length - kind.length)).trim();
      return { title, kind, line: (row.querySelector('[data-testid^="tv-dl-line-"]')?.textContent ?? '').trim() };
    }),
  )) as { title: string; kind: string; line: string }[];
}

/** The covers of the grid on show, by `data-k`. */
export async function gridCovers(grid = '#lib-grid'): Promise<string[]> {
  return (await browser.execute((g: string) => Array.from(document.querySelectorAll(`${g} [data-cover]`)).map((c) => c.getAttribute('data-k')!), grid)) as string[];
}

export { focused };
