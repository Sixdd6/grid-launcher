// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import type { CloudPanelInfo, GameSummary, SaveType } from '../api';
import { syntheticCloudGame } from './cloud';

// Server render never runs `$effect`, so the tab is exercised in its
// pre-fetch state; the API is stubbed only so the imports resolve.
vi.mock('../api', () => ({ api: { cloudRecords: vi.fn(), nativeSavePaths: vi.fn() } }));

import SavesTab from './SavesTab.svelte';

const game: GameSummary = { id: 1, name: 'Metroid' } as GameSummary;
const supported: CloudPanelInfo = { supported: true, block_reason: '', scope: 'per_game' };
const shared: CloudPanelInfo = { supported: true, block_reason: '', scope: 'shared_single' };
const blocked: CloudPanelInfo = { supported: false, block_reason: 'No emulator.', scope: 'per_game' };

function html(kind: SaveType, savePanelInfo: CloudPanelInfo | null, statePanelInfo: CloudPanelInfo | null) {
  return render(SavesTab, {
    props: {
      gameTitle: 'Metroid',
      cloudGame: syntheticCloudGame(game, 'NES', ''),
      isNative: false,
      savePanelInfo,
      statePanelInfo,
      kind,
      infoError: null,
      onKind: () => {},
    },
  }).body;
}

describe('SavesTab', () => {
  it('shows the saves list straight away, with no toggle to open it', () => {
    const body = html('save', supported, blocked);
    expect(body).toContain('cloud-panel');
    expect(body).toContain('Cloud Saves');
    expect(body).toContain('Upload Latest Save');
    expect(body).not.toContain('details-cloud-kind-save');
  });

  it('offers a Saves | States switch when both are supported, on the selected kind', () => {
    const body = html('state', supported, supported);
    expect(body).toContain('details-cloud-kind-save');
    expect(body).toContain('details-cloud-kind-state');
    expect(body).toContain('Cloud States');
    expect(body).toMatch(/data-testid="details-cloud-kind-state"[^>]*aria-selected="true"/);
  });

  it('shows the supported kind when the selected one is not', () => {
    const body = html('state', supported, blocked);
    expect(body).toContain('Cloud Saves');
  });

  it('warns above shared emulator media', () => {
    const body = html('save', shared, blocked);
    expect(body).toContain('Emulator Saves');
    expect(body).toContain('cloud-shared-note');
  });

  it('says why when nothing is supported', () => {
    const body = html('save', blocked, blocked);
    expect(body).not.toContain('cloud-panel');
    expect(body).toContain('details-cloud-unsupported');
    expect(body).toContain('No emulator.');
  });
});
