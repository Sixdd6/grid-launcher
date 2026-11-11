// Settings › Systems: RPCS3's PS3 firmware line. RPCS3 installs firmware by
// extracting PS3UPDAT.PUP into its dev_flash, so "installed" is RPCS3's own
// state (`rpcs3_firmware_status().installed`), not merely a downloaded PUP.
// Pure.
import type { Rpcs3FirmwareStatus } from '../api';
import type { FirmwareAction } from '../emulators/firmware';

/** What the PS3 firmware line shows: one installed line, the Install
 *  prompt (a PUP is downloaded but RPCS3 has not installed it), or nothing. */
export type Ps3FirmwareView = { kind: 'installed'; text: string } | { kind: 'install'; text: string } | null;

/** Whether an emulator entry is RPCS3 (the frontend's name check). */
export function isRpcs3Name(name: string): boolean {
  return name.toLowerCase().includes('rpcs3');
}

export function ps3FirmwareInstalled(status: Rpcs3FirmwareStatus | undefined): boolean {
  return status?.installed === true;
}

export function ps3FirmwareView(status: Rpcs3FirmwareStatus | undefined): Ps3FirmwareView {
  if (!status) return null;
  if (status.installed) {
    const version = status.version?.trim();
    return { kind: 'installed', text: version ? `PS3 firmware ${version} installed` : 'PS3 firmware installed' };
  }
  if (status.pup_path) return { kind: 'install', text: 'PS3 firmware downloaded. Install it to use it.' };
  return null;
}

/**
 * A system row's server-firmware action once RPCS3's own state is known:
 * firmware RPCS3 has installed is installed, whatever the server-file check
 * said (it may predate `--installfw` finishing, or the PUP may be gone).
 * A request this page started still reads as one.
 */
export function withPs3Firmware(action: FirmwareAction | null, ps3Installed: boolean): FirmwareAction | null {
  if (!ps3Installed || action === null || action === 'requested') return action;
  return 'installed';
}

/**
 * Re-checks after `rpcs3 --installfw` starts: RPCS3 runs its own dialog,
 * so GRID polls until `check` reports the firmware installed, `stopped`
 * says the page went away, or `attempts` run out. Resolves whether it saw
 * the install land.
 */
export async function pollUntilInstalled(
  check: () => Promise<boolean>,
  opts: { intervalMs: number; attempts: number; stopped?: () => boolean; sleep?: (ms: number) => Promise<void> },
): Promise<boolean> {
  const sleep = opts.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
  for (let i = 0; i < opts.attempts; i++) {
    await sleep(opts.intervalMs);
    if (opts.stopped?.()) return false;
    try {
      if (await check()) return true;
    } catch {
      // A failed read is not an answer; keep polling.
    }
  }
  return false;
}
