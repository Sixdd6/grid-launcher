// Which badges a card shows (D-UI-9): installed dot top-right, UPDATE tag
// top-left, cloud icon bottom-right, platform chip bottom-left. Pure so the
// rules are tested once and both grids obey the same ones.

/** The UPDATE tag's text. Fixed here so the two grids cannot disagree. */
export const UPDATE_TAG_TEXT = 'UPDATE';

/** Beyond this many characters a platform name is initialised for the chip. */
const CHIP_MAX_CHARS = 12;

export type BadgeInput = {
  platform: string;
  installed: boolean;
  updateLabel: string | null;
  cloudPlatforms: ReadonlySet<string>;
};

export type CardBadges = {
  installed: boolean;
  update: boolean;
  cloud: boolean;
  platform: string;
};

const key = (value: string) => value.trim().toLowerCase();

export function cardBadges(input: BadgeInput): CardBadges {
  const platformKey = key(input.platform);
  return {
    installed: input.installed,
    // An update tag is a statement about the copy on disk, so it needs one.
    update: input.installed && input.updateLabel !== null,
    // `cloudPlatforms` is usually already normalized by `cloudPlatformSet`,
    // but callers (and tests) may pass a raw set, so the match is
    // case/space-insensitive on both sides rather than trusting the input.
    cloud: [...input.cloudPlatforms].some((platform) => key(platform) === platformKey),
    platform: shortPlatformName(input.platform),
  };
}

/**
 * The platforms whose cloud sync is configured, keyed for
 * [`cardBadges`]'s lookup.
 *
 * "Configured" is read as "the platform has a default emulator", from
 * `api.getLaunchDefaults().default_emulators`. That is the signal a whole
 * grid can afford: the exact per-game answer is `cloud_panel_info`, one IPC
 * round trip per game, which a 200-card grid cannot pay. It is a sound
 * approximation in the direction that matters — cloud sync resolves its
 * save paths through the platform's emulator entry, so no default emulator
 * means cloud sync is definitely not configured, and the badge is a
 * pointer to the Details cloud panel, which still gives the precise answer.
 */
export function cloudPlatformSet(defaultEmulators: Record<string, string>): Set<string> {
  const set = new Set<string>();
  for (const [platform, emulator] of Object.entries(defaultEmulators)) {
    if (emulator.trim() === '') continue;
    set.add(key(platform));
  }
  return set;
}

/**
 * The platform chip's text. Short names pass through; a long one is
 * initialised by keeping every uppercase letter and every whole digit run
 * ("Super Nintendo Entertainment System" → "SNES", "PlayStation 3" → "PS3").
 * A long name with nothing to initialise (all lowercase) is truncated with
 * an ellipsis instead, so the chip never renders a single stray letter.
 */
export function shortPlatformName(name: string): string {
  const trimmed = name.trim();
  if (trimmed.length <= CHIP_MAX_CHARS) return trimmed;

  let initials = '';
  for (let i = 0; i < trimmed.length; i += 1) {
    const ch = trimmed[i];
    if (ch >= '0' && ch <= '9') {
      // A digit run is one token: "360" must not become "3".
      while (i < trimmed.length && trimmed[i] >= '0' && trimmed[i] <= '9') {
        initials += trimmed[i];
        i += 1;
      }
      i -= 1;
    } else if (ch >= 'A' && ch <= 'Z') {
      initials += ch;
    }
  }
  if (initials.length >= 2) return initials;
  return `${trimmed.slice(0, CHIP_MAX_CHARS - 1).trimEnd()}…`;
}
