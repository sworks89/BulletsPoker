import { NotificationsUnsupportedError } from '@icp-sdk/auth/client';

export type AlertsState = 'loading' | 'off' | 'on' | 'unsupported';

/** What a failed consent check means for the button. Only "this Internet Identity doesn't notify" hides it. */
export function stateAfterError(error: unknown): AlertsState {
  return error instanceof NotificationsUnsupportedError ? 'unsupported' : 'off';
}

export function alertsLabel(state: AlertsState): string {
  return state === 'on' ? '🔔 Alerts on: turn off' : '🔔 Get tournament alerts';
}
