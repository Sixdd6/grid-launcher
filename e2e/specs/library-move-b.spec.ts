import { existsSync } from 'node:fs';
import path from 'node:path';
import { dataDir } from '../helpers/env.js';
import {
  expectConfigSwitched,
  expectEmulatorFilesRewritten,
  expectRegistryMoved,
  expectSavesLink,
  expectTreeMoved,
  journalFile,
  reachShell,
  waitForMoveCompleted,
} from '../helpers/libraryMove.js';

const oldRoot = () => path.join(dataDir(), 'resume-old');
const newRoot = () => path.join(dataDir(), 'resume-new');

/**
 * Stage `library-move`, part B (9b): a move interrupted in its Copy phase
 * resumes in the background when GRID starts.
 *
 * `e2e/seed/library-move-seed.mjs library-move-b` writes the same library
 * as part A at `<data>/resume-old`, plus a Copy-phase move record
 * (`library-move.json`, every folder `copy` / `pending`) and a staging
 * folder under `<data>/resume-new` holding one short (cut-off) copy and one
 * finished copy. The app's startup sees the record and resumes the move from
 * `.setup()`, before the webview exists, so the spec waits for the Downloads
 * row the resume created and then checks the same end state as part A.
 *
 * The "Resuming the library move" toast is not asserted: the seeded move
 * is a few bytes and can finish before the frontend first reads the move
 * status, in which case the toast is rightly never shown.
 */
describe('library-move (b): resume an interrupted move at startup', () => {
  before(async () => {
    await reachShell();
  });

  it('finishes the resumed move on the Downloads row', async () => {
    await waitForMoveCompleted();
    expect(existsSync(journalFile())).toBe(false);
  });

  it('moves the files, re-copying the cut-off one', () => {
    expectTreeMoved(oldRoot(), newRoot());
  });

  it('switches the config and the registry to the new folder', () => {
    expectConfigSwitched(oldRoot(), newRoot());
    expectRegistryMoved(newRoot());
  });

  it('rewrites the emulator files and keeps the user-data links', () => {
    expectEmulatorFilesRewritten(oldRoot(), newRoot());
    expectSavesLink(newRoot(), 'inis');
    expectSavesLink(newRoot(), 'memcards');
  });
});
