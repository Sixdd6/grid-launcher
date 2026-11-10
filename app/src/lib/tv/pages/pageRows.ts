// A page of rows, for the Backup page and the drive pages (tvmode.js
// `rowsPage`): headings, notes, switches, radios, choices and actions, info
// rows, usage bars, then a main button at the end that Start presses from
// anywhere on the page (README § Decided Start). Pure types and helpers;
// `ui/TvPageRows.svelte` draws the rows, `TvRowsPage.svelte` the page.

/** One part of a usage bar. `used`: other files (and the pack's own on the
 *  Backup page); `reserved`: what queued backups hold; `adding`: this
 *  backup's hatched share; `grid`: GRID's files on a drive page. */
export type BarSeg = { kind: 'used' | 'reserved' | 'adding' | 'grid'; pct: number; over?: boolean };

export type Bar = {
  segs: BarSeg[];
  /** The drive page's wide bar. */
  wide?: boolean;
  /** Its accessible name. */
  label: string;
  /** A line under it ("1.2 GB set aside for queued backups"). */
  after?: string;
};

export type PageRow =
  | { kind: 'heading'; key: string; text: string }
  | { kind: 'note'; key: string; text: string; warn?: boolean; err?: boolean }
  | {
      kind: 'info';
      key: string;
      label: string;
      sub?: string;
      value?: string;
      /** A source tag ("From this computer", "Download", "Choose below"). */
      tag?: string;
      /** A fixed check in the value column (Emulators always go). */
      check?: boolean;
      bar?: Bar;
    }
  | {
      kind: 'row';
      key: string;
      label: string;
      /** Muted text after the label (" · SAMSUNG T7 · exFAT"). */
      muted?: string;
      sub?: string;
      /** The sub line in the danger colour ("Needs 76 GB · 61.4 GB free"). */
      subErr?: boolean;
      value?: string;
      /** The value in the warning colour ("Choose"). */
      valueWarn?: boolean;
      /** A switch row. */
      toggle?: boolean;
      /** A radio row (the Drive choice). */
      radio?: boolean;
      /** The warning edge (an out-of-date game still to choose). */
      warn?: boolean;
      danger?: boolean;
      /** The A hint (default "Switch" on a switch, else "Select"). */
      al?: string;
      disabled?: boolean;
      why?: string;
      bar?: Bar;
      run: () => void;
    };

/** The page's main button and the line beside it. */
export type PageMain = {
  label: string;
  /** "4.2 GB to write · 1.4 GB to download". */
  summary: string;
  disabled: boolean;
  why?: string;
  /** The Menu hint while it is enabled ("Backup", "Set up"). */
  hint: string;
  press: () => void;
};

/** A row's A hint (tvmode.js `rowsPage`: `r.al`, else Switch / Select). */
export function pageRowA(r: Extract<PageRow, { kind: 'row' }>): string {
  return r.al ?? (r.toggle !== undefined ? 'Switch' : 'Select');
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** "1 game", "3 games". */
export function games(n: number): string {
  return plural(n, 'game');
}

export { plural };
