// The top bar's clock (checklist item 126): HH:MM, current to the minute.
// One timer while a TV shell is mounted, re-armed at each minute boundary.

export function clockText(d: Date): string {
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

const state = $state({ now: clockText(new Date()) });

export const clock = {
  get text(): string {
    return state.now;
  },
};

/** Starts the clock; answers the function that stops it. */
export function startClock(): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const tick = () => {
    const d = new Date();
    state.now = clockText(d);
    timer = setTimeout(tick, 60_000 - (d.getSeconds() * 1000 + d.getMilliseconds()) + 20);
  };
  tick();
  return () => clearTimeout(timer);
}
