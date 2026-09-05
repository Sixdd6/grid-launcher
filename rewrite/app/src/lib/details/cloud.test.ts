import { describe, expect, it } from 'vitest';
import type { CloudRecord, GameSummary } from '../api';
import {
  cloudButtonLabel,
  cloudRecordSummary,
  cloudRecordTitle,
  createRequestGuard,
  deleteConfirmText,
  isNativeExecutablePlatform,
  isNativeLaunchPlatform,
  recordsStatusLine,
  restoreConfirmText,
  sharedScopeWarning,
  syntheticCloudGame,
  toggleCloudMode,
  uploadButtonLabel,
  uploadedLine,
} from './cloud';

function record(overrides: Partial<CloudRecord>): CloudRecord {
  return {
    id: 7,
    file_name: '',
    emulator: '',
    slot: null,
    size_text: '1.2 MB',
    absolute_time: '2026-09-01 10:00',
    relative_time: '1 hour ago',
    restorable: true,
    restore_tooltip: null,
    ...overrides,
  };
}

describe('cloudRecordTitle', () => {
  it('uses file_name when present', () => {
    expect(cloudRecordTitle(record({ file_name: 'slot0.srm' }), 'save')).toBe('slot0.srm');
  });

  it('trims file_name before using it', () => {
    expect(cloudRecordTitle(record({ file_name: '  slot0.srm  ' }), 'save')).toBe('slot0.srm');
  });

  it('falls back to "Cloud Save #<id>" for a blank file_name on a save record', () => {
    expect(cloudRecordTitle(record({ file_name: '', id: 12 }), 'save')).toBe('Cloud Save #12');
  });

  it('falls back to "Cloud State #<id>" for a blank file_name on a state record', () => {
    expect(cloudRecordTitle(record({ file_name: '   ', id: 5 }), 'state')).toBe('Cloud State #5');
  });
});

describe('cloudRecordSummary', () => {
  it('composes emulator and size with a bullet', () => {
    expect(cloudRecordSummary(record({ emulator: 'RetroArch', size_text: '2.0 MB' }), 'save')).toBe(
      'RetroArch • 2.0 MB'
    );
  });

  it('falls back to "Unknown emulator" for a blank emulator field', () => {
    expect(cloudRecordSummary(record({ emulator: '  ', size_text: '2.0 MB' }), 'save')).toBe(
      'Unknown emulator • 2.0 MB'
    );
  });

  it('appends the slot suffix for a save record with a non-empty slot', () => {
    expect(
      cloudRecordSummary(record({ emulator: 'Redream', size_text: '64 KB', slot: 'vmu0' }), 'save')
    ).toBe('Redream • 64 KB • Slot vmu0');
  });

  it('omits the slot suffix when slot is null', () => {
    expect(cloudRecordSummary(record({ emulator: 'Redream', size_text: '64 KB', slot: null }), 'save')).toBe(
      'Redream • 64 KB'
    );
  });

  it('omits the slot suffix when slot is blank', () => {
    expect(cloudRecordSummary(record({ emulator: 'Redream', size_text: '64 KB', slot: '   ' }), 'save')).toBe(
      'Redream • 64 KB'
    );
  });

  it('never appends a slot suffix for a state record, even when slot is set', () => {
    expect(
      cloudRecordSummary(record({ emulator: 'PCSX2', size_text: '4 MB', slot: 'vmu0' }), 'state')
    ).toBe('PCSX2 • 4 MB');
  });

  it('passes through an "Unknown size" size_text verbatim', () => {
    expect(cloudRecordSummary(record({ emulator: 'PCSX2', size_text: 'Unknown size' }), 'save')).toBe(
      'PCSX2 • Unknown size'
    );
  });
});

describe('uploadedLine', () => {
  it('composes the absolute and relative time as delivered', () => {
    expect(uploadedLine(record({ absolute_time: '2026-09-01 10:00', relative_time: '1 hour ago' }))).toBe(
      'Uploaded 2026-09-01 10:00 (1 hour ago)'
    );
  });

  it('passes through the unknown-time sentinel pair verbatim', () => {
    expect(uploadedLine(record({ absolute_time: 'Unknown upload time', relative_time: 'Unknown' }))).toBe(
      'Uploaded Unknown upload time (Unknown)'
    );
  });
});

describe('toggleCloudMode', () => {
  it('enters save mode from overview', () => {
    expect(toggleCloudMode('overview', 'save')).toBe('save');
  });

  it('enters state mode from overview', () => {
    expect(toggleCloudMode('overview', 'state')).toBe('state');
  });

  it('returns to overview when the active mode is clicked again', () => {
    expect(toggleCloudMode('save', 'save')).toBe('overview');
    expect(toggleCloudMode('state', 'state')).toBe('overview');
  });

  it('switches directly from one mode to the other', () => {
    expect(toggleCloudMode('save', 'state')).toBe('state');
    expect(toggleCloudMode('state', 'save')).toBe('save');
  });
});

describe('cloudButtonLabel', () => {
  it('is always "Manage States" for state mode, regardless of scope', () => {
    expect(cloudButtonLabel('state', 'per_game')).toBe('Manage States');
    expect(cloudButtonLabel('state', 'shared_single')).toBe('Manage States');
  });

  it('is "Manage Saves" for per-game save scope', () => {
    expect(cloudButtonLabel('save', 'per_game')).toBe('Manage Saves');
  });

  it('is "Emulator Saves" for shared-single save scope', () => {
    expect(cloudButtonLabel('save', 'shared_single')).toBe('Emulator Saves');
  });

  it('is "Emulator Saves" for shared-slotted save scope', () => {
    expect(cloudButtonLabel('save', 'shared_slotted')).toBe('Emulator Saves');
  });
});

describe('uploadButtonLabel', () => {
  it('is "Upload Emulator Saves" when the panel label is "Emulator Saves"', () => {
    expect(uploadButtonLabel('save', 'Emulator Saves')).toBe('Upload Emulator Saves');
  });

  it('is "Upload Latest Save" for a normal save panel', () => {
    expect(uploadButtonLabel('save', 'Manage Saves')).toBe('Upload Latest Save');
  });

  it('is "Upload Latest State" for a state panel regardless of label', () => {
    expect(uploadButtonLabel('state', 'Manage States')).toBe('Upload Latest State');
  });
});

describe('isNativeExecutablePlatform', () => {
  it('is true for a platform string starting with "windows"', () => {
    expect(isNativeExecutablePlatform('Windows')).toBe(true);
    expect(isNativeExecutablePlatform('windows')).toBe(true);
  });

  it('is case-insensitive and trims surrounding whitespace', () => {
    expect(isNativeExecutablePlatform('  WINDOWS  ')).toBe(true);
  });

  it('is false for an emulated platform', () => {
    expect(isNativeExecutablePlatform('SNES')).toBe(false);
  });

  it('is false for a platform that merely contains "windows" mid-string', () => {
    expect(isNativeExecutablePlatform('Not Windows')).toBe(false);
  });
});

describe('isNativeLaunchPlatform', () => {
  it('accepts Windows and Linux platform names case-insensitively', () => {
    expect(isNativeLaunchPlatform('Windows')).toBe(true);
    expect(isNativeLaunchPlatform('  WINDOWS  ')).toBe(true);
    expect(isNativeLaunchPlatform('Linux')).toBe(true);
    expect(isNativeLaunchPlatform('linux')).toBe(true);
    expect(isNativeLaunchPlatform('Windows PC')).toBe(true);
  });

  it('rejects emulated platforms', () => {
    expect(isNativeLaunchPlatform('SNES')).toBe(false);
    expect(isNativeLaunchPlatform('Not Windows')).toBe(false);
    expect(isNativeLaunchPlatform('')).toBe(false);
  });

  // The scope predicate mirrors grid-core and must stay windows-only.
  it('is wider than isNativeExecutablePlatform, which stays windows-only', () => {
    expect(isNativeExecutablePlatform('Linux')).toBe(false);
    expect(isNativeLaunchPlatform('Linux')).toBe(true);
  });
});

describe('syntheticCloudGame', () => {
  it('carries the game id as rom_id and the given platform name', () => {
    const game: GameSummary = {
      id: 42,
      name: 'Portal',
      platform_id: 3,
      path_cover_small: null,
      path_cover_large: null,
      screenshot_urls: [],
      fanart_urls: [],
    };
    const synthetic = syntheticCloudGame(game, 'Emulators');
    expect(synthetic.title).toBe('Portal');
    expect(synthetic.platform).toBe('Emulators');
    expect(synthetic.rom_id).toBe(42);
    expect(synthetic.archive_path).toBe('');
    expect(synthetic.extracted_path).toBe('');
  });
});

describe('sharedScopeWarning', () => {
  it('is empty for per-game scope', () => {
    expect(sharedScopeWarning('per_game', 'xemu')).toBe('');
  });

  it('carries the exact shared-single copy, naming the emulator', () => {
    expect(sharedScopeWarning('shared_single', 'xemu')).toBe(
      'These cloud saves are shared xemu media. Restoring or deleting one affects every game using this emulator.'
    );
  });

  it('carries the exact shared-slotted copy, naming the emulator', () => {
    expect(sharedScopeWarning('shared_slotted', 'Redream')).toBe(
      'These cloud saves are shared Redream memory-card backups. Deleting one removes the backup for every game using that emulator slot.'
    );
  });

  it('falls back to "this emulator" when the name is blank', () => {
    expect(sharedScopeWarning('shared_single', '  ')).toBe(
      'These cloud saves are shared this emulator media. Restoring or deleting one affects every game using this emulator.'
    );
  });
});

describe('restoreConfirmText', () => {
  it('builds the exact save-restore dialog with no warning', () => {
    const result = restoreConfirmText('save', 'Chrono Trigger', '');
    expect(result.title).toBe('Restore Cloud Save');
    expect(result.message).toBe(
      "Restore the selected cloud save for 'Chrono Trigger' and overwrite the local save data?"
    );
  });

  it('builds the exact state-restore dialog with no warning', () => {
    const result = restoreConfirmText('state', 'Chrono Trigger', '');
    expect(result.title).toBe('Restore Cloud State');
    expect(result.message).toBe(
      "Restore the selected cloud state for 'Chrono Trigger' and overwrite the local state data?"
    );
  });

  it('appends the Warning paragraph verbatim when a shared notice is given', () => {
    const result = restoreConfirmText('save', 'Dreamcast VMU', 'These cloud saves are shared xemu media.');
    expect(result.message).toBe(
      "Restore the selected cloud save for 'Dreamcast VMU' and overwrite the local save data?\n\n" +
        'Warning: These cloud saves are shared xemu media.'
    );
  });
});

describe('deleteConfirmText', () => {
  it('builds the exact delete dialog with no warning', () => {
    const result = deleteConfirmText('save', 'slot0.srm', '');
    expect(result.title).toBe('Delete Cloud Save');
    expect(result.message).toBe("Delete 'slot0.srm' from the server? This cannot be undone.");
  });

  it('uses the State title for a state record', () => {
    expect(deleteConfirmText('state', 'Cloud State #5', '').title).toBe('Delete Cloud State');
  });

  it('appends the Warning paragraph verbatim when a shared notice is given', () => {
    const result = deleteConfirmText('save', 'slot0.srm', 'These cloud saves are shared xemu media.');
    expect(result.message).toBe(
      "Delete 'slot0.srm' from the server? This cannot be undone.\n\n" +
        'Warning: These cloud saves are shared xemu media.'
    );
  });
});

describe('createRequestGuard', () => {
  it('issues monotonically increasing ids', () => {
    const guard = createRequestGuard();
    expect(guard.next()).toBe(1);
    expect(guard.next()).toBe(2);
    expect(guard.next()).toBe(3);
  });

  it('reports only the most recently issued id as current', () => {
    const guard = createRequestGuard();
    const first = guard.next();
    const second = guard.next();
    expect(guard.isCurrent(first)).toBe(false);
    expect(guard.isCurrent(second)).toBe(true);
  });

  it('discards a stale response that resolves after a newer request was issued', () => {
    const guard = createRequestGuard();
    const stale = guard.next();
    guard.next(); // a newer fetch starts before the stale one resolves
    expect(guard.isCurrent(stale)).toBe(false);
  });
});

describe('recordsStatusLine', () => {
  it('names the record kind and count', () => {
    expect(recordsStatusLine(3, 'save')).toBe('Showing 3 cloud saves.');
    expect(recordsStatusLine(2, 'state')).toBe('Showing 2 cloud states.');
  });

  it('keeps the plural kind label for a single record, as Python does', () => {
    expect(recordsStatusLine(1, 'save')).toBe('Showing 1 cloud saves.');
    expect(recordsStatusLine(1, 'state')).toBe('Showing 1 cloud states.');
  });

  it('says nothing when there are no records', () => {
    expect(recordsStatusLine(0, 'save')).toBe('');
    expect(recordsStatusLine(-1, 'state')).toBe('');
  });
});
