// Server › Discover's pure half: the action label, dates, the coverflow's
// geometry, the screenshot to show, and a generated logo's style and colours.
// The lists themselves are filtered and sorted in Rust
// (`grid_core::romm::discover`); nothing here re-orders them.

import type { DownloadEntry, DownloadStatus, GameSummary } from '../api';

/** A coverflow move and the Recently added highlight's glide. */
export const GLIDE_MS = 450;
/** The Recently added screenshot crossfade. */
export const CROSSFADE_MS = 600;
/** How long each Recently added game stays highlighted. */
export const ROTATE_MS = 10_000;
/** Covers further than this from the centre are hidden. */
export const COVERFLOW_REACH = 4;
/** Side covers tilt back by this many degrees. */
export const COVERFLOW_TILT = 52;

/** The caption's one button. */
export type DiscoverAction = 'Play' | 'Install' | 'Installing…' | 'Queued';

const LIVE: readonly DownloadStatus[] = ['queued', 'downloading', 'installing', 'cancelling'];

/** The live install row for `romId` (a game install still queued or
 *  running), or `null`. */
export function liveInstallStatus(entries: readonly DownloadEntry[], romId: number): DownloadStatus | null {
  const entry = entries.find((e) => e.job === 'game' && e.rom_id === romId && LIVE.includes(e.status));
  return entry ? entry.status : null;
}

/** Play when installed; otherwise Queued or Installing… while an install
 *  is live, else Install. */
export function discoverAction(installed: boolean, live: DownloadStatus | null): DiscoverAction {
  if (installed) return 'Play';
  if (live === 'queued') return 'Queued';
  if (live !== null) return 'Installing…';
  return 'Install';
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** `7 Nov 2026`, in UTC (a release date is a day, not a moment). */
export function formatDay(ms: number): string {
  const d = new Date(ms);
  return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`;
}

/** "Released 7 Nov 2026"; `''` without a date. */
export function releasedLine(seconds: number | null): string {
  return seconds === null ? '' : `Released ${formatDay(seconds * 1000)}`;
}

/** "Added today / yesterday / 3 days ago / 2 weeks ago", then the date.
 *  `''` when `createdAt` is blank or unreadable. Whole days, in UTC. */
export function addedAgo(createdAt: string, now: number): string {
  const at = Date.parse(createdAt);
  if (createdAt.trim() === '' || Number.isNaN(at)) return '';
  const dayOf = (ms: number) => Math.floor(ms / 86_400_000);
  const days = dayOf(now) - dayOf(at);
  if (days <= 0) return 'Added today';
  if (days === 1) return 'Added yesterday';
  if (days < 14) return `Added ${days} days ago`;
  if (days < 60) return `Added ${Math.round(days / 7)} weeks ago`;
  return `Added ${formatDay(at)}`;
}

/** `i` held inside `0..n-1` (the coverflow stops at its ends). */
export function clampIndex(i: number, n: number): number {
  if (n <= 0) return 0;
  return Math.max(0, Math.min(i, n - 1));
}

/** `i` wrapped into `0..n-1` (Recently added runs round). */
export function wrapIndex(i: number, n: number): number {
  if (n <= 0) return 0;
  return ((i % n) + n) % n;
}

/** One cover's place in the coverflow, `offset` covers from the centre. */
export type CoverPlace = { transform: string; zIndex: number; hidden: boolean; center: boolean };

/** The centre cover flat; each side cover tilted back, pushed away and
 *  spread out with distance; beyond [`COVERFLOW_REACH`] hidden. Only
 *  transforms change between moves, so the CSS transition glides them. */
export function coverPlace(offset: number): CoverPlace {
  const a = Math.abs(offset);
  if (offset === 0) {
    return { transform: 'translateX(0px) translateZ(0px) rotateY(0deg)', zIndex: 50, hidden: false, center: true };
  }
  const side = offset < 0 ? -1 : 1;
  const x = side * (150 + (a - 1) * 66);
  const z = -140 - (a - 1) * 40;
  return {
    transform: `translateX(${x}px) translateZ(${z}px) rotateY(${-side * COVERFLOW_TILT}deg)`,
    zIndex: 50 - a,
    hidden: a > COVERFLOW_REACH,
    center: false,
  };
}

/** What Recently added shows for a game, and what it is. A screenshot
 *  first, then fanart; a game with neither shows its cover (large, else
 *  small), fitted uncropped over a blurred copy like any off-ratio shot. */
export type ShotSource = { url: string; kind: 'screenshot' | 'fanart' | 'cover' };

export function shotSource(game: GameSummary): ShotSource | null {
  const shot = game.screenshot_urls.find((u) => u.trim() !== '');
  if (shot) return { url: shot, kind: 'screenshot' };
  const fanart = game.fanart_urls.find((u) => u.trim() !== '');
  if (fanart) return { url: fanart, kind: 'fanart' };
  const cover = game.path_cover_large || game.path_cover_small;
  return cover ? { url: cover, kind: 'cover' } : null;
}

/** Whether a picture `width`×`height` is at least as wide as 16:9 (it then
 *  fills the frame's width; a narrower one fills its height). */
export function isWide(width: number, height: number): boolean {
  if (width <= 0 || height <= 0) return true;
  return width / height >= 16 / 9;
}

/** One of the three generated logo styles, fixed per game. */
export type LogoStyle = 'a' | 'b' | 'c';

export function logoStyle(id: number): LogoStyle {
  return (['a', 'b', 'c'] as const)[wrapIndex(id, 3)];
}

/** `[h 0-360, s 0-100, l 0-100]`, rounded. */
export function rgbToHsl([r8, g8, b8]: readonly [number, number, number]): [number, number, number] {
  const r = r8 / 255;
  const g = g8 / 255;
  const b = b8 / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  let h = 0;
  let s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
  }
  return [Math.round(h), Math.round(s * 100), Math.round(l * 100)];
}

/** A generated logo's colours from the screenshot's dominant shade: the
 *  letters a light tint (88% lightness), the outline and shadow a deep shade
 *  (16%), the saturation capped so neither turns garish. */
export function logoColours(shade: readonly [number, number, number]): { letters: string; shadow: string } {
  const [h, s] = rgbToHsl(shade);
  return {
    letters: `hsl(${h} ${Math.min(s, 70)}% 88%)`,
    shadow: `hsl(${h} ${Math.min(s, 60)}% 16%)`,
  };
}
