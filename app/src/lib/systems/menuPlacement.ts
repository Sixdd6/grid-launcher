// Where an emulator's ⋯ menu opens (ProposedApp README, "Opening a row";
// prototype `syPlaceMenu`). Pure: the component measures, this decides.
//
// Below the button, right-aligned with it, when it fits. Near the bottom of
// the window it opens beside the button instead — left of it when there is
// room, else right — shifted up so it stays EDGE px inside the window. It
// never covers its button and never opens upward over it. "Window" is the
// app window; a fixed menu that would drift from its button closes when the
// list scrolls (the component's half).

export type Rect = { top: number; left: number; right: number; bottom: number };
export type Size = { width: number; height: number };
export type MenuSide = 'below' | 'left' | 'right';
export type MenuPlacement = { top: number; left: number; side: MenuSide };

/** How far inside the window the menu stays. */
export const MENU_EDGE = 8;
/** The gap between the button and the menu. */
export const MENU_GAP = 4;

export function placeMenu(button: Rect, menu: Size, win: Rect, edge = MENU_EDGE, gap = MENU_GAP): MenuPlacement {
  const minX = win.left + edge;
  const maxX = win.right - edge;
  const minY = win.top + edge;
  const maxY = win.bottom - edge;
  if (button.bottom + gap + menu.height <= maxY) {
    return {
      top: button.bottom + gap,
      left: Math.max(minX, Math.min(button.right - menu.width, maxX - menu.width)),
      side: 'below',
    };
  }
  // Beside the button: level with its top, shifted up just enough to fit,
  // never above the window's top edge.
  const top = Math.max(minY, Math.min(button.top, maxY - menu.height));
  if (button.left - gap - menu.width >= minX) {
    return { top, left: button.left - gap - menu.width, side: 'left' };
  }
  return { top, left: Math.min(button.right + gap, maxX - menu.width), side: 'right' };
}
