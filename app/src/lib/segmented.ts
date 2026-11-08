// The segmented switch's keyboard rule (WAI-ARIA radio group): the arrow keys
// move the choice to the next or previous option, wrapping at the ends, and
// Home / End jump to the first and last. Pure, so it is tested without
// mounting `SegmentedSwitch.svelte`.

/** The option index a key moves to from `index`, or `null` for a key the
 *  switch does not handle (Tab, Space, letters…). */
export function segmentKeyTarget(key: string, index: number, count: number): number | null {
  if (count <= 0) return null;
  const from = index < 0 || index >= count ? 0 : index;
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return (from + 1) % count;
    case 'ArrowLeft':
    case 'ArrowUp':
      return (from - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}

/** The option that takes Tab focus: the chosen one, else the first, so the
 *  group stays reachable when the value matches no option. */
export function segmentTabIndex(values: readonly string[], value: string): number {
  const i = values.indexOf(value);
  return i < 0 ? 0 : i;
}
