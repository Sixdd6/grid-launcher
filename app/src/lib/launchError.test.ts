import { describe, expect, it } from 'vitest';
import { isUmuMissing, LaunchFailure, toLaunchFailure, UMU_MISSING } from './launchError';

describe('toLaunchFailure', () => {
  it('reads the structured launch_game error', () => {
    const failure = toLaunchFailure({ code: 'umu_missing', message: 'umu-run is not installed.' });
    expect(failure).toBeInstanceOf(LaunchFailure);
    expect(failure).toBeInstanceOf(Error);
    expect(failure.message).toBe('umu-run is not installed.');
    expect(failure.code).toBe(UMU_MISSING);
  });

  it('gives a blank code to a plain string or an Error', () => {
    expect(toLaunchFailure('The library is being moved.')).toMatchObject({
      message: 'The library is being moved.',
      code: '',
    });
    expect(toLaunchFailure(new Error('boom'))).toMatchObject({ message: 'boom', code: '' });
  });

  it('ignores a non-string code and stringifies anything else', () => {
    expect(toLaunchFailure({ code: 7, message: 'odd' })).toMatchObject({ message: 'odd', code: '' });
    expect(toLaunchFailure(null).message).toBe('null');
    expect(toLaunchFailure({ nope: true }).message).toBe('[object Object]');
  });

  it('returns a LaunchFailure unchanged', () => {
    const failure = new LaunchFailure('x', 'umu_missing');
    expect(toLaunchFailure(failure)).toBe(failure);
  });
});

describe('isUmuMissing', () => {
  it('is true only for the umu_missing code', () => {
    expect(isUmuMissing(new LaunchFailure('x', 'umu_missing'))).toBe(true);
    expect(isUmuMissing({ code: 'umu_missing', message: 'x' })).toBe(true);
    expect(isUmuMissing(new LaunchFailure('x'))).toBe(false);
    expect(isUmuMissing('umu_missing')).toBe(false);
    expect(isUmuMissing(new Error('umu_missing'))).toBe(false);
  });
});
