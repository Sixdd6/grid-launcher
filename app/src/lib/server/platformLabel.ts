// The label shown for a platform (rail entries, the Server header's active
// platform name). Pure — mirrors `grid_core::romm::Platform::label`
// (crates/grid-core/src/romm/mod.rs) and the Python app's
// `grid_launcher/server/catalog.py` / docs/porting/01-romm-api.md:147.

import type { Platform } from '../api';

/**
 * The label to display for a platform: `display_name`, else `name`, else
 * `slug`. `display_name` is RomM's own web UI label — the custom name when
 * one is set, otherwise the platform's `name` — but older servers omit the
 * field entirely, in which case it decodes as an empty string here.
 */
export function platformLabel(p: Pick<Platform, 'display_name' | 'name' | 'slug'>): string {
  if (p.display_name) return p.display_name;
  if (p.name) return p.name;
  return p.slug;
}
