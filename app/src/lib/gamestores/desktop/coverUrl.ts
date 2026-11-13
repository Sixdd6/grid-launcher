// A store cover's `src`: a data or web URL as it is, a cached file through
// the asset protocol (or the dev server's cover route), `null` for none.
import { fileSrc } from '../../fileSrc';

export function storeCoverUrl(cover: string): string | null {
  const c = cover.trim();
  if (c === '') return null;
  if (/^(data:|https?:|blob:)/.test(c)) return c;
  return fileSrc(c);
}
