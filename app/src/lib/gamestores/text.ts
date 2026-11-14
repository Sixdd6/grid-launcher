// Small wording helpers every store screen shares: sizes as the GameStores
// prototype writes them, plurals and "5 min ago". Pure.

const GIB = 1024 ** 3;

/**
 * A size in the prototype's form (`bkFmt`): "412 GB" from 100 GB up,
 * "22.0 GB" from 1 GB, "118 MB" below, "<1 MB" for a crumb.
 */
export function sizeGb(bytes: number): string {
  const gb = Math.max(0, bytes) / GIB;
  if (gb >= 100) return `${Math.round(gb)} GB`;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  if (gb >= 0.0004) return `${Math.max(1, Math.round(gb * 1000))} MB`;
  return '<1 MB';
}

/**
 * A store tool's download size, in decimal megabytes as its release page
 * gives it: nile's 11113264-byte asset is "11 MB", legendary's "3 MB",
 * gogdl's "1 MB" (plan, package C: the pin's real size).
 */
export function toolSize(bytes: number): string {
  return `${Math.max(1, Math.round(Math.max(0, bytes) / 1_000_000))} MB`;
}

/** "1 game", "3 games"; `many` defaults to `one` + "s". */
export function plural(n: number, one: string, many: string = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/**
 * How long ago `at` (unix seconds) was, from `now` (unix seconds): "just now",
 * "5 min ago", "2 hours ago", "yesterday", "3 days ago". `0` is "never".
 */
export function agoText(at: number, now: number): string {
  if (at <= 0) return 'never';
  const s = Math.max(0, now - at);
  if (s < 60) return 'just now';
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return h === 1 ? '1 hour ago' : `${h} hours ago`;
  const d = Math.floor(h / 24);
  if (d === 1) return 'yesterday';
  if (d < 7) return `${d} days ago`;
  const w = Math.floor(d / 7);
  if (d < 30) return w === 1 ? '1 week ago' : `${w} weeks ago`;
  const mo = Math.floor(d / 30);
  return mo <= 1 ? 'last month' : `${mo} months ago`;
}

/**
 * "today, 21:40", "yesterday, 19:12" or "12 Nov, 08:05" for a save's change
 * time (the cloud conflict's two sides), in the local time zone.
 */
export function changedAt(at: number, now: number): string {
  const d = new Date(at * 1000);
  const n = new Date(now * 1000);
  const hm = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(n) - day(d)) / 86_400_000);
  if (diff === 0) return `today, ${hm}`;
  if (diff === 1) return `yesterday, ${hm}`;
  const month = d.toLocaleString('en-GB', { month: 'short' });
  return `${d.getDate()} ${month}, ${hm}`;
}

/** The text before the first " — " separator a DLC title repeats its game with. */
export function dlcShortTitle(dlcTitle: string, gameTitle: string): string {
  const prefix = `${gameTitle} — `;
  return dlcTitle.startsWith(prefix) ? dlcTitle.slice(prefix.length) : dlcTitle;
}
