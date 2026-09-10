import { describe, expect, it } from 'vitest';
import {
  firmwareInstallLabel,
  firmwarePassFinished,
  firmwareRequested,
  firmwareStatusSettled,
  NO_FIRMWARE_REQUEST,
} from './firmware';

describe('firmware install request', () => {
  it('labels the button by what the frontend actually knows', () => {
    expect(firmwareInstallLabel(false)).toBe('Install');
    // Not "Installing…": the command is fire-and-forget, so no progress is
    // known — only that the pass was asked for.
    expect(firmwareInstallLabel(true)).toBe('Requested…');
  });

  it('pends for the platform the request was made for', () => {
    expect(firmwareRequested(7)).toEqual({ platformId: 7, pending: true });
  });

  it('clears the request when that platform status answers', () => {
    const pending = firmwareRequested(7);
    expect(firmwareStatusSettled(pending, 7)).toEqual(NO_FIRMWARE_REQUEST);
  });

  it('leaves the request alone when another platform status answers', () => {
    const pending = firmwareRequested(7);
    expect(firmwareStatusSettled(pending, 19)).toBe(pending);
  });

  it('clears the request and refetches on the pass event for the platform on screen', () => {
    const pending = firmwareRequested(7);
    const out = firmwarePassFinished(pending, { platform_id: 7, ok: true }, 7);
    expect(out.state).toEqual(NO_FIRMWARE_REQUEST);
    expect(out.refetch).toBe(true);
  });

  it('clears the request on a failed pass too, so the button never sticks', () => {
    const pending = firmwareRequested(7);
    const out = firmwarePassFinished(pending, { platform_id: 7, ok: false }, 7);
    expect(out.state).toEqual(NO_FIRMWARE_REQUEST);
    expect(out.refetch).toBe(true);
  });

  it('clears a request whose platform is no longer on screen, without refetching', () => {
    // The user pressed Install on platform 7, then moved to 19. The event
    // still ends 7's request; refetching 19's status would answer a question
    // nobody asked.
    const pending = firmwareRequested(7);
    const out = firmwarePassFinished(pending, { platform_id: 7, ok: true }, 19);
    expect(out.state).toEqual(NO_FIRMWARE_REQUEST);
    expect(out.refetch).toBe(false);
  });

  it('refetches for the platform on screen even with no request of its own', () => {
    // A background pass (a game install finished) ended for the platform
    // being viewed: its file count may have changed.
    const out = firmwarePassFinished(NO_FIRMWARE_REQUEST, { platform_id: 7, ok: true }, 7);
    expect(out.state).toEqual(NO_FIRMWARE_REQUEST);
    expect(out.refetch).toBe(true);
  });

  it('ignores an event for another platform while one is pending', () => {
    const pending = firmwareRequested(7);
    const out = firmwarePassFinished(pending, { platform_id: 19, ok: true }, 7);
    expect(out.state).toBe(pending);
    expect(out.refetch).toBe(false);
  });

  it('does nothing on an event with no platform selected', () => {
    const out = firmwarePassFinished(NO_FIRMWARE_REQUEST, { platform_id: 7, ok: true }, null);
    expect(out.refetch).toBe(false);
  });
});
