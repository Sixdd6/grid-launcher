// Controllers: brand, default button assignment, display name, and the
// positional → logical mapping (TVMode README § Decided "Button
// assignments"; checklist items 38, 49–56, 67–68). Pure. The Rust reader
// sends face buttons by POSITION (south, east, west, north); everything that
// depends on the assignment happens here.
import type { PadButton, PadInfo, TvAssignment } from '../../api';
import type { TvButton } from './buttons';

/** Whose layout a controller has, for prompts. */
export type PadBrand = 'xbox' | 'ps' | 'nintendo' | '8bitdo';

const SONY = 0x054c;
const NINTENDO = 0x057e;
const EIGHTBITDO = 0x2dc8;

type PadId = Pick<PadInfo, 'name' | 'vendor'>;

/**
 * The brand, from the USB vendor id or the name: Sony 054c, Nintendo 057e,
 * 8BitDo 2dc8. Everything else (Microsoft 045e, and 8BitDo pads in X-input
 * mode, which report as Microsoft) is Xbox.
 */
export function padBrand(pad: PadId): PadBrand {
  if (pad.vendor === SONY || /playstation|dualsense|dualshock/i.test(pad.name)) return 'ps';
  if (pad.vendor === NINTENDO || /nintendo|joy-con|switch/i.test(pad.name)) return 'nintendo';
  if (pad.vendor === EIGHTBITDO || /8bitdo/i.test(pad.name)) return '8bitdo';
  return 'xbox';
}

/**
 * The assignment a controller starts on (items 51–52): Nintendo style for a
 * Nintendo controller, and for an 8BitDo whose name (outside parentheses)
 * holds Pro, SN30, Lite, Zero or Micro; Xbox style for everything else.
 */
export function guessAssign(pad: PadId): TvAssignment {
  const model = pad.name.replace(/\(.*?\)/g, '');
  if (pad.vendor === NINTENDO || /nintendo|joy-con|switch/i.test(pad.name)) return 'nintendo';
  if ((pad.vendor === EIGHTBITDO || /8bitdo/i.test(pad.name)) && /\b(pro|sn30|lite|zero|micro)\b/i.test(model)) {
    return 'nintendo';
  }
  return 'xbox';
}

/** The stored choice for this controller, else the one it starts on (item 53). */
export function assignmentFor(pad: Pick<PadInfo, 'key' | 'name' | 'vendor'>, stored: Readonly<Record<string, TvAssignment>>): TvAssignment {
  return stored[pad.key] ?? guessAssign(pad);
}

/** The name the hint bar shows (item 38): the device name without any
 *  parenthesised part, or "Controller" when that leaves nothing. */
export function padDisplayName(name: string): string {
  return name.replace(/\(.*?\)/g, '').trim() || 'Controller';
}

const FIXED: Partial<Record<PadButton, TvButton>> = {
  left_bumper: 'LB',
  right_bumper: 'RB',
  left_trigger: 'LT',
  right_trigger: 'RT',
  select: 'VIEW',
  start: 'MENU',
  dpad_up: 'UP',
  dpad_down: 'DOWN',
  dpad_left: 'LEFT',
  dpad_right: 'RIGHT',
};

const FACES: Record<TvAssignment, Partial<Record<PadButton, TvButton>>> = {
  // Bottom selects, right goes back, left is X, top is Y (item 49).
  xbox: { south: 'A', east: 'B', west: 'X', north: 'Y' },
  // Right selects, bottom goes back, top is X, left is Y (item 50).
  nintendo: { east: 'A', south: 'B', north: 'X', west: 'Y' },
};

/**
 * What a physical button means under an assignment. Only the four face
 * buttons move (item 55). Guide and the stick presses are bound to nothing
 * (ruling 5; the design gives L3 / R3 no role).
 */
export function logicalButton(button: PadButton, assign: TvAssignment): TvButton | null {
  return FACES[assign][button] ?? FIXED[button] ?? null;
}
