import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://localhost/${encodeURIComponent(path)}`,
}));

import { DEV_COVERS_ROUTE, fileSrcFor } from './fileSrc';

describe('fileSrcFor', () => {
  it('uses the asset protocol outside the dev server', () => {
    expect(fileSrcFor('/home/u/.cache/grid-launcher/covers/abc.webp', false)).toBe(
      'asset://localhost/%2Fhome%2Fu%2F.cache%2Fgrid-launcher%2Fcovers%2Fabc.webp',
    );
  });

  it('serves the file by name from the dev route on the dev server', () => {
    expect(fileSrcFor('/home/u/.cache/grid-launcher/covers/abc.bg0.jpg', true)).toBe(
      `${DEV_COVERS_ROUTE}abc.bg0.jpg`,
    );
  });

  it('takes the name after a Windows separator too', () => {
    expect(fileSrcFor('C:\\Users\\u\\cache\\covers\\abc.png', true)).toBe(
      `${DEV_COVERS_ROUTE}abc.png`,
    );
  });

  it('falls back to the asset protocol when the path has no file name', () => {
    expect(fileSrcFor('/covers/', true)).toBe('asset://localhost/%2Fcovers%2F');
  });
});
