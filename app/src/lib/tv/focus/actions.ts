// Svelte actions for TV markup. `use:tvf` makes an element focusable
// (`data-f`) and gives the shell what pressing a button on it does. Every
// press reaches an item through here, whatever the input: a controller or
// the keyboard (TV mode ignores the mouse), so a TV item never has its own
// `onclick`.

export type ItemHandlers = {
  /** A. */
  a?: () => void;
  /** X: the item's quick or extra action (its `data-xl` names it). */
  x?: () => void;
  /** Left / Right on a slider row (`data-adj`). */
  adjust?: (delta: -1 | 1) => void;
  /** The item took focus (a rail row selects its pane; a cover shows its
   *  art). */
  focus?: () => void;
};

const handlers = new WeakMap<Element, ItemHandlers>();

export function handlersOf(el: Element | null): ItemHandlers | undefined {
  return el ? handlers.get(el) : undefined;
}

export function tvf(node: HTMLElement, h: ItemHandlers = {}) {
  node.setAttribute('data-f', '');
  handlers.set(node, h);
  return {
    update(next: ItemHandlers = {}) {
      handlers.set(node, next);
    },
    destroy() {
      handlers.delete(node);
    },
  };
}
