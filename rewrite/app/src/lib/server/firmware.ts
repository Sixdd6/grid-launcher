// The firmware chip's Install request, as a pure state machine (design §6).
// Server.svelte holds one of these in `$state` and does nothing to it that is
// not one of the transitions below, so the rule the review asked for — the
// button never claims progress it cannot know, and never stays disabled
// forever — is testable without a component harness.
//
// The chip's Install command is fire-and-forget: the backend answers the
// command immediately and announces the end of the background pass with
// `FIRMWARE_PASS_FINISHED_EVENT`. Two things can therefore end a request:
// that event, and any status answer for the same platform (the next platform
// selection, or the refetch the event triggers). Whichever lands first wins.

import type { FirmwarePassFinished } from '../api';

/** Which platform has an Install in flight, if any. */
export type FirmwareRequest = {
  /** The platform the request was made for; `null` when none is pending. */
  platformId: number | null;
  pending: boolean;
};

/** No Install in flight — the initial state, and the state after every end. */
export const NO_FIRMWARE_REQUEST: FirmwareRequest = { platformId: null, pending: false };

/**
 * The Install button's text. "Requested…" rather than "Installing…": the
 * command only says the pass was asked for, so claiming progress would state
 * a fact the frontend cannot know.
 */
export function firmwareInstallLabel(pending: boolean): string {
  return pending ? 'Requested…' : 'Install';
}

/** Install was pressed for `platformId`. */
export function firmwareRequested(platformId: number): FirmwareRequest {
  return { platformId, pending: true };
}

/**
 * A `platform_firmware_status` call for `platformId` has answered (or been
 * refused). The request it belongs to is over: the chip now shows a fresh
 * answer, so leaving the button disabled would outlast anything it could be
 * waiting for. A status answer for a DIFFERENT platform leaves the request
 * alone — it belongs to another question.
 */
export function firmwareStatusSettled(
  state: FirmwareRequest,
  platformId: number,
): FirmwareRequest {
  return state.platformId === platformId ? NO_FIRMWARE_REQUEST : state;
}

/**
 * A `firmware-pass-finished` event arrived. `refetch` is whether the chip's
 * status should be read again: only for the platform on screen, since a
 * status answer for any other platform would be thrown away by the staleness
 * guard anyway.
 *
 * `ok` is deliberately unused here. The chip states what the server offers,
 * not what one pass managed to fetch, and the refetched status is the honest
 * answer either way; grid-core has already logged the warnings.
 */
export function firmwarePassFinished(
  state: FirmwareRequest,
  event: FirmwarePassFinished,
  activePlatform: number | null,
): { state: FirmwareRequest; refetch: boolean } {
  return {
    state: firmwareStatusSettled(state, event.platform_id),
    refetch: activePlatform !== null && event.platform_id === activePlatform,
  };
}
