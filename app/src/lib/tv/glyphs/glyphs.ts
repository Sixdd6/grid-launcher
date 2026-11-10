// TV mode's button prompts: Kenney's Input Prompts 1.5A (CC0 1.0, see
// License.txt beside this file), as decided in the TVMode README § Decided
// "Glyph art". Which file each prompt set draws for each button is
// `map.json` (copied from docs/design/prototype/glyphs/); GLYPH_FILES below
// is the same table, and glyphs.test.ts keeps the two equal. No brand logo
// is used and the Home / Guide button has no glyph (ruling 5: it does
// nothing). Pure: Glyph.svelte draws one as a CSS mask in the text colour.

/** A prompt set (Settings › TV mode › Button prompts, minus "Match"). */
export type PromptSet = 'xbox' | 'ps' | 'nintendo' | '8bitdo';

/** Every button a prompt can show. RS is the right stick (Scroll), DPAD the
 *  D-pad (Adjust, Read), LS the left stick press. */
export type GlyphButton = 'A' | 'B' | 'X' | 'Y' | 'LB' | 'RB' | 'LT' | 'RT' | 'VIEW' | 'MENU' | 'RS' | 'LS' | 'DPAD';

export const GLYPH_BUTTONS: readonly GlyphButton[] = [
  'A', 'B', 'X', 'Y', 'LB', 'RB', 'LT', 'RT', 'VIEW', 'MENU', 'RS', 'LS', 'DPAD',
];

export const PROMPT_SETS: readonly PromptSet[] = ['xbox', 'ps', 'nintendo', '8bitdo'];

export const GLYPH_FILES: Record<PromptSet, Record<GlyphButton, string>> = {
  xbox: {
    A: 'xbox_button_a.svg',
    B: 'xbox_button_b.svg',
    X: 'xbox_button_x.svg',
    Y: 'xbox_button_y.svg',
    LB: 'xbox_lb.svg',
    RB: 'xbox_rb.svg',
    LT: 'xbox_lt.svg',
    RT: 'xbox_rt.svg',
    VIEW: 'xbox_button_view.svg',
    MENU: 'xbox_button_menu.svg',
    RS: 'xbox_stick_r_vertical.svg',
    LS: 'xbox_ls.svg',
    DPAD: 'xbox_dpad.svg',
  },
  ps: {
    A: 'playstation_button_cross.svg',
    B: 'playstation_button_circle.svg',
    X: 'playstation_button_square.svg',
    Y: 'playstation_button_triangle.svg',
    LB: 'playstation_trigger_l1.svg',
    RB: 'playstation_trigger_r1.svg',
    LT: 'playstation_trigger_l2.svg',
    RT: 'playstation_trigger_r2.svg',
    VIEW: 'playstation5_button_create.svg',
    MENU: 'playstation5_button_options.svg',
    RS: 'playstation_stick_r_vertical.svg',
    LS: 'playstation_button_l3.svg',
    DPAD: 'playstation_dpad.svg',
  },
  nintendo: {
    A: 'switch_button_a.svg',
    B: 'switch_button_b.svg',
    X: 'switch_button_x.svg',
    Y: 'switch_button_y.svg',
    LB: 'switch_button_l.svg',
    RB: 'switch_button_r.svg',
    LT: 'switch_button_zl.svg',
    RT: 'switch_button_zr.svg',
    VIEW: 'switch_button_minus.svg',
    MENU: 'switch_button_plus.svg',
    RS: 'switch_stick_r_vertical.svg',
    LS: 'switch_stick_l_press.svg',
    DPAD: 'switch_dpad.svg',
  },
  '8bitdo': {
    A: 'switch_button_a.svg',
    B: 'switch_button_b.svg',
    X: 'switch_button_x.svg',
    Y: 'switch_button_y.svg',
    LB: 'switch_button_l.svg',
    RB: 'switch_button_r.svg',
    LT: 'playstation_trigger_l2_alternative.svg',
    RT: 'playstation_trigger_r2_alternative.svg',
    VIEW: 'switch_button_minus.svg',
    MENU: 'switch_button_plus.svg',
    RS: 'switch_stick_r_vertical.svg',
    LS: 'switch_stick_l_press.svg',
    DPAD: 'switch_dpad.svg',
  },
};

/** Each glyph's accessible name: the button as that controller prints it.
 *  The 8BitDo set reads View / Menu as "Minus" / "Plus" (ruling 4). */
export const GLYPH_LABELS: Record<PromptSet, Record<GlyphButton, string>> = {
  xbox: {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'LB', RB: 'RB', LT: 'LT', RT: 'RT',
    VIEW: 'View', MENU: 'Menu', RS: 'Right stick', LS: 'Left stick', DPAD: 'D-pad',
  },
  ps: {
    A: 'Cross', B: 'Circle', X: 'Square', Y: 'Triangle', LB: 'L1', RB: 'R1', LT: 'L2', RT: 'R2',
    VIEW: 'Create', MENU: 'Options', RS: 'Right stick', LS: 'L3', DPAD: 'D-pad',
  },
  nintendo: {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'L', RB: 'R', LT: 'ZL', RT: 'ZR',
    VIEW: 'Minus', MENU: 'Plus', RS: 'Right stick', LS: 'Left stick', DPAD: 'D-pad',
  },
  '8bitdo': {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'L', RB: 'R', LT: 'L2', RT: 'R2',
    VIEW: 'Minus', MENU: 'Plus', RS: 'Right stick', LS: 'Left stick', DPAD: 'D-pad',
  },
};

// The bundled URL of every SVG in this folder, keyed by file name. Emitted
// as files, never inlined as `data:` URLs: the app's CSP (`img-src`, which
// covers CSS masks) allows no `data:` images, and an inlined glyph's mask
// never painted.
const URLS: Record<string, string> = Object.fromEntries(
  Object.entries(
    import.meta.glob('./*.svg', { eager: true, query: '?no-inline', import: 'default' }) as Record<string, string>,
  ).map(([path, url]) => [path.replace(/^\.\//, ''), url]),
);

/** The glyph's URL, and its accessible name. */
export function glyph(set: PromptSet, button: GlyphButton): { url: string; label: string } {
  return { url: URLS[GLYPH_FILES[set][button]] ?? '', label: GLYPH_LABELS[set][button] };
}

/** The file names this module can serve (for the test). */
export function bundledFiles(): string[] {
  return Object.keys(URLS);
}
