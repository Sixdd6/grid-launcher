// Settings › Cloud saves in TV mode (checklist items 241–242; tvmode.js
// `paneRows('cloud')`): the desktop form's five settings as rows, each
// saved as soon as it changes.
import type { CloudSettings } from '../../../../api';
import type { Row } from '../../../rows';

export function delayText(v: number): string {
  return `${v} s`;
}

export function retainText(v: number): string {
  return v === 0 ? 'Unlimited' : String(v);
}

/** The rows; while the settings load, nothing; a failed read says why. */
export function cloudRows(s: CloudSettings | null, error: string | null, save: (next: CloudSettings) => void): Row[] {
  if (s === null) return error ? [{ type: 'info', key: 'cloud-error', label: "Couldn't read the cloud save settings", value: error }] : [];
  const set = (patch: Partial<CloudSettings>) => save({ ...s, ...patch });
  return [
    {
      type: 'switch',
      key: 'restore',
      label: 'Restore before launch',
      sub: 'Fetch the latest cloud save before a game starts',
      on: s.download_on_launch,
      set: (on) => set({ download_on_launch: on }),
    },
    {
      type: 'switch',
      key: 'upload',
      label: 'Upload after exit',
      sub: 'Send new saves when a game closes',
      on: s.upload_on_exit,
      set: (on) => set({ upload_on_exit: on }),
    },
    { type: 'switch', key: 'skip-newer', label: 'Skip when the local save is newer', on: s.skip_if_local_newer, set: (on) => set({ skip_if_local_newer: on }) },
    {
      type: 'slider',
      key: 'delay',
      label: 'Upload delay',
      value: s.upload_delay_seconds,
      min: 0,
      max: 60,
      step: 5,
      format: delayText,
      set: (v) => set({ upload_delay_seconds: v }),
    },
    {
      type: 'slider',
      key: 'retain',
      label: 'Saves to keep per game',
      value: s.retention_limit,
      min: 0,
      max: 50,
      step: 1,
      format: retainText,
      set: (v) => set({ retention_limit: v }),
    },
  ];
}
