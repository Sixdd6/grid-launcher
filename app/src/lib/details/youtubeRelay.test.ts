import { describe, expect, it } from 'vitest';
import {
  acceptRelayMessage,
  embedPaused,
  embedShowing,
  embedStep,
  FALLBACK_ERRORS,
  initialEmbed,
  parseRelayEvent,
  READY_TIMEOUT_MS,
  relayMessage,
  relayOrigin,
  type EmbedInput,
  type EmbedState,
} from './youtubeRelay';

const ORIGIN = 'http://127.0.0.1:41234';
const frame = { name: 'relay window' };

describe('relayOrigin', () => {
  it('is the loopback origin of a relay URL', () => {
    expect(relayOrigin(`${ORIGIN}/abc/yt.html?v=dQw4w9WgXcQ&vol=50`)).toBe(ORIGIN);
  });

  it('refuses anything that is not http on 127.0.0.1 with a port', () => {
    expect(relayOrigin('https://127.0.0.1:41234/x')).toBeNull();
    expect(relayOrigin('http://localhost:41234/x')).toBeNull();
    expect(relayOrigin('http://127.0.0.1/x')).toBeNull();
    expect(relayOrigin('http://evil.example/x')).toBeNull();
    expect(relayOrigin('not a url')).toBeNull();
  });
});

describe('parseRelayEvent', () => {
  it('reads every relay message', () => {
    expect(parseRelayEvent({ source: 'grid-yt', type: 'boot' })).toEqual({ type: 'boot' });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'linked' })).toEqual({ type: 'linked' });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'ready' })).toEqual({ type: 'ready' });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'state', data: 1 })).toEqual({ type: 'state', data: 1 });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'volume', value: 37.4, muted: true })).toEqual({
      type: 'volume',
      value: 37,
      muted: true,
    });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'error', code: 150 })).toEqual({ type: 'error', code: 150 });
    expect(parseRelayEvent({ source: 'grid-yt', type: 'error', code: 'load' })).toEqual({ type: 'error', code: 'load' });
  });

  it('ignores messages that are not the relay protocol', () => {
    expect(parseRelayEvent(null)).toBeNull();
    expect(parseRelayEvent('ready')).toBeNull();
    expect(parseRelayEvent({ type: 'ready' })).toBeNull();
    expect(parseRelayEvent({ source: 'other', type: 'ready' })).toBeNull();
    expect(parseRelayEvent({ source: 'grid-yt', type: 'state', data: '1' })).toBeNull();
    expect(parseRelayEvent({ source: 'grid-yt', type: 'volume', value: Number.NaN })).toBeNull();
    expect(parseRelayEvent({ source: 'grid-yt', type: 'error', code: 'boom' })).toBeNull();
    expect(parseRelayEvent({ source: 'grid-yt', type: 'eval' })).toBeNull();
  });
});

describe('acceptRelayMessage', () => {
  const ready = { source: 'grid-yt', type: 'ready' };

  it('accepts the relay origin from the relay window', () => {
    expect(acceptRelayMessage({ origin: ORIGIN, source: frame, data: ready }, { origin: ORIGIN, source: frame })).toEqual({
      type: 'ready',
    });
  });

  it('refuses another origin, another window, or no relay yet', () => {
    expect(acceptRelayMessage({ origin: 'http://127.0.0.1:1', source: frame, data: ready }, { origin: ORIGIN, source: frame })).toBeNull();
    expect(acceptRelayMessage({ origin: 'tauri://localhost', source: frame, data: ready }, { origin: ORIGIN, source: frame })).toBeNull();
    expect(acceptRelayMessage({ origin: ORIGIN, source: {}, data: ready }, { origin: ORIGIN, source: frame })).toBeNull();
    expect(acceptRelayMessage({ origin: ORIGIN, source: null, data: ready }, { origin: ORIGIN, source: null })).toBeNull();
    expect(acceptRelayMessage({ origin: ORIGIN, source: frame, data: ready }, { origin: null, source: frame })).toBeNull();
  });
});

describe('relayMessage', () => {
  it('tags every command with the protocol source', () => {
    expect(relayMessage({ cmd: 'seek', by: -10 })).toEqual({ cmd: 'seek', by: -10, source: 'grid-yt' });
    expect(relayMessage({ cmd: 'volume', value: 60 })).toEqual({ cmd: 'volume', value: 60, source: 'grid-yt' });
    expect(relayMessage({ cmd: 'hello' })).toEqual({ cmd: 'hello', source: 'grid-yt' });
  });
});

function run(...inputs: EmbedInput[]): EmbedState {
  return inputs.reduce(embedStep, initialEmbed());
}

describe('embedStep', () => {
  it('waits about 10 s for the player', () => {
    expect(READY_TIMEOUT_MS).toBe(10_000);
  });

  it('goes resolving → loading → ready, and tracks the player state', () => {
    expect(initialEmbed().phase).toBe('resolving');
    // The player's frame is up from the start, before the relay URL lands.
    expect(embedShowing(initialEmbed())).toBe(true);
    expect(run({ type: 'url' }).phase).toBe('loading');
    const s = run({ type: 'url' }, { type: 'boot' }, { type: 'linked' }, { type: 'ready' }, { type: 'state', data: 1 });
    expect(s).toMatchObject({ phase: 'ready', linked: true, player: 1, reason: null });
    expect(embedShowing(s)).toBe(true);
    expect(embedPaused(s)).toBe(false);
    expect(embedPaused(embedStep(s, { type: 'state', data: 2 }))).toBe(true);
    expect(embedPaused(embedStep(s, { type: 'state', data: 3 }))).toBe(false);
  });

  it('falls back on every YouTube error, before or after ready', () => {
    for (const code of FALLBACK_ERRORS) {
      expect(run({ type: 'url' }, { type: 'error', code })).toMatchObject({ phase: 'fallback', reason: 'error', code });
      expect(run({ type: 'url' }, { type: 'ready' }, { type: 'error', code })).toMatchObject({ phase: 'fallback', code });
    }
    expect(FALLBACK_ERRORS).toEqual([2, 5, 100, 101, 150, 153]);
    expect(run({ type: 'url' }, { type: 'linked' }, { type: 'error', code: 'load' })).toMatchObject({
      phase: 'fallback',
      linked: true,
      code: 'load',
    });
  });

  it('falls back on the timeout only before ready', () => {
    expect(run({ type: 'timeout' })).toMatchObject({ phase: 'fallback', reason: 'timeout' });
    expect(run({ type: 'url' }, { type: 'timeout' })).toMatchObject({ phase: 'fallback', reason: 'timeout' });
    expect(run({ type: 'url' }, { type: 'ready' }, { type: 'timeout' }).phase).toBe('ready');
  });

  it('falls back when there is no relay URL', () => {
    expect(run({ type: 'urlError' })).toMatchObject({ phase: 'fallback', reason: 'url' });
  });

  it('stays in the fallback whatever comes after', () => {
    const s = run({ type: 'url' }, { type: 'error', code: 150 }, { type: 'ready' }, { type: 'state', data: 1 }, { type: 'timeout' });
    expect(s).toMatchObject({ phase: 'fallback', reason: 'error', code: 150, player: -1 });
    expect(embedShowing(s)).toBe(false);
  });

  it('ignores a ready that comes before the URL', () => {
    expect(run({ type: 'ready' }).phase).toBe('resolving');
  });
});
