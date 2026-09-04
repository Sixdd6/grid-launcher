import { describe, expect, it } from 'vitest';
import { createRing, pushSample, SAMPLE_COUNT, samplesOf } from './ring';

describe('ring', () => {
  it('holds 60 samples by default (design §8: 60 one-second samples)', () => {
    expect(SAMPLE_COUNT).toBe(60);
    expect(createRing().capacity).toBe(60);
  });

  it('reads back an empty ring as an empty list', () => {
    expect(samplesOf(createRing(3))).toEqual([]);
  });

  it('returns samples oldest first while under capacity', () => {
    const ring = createRing(3);
    pushSample(ring, { net: 1, disk: 0 });
    pushSample(ring, { net: 2, disk: 0 });
    expect(samplesOf(ring)).toEqual([
      { net: 1, disk: 0 },
      { net: 2, disk: 0 },
    ]);
  });

  it('drops the oldest sample once full and keeps reading oldest first', () => {
    const ring = createRing(3);
    for (let i = 1; i <= 5; i += 1) pushSample(ring, { net: i, disk: i * 10 });
    expect(samplesOf(ring)).toEqual([
      { net: 3, disk: 30 },
      { net: 4, disk: 40 },
      { net: 5, disk: 50 },
    ]);
    expect(ring.count).toBe(3);
  });

  it('never grows the backing array past its capacity', () => {
    const ring = createRing(4);
    for (let i = 0; i < 100; i += 1) pushSample(ring, { net: i, disk: 0 });
    expect(ring.buf.length).toBe(4);
    expect(samplesOf(ring).map((s) => s.net)).toEqual([96, 97, 98, 99]);
  });
});
