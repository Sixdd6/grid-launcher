// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { render } from 'svelte/server';
import type { CloudPanelInfo, GameSummary } from '../api';
import { syntheticCloudGame } from './cloud';
import type { CloudMode } from './cloud';

// Server render never runs `$effect`, so the tab is exercised in its
// pre-fetch state; the API is stubbed only so the import resolves.
vi.mock('../api', () => ({ api: { cloudRecords: vi.fn() } }));

import SavesTab from './SavesTab.svelte';

const game: GameSummary = { id: 1, name: 'Metroid' } as GameSummary;
const supported: CloudPanelInfo = { supported: true, block_reason: '', scope: 'per_game' };
const blocked: CloudPanelInfo = { supported: false, block_reason: 'No emulator.', scope: 'per_game' };

function html(
  cloudMode: CloudMode,
  savePanelInfo: CloudPanelInfo | null,
  statePanelInfo: CloudPanelInfo | null
) {
  return render(SavesTab, {
    props: {
      gameTitle: 'Metroid',
      cloudGame: syntheticCloudGame(game, 'NES'),
      isNative: false,
      savePanelInfo,
      statePanelInfo,
      cloudMode,
      infoError: null,
      onToggle: () => {},
    },
  }).body;
}

describe('SavesTab overview', () => {
  it('shows a latest-record card per supported kind while records load', () => {
    const body = html('overview', supported, supported);
    expect(body).toContain('details-latest-save');
    expect(body).toContain('Latest cloud save');
    expect(body).toContain('Checking cloud saves…');
    expect(body).toContain('details-latest-state');
    expect(body).toContain('Latest cloud state');
  });

  it('shows no state card when states are not supported', () => {
    const body = html('overview', supported, blocked);
    expect(body).toContain('details-latest-save');
    expect(body).not.toContain('details-latest-state');
  });

  it('shows no cards at all when nothing is supported', () => {
    const body = html('overview', blocked, blocked);
    expect(body).not.toContain('details-latest-save');
    expect(body).toContain('details-cloud-unsupported');
  });

  it('hides the cards and shows the panel without a Back button in manage mode', () => {
    const body = html('save', supported, supported);
    expect(body).not.toContain('details-latest-save');
    expect(body).toContain('cloud-panel');
    expect(body).not.toContain('cloud-back');
  });
});
