/**
 * Settings › Systems (app/src/lib/systems/SystemsPage.svelte), the one place
 * emulators are picked, installed, edited and removed. Test keys:
 *
 * - a system row is `sys-row-<slug>` (its toggle, picker, status, core,
 *   firmware and tips carry the same `<slug>` suffix);
 * - the emulator line inside it (name, version, Update, Open, ⋯) is keyed
 *   by the system's display label: `lineKey('Sony PlayStation 2')`;
 * - an emulator under Other emulators is `sys-other-<name key>`, its line
 *   `otherKey(name)`.
 */
import { TRANSITION_TIMEOUT } from './env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** The page's test-key rule (`systems/rows.ts` `testKeyOf`). */
export function keyOf(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

/** The emulator line key inside a system row, from the system's label. */
export const lineKey = (systemLabel: string) => keyOf(systemLabel);

/** The emulator line key of an entry under Other emulators. */
export const otherKey = (name: string) => keyOf(`other-${name}`);

/** Opens Settings › Systems and waits for its rows. */
export async function openSystems(): Promise<void> {
  await $(testId('nav-settings')).click();
  await $(testId('settings-view')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the settings view never opened',
  });
  await $(testId('settings-nav-systems')).click();
  await $(testId('systems-list')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Settings › Systems never listed its rows',
  });
}

/** The row test keys, top to bottom (Windows last, on Linux). */
export async function rowOrder(): Promise<string[]> {
  return browser.execute(() =>
    Array.from(document.querySelectorAll('[data-testid="systems-list"] > [data-testid^="sys-row-"]')).map((r) =>
      (r as HTMLElement).dataset.testid!.replace('sys-row-', ''),
    ),
  );
}

export async function statusOf(slug: string): Promise<string> {
  return (await $(testId(`sys-status-${slug}`)).getAttribute('data-status')) ?? '';
}

export async function waitForStatus(slug: string, status: string, timeout = TRANSITION_TIMEOUT): Promise<void> {
  await browser.waitUntil(async () => (await statusOf(slug)) === status, {
    timeout,
    timeoutMsg: `${slug} never showed status ${status} (it shows ${await statusOf(slug)})`,
  });
}

/**
 * Sets a native `<select>`'s value and dispatches `change`: WebKitGTK's
 * WebDriver select fires no `change` event, so make the pick the way the
 * browser reports it.
 */
export async function selectValue(id: string, value: string): Promise<void> {
  await browser.execute(
    (selector, v) => {
      const select = document.querySelector(selector) as HTMLSelectElement | null;
      if (!select) throw new Error(`no element matched ${selector}`);
      select.value = v;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    },
    testId(id),
    value,
  );
}

/** Picks a system's emulator: `use:<name>`, `install:<source id>`, or `''` (None). */
export async function pick(slug: string, value: string): Promise<void> {
  await selectValue(`sys-pick-${slug}`, value);
}

/** Each picker group's options as `value=text`. */
export async function groups(slug: string): Promise<Record<string, string[]>> {
  return browser.execute((id) => {
    const select = document.querySelector(`[data-testid="${id}"]`) as HTMLSelectElement;
    const out: Record<string, string[]> = {};
    for (const g of Array.from(select.querySelectorAll('optgroup'))) {
      out[g.label] = Array.from(g.querySelectorAll('option')).map((o) => `${o.value}=${o.textContent?.trim()}`);
    }
    return out;
  }, `sys-pick-${slug}`);
}

/** The `<option>` values of any select, in DOM order. */
export async function optionValues(id: string): Promise<string[]> {
  return browser.execute((selector) => {
    const el = document.querySelector(selector) as HTMLSelectElement | null;
    if (!el) throw new Error(`no element matched ${selector}`);
    return Array.from(el.options).map((o) => o.value);
  }, testId(id));
}

/** Opens a system row (a no-op when it is already open). */
export async function openRow(slug: string): Promise<void> {
  const row = $(testId(`sys-row-${slug}`));
  await row.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `no Systems row ${slug}` });
  if (!((await row.getAttribute('class')) ?? '').split(/\s+/).includes('open')) {
    await $(testId(`sys-toggle-${slug}`)).click();
  }
  await browser.waitUntil(async () => ((await row.getAttribute('class')) ?? '').split(/\s+/).includes('open'), {
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: `the ${slug} row never opened`,
  });
}

/** Opens an emulator line's ⋯ menu. A script click: WebDriver's own click
 *  scrolls first, and a scroll closes the menu by design. */
async function openMenu(line: string): Promise<void> {
  const more = $(testId(`sys-more-${line}`));
  await more.waitForExist({ timeout: TRANSITION_TIMEOUT, timeoutMsg: `no emulator line ${line}` });
  await more.scrollIntoView({ block: 'center' });
  await browser.pause(150);
  await browser.execute((sel) => (document.querySelector(sel) as HTMLElement).click(), testId(`sys-more-${line}`));
  await $(testId('sys-menu')).waitForDisplayed({ timeout: TRANSITION_TIMEOUT, timeoutMsg: 'the ⋯ menu never opened' });
}

/** Opens the settings sheet for an emulator line (⋯ › Settings…). */
export async function editEmulator(line: string): Promise<void> {
  await openMenu(line);
  await browser.execute(() => (document.querySelector('[data-testid="sys-menu-settings"]') as HTMLElement).click());
  await $(testId('systems-sheet')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'the settings sheet never opened',
  });
  await $(testId('emu-form-path')).waitForExist({ timeout: TRANSITION_TIMEOUT });
}

/** Saves the open sheet and waits for it to close. */
export async function saveSheet(): Promise<void> {
  await $(testId('emu-form-save')).click();
  await $(testId('systems-sheet')).waitForExist({
    timeout: TRANSITION_TIMEOUT,
    reverse: true,
    timeoutMsg: 'the settings sheet stayed open after Save',
  });
}

/**
 * Removes an emulator (⋯ › Remove, twice), checking the first click only
 * asks. Resolves once the line is gone.
 */
export async function removeEmulator(line: string, name: string): Promise<void> {
  await openMenu(line);
  const remove = $(testId('sys-menu-remove'));
  await remove.click();
  await expect(remove).toHaveText(`Remove ${name}? Games and saves stay`);
  await remove.click();
  await $(testId(`sys-more-${line}`)).waitForExist({
    timeout: TRANSITION_TIMEOUT * 2,
    reverse: true,
    timeoutMsg: `${name} was still listed after Remove`,
  });
}

/**
 * Adds an emulator by hand. `which` picks a known emulator in "Which
 * emulator is it?"; without it the sheet stays on Something else and
 * `name` is typed. Leaves the sheet open, for the caller's Save.
 */
export async function startManualAdd(opts: { which?: string; name?: string; path: string }): Promise<void> {
  await $(testId('systems-add-manual')).click();
  await $(testId('systems-sheet')).waitForDisplayed({
    timeout: TRANSITION_TIMEOUT,
    timeoutMsg: 'Add an emulator manually never opened its sheet',
  });
  if (opts.which !== undefined) await selectValue('emu-form-which', opts.which);
  if (opts.name !== undefined) await $(testId('emu-form-name')).setValue(opts.name);
  await $(testId('emu-form-path')).setValue(opts.path);
}

/** Adds an emulator by hand and saves it. */
export async function addManual(opts: { which?: string; name?: string; path: string }): Promise<void> {
  await startManualAdd(opts);
  await saveSheet();
}

/**
 * The line key of the emulator named `name`, wherever it shows: a system's
 * emulator (opening rows as needed, since a closed row renders no line) or
 * under Other emulators.
 */
export async function findLine(name: string, timeout = TRANSITION_TIMEOUT): Promise<string> {
  let key = '';
  await browser
    .waitUntil(
      async () => {
        key = await browser.execute((n) => {
          const hit = Array.from(document.querySelectorAll('[data-testid^="sys-emu-name-"]')).find(
            (el) => el.textContent?.trim() === n,
          );
          if (hit) return (hit as HTMLElement).dataset.testid!.replace('sys-emu-name-', '');
          for (const t of Array.from(document.querySelectorAll('[data-testid="systems-list"] [data-testid^="sys-toggle-"]'))) {
            const b = t as HTMLButtonElement;
            if (!b.disabled && b.getAttribute('aria-expanded') === 'false') b.click();
          }
          return '';
        }, name);
        return key !== '';
      },
      { timeout },
    )
    .catch(() => undefined);
  if (key === '') throw new Error(`Settings › Systems never showed an emulator line for ${name}`);
  return key;
}

/** Whether any emulator line is named `name` right now (rows as they are). */
export async function hasLine(name: string): Promise<boolean> {
  return browser.execute(
    (n) =>
      Array.from(document.querySelectorAll('[data-testid^="sys-emu-name-"]')).some((el) => el.textContent?.trim() === n),
    name,
  );
}

/** The text of the Setup tips beside `name`'s line ('' when it has none). */
export async function tipsText(name: string): Promise<string> {
  await findLine(name);
  return browser.execute((n) => {
    const hit = Array.from(document.querySelectorAll('[data-testid^="sys-emu-name-"]')).find(
      (el) => el.textContent?.trim() === n,
    );
    const tips = hit?.closest('li')?.querySelector('[data-testid^="sys-tips-"]');
    return tips?.textContent?.replace(/\s+/g, ' ').trim() ?? '';
  }, name);
}

/** The `name = "…"` emulator entries in config.toml text, in file order. */
export function configEmulatorNames(config: string): string[] {
  return config
    .split(/^\[\[emulators\]\]\s*$/m)
    .slice(1)
    .map((block) => block.split(/^\[/m)[0].match(/^name = "([^"]*)"/m)?.[1] ?? '');
}
