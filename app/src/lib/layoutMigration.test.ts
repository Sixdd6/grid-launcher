import { describe, expect, it } from 'vitest';
import { migrationToastText } from './layoutMigration';

describe('migrationToastText', () => {
  it('names the failure and says the migration retries', () => {
    expect(migrationToastText('rename /library/Games -> /library/games: crosses devices')).toBe(
      'Library reorganization did not finish: rename /library/Games -> /library/games: crosses devices. It will retry on next launch.',
    );
  });
});
