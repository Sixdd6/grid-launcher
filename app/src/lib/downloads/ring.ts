// A fixed-size ring of transfer-rate samples (design §8: "a ring buffer per
// entry ... 60 one-second samples"). Pure data, no store imports, so the
// sampler and the sparkline can share it without an import cycle.

/** Samples per entry: one per second, one minute of history. */
export const SAMPLE_COUNT = 60;

/** One second of transfer: bytes per second over the network and to disk. */
export type Sample = { net: number; disk: number };

export type Ring = {
  readonly capacity: number;
  /** Backing storage; never longer than `capacity`. */
  buf: Sample[];
  /** Index the next push writes to. */
  head: number;
  /** How many slots hold a sample (≤ capacity). */
  count: number;
};

export function createRing(capacity: number = SAMPLE_COUNT): Ring {
  return { capacity, buf: [], head: 0, count: 0 };
}

/** Appends `sample`, overwriting the oldest one when the ring is full. */
export function pushSample(ring: Ring, sample: Sample): void {
  ring.buf[ring.head] = sample;
  ring.head = (ring.head + 1) % ring.capacity;
  ring.count = Math.min(ring.count + 1, ring.capacity);
}

/** The ring's samples, oldest first. */
export function samplesOf(ring: Ring): Sample[] {
  const out: Sample[] = [];
  const start = (ring.head - ring.count + ring.capacity) % ring.capacity;
  for (let i = 0; i < ring.count; i += 1) {
    out.push(ring.buf[(start + i) % ring.capacity]);
  }
  return out;
}
