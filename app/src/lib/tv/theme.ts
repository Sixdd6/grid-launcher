// TV mode's theme (ruling 22, checklist item 13): the one Theme setting is
// shared with the desktop; in TV mode "Follow system" shows dark (a dark
// room), and Dark or Light apply as chosen. Pure.
import type { ResolvedTheme, ThemeChoice } from '../theme';

export function tvTheme(choice: ThemeChoice): ResolvedTheme {
  return choice === 'light' ? 'light' : 'dark';
}
