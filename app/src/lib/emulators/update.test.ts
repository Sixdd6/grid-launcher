import { describe, expect, it } from 'vitest';
import { installedToastText } from './update';

describe('installedToastText', () => {
  it('says "Installed" for a first install and "Updated" for an update', () => {
    expect(installedToastText('PCSX2', true)).toBe("Installed emulator 'PCSX2' from source.");
    expect(installedToastText('PCSX2', false)).toBe("Updated emulator 'PCSX2' from source.");
  });
});
