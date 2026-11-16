/**
 * Legacy names for the RomM screens that replaced the Server view
 * (romm-storefront-plan Step 3): a platform and Discover now live in the
 * Library's RomM group. New code imports from `romm.ts`.
 */
import { openRommDiscover, openRommPlatform, platformSlug } from './romm.js';

/**
 * Opens the mock RomM platform `id` (its old `platform-btn-<id>`) as
 * Library › RomM › <platform>, and waits for its header.
 */
export async function openPlatform(id: number | string): Promise<void> {
  await openRommPlatform(typeof id === 'number' || /^\d+$/.test(id) ? await platformSlug(Number(id)) : id);
}

/** Selects Library › RomM › Discover and waits for it. */
export async function openDiscover(): Promise<void> {
  await openRommDiscover();
}
