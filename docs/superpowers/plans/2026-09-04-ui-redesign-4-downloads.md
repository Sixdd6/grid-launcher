# Desktop UI redesign 4 — Downloads view Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the flat Downloads list into the redesign's full view — Active / Queued / Completed segments with counts and a legend, one row per transfer with a kind badge, progress bar, a 120×38 network/disk sparkline panel and the existing action buttons — and give the footer strip a 60-sample sparkline of the current transfer.

**Architecture:** Every rule the view needs lives in pure modules under `app/src/lib/downloads/` that vitest covers: `ring.ts` (a fixed 60-slot ring buffer), `sampler.ts` (byte-delta accumulation per progress event, one sample per second), `segments.ts` (status → segment, counts, the legend), `sparkline.ts` (SVG path strings). The downloads store owns one sampler and a 1-second timer and exposes `samplesFor(id)`; no new IPC — the samples are derived from the `downloads-changed` snapshots the backend already emits. The one backend change is a 50-entry cap on terminal (Completed / Failed / Cancelled) entries inside `QueueState`, because the entry list is owned there and a frontend-only cap would either hide entries the backend keeps or fire dismiss calls from a store side effect.

**Tech Stack:** Rust (grid-core `library::queue`), Svelte 5 runes + TypeScript + vitest, WebdriverIO E2E against the mock RomM server.

**Spec:** `docs/superpowers/specs/2026-09-04-desktop-ui-redesign-design.md` — binding. This plan implements **delivery item 4 only** (§12.4): §8 Downloads view, D-UI-6, the Downloads half of D-UI-7, the download footer strip bullet of §3, and the §11 new ids `downloads-seg-<name>` and `download-graph-<id>`. Plan 5 (Emulators/Settings rails, removal of the old modal ids) is explicitly NOT implemented here. TV mode and core downloads are out of scope (§13).

Plan 1 already mounted Downloads as a full view: `Shell.svelte` renders `<div data-testid="downloads-view" class="view" hidden={view !== 'downloads'}>` and every spec waits on `downloads-view`, so the §11 rename `downloads-drawer` → the view root is **already done** and no id changes in this plan. `app.css` already defines `--graph-disk: #2dd4bf` (the §4 disk-graph teal) and `--primary`, so this plan adds **no token**.

All paths below are relative to `rewrite/` unless they start with `docs/`.

## Deliberate deviations, and why

Each of these is a decision the plan makes against, or beyond, the spec text. They are listed here so a reviewer can reject the decision rather than discover it buried in a task.

- **Segments are stacked sections, not a filter control.** §8 says "Segments Active, Queued, Completed, each with a count". A segmented *filter* (one segment visible at a time) would make a row vanish from the DOM as it moves queued → downloading → installing → completed, and nine existing E2E specs (`downloads`, `content`, `firmware`, `native`, `ps3-install`, `updates`, `emulator-catalog`, `install-a`, the cloud-saves helper) wait on `download-detail-<id>` text across exactly those transitions without clicking anything. The view therefore renders all three segments stacked in §8's order, each with its heading, count and rows, so `download-row-<id>` exists exactly once at all times. `downloads-seg-<name>` is the section element; `downloads-seg-count-<name>` is its count. Steam and GOG show their download lists the same way (Up next / Completed as stacked groups).
- **The Completed cap lives in the backend queue.** §8 says "Completed keeps the last 50 entries" and "no new IPC". The entry list is owned by `QueueState` in grid-core; the frontend only mirrors snapshots. A frontend-only cap would leave the backend list growing without bound and make `list_downloads` disagree with the UI; a frontend auto-dismiss would fire `dismiss_download` calls from inside a store event handler. So `QueueState` drops the oldest terminal entries beyond 50 on every terminal transition. That is a Rust change but **not an IPC change**: no command, event or payload shape changes.
- **ETA appears in the graph caption.** D-UI-6 lists "speed, ETA"; §8's row description does not mention ETA. The row's detail line stays the existing `entryDetail` text verbatim (speed included). The caption under the sparkline panel shows `<rate>/s · <ETA> left` while downloading with a known total, so D-UI-6 is honoured without changing a string E2E reads.
- **The footer sparkline draws both series.** §3 says "a 60-sample sparkline"; the same `Sparkline.svelte` component draws network and disk in the footer at 120×18, so an installing transfer (which moves no network bytes) still shows a live line.
- **The global "No downloads yet" text is replaced by per-segment empty lines.** With three always-present segments a single global empty state would sit beside three empty headings. No spec asserts the old text.

## Global Constraints

- **Token secrecy (hard):** tokens live only in the OS keyring and the redacting in-memory type; never in files, logs, errors, IPC, or console output. Nothing in this plan touches a token, and nothing it adds may log or render one.
- **No new IPC.** The sparkline samples are derived in the frontend store from the `downloaded_bytes` and `install_processed_bytes` deltas between successive `downloads-changed` snapshots (the backend throttles those to one per 100ms for download progress and one per 150ms for install progress; status transitions always emit). No command, event, or payload shape is added or changed.
- **View root:** `downloads-view` (plan 1's id) stays the root. The list column takes `.view-content` (**max-width 1100px, centred** — D-UI-7).
- **Segments, in this order:** Active (`downloading`, `installing`, `cancelling`), Queued (`queued`), Completed (`completed`, `failed`, `cancelled`); ids `downloads-seg-active` / `-queued` / `-completed`, counts `downloads-seg-count-<name>`. The legend, verbatim: `Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled` (`downloads-legend`).
- **Row:** `download-row-<id>` kept, and inside it the `.title` class on the title span (two specs read `${row} .title`), `download-kind-<id>` (rendered only when `kindLabel` is non-empty), `download-detail-<id>` with the existing `entryDetail` text **verbatim**, a progress bar, the sparkline panel **`download-graph-<id>`** (an `<svg>` 120×38 with two `<path>` children: network in `var(--primary)`, disk in `var(--graph-disk)`, 60 one-second samples), and the buttons from `actionFor` with their existing ids `download-action-cancel-<id>` / `-retry-<id>` / `-dismiss-<id>`.
- **Sampling:** one ring of 60 samples per entry; a sample is `{ net, disk }` in bytes per second; the ring is fed once per second from the accumulated byte deltas; a terminal entry's ring freezes; a dismissed entry's ring is dropped.
- **Completed keeps the last 50 terminal entries** (backend `QueueState`, `TERMINAL_HISTORY = 50`); live entries are never pruned.
- **Footer strip** `downloads-footer` kept: hidden when nothing is live; otherwise `⬇ <title> · <percent> · <speed>` (existing `footerLine`, verbatim), a 120×18 sparkline `downloads-footer-graph` of the current transfer, and the `Open Downloads` link; clicking anywhere on it opens the Downloads view.
- **Only `app.css` tokens for colours** (`--primary`, `--graph-disk`, `--danger`, `--surface`, `--border`, `--text-*`); motion only via the `--m-*` tokens. The two literal reds in the current `Downloads.svelte` (`#e5484d`) become `var(--danger)`.
- **Every task ends with**, from `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p app --all-targets --features e2e -- -D warnings` clean; `cargo test --workspace` green **when Rust changed**; and from `rewrite/app`: `npm run check` and `npx vitest run` green. Then a commit whose subject starts `rewrite: `. **The final code task runs every E2E group (`scripts/e2e.sh` with no argument) and must be green.**
- **Never** run `git checkout`, `git restore`, `git reset`, or `git stash`. Commit with explicit pathspecs.
- **No component test harness exists** in this repo (no `@testing-library/svelte`, no jsdom). Every `.svelte` change is verified by an extracted, unit-tested pure module plus `npm run check` and E2E — never by a fabricated component test.

---

## File map

| File | Responsibility |
|---|---|
| `app/src/lib/downloads/ring.ts` (+ test) | `Sample`, `Ring`, `createRing`, `pushSample`, `samplesOf`, `SAMPLE_COUNT` |
| `app/src/lib/downloads/sampler.ts` (+ test) | per-entry delta accumulation and the once-per-second tick |
| `app/src/lib/downloads/segments.ts` (+ test) | `Segment`, `SEGMENTS`, `segmentOf`, `segmentLabel`, `segmentEmptyText`, `groupBySegment`, `LEGEND_TEXT` |
| `app/src/lib/downloads/format.ts` (+ test) | adds `currentTransfer`, `etaText`, `graphCaption`; `footerLine` reuses `currentTransfer` |
| `app/src/lib/downloads/sparkline.ts` (+ test) | `sharedMax`, `linePath`, `sparklinePaths` — SVG `d` strings |
| `app/src/lib/downloads/Sparkline.svelte` | the one `<svg>` with two paths, used by the row panel and the footer |
| `app/src/lib/stores/downloads.svelte.ts` (+ test) | the sampler, the 1-second timer, `samplesFor(id)` |
| `crates/grid-core/src/library/queue.rs` | `TERMINAL_HISTORY`, `prune_terminal` |
| `app/src/lib/Downloads.svelte` | the view: legend, three segments, rows with the graph panel |
| `app/src/lib/DownloadsFooter.svelte` | the strip's sparkline |
| `e2e/specs/downloads.spec.ts` | segment, legend, badge, graph and footer cases |
| `SPEC.md`, `rewrite/README.md`, `docs/porting/03-library-install.md` | documentation |

---

### Task 1: The ring buffer, the sampler, the segments, and three small `format.ts` helpers

**Files:**
- Create: `app/src/lib/downloads/ring.ts`, `app/src/lib/downloads/ring.test.ts`
- Create: `app/src/lib/downloads/sampler.ts`, `app/src/lib/downloads/sampler.test.ts`
- Create: `app/src/lib/downloads/segments.ts`, `app/src/lib/downloads/segments.test.ts`
- Modify: `app/src/lib/downloads/format.ts` (`footerLine` at the tail; append three exports)
- Modify: `app/src/lib/downloads/format.test.ts` (append tests at the tail)

**Interfaces:**
- Consumes: `DownloadEntry`, `DownloadStatus` from `app/src/lib/api.ts`; `formatSize`, `percent` from `format.ts`.
- Produces, used by Tasks 2, 3, 5 and 6:
  - `ring.ts`: `export const SAMPLE_COUNT = 60`; `export type Sample = { net: number; disk: number }`; `export type Ring = { capacity: number; buf: Sample[]; head: number; count: number }`; `createRing(capacity?: number): Ring`; `pushSample(ring: Ring, sample: Sample): void`; `samplesOf(ring: Ring): Sample[]` (oldest first).
  - `sampler.ts`: `export const SAMPLE_INTERVAL_MS = 1000`; `export type Sampler`; `createSampler(nowMs: number): Sampler`; `observe(sampler: Sampler, entries: DownloadEntry[]): void`; `tick(sampler: Sampler, nowMs: number): void`; `graphsOf(sampler: Sampler): Record<number, Sample[]>`.
  - `segments.ts`: `export type Segment = 'active' | 'queued' | 'completed'`; `export const SEGMENTS: readonly Segment[]`; `export const LEGEND_TEXT: string`; `segmentOf(status: DownloadStatus): Segment`; `segmentLabel(seg: Segment): string`; `segmentEmptyText(seg: Segment): string`; `groupBySegment(entries: DownloadEntry[]): Record<Segment, DownloadEntry[]>`.
  - `format.ts`: `currentTransfer(entries: DownloadEntry[]): DownloadEntry | null`; `etaText(e: DownloadEntry): string`; `graphCaption(e: DownloadEntry): string`.

- [ ] **Step 1: Write the failing ring tests**

Create `app/src/lib/downloads/ring.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { createRing, pushSample, SAMPLE_COUNT, samplesOf } from './ring';

describe('ring', () => {
  it('holds 60 samples by default (design §8: 60 one-second samples)', () => {
    expect(SAMPLE_COUNT).toBe(60);
    expect(createRing().capacity).toBe(60);
  });

  it('reads back an empty ring as an empty list', () => {
    expect(samplesOf(createRing(3))).toEqual([]);
  });

  it('returns samples oldest first while under capacity', () => {
    const ring = createRing(3);
    pushSample(ring, { net: 1, disk: 0 });
    pushSample(ring, { net: 2, disk: 0 });
    expect(samplesOf(ring)).toEqual([
      { net: 1, disk: 0 },
      { net: 2, disk: 0 },
    ]);
  });

  it('drops the oldest sample once full and keeps reading oldest first', () => {
    const ring = createRing(3);
    for (let i = 1; i <= 5; i += 1) pushSample(ring, { net: i, disk: i * 10 });
    expect(samplesOf(ring)).toEqual([
      { net: 3, disk: 30 },
      { net: 4, disk: 40 },
      { net: 5, disk: 50 },
    ]);
    expect(ring.count).toBe(3);
  });

  it('never grows the backing array past its capacity', () => {
    const ring = createRing(4);
    for (let i = 0; i < 100; i += 1) pushSample(ring, { net: i, disk: 0 });
    expect(ring.buf.length).toBe(4);
    expect(samplesOf(ring).map((s) => s.net)).toEqual([96, 97, 98, 99]);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run, from `rewrite/app`: `npx vitest run src/lib/downloads/ring.test.ts`
Expected: FAIL — `Failed to resolve import "./ring"`.

- [ ] **Step 3: Write `ring.ts`**

Create `app/src/lib/downloads/ring.ts`:

```ts
// A fixed-size ring of transfer-rate samples (design §8: "a ring buffer per
// entry ... 60 one-second samples"). Pure data, no store imports, so the
// sampler and the sparkline can share it without an import cycle.

/** Samples per entry: one per second, one minute of history. */
export const SAMPLE_COUNT = 60;

/** One second of transfer: bytes per second over the network and to disk. */
export type Sample = { net: number; disk: number };

export type Ring = {
  readonly capacity: number;
  /** Backing storage; never longer than `capacity`. */
  buf: Sample[];
  /** Index the next push writes to. */
  head: number;
  /** How many slots hold a sample (≤ capacity). */
  count: number;
};

export function createRing(capacity: number = SAMPLE_COUNT): Ring {
  return { capacity, buf: [], head: 0, count: 0 };
}

/** Appends `sample`, overwriting the oldest one when the ring is full. */
export function pushSample(ring: Ring, sample: Sample): void {
  ring.buf[ring.head] = sample;
  ring.head = (ring.head + 1) % ring.capacity;
  ring.count = Math.min(ring.count + 1, ring.capacity);
}

/** The ring's samples, oldest first. */
export function samplesOf(ring: Ring): Sample[] {
  const out: Sample[] = [];
  const start = (ring.head - ring.count + ring.capacity) % ring.capacity;
  for (let i = 0; i < ring.count; i += 1) {
    out.push(ring.buf[(start + i) % ring.capacity]);
  }
  return out;
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npx vitest run src/lib/downloads/ring.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Write the failing sampler tests**

Create `app/src/lib/downloads/sampler.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { DownloadEntry } from '../api';
import { SAMPLE_COUNT } from './ring';
import { createSampler, graphsOf, observe, SAMPLE_INTERVAL_MS, tick } from './sampler';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Game',
    platform: 'Platform',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

describe('sampler', () => {
  it('ticks once per second', () => {
    expect(SAMPLE_INTERVAL_MS).toBe(1000);
  });

  it('starts a track at the entry\'s current counters so a mid-transfer start adds no delta', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 5_000 })]);
    tick(s, 1000);
    expect(graphsOf(s)).toEqual({ 1: [{ net: 0, disk: 0 }] });
  });

  it('accumulates the byte deltas of several events into one sample per tick', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 1_000 })]);
    observe(s, [entry({ downloaded_bytes: 2_500, install_processed_bytes: 400 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([{ net: 2_500, disk: 400 }]);
    // The pending deltas were consumed: the next second starts from zero.
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([
      { net: 2_500, disk: 400 },
      { net: 0, disk: 0 },
    ]);
  });

  it('normalises a late tick to bytes per second', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 3_000 })]);
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([{ net: 1_500, disk: 0 }]);
  });

  it('ignores a tick with no elapsed time', () => {
    const s = createSampler(1000);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 10 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([]);
    tick(s, 2000);
    expect(graphsOf(s)[1]).toEqual([{ net: 10, disk: 0 }]);
  });

  it('clamps a counter that moves backwards to a zero delta', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 500 })]);
    observe(s, [entry({ downloaded_bytes: 100 })]);
    observe(s, [entry({ downloaded_bytes: 200 })]);
    tick(s, 1000);
    expect(graphsOf(s)[1]).toEqual([{ net: 100, disk: 0 }]);
  });

  it('freezes a terminal entry\'s ring instead of appending zeros', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    observe(s, [entry({ downloaded_bytes: 800 })]);
    tick(s, 1000);
    observe(s, [entry({ status: 'completed', downloaded_bytes: 800 })]);
    tick(s, 2000);
    tick(s, 3000);
    expect(graphsOf(s)[1]).toEqual([{ net: 800, disk: 0 }]);
  });

  it('samples queued, installing and cancelling entries too (they are live)', () => {
    const s = createSampler(0);
    observe(s, [
      entry({ id: 1, status: 'queued' }),
      entry({ id: 2, status: 'installing', install_processed_bytes: 0 }),
      entry({ id: 3, status: 'cancelling' }),
    ]);
    observe(s, [
      entry({ id: 1, status: 'queued' }),
      entry({ id: 2, status: 'installing', install_processed_bytes: 640 }),
      entry({ id: 3, status: 'cancelling' }),
    ]);
    tick(s, 1000);
    expect(graphsOf(s)).toEqual({
      1: [{ net: 0, disk: 0 }],
      2: [{ net: 0, disk: 640 }],
      3: [{ net: 0, disk: 0 }],
    });
  });

  it('drops the track when the entry leaves the snapshot', () => {
    const s = createSampler(0);
    observe(s, [entry({ id: 1 }), entry({ id: 2 })]);
    tick(s, 1000);
    observe(s, [entry({ id: 2 })]);
    expect(Object.keys(graphsOf(s))).toEqual(['2']);
  });

  it('keeps only the newest SAMPLE_COUNT samples', () => {
    const s = createSampler(0);
    observe(s, [entry({ downloaded_bytes: 0 })]);
    for (let i = 1; i <= SAMPLE_COUNT + 5; i += 1) {
      observe(s, [entry({ downloaded_bytes: i * 10 })]);
      tick(s, i * 1000);
    }
    const samples = graphsOf(s)[1];
    expect(samples).toHaveLength(SAMPLE_COUNT);
    expect(samples[0]).toEqual({ net: 10, disk: 0 });
    expect(samples[SAMPLE_COUNT - 1]).toEqual({ net: 10, disk: 0 });
  });
});
```

- [ ] **Step 6: Run it to verify it fails**

Run: `npx vitest run src/lib/downloads/sampler.test.ts`
Expected: FAIL — `Failed to resolve import "./sampler"`.

- [ ] **Step 7: Write `sampler.ts`**

Create `app/src/lib/downloads/sampler.ts`:

```ts
// Design §8: "the downloads store keeps a ring buffer per entry fed from
// `downloaded_bytes` and `install_processed_bytes` deltas on each progress
// event, sampled once per second. No new IPC."
//
// `observe` runs on every `downloads-changed` snapshot and folds each
// entry's byte-counter movement into a pending delta. `tick` runs once per
// second, turns the pending deltas into one bytes-per-second sample per live
// entry, and resets them. Pure: the store owns the timer and the clock.
import type { DownloadEntry, DownloadStatus } from '../api';
import { createRing, pushSample, samplesOf, type Ring, type Sample } from './ring';

/** How often the store calls `tick`. */
export const SAMPLE_INTERVAL_MS = 1000;

const LIVE_STATUSES = new Set<DownloadStatus>(['queued', 'downloading', 'installing', 'cancelling']);

type Track = {
  ring: Ring;
  lastDownloaded: number;
  lastInstalled: number;
  pendingNet: number;
  pendingDisk: number;
  /** False once the entry is terminal: the ring freezes. */
  live: boolean;
};

export type Sampler = {
  tracks: Map<number, Track>;
  lastTickAt: number;
};

export function createSampler(nowMs: number): Sampler {
  return { tracks: new Map(), lastTickAt: nowMs };
}

/**
 * Folds one snapshot into the pending deltas. A new entry starts a track at
 * its current counters (an app that comes up mid-transfer must not book the
 * whole downloaded-so-far figure as one second's rate); a counter that moves
 * backwards contributes nothing; an entry missing from the snapshot loses
 * its track.
 */
export function observe(sampler: Sampler, entries: DownloadEntry[]): void {
  const seen = new Set<number>();
  for (const e of entries) {
    seen.add(e.id);
    const track = sampler.tracks.get(e.id);
    if (track === undefined) {
      sampler.tracks.set(e.id, {
        ring: createRing(),
        lastDownloaded: e.downloaded_bytes,
        lastInstalled: e.install_processed_bytes,
        pendingNet: 0,
        pendingDisk: 0,
        live: LIVE_STATUSES.has(e.status),
      });
      continue;
    }
    track.pendingNet += Math.max(0, e.downloaded_bytes - track.lastDownloaded);
    track.pendingDisk += Math.max(0, e.install_processed_bytes - track.lastInstalled);
    track.lastDownloaded = e.downloaded_bytes;
    track.lastInstalled = e.install_processed_bytes;
    track.live = LIVE_STATUSES.has(e.status);
  }
  for (const id of Array.from(sampler.tracks.keys())) {
    if (!seen.has(id)) sampler.tracks.delete(id);
  }
}

/**
 * Emits one sample per live track, normalised to bytes per second over the
 * time since the previous tick, and clears the pending deltas. A tick with
 * no elapsed time is ignored rather than dividing by zero.
 */
export function tick(sampler: Sampler, nowMs: number): void {
  const elapsed = nowMs - sampler.lastTickAt;
  if (elapsed <= 0) return;
  sampler.lastTickAt = nowMs;
  const perSecond = 1000 / elapsed;
  for (const track of sampler.tracks.values()) {
    if (!track.live) continue;
    pushSample(track.ring, {
      net: track.pendingNet * perSecond,
      disk: track.pendingDisk * perSecond,
    });
    track.pendingNet = 0;
    track.pendingDisk = 0;
  }
}

/** Every track's samples, oldest first, keyed by entry id. */
export function graphsOf(sampler: Sampler): Record<number, Sample[]> {
  const out: Record<number, Sample[]> = {};
  for (const [id, track] of sampler.tracks) {
    out[id] = samplesOf(track.ring);
  }
  return out;
}
```

- [ ] **Step 8: Run it to verify it passes**

Run: `npx vitest run src/lib/downloads/sampler.test.ts`
Expected: PASS (10 tests).

- [ ] **Step 9: Write the failing segment tests**

Create `app/src/lib/downloads/segments.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { DownloadEntry, DownloadStatus } from '../api';
import {
  groupBySegment,
  LEGEND_TEXT,
  segmentEmptyText,
  segmentLabel,
  segmentOf,
  SEGMENTS,
} from './segments';

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Game',
    platform: 'Platform',
    status: 'queued',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

describe('segments', () => {
  it('lists the three segments in design §8 order', () => {
    expect(SEGMENTS).toEqual(['active', 'queued', 'completed']);
  });

  it('carries the legend line verbatim', () => {
    expect(LEGEND_TEXT).toBe(
      'Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled',
    );
  });

  it('maps every status to exactly one segment', () => {
    const expected: Record<DownloadStatus, string> = {
      downloading: 'active',
      installing: 'active',
      cancelling: 'active',
      queued: 'queued',
      completed: 'completed',
      failed: 'completed',
      cancelled: 'completed',
    };
    for (const [status, seg] of Object.entries(expected)) {
      expect(segmentOf(status as DownloadStatus)).toBe(seg);
    }
  });

  it('labels the segments', () => {
    expect(segmentLabel('active')).toBe('Active');
    expect(segmentLabel('queued')).toBe('Queued');
    expect(segmentLabel('completed')).toBe('Completed');
  });

  it('has an empty line per segment', () => {
    expect(segmentEmptyText('active')).toBe('No active transfers');
    expect(segmentEmptyText('queued')).toBe('Nothing waiting');
    expect(segmentEmptyText('completed')).toBe('Nothing finished yet');
  });

  it('groups entries by segment and keeps the snapshot order inside each group', () => {
    const entries = [
      entry({ id: 5, status: 'completed' }),
      entry({ id: 4, status: 'queued' }),
      entry({ id: 3, status: 'installing' }),
      entry({ id: 2, status: 'failed' }),
      entry({ id: 1, status: 'downloading' }),
    ];
    const groups = groupBySegment(entries);
    expect(groups.active.map((e) => e.id)).toEqual([3, 1]);
    expect(groups.queued.map((e) => e.id)).toEqual([4]);
    expect(groups.completed.map((e) => e.id)).toEqual([5, 2]);
  });

  it('always returns all three keys, empty when nothing matches', () => {
    expect(groupBySegment([])).toEqual({ active: [], queued: [], completed: [] });
  });
});
```

- [ ] **Step 10: Run it to verify it fails**

Run: `npx vitest run src/lib/downloads/segments.test.ts`
Expected: FAIL — `Failed to resolve import "./segments"`.

- [ ] **Step 11: Write `segments.ts`**

Create `app/src/lib/downloads/segments.ts`:

```ts
// Design §8: "Segments Active (live), Queued, Completed (terminal,
// dismissable), each with a count; a legend line beside them". The view
// renders all three stacked in this order (see the plan's "Deliberate
// deviations": a filter control would make rows vanish mid-transition and
// break every spec that waits on `download-detail-<id>`).
import type { DownloadEntry, DownloadStatus } from '../api';

export type Segment = 'active' | 'queued' | 'completed';

export const SEGMENTS: readonly Segment[] = ['active', 'queued', 'completed'];

/** Verbatim from design §8. */
export const LEGEND_TEXT =
  'Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled';

export function segmentOf(status: DownloadStatus): Segment {
  switch (status) {
    case 'downloading':
    case 'installing':
    case 'cancelling':
      return 'active';
    case 'queued':
      return 'queued';
    default:
      return 'completed';
  }
}

export function segmentLabel(seg: Segment): string {
  switch (seg) {
    case 'active':
      return 'Active';
    case 'queued':
      return 'Queued';
    default:
      return 'Completed';
  }
}

export function segmentEmptyText(seg: Segment): string {
  switch (seg) {
    case 'active':
      return 'No active transfers';
    case 'queued':
      return 'Nothing waiting';
    default:
      return 'Nothing finished yet';
  }
}

/** Splits a snapshot (newest first) into the three segments, order kept. */
export function groupBySegment(entries: DownloadEntry[]): Record<Segment, DownloadEntry[]> {
  const groups: Record<Segment, DownloadEntry[]> = { active: [], queued: [], completed: [] };
  for (const e of entries) groups[segmentOf(e.status)].push(e);
  return groups;
}
```

- [ ] **Step 12: Run it to verify it passes**

Run: `npx vitest run src/lib/downloads/segments.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 13: Write the failing `format.ts` tests**

Append to `app/src/lib/downloads/format.test.ts` (extend the import line first so it reads `import { actionFor, aggregate, currentTransfer, entryDetail, etaText, footerLine, formatSize, graphCaption, kindLabel, percent } from './format';`):

```ts
describe('currentTransfer', () => {
  it('is null when nothing is live', () => {
    expect(currentTransfer([])).toBeNull();
    expect(currentTransfer([entry({ status: 'completed' }), entry({ status: 'failed' })])).toBeNull();
  });

  it('prefers downloading, then installing, then the first other live entry', () => {
    const queued = entry({ id: 1, status: 'queued' });
    const installing = entry({ id: 2, status: 'installing' });
    const downloading = entry({ id: 3, status: 'downloading' });
    expect(currentTransfer([queued, installing, downloading])?.id).toBe(3);
    expect(currentTransfer([queued, installing])?.id).toBe(2);
    expect(currentTransfer([queued])?.id).toBe(1);
    expect(currentTransfer([entry({ id: 9, status: 'cancelling' })])?.id).toBe(9);
  });

  it('is the entry footerLine describes', () => {
    const entries = [entry({ id: 1, status: 'installing' }), entry({ id: 2, title: 'Two', status: 'downloading' })];
    expect(currentTransfer(entries)?.title).toBe('Two');
    expect(footerLine(entries)).toContain('⬇ Two ·');
  });
});

describe('etaText', () => {
  it('is empty unless downloading with a known total and a positive speed', () => {
    expect(etaText(entry({ status: 'queued' }))).toBe('');
    expect(etaText(entry({ status: 'installing', install_processed_bytes: 1, install_total_bytes: 2 }))).toBe('');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1, total_bytes: 0, speed_bps: 10 }))).toBe('');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1, total_bytes: 10, speed_bps: 0 }))).toBe('');
  });

  it('formats seconds, minutes and hours, rounding up', () => {
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 100 }))).toBe('5s left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 4 }))).toBe('2m 5s left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 500, total_bytes: 1000, speed_bps: 0.1 }))).toBe('1h 23m left');
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 999, total_bytes: 1000, speed_bps: 1000 }))).toBe('1s left');
  });

  it('never goes negative when downloaded exceeds total', () => {
    expect(etaText(entry({ status: 'downloading', downloaded_bytes: 1200, total_bytes: 1000, speed_bps: 10 }))).toBe('0s left');
  });
});

describe('graphCaption', () => {
  it('shows the rate and the ETA while downloading', () => {
    expect(graphCaption(entry({ status: 'downloading', downloaded_bytes: 0, total_bytes: 2048, speed_bps: 1024 }))).toBe(
      '1.0 KB/s · 2s left',
    );
  });

  it('shows only the rate when the ETA is unknown', () => {
    expect(graphCaption(entry({ status: 'downloading', downloaded_bytes: 0, total_bytes: 0, speed_bps: 512 }))).toBe('512 B/s');
    expect(graphCaption(entry({ status: 'cancelling', speed_bps: 512 }))).toBe('512 B/s');
  });

  it('names the disk phase while installing and is blank otherwise', () => {
    expect(graphCaption(entry({ status: 'installing' }))).toBe('Writing to disk');
    expect(graphCaption(entry({ status: 'queued' }))).toBe('');
    expect(graphCaption(entry({ status: 'completed' }))).toBe('');
    expect(graphCaption(entry({ status: 'failed' }))).toBe('');
  });
});
```

- [ ] **Step 14: Run it to verify it fails**

Run: `npx vitest run src/lib/downloads/format.test.ts`
Expected: FAIL — `currentTransfer`, `etaText`, `graphCaption` are not exported.

- [ ] **Step 15: Extend `format.ts`**

In `app/src/lib/downloads/format.ts`, replace the whole `footerLine` function (from its doc comment through its closing brace) with:

```ts
/**
 * "The current transfer": the first downloading entry, else the first
 * installing one, else the first entry in any other live state — the same
 * precedence the old drawer footer's progress bar used. `null` when nothing
 * is live. The strip's line and its sparkline both key off this.
 */
export function currentTransfer(entries: DownloadEntry[]): DownloadEntry | null {
  const live = entries.filter((e) => LIVE_STATUSES.includes(e.status));
  if (live.length === 0) return null;
  return (
    live.find((e) => e.status === 'downloading') ??
    live.find((e) => e.status === 'installing') ??
    live[0]
  );
}

/**
 * The 28px status strip's one line (design §3):
 * `⬇ <title> · <percent> · <speed>`, or `null` when nothing is live and the
 * strip hides itself.
 *
 * An unmeasurable percent renders as an em dash rather than a fake `0%`,
 * and the speed slot carries the phase word when there is no byte rate to
 * show (an install reads local bytes, and a queued job has not started).
 */
export function footerLine(entries: DownloadEntry[]): string | null {
  const current = currentTransfer(entries);
  if (current === null) return null;

  const dash = '—';
  let pct = dash;
  let speed: string;
  switch (current.status) {
    case 'downloading':
      if (current.total_bytes > 0) pct = `${percent(current.downloaded_bytes, current.total_bytes)}%`;
      speed = `${formatSize(current.speed_bps)}/s`;
      break;
    case 'installing':
      if (current.install_total_bytes > 0) {
        pct = `${percent(current.install_processed_bytes, current.install_total_bytes)}%`;
      }
      speed = 'Installing';
      break;
    case 'cancelling':
      speed = 'Cancelling';
      break;
    default:
      speed = 'Queued';
      break;
  }
  return `⬇ ${current.title} · ${pct} · ${speed}`;
}

/**
 * Time remaining for a download with a known total and a measured rate
 * (D-UI-6 names an ETA): `<s>s left`, `<m>m <s>s left`, `<h>h <m>m left`,
 * rounded up. Empty for every other state — an install reads local bytes at
 * a rate the backend does not report, and a queued job has no rate yet.
 */
export function etaText(e: DownloadEntry): string {
  if (e.status !== 'downloading' || e.total_bytes <= 0 || e.speed_bps <= 0) return '';
  const remaining = Math.max(0, e.total_bytes - e.downloaded_bytes);
  const secs = Math.ceil(remaining / e.speed_bps);
  if (secs >= 3600) return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m left`;
  if (secs >= 60) return `${Math.floor(secs / 60)}m ${secs % 60}s left`;
  return `${secs}s left`;
}

/**
 * The one-line caption under a row's sparkline panel: the network rate and
 * the ETA while bytes are moving over the network, the phase word while the
 * install writes to disk, blank for queued and terminal rows.
 */
export function graphCaption(e: DownloadEntry): string {
  switch (e.status) {
    case 'downloading':
    case 'cancelling': {
      const rate = `${formatSize(e.speed_bps)}/s`;
      const eta = etaText(e);
      return eta === '' ? rate : `${rate} · ${eta}`;
    }
    case 'installing':
      return 'Writing to disk';
    default:
      return '';
  }
}
```

- [ ] **Step 16: Run it to verify it passes**

Run: `npx vitest run src/lib/downloads/format.test.ts`
Expected: PASS — every existing `footerLine` case still green plus the nine new ones.

- [ ] **Step 17: Full gate and commit**

From `rewrite/app`: `npm run check` and `npx vitest run` — both green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/downloads/ring.ts rewrite/app/src/lib/downloads/ring.test.ts \
  rewrite/app/src/lib/downloads/sampler.ts rewrite/app/src/lib/downloads/sampler.test.ts \
  rewrite/app/src/lib/downloads/segments.ts rewrite/app/src/lib/downloads/segments.test.ts \
  rewrite/app/src/lib/downloads/format.ts rewrite/app/src/lib/downloads/format.test.ts
git commit -m "rewrite: add the downloads ring buffer, sampler and segment rules"
```

---

### Task 2: The sparkline path module and the `Sparkline.svelte` component

**Files:**
- Create: `app/src/lib/downloads/sparkline.ts`, `app/src/lib/downloads/sparkline.test.ts`
- Create: `app/src/lib/downloads/Sparkline.svelte`

**Interfaces:**
- Consumes: `Sample`, `SAMPLE_COUNT` from Task 1's `ring.ts`.
- Produces, used by Tasks 5 and 6:
  - `sparkline.ts`: `export type SparklineBox = { width: number; height: number }`; `sharedMax(samples: Sample[]): number` (≥ 1); `linePath(values: number[], capacity: number, box: SparklineBox, max: number): string`; `sparklinePaths(samples: Sample[], box: SparklineBox, capacity?: number): { net: string; disk: string; max: number }`.
  - `Sparkline.svelte` props: `{ samples: Sample[]; width: number; height: number; label: string; testId?: string }`. Renders `<svg data-testid={testId} class="spark" role="img" aria-label={label}>` with exactly two children: `<path class="net">` (stroke `var(--primary)`) then `<path class="disk">` (stroke `var(--graph-disk)`). Both paths are always present, with an empty `d` when there is nothing to draw, so E2E can assert structure without waiting on samples.

- [ ] **Step 1: Write the failing path tests**

Create `app/src/lib/downloads/sparkline.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { SAMPLE_COUNT } from './ring';
import { linePath, sharedMax, sparklinePaths } from './sparkline';

describe('sharedMax', () => {
  it('is the largest value across both series', () => {
    expect(sharedMax([{ net: 3, disk: 9 }, { net: 12, disk: 1 }])).toBe(12);
    expect(sharedMax([{ net: 3, disk: 90 }, { net: 12, disk: 1 }])).toBe(90);
  });

  it('is at least 1 so a flat line never divides by zero', () => {
    expect(sharedMax([])).toBe(1);
    expect(sharedMax([{ net: 0, disk: 0 }])).toBe(1);
    expect(sharedMax([{ net: 0.25, disk: 0 }])).toBe(1);
  });
});

describe('linePath', () => {
  const box = { width: 20, height: 10 };

  it('is empty with no values', () => {
    expect(linePath([], 3, box, 10)).toBe('');
  });

  it('anchors the newest sample at the right edge and leaves 1px of padding top and bottom', () => {
    // capacity 3 over 20px → one step is 10px. Two values fill the last two
    // slots: x = 10 and x = 20. 0 sits 1px above the bottom, max 1px below
    // the top.
    expect(linePath([0, 10], 3, box, 10)).toBe('M 10 9 L 20 1');
  });

  it('draws a full buffer from the left edge', () => {
    expect(linePath([5, 5, 5], 3, box, 10)).toBe('M 0 5 L 10 5 L 20 5');
  });

  it('renders a single sample as a zero-length segment (a dot with round caps)', () => {
    expect(linePath([10], 3, box, 10)).toBe('M 20 1 L 20 1');
  });

  it('rounds coordinates to two decimals', () => {
    const values = new Array<number>(SAMPLE_COUNT).fill(0);
    const d = linePath(values, SAMPLE_COUNT, { width: 120, height: 38 }, 1);
    expect(d.startsWith('M 0 37 L 2.03 37 L 4.07 37')).toBe(true);
    expect(d.endsWith('L 120 37')).toBe(true);
  });

  it('clamps a value above max to the top', () => {
    expect(linePath([50], 1, box, 10)).toBe('M 0 1 L 0 1');
  });
});

describe('sparklinePaths', () => {
  it('draws both series on one shared scale', () => {
    const box = { width: 20, height: 10 };
    const paths = sparklinePaths([{ net: 10, disk: 5 }, { net: 0, disk: 10 }], box, 2);
    expect(paths.max).toBe(10);
    expect(paths.net).toBe('M 0 1 L 20 9');
    expect(paths.disk).toBe('M 0 5 L 20 1');
  });

  it('defaults to the 60-sample capacity and empty paths for no samples', () => {
    const paths = sparklinePaths([], { width: 120, height: 38 });
    expect(paths).toEqual({ net: '', disk: '', max: 1 });
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/downloads/sparkline.test.ts`
Expected: FAIL — `Failed to resolve import "./sparkline"`.

- [ ] **Step 3: Write `sparkline.ts`**

Create `app/src/lib/downloads/sparkline.ts`:

```ts
// SVG path strings for the transfer sparklines (design §8: "120×38: network
// in primary, disk in teal, 60 one-second samples"). Pure string maths so
// the component stays a template and vitest covers the geometry.
import { SAMPLE_COUNT, type Sample } from './ring';

export type SparklineBox = { width: number; height: number };

/** The scale both series share: the largest value seen, never below 1. */
export function sharedMax(samples: Sample[]): number {
  let max = 1;
  for (const s of samples) {
    if (s.net > max) max = s.net;
    if (s.disk > max) max = s.disk;
  }
  return max;
}

function fmt(n: number): string {
  return Number(n.toFixed(2)).toString();
}

/**
 * One series as `M x y L x y …`. The newest value sits on the right edge;
 * a partial buffer starts part-way across so the line grows leftwards as
 * samples arrive rather than stretching. One pixel of padding keeps a
 * stroke on 0 or `max` inside the box. A lone value becomes a zero-length
 * segment so `stroke-linecap: round` draws a dot.
 */
export function linePath(values: number[], capacity: number, box: SparklineBox, max: number): string {
  const n = values.length;
  if (n === 0) return '';
  const step = capacity > 1 ? box.width / (capacity - 1) : 0;
  const inner = box.height - 2;
  const parts: string[] = [];
  for (let i = 0; i < n; i += 1) {
    const x = (capacity - n + i) * step;
    const ratio = Math.min(1, Math.max(0, values[i] / max));
    const y = box.height - 1 - ratio * inner;
    parts.push(`${i === 0 ? 'M' : 'L'} ${fmt(x)} ${fmt(y)}`);
  }
  if (n === 1) parts.push(parts[0].replace(/^M/, 'L'));
  return parts.join(' ');
}

/** Both series, on one shared scale, ready for two `<path d>` attributes. */
export function sparklinePaths(
  samples: Sample[],
  box: SparklineBox,
  capacity: number = SAMPLE_COUNT,
): { net: string; disk: string; max: number } {
  const max = sharedMax(samples);
  return {
    net: linePath(samples.map((s) => s.net), capacity, box, max),
    disk: linePath(samples.map((s) => s.disk), capacity, box, max),
    max,
  };
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npx vitest run src/lib/downloads/sparkline.test.ts`
Expected: PASS (10 tests).

- [ ] **Step 5: Create `Sparkline.svelte`**

Create `app/src/lib/downloads/Sparkline.svelte`:

```svelte
<script lang="ts">
  import type { Sample } from './ring';
  import { sparklinePaths } from './sparkline';

  // One sparkline for both the row panel (120×38) and the footer strip
  // (120×18). Network is the primary colour, disk is the §4 teal, both on
  // one shared scale so a viewer can compare them. The two paths always
  // exist — with an empty `d` before the first sample — so the structure is
  // stable for E2E and the layout never jumps.
  let {
    samples,
    width,
    height,
    label,
    testId = undefined,
  }: {
    samples: Sample[];
    width: number;
    height: number;
    label: string;
    testId?: string;
  } = $props();

  let paths = $derived(sparklinePaths(samples, { width, height }));
</script>

<svg
  data-testid={testId}
  class="spark"
  viewBox={`0 0 ${width} ${height}`}
  {width}
  {height}
  role="img"
  aria-label={label}
>
  <path class="net" d={paths.net} />
  <path class="disk" d={paths.disk} />
</svg>

<style>
  .spark {
    display: block;
    flex: none;
    border-radius: var(--r-control);
    background: var(--surface);
  }

  path {
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .net {
    stroke: var(--primary);
  }

  .disk {
    stroke: var(--graph-disk);
  }
</style>
```

- [ ] **Step 6: Full gate and commit**

From `rewrite/app`: `npm run check` and `npx vitest run` — both green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/downloads/sparkline.ts rewrite/app/src/lib/downloads/sparkline.test.ts \
  rewrite/app/src/lib/downloads/Sparkline.svelte
git commit -m "rewrite: add the downloads sparkline geometry and component"
```

---

### Task 3: The store — sampler, one-second timer, `samplesFor`

**Files:**
- Modify: `app/src/lib/stores/downloads.svelte.ts` (whole file, 23 lines)
- Create: `app/src/lib/stores/downloads.test.ts`

**Interfaces:**
- Consumes: Task 1's `createSampler`, `observe`, `tick`, `graphsOf`, `SAMPLE_INTERVAL_MS` from `../downloads/sampler`; `Sample` from `../downloads/ring`; `api.listDownloads`, `DownloadsSnapshot` from `../api`.
- Produces, used by Tasks 5 and 6: the `downloads` object keeps `entries` and `hasLive` unchanged (five other modules read them) and gains `samplesFor(id: number): Sample[]` (oldest first; a shared empty array for an unknown id). `init(): Promise<UnlistenFn>` — the returned function now also clears the timer.

- [ ] **Step 1: Write the failing store test**

Create `app/src/lib/stores/downloads.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DownloadEntry, DownloadsSnapshot } from '../api';

// `downloads.svelte.ts` is module-scoped state, so each test takes a fresh
// module instance: `vi.resetModules()` plus a dynamic `import()` after the
// fakes are wired with `vi.doMock`. Fake timers also fake `Date.now`, which
// is the clock the store hands the sampler.

function entry(overrides: Partial<DownloadEntry>): DownloadEntry {
  return {
    id: 1,
    job: 'game',
    kind: 'base',
    rom_id: 1,
    source_id: '',
    title: 'Game',
    platform: 'Platform',
    status: 'downloading',
    downloaded_bytes: 0,
    total_bytes: 0,
    speed_bps: 0,
    install_processed_bytes: 0,
    install_total_bytes: 0,
    error: '',
    ...overrides,
  };
}

type SnapshotHandler = (event: { payload: DownloadsSnapshot }) => void;

function wire(initial: DownloadEntry[]) {
  const captured: { handler?: SnapshotHandler } = {};
  const unlisten = vi.fn();
  vi.doMock('../api', () => ({
    api: { listDownloads: async () => ({ entries: initial }) },
  }));
  vi.doMock('@tauri-apps/api/event', () => ({
    listen: async (_name: string, handler: SnapshotHandler) => {
      captured.handler = handler;
      return unlisten;
    },
  }));
  return {
    unlisten,
    emit(entries: DownloadEntry[]) {
      captured.handler!({ payload: { entries } });
    },
  };
}

describe('downloads store sampling', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-09-04T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.doUnmock('../api');
    vi.doUnmock('@tauri-apps/api/event');
  });

  it('exposes one sample per second built from the byte deltas of the snapshots', async () => {
    const mock = wire([entry({ id: 1, downloaded_bytes: 1_000, total_bytes: 10_000 })]);
    const { downloads, init } = await import('./downloads.svelte');
    const stop = await init();

    expect(downloads.entries).toHaveLength(1);
    expect(downloads.samplesFor(1)).toEqual([]);

    mock.emit([entry({ id: 1, downloaded_bytes: 3_000, total_bytes: 10_000 })]);
    mock.emit([entry({ id: 1, status: 'installing', downloaded_bytes: 3_000, install_processed_bytes: 500 })]);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(downloads.samplesFor(1)).toEqual([{ net: 2_000, disk: 500 }]);
    expect(downloads.entries[0].status).toBe('installing');

    stop();
    expect(mock.unlisten).toHaveBeenCalledTimes(1);
  });

  it('stops sampling once stopped and returns an empty list for an unknown id', async () => {
    const mock = wire([entry({ id: 1 })]);
    const { downloads, init } = await import('./downloads.svelte');
    const stop = await init();
    stop();

    mock.emit([entry({ id: 1, downloaded_bytes: 999 })]);
    await vi.advanceTimersByTimeAsync(3_000);

    expect(downloads.samplesFor(1)).toEqual([]);
    expect(downloads.samplesFor(42)).toEqual([]);
  });

  it('keeps hasLive on the live statuses only', async () => {
    const mock = wire([entry({ id: 1, status: 'completed' })]);
    const { downloads, init } = await import('./downloads.svelte');
    await init();
    expect(downloads.hasLive).toBe(false);
    mock.emit([entry({ id: 2, status: 'queued' })]);
    expect(downloads.hasLive).toBe(true);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npx vitest run src/lib/stores/downloads.test.ts`
Expected: FAIL — `downloads.samplesFor is not a function`.

- [ ] **Step 3: Rewrite the store**

Replace the whole of `app/src/lib/stores/downloads.svelte.ts` with:

```ts
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { api, type DownloadEntry, type DownloadsSnapshot } from '../api';
import type { Sample } from '../downloads/ring';
import { createSampler, graphsOf, observe, SAMPLE_INTERVAL_MS, tick } from '../downloads/sampler';

const LIVE_STATUSES = new Set(['queued', 'downloading', 'installing', 'cancelling']);

const state = $state<{ entries: DownloadEntry[]; graphs: Record<number, Sample[]> }>({
  entries: [],
  graphs: {},
});

// Design §8: the store keeps a ring buffer per entry fed from the byte
// deltas of every `downloads-changed` snapshot, sampled once per second.
// The sampler is plain (non-reactive) state; `state.graphs` is its
// once-per-second reactive mirror, so rows re-render on the tick and not on
// every 100ms progress event.
let sampler = createSampler(Date.now());

const NO_SAMPLES: Sample[] = [];

export const downloads = {
  get entries() {
    return state.entries;
  },
  get hasLive() {
    return state.entries.some((e) => LIVE_STATUSES.has(e.status));
  },
  /** The entry's transfer-rate samples, oldest first; empty for an unknown id. */
  samplesFor(id: number): Sample[] {
    return state.graphs[id] ?? NO_SAMPLES;
  },
};

function apply(snapshot: DownloadsSnapshot): void {
  state.entries = snapshot.entries;
  observe(sampler, snapshot.entries);
}

export async function init(): Promise<UnlistenFn> {
  sampler = createSampler(Date.now());
  state.graphs = {};
  apply(await api.listDownloads());
  const timer = setInterval(() => {
    tick(sampler, Date.now());
    state.graphs = graphsOf(sampler);
  }, SAMPLE_INTERVAL_MS);
  const unlisten = await listen<DownloadsSnapshot>('downloads-changed', (e) => {
    apply(e.payload);
  });
  return () => {
    clearInterval(timer);
    unlisten();
  };
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npx vitest run src/lib/stores/downloads.test.ts`
Expected: PASS (3 tests). Also run `npx vitest run src/lib/stores/installedRefresh.test.ts` — it mocks `./downloads.svelte` with `{ downloads: { entries: [] } }` and must stay green.

- [ ] **Step 5: Full gate and commit**

From `rewrite/app`: `npm run check` and `npx vitest run` — both green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/stores/downloads.svelte.ts rewrite/app/src/lib/stores/downloads.test.ts
git commit -m "rewrite: sample transfer rates in the downloads store"
```

---

### Task 4: The 50-entry terminal history cap in `QueueState`

**Files:**
- Modify: `crates/grid-core/src/library/queue.rs` — a constant beside `DownloadsSnapshot` (~line 80), the tail of `finish_external` (~line 224), `download_finished` (~line 271), `finalize_finished` (~line 310), `request_cancel` (~line 357), a new private method beside `alloc_id` (~line 449), and two tests appended inside `mod tests`.

**Interfaces:**
- Consumes: `QueueState`, `DownloadStatus`, `JobKey`, `Admission`, `CancelAction`, `LibraryError` — all already in `queue.rs`.
- Produces: `pub const TERMINAL_HISTORY: usize = 50;` on the module. No IPC, event or payload change. `QueueState::snapshot()` never returns more than 50 terminal entries.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` at the tail of `crates/grid-core/src/library/queue.rs` (before its closing `}`):

```rust
    // --- terminal history cap (design §8: "Completed keeps the last 50") --

    #[test]
    fn terminal_entries_are_capped_at_the_history_limit_oldest_first() {
        let mut state = QueueState::default();
        for rom in 1..=(TERMINAL_HISTORY as i64 + 5) {
            let id = admit_idle(&mut state, rom);
            state.download_finished(id, Ok(()), true);
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.entries.len(), TERMINAL_HISTORY);
        // Ids are allocated in admission order, so the five oldest are 1..=5.
        assert!(snapshot.entries.iter().all(|entry| entry.id > 5));
        // Newest first, untouched.
        assert_eq!(snapshot.entries[0].id, TERMINAL_HISTORY as u64 + 5);
    }

    #[test]
    fn every_terminal_transition_prunes_including_cancel_and_external() {
        let mut state = QueueState::default();
        // Fill the history with completed installs.
        for rom in 1..=(TERMINAL_HISTORY as i64) {
            let id = admit_idle(&mut state, rom);
            state.download_finished(id, Ok(()), true);
        }
        // A failed finalize is terminal and evicts id 1.
        let failed = admit_idle(&mut state, 1000);
        state.download_finished(failed, Ok(()), false);
        state.finalize_finished(failed, Err(LibraryError::Cancelled), "");
        assert!(state.entry(1).is_none());
        assert_eq!(state.snapshot().entries.len(), TERMINAL_HISTORY);

        // A queued entry cancelled out of the queue is terminal and evicts id 2.
        let live = admit_idle(&mut state, 1001);
        let queued = match state.admit(JobKey::Rom(1002), "Title", "Platform", "base") {
            Admission::Queued(id) => id,
            other => panic!("expected Queued, got {other:?}"),
        };
        assert_eq!(state.request_cancel(queued), CancelAction::RemovedFromQueue);
        assert!(state.entry(2).is_none());

        // An external (firmware) row finishing is terminal and evicts id 3.
        let external = state.admit_external("PS3 Firmware", "PS3");
        state.finish_external(external, "");
        assert!(state.entry(3).is_none());

        // The live download was never touched and still owns its slot.
        assert_eq!(state.entry(live).unwrap().status, DownloadStatus::Downloading);
        assert_eq!(state.next_ready(), None);
        let terminal = state
            .snapshot()
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.status,
                    DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
                )
            })
            .count();
        assert_eq!(terminal, TERMINAL_HISTORY);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run, from `rewrite/`: `cargo test -p grid-core --lib library::queue::tests::terminal_entries_are_capped_at_the_history_limit_oldest_first library::queue::tests::every_terminal_transition_prunes_including_cancel_and_external`
Expected: compile error — `TERMINAL_HISTORY` not found.

- [ ] **Step 3: Add the constant and the pruning**

In `crates/grid-core/src/library/queue.rs`, directly above `/// The full entry list, newest first (reverse of insertion order).` (the `DownloadsSnapshot` doc comment), add:

```rust
/// How many terminal entries (`Completed`, `Failed`, `Cancelled`) the list
/// keeps (design §8: "Completed keeps the last 50 entries"). Every terminal
/// transition drops the oldest ones past this; live entries are never
/// counted or pruned. The cap lives here, not in the frontend, because this
/// list is the source of truth `list_downloads` returns.
pub const TERMINAL_HISTORY: usize = 50;
```

Directly below the `alloc_id` method (inside `impl QueueState`), add:

```rust
    fn is_terminal(status: DownloadStatus) -> bool {
        matches!(
            status,
            DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
        )
    }

    /// Drops the oldest terminal entries past [`TERMINAL_HISTORY`]. Entries
    /// are stored oldest first, so a forward `retain` evicts in age order.
    /// A terminal entry never owns a slot and never sits in `waiting`, so
    /// nothing else needs updating.
    fn prune_terminal(&mut self) {
        let terminal = self
            .entries
            .iter()
            .filter(|entry| Self::is_terminal(entry.status))
            .count();
        let mut excess = terminal.saturating_sub(TERMINAL_HISTORY);
        if excess == 0 {
            return;
        }
        self.entries.retain(|entry| {
            if excess > 0 && Self::is_terminal(entry.status) {
                excess -= 1;
                false
            } else {
                true
            }
        });
    }
```

Then call it at each terminal transition:

1. In `finish_external`, the function currently ends with the `if error.is_empty() { … } else { … }` block. Add `self.prune_terminal();` as the last statement of the function, after that block.
2. In `download_finished`, add `self.prune_terminal();` as the last statement, after the `match result { … }` block.
3. In `finalize_finished`, add `self.prune_terminal();` as the last statement, after the `match result { … }` block.
4. In `request_cancel`, inside the `if let Some(pos) = self.waiting.iter().position(…)` block, add `self.prune_terminal();` on the line before `return CancelAction::RemovedFromQueue;`.

The `entry` borrows in those functions end at their last use inside the `match` / `if let`, so the `&mut self` call compiles without restructuring.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p grid-core --lib library::queue`
Expected: every queue test green, including the two new ones. Then `cargo test --workspace` — `install_service.rs` drives the queue through the service and must stay green (no test there accumulates 50 terminal entries).

- [ ] **Step 5: Full gate and commit**

From `rewrite/`: `cargo fmt`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p app --all-targets --features e2e -- -D warnings`; `cargo test --workspace`. From `rewrite/app`: `npm run check`; `npx vitest run`.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/crates/grid-core/src/library/queue.rs
git commit -m "rewrite: keep the last 50 terminal download entries"
```

---

### Task 5: The Downloads view — legend, segments, rows with the graph panel

**Files:**
- Modify: `app/src/lib/Downloads.svelte` (whole file, 242 lines)

**Interfaces:**
- Consumes: `downloads.entries`, `downloads.samplesFor` (Task 3); `SEGMENTS`, `LEGEND_TEXT`, `segmentLabel`, `segmentEmptyText`, `groupBySegment` (Task 1's `segments.ts`); `actionFor`, `entryDetail`, `graphCaption`, `kindLabel`, `percent` (`format.ts`); `Sparkline.svelte` (Task 2); `api.cancelInstall` / `retryInstall` / `dismissDownload` (existing).
- Produces, asserted by Task 7: `downloads-legend`; `downloads-seg-active` / `-queued` / `-completed` (section elements, always present); `downloads-seg-count-<name>`; `downloads-graph-key`; inside each `download-row-<id>`: `.title`, `download-kind-<id>` (only when the label is non-empty), `download-detail-<id>`, `download-graph-<id>` (the `<svg>`), `download-graph-caption-<id>`, `download-action-cancel-<id>` / `-retry-<id>` / `-dismiss-<id>`.

- [ ] **Step 1: Rewrite `Downloads.svelte`**

Replace the whole of `app/src/lib/Downloads.svelte` with:

```svelte
<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { api, type DownloadEntry } from './api';
  import { actionFor, entryDetail, graphCaption, kindLabel, percent } from './downloads/format';
  import {
    groupBySegment,
    LEGEND_TEXT,
    segmentEmptyText,
    segmentLabel,
    SEGMENTS,
  } from './downloads/segments';
  import Sparkline from './downloads/Sparkline.svelte';

  let errors = $state<Record<number, string>>({});
  let pending = $state<Record<number, boolean>>({});

  // Design §8: three stacked segments in this order. A row moves between
  // them as its status changes; its `download-row-<id>` element exists
  // exactly once at all times, which every install spec relies on.
  let groups = $derived(groupBySegment(downloads.entries));

  function errorMessage(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function runAction(id: number, action: () => Promise<void>) {
    const { [id]: _dropped, ...rest } = errors;
    errors = rest;
    pending = { ...pending, [id]: true };
    try {
      await action();
    } catch (err) {
      errors = { ...errors, [id]: errorMessage(err) };
    } finally {
      const { [id]: _cleared, ...remaining } = pending;
      pending = remaining;
    }
  }

  const cancel = (id: number) => runAction(id, () => api.cancelInstall(id));
  const retry = (id: number) => runAction(id, () => api.retryInstall(id));
  const dismiss = (id: number) => runAction(id, () => api.dismissDownload(id));

  type Progress = { pct: number; indeterminate: boolean };

  function rowProgress(e: DownloadEntry): Progress {
    switch (e.status) {
      case 'downloading':
      case 'cancelling':
        return e.total_bytes > 0
          ? { pct: percent(e.downloaded_bytes, e.total_bytes), indeterminate: false }
          : { pct: 0, indeterminate: true };
      case 'installing':
        return e.install_total_bytes > 0
          ? { pct: percent(e.install_processed_bytes, e.install_total_bytes), indeterminate: false }
          : { pct: 0, indeterminate: true };
      case 'completed':
        return { pct: 100, indeterminate: false };
      case 'queued':
        return { pct: 0, indeterminate: false };
      default: // failed, cancelled
        return {
          pct: e.total_bytes > 0 ? percent(e.downloaded_bytes, e.total_bytes) : 0,
          indeterminate: false,
        };
    }
  }
</script>

<!-- D-UI-7: `.view-content` caps the column at 1100px and centres it. -->
<section class="downloads view-content" aria-label="Downloads">
  <header class="head">
    <div class="head-text">
      <h1>Downloads</h1>
      <p data-testid="downloads-legend" class="legend">{LEGEND_TEXT}</p>
    </div>
    <div data-testid="downloads-graph-key" class="graph-key" aria-label="Graph colours">
      <span class="key-item"><span class="swatch net" aria-hidden="true"></span>Network</span>
      <span class="key-item"><span class="swatch disk" aria-hidden="true"></span>Disk</span>
    </div>
  </header>

  {#each SEGMENTS as seg (seg)}
    {@const rows = groups[seg]}
    <section data-testid={`downloads-seg-${seg}`} class="segment" aria-label={segmentLabel(seg)}>
      <h2 class="seg-head">
        <span>{segmentLabel(seg)}</span>
        <span data-testid={`downloads-seg-count-${seg}`} class="seg-count">{rows.length}</span>
      </h2>
      {#if rows.length === 0}
        <p class="seg-empty">{segmentEmptyText(seg)}</p>
      {:else}
        {#each rows as e (e.id)}
          {@const action = actionFor(e.status, e.kind)}
          {@const progress = rowProgress(e)}
          <div data-testid={`download-row-${e.id}`} class="row">
            <div class="row-text">
              <span class="title-row">
                <span class="title">{e.title}</span>
                {#if kindLabel(e.kind)}
                  <span data-testid={`download-kind-${e.id}`} class="kind">{kindLabel(e.kind)}</span>
                {/if}
                <span class="platform">{e.platform}</span>
              </span>
              <span data-testid={`download-detail-${e.id}`} class="detail">{entryDetail(e)}</span>
              <span class="bar-track" class:indeterminate={progress.indeterminate}>
                <span class="bar-fill" style={progress.indeterminate ? '' : `width: ${progress.pct}%`}></span>
              </span>
              {#if errors[e.id]}
                <p class="row-error">{errors[e.id]}</p>
              {/if}
            </div>

            <!-- Design §8: the 120×38 sparkline panel beside the buttons —
                 network in primary, disk in teal, 60 one-second samples. -->
            <div class="graph">
              <Sparkline
                samples={downloads.samplesFor(e.id)}
                width={120}
                height={38}
                label={`Transfer rate for ${e.title}`}
                testId={`download-graph-${e.id}`}
              />
              <span data-testid={`download-graph-caption-${e.id}`} class="graph-caption">
                {graphCaption(e)}
              </span>
            </div>

            <div class="row-actions">
              {#if action === 'cancel'}
                <button data-testid={`download-action-cancel-${e.id}`} disabled={pending[e.id]} onclick={() => cancel(e.id)}>Cancel</button>
              {:else if action === 'retry-dismiss'}
                <button data-testid={`download-action-retry-${e.id}`} disabled={pending[e.id]} onclick={() => retry(e.id)}>Retry</button>
                <button data-testid={`download-action-dismiss-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {:else if action === 'dismiss'}
                <button data-testid={`download-action-dismiss-${e.id}`} class="secondary" disabled={pending[e.id]} onclick={() => dismiss(e.id)}>Dismiss</button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </section>
  {/each}
</section>

<style>
  .downloads {
    padding: 24px;
  }

  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  .head-text {
    min-width: 0;
  }

  .head h1 {
    margin: 0 0 4px;
  }

  .legend {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .graph-key {
    display: flex;
    flex: none;
    gap: 12px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .key-item {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .swatch {
    width: 14px;
    height: 2px;
    border-radius: 1px;
  }

  .swatch.net {
    background: var(--primary);
  }

  .swatch.disk {
    background: var(--graph-disk);
  }

  .segment {
    margin-bottom: 20px;
  }

  .seg-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-h);
  }

  .seg-count {
    padding: 0 7px;
    border-radius: var(--r-pill);
    background: var(--surface);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .seg-empty {
    margin: 0;
    padding: 10px 16px;
    border-radius: var(--r-row);
    border: 1px dashed var(--border);
    font-size: 12px;
    color: var(--text-muted);
  }

  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    margin-bottom: 8px;
    border-radius: var(--r-row);
    background: var(--surface);
    transition: background var(--m-fast) ease;
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .title {
    flex: 0 1 auto;
    min-width: 0;
    color: var(--text-h);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    flex: none;
    padding: 1px 6px;
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text);
    opacity: 0.8;
    white-space: nowrap;
  }

  .platform {
    flex: none;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .detail {
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bar-track {
    position: relative;
    display: block;
    width: 100%;
    height: 4px;
    margin-top: 2px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .bar-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    border-radius: 2px;
    background: var(--primary);
    transition: width var(--m-base) ease;
  }

  .bar-track.indeterminate .bar-fill {
    width: 35% !important;
    animation: indeterminate 1.1s ease-in-out infinite;
  }

  @keyframes indeterminate {
    0% {
      left: -35%;
    }
    100% {
      left: 100%;
    }
  }

  .row-error {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--danger);
  }

  .graph {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    width: 120px;
  }

  .graph-caption {
    min-height: 13px;
    font-size: 10px;
    line-height: 13px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .row-actions button {
    font: inherit;
    font-size: 12px;
    padding: 5px 12px;
    border-radius: var(--r-chip);
    border: 1px solid transparent;
    background: var(--primary);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--m-fast) ease;
  }

  .row-actions button:hover:not(:disabled) {
    background: var(--primary-hover);
  }

  .row-actions button.secondary {
    background: transparent;
    border-color: var(--border);
    color: var(--text-h);
  }

  .row-actions button.secondary:hover:not(:disabled) {
    background: var(--surface);
  }

  .row-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
```

- [ ] **Step 2: Run the frontend gates**

From `rewrite/app`: `npm run check` (svelte-check must accept the `{@const}` inside the nested `{#each}` and the `Sparkline` props) and `npx vitest run`.

- [ ] **Step 3: Run the two E2E groups that read the most from the rows**

From `rewrite/`: `scripts/e2e.sh downloads` and `scripts/e2e.sh firmware`. Both must be green with the existing specs untouched — that is the proof the row ids, the `.title` class and every `download-detail-<id>` text survived the layout change.

- [ ] **Step 4: Full gate and commit**

From `rewrite/app`: `npm run check` and `npx vitest run` — both green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/Downloads.svelte
git commit -m "rewrite: redesign the Downloads view with segments and sparklines"
```

---

### Task 6: The footer strip's sparkline

**Files:**
- Modify: `app/src/lib/DownloadsFooter.svelte` (whole file, 83 lines)

**Interfaces:**
- Consumes: `downloads.entries`, `downloads.samplesFor` (Task 3); `currentTransfer`, `footerLine` (Task 1's `format.ts`); `Sparkline.svelte` (Task 2).
- Produces, asserted by Task 7: `downloads-footer` (kept), `downloads-aggregate` (kept: the `⬇ … · … · …` line), `downloads-footer-graph` (the 120×18 `<svg>`, present only while something is live).

- [ ] **Step 1: Rewrite `DownloadsFooter.svelte`**

Replace the whole of `app/src/lib/DownloadsFooter.svelte` with:

```svelte
<script lang="ts">
  import { downloads } from './stores/downloads.svelte';
  import { currentTransfer, footerLine } from './downloads/format';
  import Sparkline from './downloads/Sparkline.svelte';

  let { onOpen }: { onOpen: () => void } = $props();

  let current = $derived(currentTransfer(downloads.entries));
  let line = $derived(footerLine(downloads.entries));
</script>

<!-- Always mounted, hidden when nothing is live (design §3). Clicking
     anywhere on the strip opens the Downloads view. The sparkline is the
     current transfer's 60 samples at 120×18 — the same component and the
     same ring the Downloads view draws at 120×38. -->
<footer
  data-testid="downloads-footer"
  class="strip"
  hidden={current === null}
  role="button"
  tabindex="0"
  onclick={onOpen}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onOpen();
    }
  }}
>
  <span data-testid="downloads-aggregate" class="line">{line ?? ''}</span>
  {#if current !== null}
    <Sparkline
      samples={downloads.samplesFor(current.id)}
      width={120}
      height={18}
      label={`Transfer rate for ${current.title}`}
      testId="downloads-footer-graph"
    />
  {/if}
  <span class="open-link">Open Downloads</span>
</footer>

<style>
  .strip {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    height: var(--footer-h);
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    background: var(--surface-2);
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
    z-index: 10;
  }

  .strip[hidden] {
    display: none;
  }

  .strip:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: -2px;
  }

  .line {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-h);
  }

  .open-link {
    flex: none;
    color: var(--primary);
    text-decoration: underline;
    white-space: nowrap;
  }
</style>
```

- [ ] **Step 2: Run the frontend gates**

From `rewrite/app`: `npm run check` and `npx vitest run`.

- [ ] **Step 3: Run the downloads E2E group**

From `rewrite/`: `scripts/e2e.sh downloads` — the existing "shows the live transfer on the footer strip and opens the view from it" and "hides the footer strip once nothing is live" cases must stay green.

- [ ] **Step 4: Full gate and commit**

From `rewrite/app`: `npm run check` and `npx vitest run` — both green. No Rust changed.

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/app/src/lib/DownloadsFooter.svelte
git commit -m "rewrite: draw the current transfer's sparkline on the footer strip"
```

---

### Task 7: E2E — segments, legend, badge, graph structure, footer text

**Files:**
- Modify: `e2e/specs/downloads.spec.ts` (whole file)

**Interfaces:**
- Consumes: every id Tasks 5 and 6 produced — `downloads-legend`, `downloads-seg-<name>`, `downloads-seg-count-<name>`, `downloads-graph-key`, `download-graph-<id>`, `download-graph-caption-<id>`, `download-kind-<id>`, `downloads-footer`, `downloads-aggregate`, `downloads-footer-graph` — plus the survivors `download-row-<id>`, `download-detail-<id>`, `download-action-*`.
- Produces: nothing new. This task's deliverable is a green suite.

**What the mock can and cannot drive.** The `downloads` group's mock streams rom 301's ~2MB in ~20KB chunks with a 100ms gap, so a download is genuinely in flight for a few seconds and the backend emits progress snapshots every ≥100ms. That is enough to assert that a graph *exists* with two paths, that a row sits in the right segment, and that the footer line reads `⬇ Big Arcade Game · <pct or —> · <rate>/s`. It is **not** enough to assert a sample count or a non-empty path: a sample lands once per wall-clock second, WebDriver round trips are hundreds of milliseconds, and the transfer may finish before the second tick. The spec therefore asserts structure, never timing — the pure-module tests own the sampling maths.

- [ ] **Step 1: Rewrite `downloads.spec.ts`**

Replace the whole of `e2e/specs/downloads.spec.ts` with:

```ts
import path from 'node:path';
import {
  APP_START_TIMEOUT,
  dataDir,
  FIXTURE_TOKEN,
  INSTALL_TIMEOUT,
  mockUrl,
  THROTTLED_DOWNLOAD_TIMEOUT,
  TRANSITION_TIMEOUT,
} from '../helpers/env.js';

const testId = (id: string) => `[data-testid="${id}"]`;

/** Verbatim from design §8; `segments.ts` carries the same string. */
const LEGEND =
  'Active: downloading or installing · Queued: waiting for a slot · Completed: finished, failed, or cancelled';

/**
 * Stage `downloads`: this group's mock server is started with
 * `--throttle-ms 100` (see rewrite/scripts/e2e.sh's mock_args_for_group),
 * so content requests stream in ~20KB chunks with a 100ms gap between them.
 * Rom 301 ("Big Arcade Game", ~2MB — see mock-romm/server.mjs's
 * `BIG_CONTENT_BYTES`) is the fixture sized to actually span several of
 * those chunks — a comfortable, real in-flight download window to cancel,
 * long enough to outlast a full second `install()` round-trip through the
 * five-view shell. Rom 201 (Pac-Man) is small and used only to prove
 * queuing: its own download slot never opens until 301's does, regardless
 * of its size.
 *
 * The queue hands out entry ids in strict admission order and never reuses
 * one (see grid-core/src/library/queue.rs's `alloc_id`), so across this
 * spec's one fresh app instance the ids are deterministic: 1 = rom 301's
 * first install, 2 = rom 201's install, 3 = rom 301's retried install.
 *
 * The redesign (design §8) splits the view into three stacked segments and
 * gives every row a sparkline panel. Sampling is once per wall-clock second
 * and WebDriver round trips are hundreds of milliseconds, so nothing here
 * asserts a sample count or a drawn path — only that the graph element with
 * its two series exists and that rows sit in the right segment.
 */
describe('downloads', () => {
  before(async () => {
    await $(testId('connect-server-url')).waitForExist({
      timeout: APP_START_TIMEOUT,
      timeoutMsg: 'the connect form never appeared — the app did not reach a usable state',
    });
    await $(testId('connect-server-url')).setValue(mockUrl());
    await $(testId('connect-secret')).setValue(FIXTURE_TOKEN);
    await $(testId('connect-submit')).click();
    await $(testId('platform-btn-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the library never rendered a platform button after connecting',
    });

    // Install needs a library path before it will admit anything.
    await $(testId('library-path-input')).setValue(path.join(dataDir(), 'library'));
    await $(testId('library-path-save')).click();
    await $(testId('library-path-banner')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the library-path banner never hid after saving a path',
    });

    await $(testId('platform-btn-2')).click();
    await $(testId('game-card-301')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'platform 2 never rendered its game cards',
    });

    await showDownloads();
  });

  // The five views no longer stack (design §3): one pill click swaps which
  // root is displayed, so a spec has to be on the right view before it reads
  // text from it or clicks anything inside it. Every row assertion below
  // reads through the Downloads view, so `install` leaves it displayed.
  async function showDownloads() {
    await $(testId('nav-downloads')).click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the downloads view never opened',
    });
  }

  async function install(cardTestId: string) {
    await $(testId('nav-server')).click();
    await $(testId('server-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the server view never came back',
    });
    await $(testId(cardTestId)).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT });
    await $(testId('details-install')).click();
    await $(testId('details-close')).click();
    await $(testId('details-panel')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await showDownloads();
  }

  /** The row with `id`, scoped to the segment it must live in right now. */
  function rowIn(segment: 'active' | 'queued' | 'completed', id: number) {
    return $(testId(`downloads-seg-${segment}`)).$(testId(`download-row-${id}`));
  }

  it('renders the three segments, their counts and the legend before anything runs', async () => {
    await expect($(testId('downloads-legend'))).toHaveText(LEGEND);
    for (const seg of ['active', 'queued', 'completed'] as const) {
      await expect($(testId(`downloads-seg-${seg}`))).toExist();
      await expect($(testId(`downloads-seg-count-${seg}`))).toHaveText('0');
    }
    await expect($(testId('downloads-graph-key'))).toExist();
  });

  it('starts the first install downloading in Active and queues the second in Queued', async () => {
    await install('game-card-301');
    await $(testId('download-row-1')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared for the first install',
    });
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()).startsWith('Downloading'),
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the throttled download for rom 301 never entered the downloading state',
      },
    );
    await expect(rowIn('active', 1)).toExist();
    await expect($(testId('downloads-seg-count-active'))).toHaveText('1');
    // A base game carries no kind badge (design §8: "base none").
    expect(await $(testId('download-kind-1')).isExisting()).toBe(false);

    await install('game-card-201');
    await $(testId('download-row-2')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'no download-row appeared for the second install',
    });
    await expect($(testId('download-detail-2'))).toHaveText('Queued');
    await expect(rowIn('queued', 2)).toExist();
    await expect($(testId('downloads-seg-count-queued'))).toHaveText('1');
  });

  it('gives every row a sparkline panel with a network and a disk series', async () => {
    for (const id of [1, 2]) {
      const graph = $(testId(`download-graph-${id}`));
      await expect(graph).toExist();
      expect(await graph.getTagName()).toBe('svg');
      const paths = await graph.$$('path');
      expect(paths.length).toBe(2);
      expect(await paths[0].getAttribute('class')).toContain('net');
      expect(await paths[1].getAttribute('class')).toContain('disk');
      await expect($(testId(`download-graph-caption-${id}`))).toExist();
    }
  });

  it('shows the live transfer on the footer strip with its sparkline and opens the view from it', async () => {
    const strip = $(testId('downloads-footer'));
    await strip.waitForDisplayed({
      timeout: INSTALL_TIMEOUT,
      timeoutMsg: 'the downloads strip never appeared for a live transfer',
    });
    // `⬇ <title> · <percent> · <speed>` (design §3). The percent is a
    // number while the total is known and an em dash otherwise; the speed
    // slot is a byte rate while downloading.
    expect(await $(testId('downloads-aggregate')).getText()).toMatch(
      /^⬇ Big Arcade Game · (\d{1,3}%|—) · [\d.]+ [KMGT]?B\/s$/,
    );
    await expect($(testId('downloads-footer-graph'))).toExist();
    expect(await strip.getText()).toContain('Open Downloads');

    await $(testId('nav-library')).click();
    await strip.click();
    await $(testId('downloads-view')).waitForDisplayed({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'clicking the strip did not open the Downloads view',
    });
  });

  it('cancels the active throttled download and moves it to Completed', async () => {
    await $(testId('download-action-cancel-1')).click();
    await browser.waitUntil(
      async () => (await $(testId('download-detail-1')).getText()) === 'Cancelled',
      {
        timeout: TRANSITION_TIMEOUT,
        timeoutMsg: 'the cancelled download never showed the Cancelled status',
      },
    );
    // Only the row's segment is asserted here, not the Completed count:
    // cancelling frees the download slot, and entry 2 (small, unthrottled
    // past the chunk gap) can reach Completed within a WebDriver round trip.
    await expect(rowIn('completed', 1)).toExist();
  });

  it('retries the cancelled download and lets it complete', async () => {
    await $(testId('download-action-retry-1')).click();
    // Retry dismisses the old entry (id 1) and creates a fresh one (id 3 —
    // see the queue id-allocation note above).
    await $(testId('download-row-1')).waitForExist({ timeout: TRANSITION_TIMEOUT, reverse: true });
    await $(testId('download-row-3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      timeoutMsg: 'the retried download never created a new row',
    });

    await browser.waitUntil(
      async () => (await $(testId('download-detail-3')).getText()).startsWith('Completed'),
      {
        // Entry 2 (rom 201) may still be occupying the download slot ahead
        // of this retry, plus the throttle itself, so this gets the
        // generous throttled-download budget.
        timeout: THROTTLED_DOWNLOAD_TIMEOUT,
        timeoutMsg: 'the retried download never reached Completed',
      },
    );
    await expect(rowIn('completed', 3)).toExist();
  });

  it('dismiss removes the completed row', async () => {
    await $(testId('download-action-dismiss-3')).click();
    await $(testId('download-row-3')).waitForExist({
      timeout: TRANSITION_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the completed row was still there after dismissing it',
    });
  });

  // The other half of the strip's contract (design §3): it is always
  // mounted, and hides itself once no entry is in a live state. Entry 2
  // (rom 201) is the last one that can still be running by now, so this
  // gets the install budget rather than a transition one.
  it('hides the footer strip once nothing is live', async () => {
    await $(testId('downloads-footer')).waitForDisplayed({
      timeout: INSTALL_TIMEOUT,
      reverse: true,
      timeoutMsg: 'the downloads strip stayed visible with no live transfer left',
    });
    await expect($(testId('downloads-seg-count-active'))).toHaveText('0');
    await expect($(testId('downloads-seg-count-queued'))).toHaveText('0');
  });
});
```

- [ ] **Step 2: Run the downloads group**

From `rewrite/`: `scripts/e2e.sh downloads`
Expected: green. If the footer regex fails on a first-tick reading, the failing text is in the assertion message: the pattern already accepts `—` for an unknown total and any unit prefix; do not loosen it further without reading the actual text.

- [ ] **Step 3: Run the full sweep**

From `rewrite/`: `scripts/e2e.sh` (no argument). Every group must be green — `firmware` (asserts `download-kind-2` = `Firmware` and `${row} .title`), `updates` (asserts `download-kind-<id>` = `Update`), `content` (reads `${row} .title`), `native`, `ps3-install`, `emulator-catalog`, `install`, `cloud-saves` all read the redesigned rows.

- [ ] **Step 4: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add rewrite/e2e/specs/downloads.spec.ts
git commit -m "rewrite: cover the Downloads segments, graphs and footer in E2E"
```

---

### Task 8: Documentation

**Files:**
- Modify: `SPEC.md` (the download-strip paragraph under `# Top Bar`, and the `**Downloads**` bullet under `# Main Sections`)
- Modify: `rewrite/README.md` (the `downloads` row of the stage table; the "Residual manual checklist" list)
- Modify: `docs/porting/03-library-install.md` (append one section)

**Interfaces:**
- Consumes: nothing. Documentation only.
- Produces: nothing code reads.

- [ ] **Step 1: Update the two SPEC.md passages**

In `SPEC.md`, replace the paragraph

```markdown
A 28px download strip sits at the bottom of the window. It is hidden while nothing is
transferring; otherwise it reads `⬇ <title> · <percent> · <speed>` and opens the
Downloads view when clicked.
```

with

```markdown
A 28px download strip sits at the bottom of the window. It is hidden while nothing is
transferring; otherwise it reads `⬇ <title> · <percent> · <speed>` beside a small
sparkline of the current transfer's last 60 seconds (network in the primary colour, disk
in teal) and an "Open Downloads" link. Clicking anywhere on the strip opens the Downloads
view.
```

and replace the bullet

```markdown
- **Downloads** contains a list of queued, active, installing, completed, and failed download jobs. Each entry should display status/progress details and support the appropriate action such as Cancel, Retry, or Dismiss.
```

with

```markdown
- **Downloads** is a full view, capped at 1100px and centred, with three stacked
  segments in this order — Active (downloading, installing, cancelling), Queued, and
  Completed (finished, failed, or cancelled) — each with a count, under the legend
  "Active: downloading or installing · Queued: waiting for a slot · Completed: finished,
  failed, or cancelled". Each row shows the title, a kind badge (none for a base game;
  Update, Content, Emulator, Compat tool, or Firmware otherwise), the platform, the
  status/progress line, a progress bar, a 120×38 sparkline panel of the last 60 seconds
  (network in the primary colour, disk in teal, with the rate and ETA under it), and the
  action for its state: Cancel, Retry and Dismiss, or Dismiss. A live firmware row offers
  no action. The Completed segment keeps the last 50 entries.
```

- [ ] **Step 2: Update the README stage row and add the manual checks**

In `rewrite/README.md`, in the stage table row that starts `| \`downloads\` | \`downloads.spec.ts\` |`, replace the cell text after the second `|` with:

```markdown
this group's mock server runs with `--throttle-ms 100` (chunked slow streaming — see `mock-romm/server.mjs`'s `e2e_throttle`) against the ~2MB "Big Arcade Game" fixture (rom 301), giving a real in-flight download to interact with: the three segments, their counts and the verbatim legend render before anything runs; a first install sits in Active and a second queues behind it in Queued; a base row carries no kind badge; every row has a `download-graph-<id>` svg with a network and a disk path (structure only — sampling is once per wall-clock second, so no spec asserts a sample count); the footer strip reads `⬇ Big Arcade Game · <pct> · <rate>/s` with its own sparkline and opens the view; cancelling the active download shows `Cancelled` in Completed; retrying it reaches `Completed`; dismissing removes the row |
```

Then append to the "Residual manual checklist" list:

```markdown
- **Download sparklines**: install a game large enough to run for a minute against a
  live server. In the Downloads view confirm the row's graph grows from the right, one
  point per second, in the primary colour during the download and in teal once the
  install phase writes to disk; the caption under it shows the rate and an ETA that
  counts down; the footer strip draws the same line at 120×18 and stops updating (but
  keeps its last shape) when the row completes.
- **Completed history**: with more than 50 finished rows (repeat a small install and
  dismiss nothing), confirm the Completed segment holds exactly 50 and the oldest rows
  disappear first while any active or queued row stays.
- **Light theme graphs**: switch Settings › Appearance to Light and confirm both series
  are legible against the row surface.
```

- [ ] **Step 3: Note the view and the sampling in the porting doc**

In `docs/porting/03-library-install.md`, append:

```markdown
## Downloads view (rewrite only)

The Rust/Tauri rewrite replaces the Python drawer with a full Downloads view (design
§8). Three segments, stacked in this order, group the entries by status: Active
(`downloading`, `installing`, `cancelling`), Queued (`queued`), Completed (`completed`,
`failed`, `cancelled`). The row texts above are unchanged; the kind badge and the action
affordance are the `kindLabel` / `actionFor` rules already ported.

Each row carries a 120×38 sparkline of the last 60 seconds — network bytes per second in
the primary colour, disk (extraction) bytes per second in teal. The samples are not an
IPC: the frontend store folds the `downloaded_bytes` and `install_processed_bytes`
deltas between successive `downloads-changed` snapshots into a pending delta per entry
and, once per second, turns it into one sample (normalised to bytes per second over the
real elapsed time). A track starts at the entry's current counters, so an app that
comes up mid-transfer does not book the whole downloaded-so-far figure as one second's
rate; a terminal entry's ring freezes; a dismissed entry's ring is dropped. The footer
strip draws the current transfer's ring at 120×18.

`QueueState` keeps at most 50 terminal entries (`TERMINAL_HISTORY`): every terminal
transition — download finished, finalize finished, a queued entry cancelled, an external
firmware row finished — drops the oldest terminal entries past the cap. Live entries are
never counted or pruned.
```

- [ ] **Step 4: Commit**

```bash
cd /home/six/Documents/Programming/grid-launcher
git add SPEC.md rewrite/README.md docs/porting/03-library-install.md
git commit -m "rewrite: document the redesigned Downloads view"
```

---

## Self-review

**1. Spec coverage.**

| Spec requirement (§8 / D-UI-6 / D-UI-7 / §3 strip / §11) | Task |
|---|---|
| Downloads is a full view (D-UI-6), root id = the view (§11) | already done by plan 1 (`downloads-view`); confirmed in the header |
| Segments Active / Queued / Completed, each with a count | 1 (`segments.ts`), 5 (`downloads-seg-<name>`, `downloads-seg-count-<name>`) — **stacked, not filtered**, see "Deliberate deviations" |
| Legend line verbatim | 1 (`LEGEND_TEXT`, tested verbatim), 5 (`downloads-legend`), 7 (asserted) |
| Row: title + kind badge (base none / Update / Content / Emulator / Compat tool / Firmware) | 5 (existing `kindLabel`; `download-kind-<id>` only when non-empty), 7 (base has none; `firmware` and `updates` specs assert the others) |
| Row: detail line = existing `entryDetail` | 5 (`download-detail-<id>`, text unchanged), 7 |
| Row: progress bar | 5 (`.bar-track`, `rowProgress` kept) |
| Row: sparkline panel 120×38, network primary, disk teal, 60 one-second samples | 1 (`SAMPLE_COUNT = 60`), 2 (`sparkline.ts`, `Sparkline.svelte` with `--primary` / `--graph-disk`), 5 (`download-graph-<id>`), 7 (structure asserted) |
| Row: action buttons from `actionFor` | 5 (ids unchanged) |
| D-UI-6: speed and ETA | 1 (`etaText`, `graphCaption`), 5 (caption under the graph) — see "Deliberate deviations" |
| Sampling: ring per entry from `downloaded_bytes` / `install_processed_bytes` deltas per progress event, once per second, no new IPC | 1 (`ring.ts`, `sampler.ts`), 3 (store timer + `samplesFor`); no Rust IPC change anywhere |
| Completed keeps the last 50 entries | 4 (`TERMINAL_HISTORY`, `prune_terminal`) — location decided and argued in "Deliberate deviations" |
| D-UI-7: list column capped at 1100px and centred | 5 (`.view-content` on the section) |
| §3 strip: hidden when nothing is live; `⬇ <title> · <percent> · <speed>`; 60-sample sparkline; "Open Downloads"; click opens the view | 1 (`currentTransfer`), 6 (`downloads-footer-graph`), 7 (text regex, graph, click) |
| §11: `download-row-<id>`, `download-detail-<id>`, `download-kind-<id>`, `downloads-footer` survive; `downloads-seg-<name>`, `download-graph-<id>` added | 5, 6, 7 |
| Colours only via `app.css` tokens; motion via `--m-*` | 2, 5, 6 (`--primary`, `--graph-disk`, `--danger`, `--surface`, `--border`; `--m-fast` / `--m-base`) |
| SPEC.md and README updated for the sections changed (§12) | 8 |

No §8 or §3-strip requirement is left without a task. The two requirements not implemented as written — segments as a filter control, and the cap's unstated location — are called out at the top of the plan.

**2. Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Tasks 5, 6 and 7 each contain the whole file, not a diff. Task 4 names the exact insertion points by function and gives the full method bodies. Task 1's `format.ts` step replaces the whole `footerLine` function so its refactor onto `currentTransfer` cannot be half-applied.

**3. Type consistency.**

- `Sample = { net: number; disk: number }` is defined once in Task 1's `ring.ts` and imported by `sampler.ts` (Task 1), `sparkline.ts` and `Sparkline.svelte` (Task 2), and the store (Task 3).
- `createSampler(nowMs)`, `observe(sampler, entries)`, `tick(sampler, nowMs)`, `graphsOf(sampler)` have those argument orders in `sampler.ts`, its test, and the store — checked at all three.
- `sparklinePaths(samples, box, capacity?)` returns `{ net, disk, max }`; `Sparkline.svelte` reads `paths.net` / `paths.disk`; the test asserts the same three keys.
- `Sparkline.svelte` props `{ samples, width, height, label, testId? }` are passed with exactly those names in `Downloads.svelte` (Task 5) and `DownloadsFooter.svelte` (Task 6).
- `downloads.samplesFor(id: number): Sample[]` is the name in the store (Task 3), its test, and both components.
- `groupBySegment` returns `Record<Segment, DownloadEntry[]>`; `Downloads.svelte` indexes it with `groups[seg]` inside `{#each SEGMENTS}` — `SEGMENTS` is `readonly Segment[]`, so `seg` is `Segment`.
- `currentTransfer` returns `DownloadEntry | null`; the footer guards with `current !== null` before reading `current.id` / `current.title`.
- `TERMINAL_HISTORY: usize` is compared against `snapshot.entries.len()` (usize) and cast with `as i64` / `as u64` only where a rom id or entry id is built in the tests.
- Test ids: every id the Task 7 spec touches (`downloads-legend`, `downloads-seg-*`, `downloads-seg-count-*`, `downloads-graph-key`, `download-graph-<id>`, `download-graph-caption-<id>`, `download-kind-<id>`, `downloads-aggregate`, `downloads-footer-graph`) is produced in Task 5 or 6 and listed in that task's Interfaces block.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-04-ui-redesign-4-downloads.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration. REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`.
2. **Inline Execution** — execute the tasks in one session with checkpoints. REQUIRED SUB-SKILL: `superpowers:executing-plans`.

Which approach?
