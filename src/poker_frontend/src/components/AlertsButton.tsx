import { useEffect, useState } from 'react';
import { alertsLabel, stateAfterError, type AlertsState } from '../alerts';
import { useApp } from '../context';
import { errorText } from '../format';
import { authClient } from '../ic';

/** Lets a signed-in player ask Internet Identity for tournament alerts on this device. */
export function AlertsButton() {
  const { principal, notify } = useApp();
  const [state, setState] = useState<AlertsState>('loading');

  useEffect(() => {
    if (!principal) return;
    let live = true;
    authClient
      .getNotificationConsentStatus()
      .then(({ granted }) => live && setState(granted ? 'on' : 'off'))
      .catch((e) => live && setState(stateAfterError(e)));
    return () => {
      live = false;
    };
  }, [principal]);

  const turnOn = async () => {
    try {
      // Nothing may be awaited before this: browsers only open the window for a click.
      const { granted } = await authClient.requestNotificationConsent();
      setState(granted ? 'on' : 'off');
      if (granted) notify("Alerts on. You'll hear a few minutes before your scheduled tournaments.");
    } catch (e) {
      setState(stateAfterError(e));
      if (stateAfterError(e) !== 'unsupported') notify(errorText(e), 'err');
    }
  };

  const turnOff = async () => {
    try {
      await authClient.revokeNotificationConsent();
      setState('off');
      notify('Alerts off.');
    } catch (e) {
      notify(errorText(e), 'err');
    }
  };

  if (!principal || state === 'loading' || state === 'unsupported') return null;
  return (
    <div className="alerts-row">
      <button className="btn ghost small" onClick={state === 'on' ? turnOff : turnOn}>
        {alertsLabel(state)}
      </button>
      {state === 'off' && (
        <span className="muted small">
          A reminder a few minutes before a scheduled tournament you've registered for. Internet Identity delivers it and asks your permission first. On iPhone, it also asks you to install its ICP Alerts app.
        </span>
      )}
    </div>
  );
}
