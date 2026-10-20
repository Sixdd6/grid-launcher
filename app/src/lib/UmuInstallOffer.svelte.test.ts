// @vitest-environment node
//
// The idle offer under a Proton launch error: one enabled button and no
// "installing" note until the user asks (the busy rule itself is
// `umuInstallBusy`, tested in emulators/compatTools.test.ts).
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import UmuInstallOffer from './UmuInstallOffer.svelte';

describe('UmuInstallOffer', () => {
  it('renders an enabled Install umu-launcher button and no note', () => {
    const { body } = render(UmuInstallOffer);
    const button = body.match(/<button[^>]*data-testid="umu-install-offer-button"[^>]*>([^<]*)</);
    expect(button).not.toBeNull();
    expect(button![0]).not.toMatch(/disabled/);
    expect(button![1].trim()).toBe('Install umu-launcher');
    expect(body).not.toContain('umu-install-offer-note');
    expect(body).not.toContain('umu-install-offer-error');
  });
});
