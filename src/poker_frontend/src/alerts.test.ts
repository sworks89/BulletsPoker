import { describe, expect, it } from 'vitest';
import { NotificationsUnsupportedError } from '@icp-sdk/auth/client';
import { alertsLabel, stateAfterError } from './alerts';

describe('the tournament alerts button', () => {
  it('disappears only where Internet Identity sends no notifications', () => {
    expect(stateAfterError(new NotificationsUnsupportedError())).toBe('unsupported');
  });

  it('stays available after any other failure, so the player can retry', () => {
    expect(stateAfterError(new Error('network'))).toBe('off');
  });

  it('says what pressing it will do', () => {
    expect(alertsLabel('off')).toContain('Turn on alerts');
    expect(alertsLabel('on')).toContain('turn off');
  });
});
