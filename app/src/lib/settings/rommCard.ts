// Settings › Game stores › RomM card (RommStorefront README, "RomM card";
// prototype `rmCard`): what the card shows for a status — the pill, the
// sub-line, the warning, the fields and the actions in order. Pure; the
// words are `romm/words.ts`'s.
import type { RommStatus } from '../api';
import {
  ROMM_CARD_ACTIONS,
  rommAccountLine,
  rommCardWarning,
  rommGamesLine,
  rommHost,
  rommNotConnectedText,
  rommPill,
  rommSignInKind,
  rommState,
  rommStatusLine,
  type RommPill,
  type RommState,
} from '../romm/words';

export type RommCardActionId = 'show' | 'retry' | 'signin' | 'edit' | 'reconnect' | 'open' | 'disconnect' | 'connect';

export type RommCardAction = {
  id: RommCardActionId;
  label: string;
  primary: boolean;
  destructive: boolean;
  disabled: boolean;
};

export type RommCardField = {
  key: 'server' | 'user' | 'signin' | 'status' | 'games';
  label: string;
  value: string;
  /** The Server field of a signed-in card is a link that opens the server. */
  link: boolean;
};

export type RommCardView = {
  state: RommState;
  /** The dashed card: RomM not connected and not connecting to a stored server. */
  off: boolean;
  pill: RommPill;
  account: string;
  warning: { strong: string; text: string } | null;
  /** The not-connected card's body; `null` otherwise. */
  lede: string | null;
  fields: RommCardField[];
  actions: RommCardAction[];
};

function action(id: RommCardActionId, label: string, o: Partial<Pick<RommCardAction, 'primary' | 'destructive' | 'disabled'>> = {}): RommCardAction {
  return { id, label, primary: o.primary ?? false, destructive: o.destructive ?? false, disabled: o.disabled ?? false };
}

/** The Status field: the words' status line, or "Connecting to <host>…" while a probe runs. */
export function rommCardStatusLine(status: RommStatus): string {
  if (status.busy) return `Connecting to ${rommHost(status)}…`;
  return rommStatusLine(status);
}

/** The Games field: "45 on the server · 11 installed here" while connected
 *  and the server's count is known, else "11 installed here". */
export function rommCardGamesLine(status: RommStatus, onServer: number | null): string {
  if (status.connection === 'connected' && onServer !== null) return rommGamesLine(onServer, status.installed_count);
  return `${status.installed_count} installed here`;
}

/**
 * The card for `status`. `onServer` is the server's game count (`null` until
 * known); `retrying` is true while the card's own Reconnect / Retry runs.
 */
export function rommCardView(status: RommStatus, onServer: number | null, retrying = false): RommCardView {
  const state = rommState(status);
  // A connect from a card with no stored server stays dashed while it runs:
  // there is no server to show yet.
  const off = status.connection === 'not_connected' && !(status.busy && status.server_url !== '');
  const busy = retrying || status.busy;
  // Any probe spins the pill, Reconnect from Connected included (prototype
  // `rmCard`: `RM.busy` → Connecting…).
  const pill: RommPill = busy && !off ? { text: 'Connecting…', tone: 'busy' } : rommPill(status);
  const base = { state, off, pill, account: rommAccountLine(status), warning: rommCardWarning(status) };

  if (off) {
    const stored = status.server_url !== '';
    return {
      ...base,
      lede: rommNotConnectedText(status.installed_count),
      fields: stored
        ? [
            { key: 'server', label: 'Server', value: status.server_url, link: false },
            { key: 'user', label: 'User', value: status.username, link: false },
            { key: 'status', label: 'Status', value: 'Not connected', link: false },
          ]
        : [],
      actions: [
        action('connect', ROMM_CARD_ACTIONS.connect, { primary: true }),
        ...(stored ? [action('open', ROMM_CARD_ACTIONS.openInBrowser)] : []),
      ],
    };
  }

  const fields: RommCardField[] = [
    { key: 'server', label: 'Server', value: status.server_url, link: true },
    { key: 'user', label: 'User', value: status.username, link: false },
    { key: 'signin', label: 'Sign-in', value: rommSignInKind(status), link: false },
    { key: 'status', label: 'Status', value: rommCardStatusLine(status), link: false },
    { key: 'games', label: 'Games', value: rommCardGamesLine(status, onServer), link: false },
  ];

  const actions: RommCardAction[] = [];
  if (status.connection === 'connected') {
    actions.push(action('show', ROMM_CARD_ACTIONS.showInLibrary, { primary: true }));
  } else if (status.connection === 'unreachable') {
    actions.push(action('retry', busy ? ROMM_CARD_ACTIONS.retrying : ROMM_CARD_ACTIONS.retry, { primary: true, disabled: busy }));
  } else if (status.connection === 'expired') {
    actions.push(action('signin', ROMM_CARD_ACTIONS.signInAgain, { primary: true, disabled: busy }));
  }
  actions.push(action('edit', ROMM_CARD_ACTIONS.edit, { disabled: busy }));
  if (status.connection === 'connected') {
    actions.push(action('reconnect', busy ? ROMM_CARD_ACTIONS.reconnecting : ROMM_CARD_ACTIONS.reconnect, { disabled: busy }));
  }
  actions.push(action('open', ROMM_CARD_ACTIONS.openInBrowser));
  actions.push(action('disconnect', ROMM_CARD_ACTIONS.disconnect, { destructive: true, disabled: busy }));
  return { ...base, lede: null, fields, actions };
}
