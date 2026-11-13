// A store image's `src` (GOG covers, screenshots, an installer's cover).
// The app's CSP refuses web images, so a store's own address goes through
// `ensure_image`, whose store branch caches it under `<data>/stores/art`
// with no credential (never the RomM token) and answers the file. A data
// or blob URL is used as it is; a path GRID already cached goes through
// the asset protocol.
import { api } from '../api';
import { fileSrc } from '../fileSrc';

export type EnsureImage = (url: string) => Promise<string>;

/** The `src` for a store image, or `null` for none or a failed fetch. */
export async function storeArtSrc(url: string, ensure: EnsureImage = (u) => api.ensureImage(u)): Promise<string | null> {
  const u = url.trim();
  if (u === '') return null;
  if (/^(data:|blob:)/.test(u)) return u;
  if (/^(https?:)?\/\//.test(u)) {
    try {
      return fileSrc(await ensure(u));
    } catch {
      return null;
    }
  }
  return fileSrc(u);
}
