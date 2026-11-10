import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DetailsSubject } from '../details/subject';
import { navigation, registerNavigation, type NavigationHandler } from './navigation.svelte';

function handler(): NavigationHandler & { calls: unknown[][] } {
  const calls: unknown[][] = [];
  return {
    calls,
    go: (view) => calls.push(['go', view]),
    goSettings: (page, opts) => calls.push(['settings', page, opts]),
  };
}

describe('navigation', () => {
  const offs: (() => void)[] = [];
  afterEach(() => {
    for (const off of offs.splice(0)) off();
  });

  it('does nothing while no shell is registered', () => {
    expect(() => {
      navigation.go('downloads');
      navigation.goSettings('systems', { system: 'SNES' });
      navigation.openDetails({} as DetailsSubject);
    }).not.toThrow();
  });

  it('sends each intent to the registered handler', () => {
    const h = handler();
    offs.push(registerNavigation(h));
    navigation.go('downloads');
    navigation.goSettings('systems', { system: 'SNES' });
    navigation.goSettings('updates');
    expect(h.calls).toEqual([
      ['go', 'downloads'],
      ['settings', 'systems', { system: 'SNES' }],
      ['settings', 'updates', {}],
    ]);
  });

  it('skips openDetails when the handler has none, and calls it when it has', () => {
    const h = handler();
    offs.push(registerNavigation(h));
    navigation.openDetails({} as DetailsSubject);
    expect(h.calls).toEqual([]);
    const open = vi.fn();
    offs.push(registerNavigation({ ...h, openDetails: open }));
    const subject = { name: 'Zelda' } as DetailsSubject;
    navigation.openDetails(subject);
    expect(open).toHaveBeenCalledWith(subject);
  });

  it('goes to the newest handler, and an old unregister leaves it in place', () => {
    const first = handler();
    const second = handler();
    const offFirst = registerNavigation(first);
    offs.push(registerNavigation(second));
    offFirst();
    navigation.go('library');
    expect(first.calls).toEqual([]);
    expect(second.calls).toEqual([['go', 'library']]);
  });

  it('stops after its own unregister', () => {
    const h = handler();
    const off = registerNavigation(h);
    off();
    navigation.go('server');
    expect(h.calls).toEqual([]);
  });
});
