// TV mode's button prompts: Kenney's Input Prompts 1.5A (CC0 1.0, see
// License.txt beside this file), as decided in the TVMode README § Decided
// "Glyph art". Which file each prompt set draws for each button is
// `map.json` (copied from docs/design/prototype/glyphs/); GLYPH_FILES below
// is the same table, and glyphs.test.ts keeps the two equal. No brand logo
// is used and the Home / Guide button has no glyph (ruling 5: it does
// nothing). Pure: Glyph.svelte draws one as a CSS mask in the text colour.

/** A prompt set: one of Settings › TV mode › Button prompts' (minus
 *  "Match"), or the keyboard's keys while it leads (README § Decided,
 *  Keyboard prompts). */
export type PromptSet = 'xbox' | 'ps' | 'nintendo' | '8bitdo' | 'keyboard';

/** Every button a prompt can show. RS is the right stick (Scroll), DPAD the
 *  D-pad (Adjust, Read), LS the left stick press, R3 the right stick press
 *  (the on-screen keyboard's Paste; Ctrl+V on the keyboard). */
export type GlyphButton = 'A' | 'B' | 'X' | 'Y' | 'LB' | 'RB' | 'LT' | 'RT' | 'VIEW' | 'MENU' | 'RS' | 'LS' | 'R3' | 'DPAD';

export const GLYPH_BUTTONS: readonly GlyphButton[] = [
  'A', 'B', 'X', 'Y', 'LB', 'RB', 'LT', 'RT', 'VIEW', 'MENU', 'RS', 'LS', 'R3', 'DPAD',
];

export const PROMPT_SETS: readonly PromptSet[] = ['xbox', 'ps', 'nintendo', '8bitdo', 'keyboard'];

/** A glyph's file, or several drawn side by side (the keyboard's Page Up and
 *  Page Down for the right stick, Ctrl and V for R3). */
export type GlyphFile = string | readonly string[];

export const GLYPH_FILES: Record<PromptSet, Record<GlyphButton, GlyphFile>> = {
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
    R3: 'xbox_rs.svg',
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
    R3: 'playstation_button_r3.svg',
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
    R3: 'switch_stick_r_press.svg',
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
    R3: 'switch_stick_r_press.svg',
    DPAD: 'switch_dpad.svg',
  },
  keyboard: {
    A: 'keyboard_enter.svg',
    B: 'keyboard_escape.svg',
    X: 'keyboard_x.svg',
    Y: 'keyboard_y.svg',
    LB: 'keyboard_q.svg',
    RB: 'keyboard_e.svg',
    LT: 'keyboard_z.svg',
    RT: 'keyboard_r.svg',
    VIEW: 'keyboard_v.svg',
    MENU: 'keyboard_m.svg',
    RS: ['keyboard_page_up.svg', 'keyboard_page_down.svg'],
    LS: 'keyboard_arrows_all.svg',
    R3: ['keyboard_ctrl.svg', 'keyboard_v.svg'],
    DPAD: 'keyboard_arrows_all.svg',
  },
};

/** Each glyph's accessible name: the button as that controller prints it.
 *  The 8BitDo set reads View / Menu as "Minus" / "Plus" (ruling 4). */
export const GLYPH_LABELS: Record<PromptSet, Record<GlyphButton, string>> = {
  xbox: {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'LB', RB: 'RB', LT: 'LT', RT: 'RT',
    VIEW: 'View', MENU: 'Menu', RS: 'Right stick', LS: 'Left stick', R3: 'RS', DPAD: 'D-pad',
  },
  ps: {
    A: 'Cross', B: 'Circle', X: 'Square', Y: 'Triangle', LB: 'L1', RB: 'R1', LT: 'L2', RT: 'R2',
    VIEW: 'Create', MENU: 'Options', RS: 'Right stick', LS: 'L3', R3: 'R3', DPAD: 'D-pad',
  },
  nintendo: {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'L', RB: 'R', LT: 'ZL', RT: 'ZR',
    VIEW: 'Minus', MENU: 'Plus', RS: 'Right stick', LS: 'Left stick', R3: 'Right stick press', DPAD: 'D-pad',
  },
  '8bitdo': {
    A: 'A', B: 'B', X: 'X', Y: 'Y', LB: 'L', RB: 'R', LT: 'L2', RT: 'R2',
    VIEW: 'Minus', MENU: 'Plus', RS: 'Right stick', LS: 'Left stick', R3: 'Right stick press', DPAD: 'D-pad',
  },
  keyboard: {
    A: 'Enter', B: 'Esc', X: 'X', Y: 'Y', LB: 'Q', RB: 'E', LT: 'Z', RT: 'R',
    VIEW: 'V', MENU: 'M', RS: 'Page Up / Page Down', LS: 'Arrow keys', R3: 'Ctrl+V', DPAD: 'Arrow keys',
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

/** The glyph's URLs (usually one), and its accessible name. */
export function glyph(set: PromptSet, button: GlyphButton): { urls: string[]; label: string } {
  const f = GLYPH_FILES[set][button];
  const files: readonly string[] = typeof f === 'string' ? [f] : f;
  return { urls: files.map((x) => URLS[x] ?? ''), label: GLYPH_LABELS[set][button] };
}

/** The file names this module can serve (for the test). */
export function bundledFiles(): string[] {
  return Object.keys(URLS);
}
