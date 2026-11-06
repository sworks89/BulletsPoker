import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
import { alertForward, readTournamentParam } from './links';
import { captureInstallPrompt } from './pwa';
import { readRefParam, rememberRef } from './referrals';

// Keep a ?ref= code for sign-up, then drop it from the address bar so a shared
// link or screenshot of this page doesn't carry it along.
const ref = readRefParam(window.location.search);
if (ref) {
  rememberRef(ref);
  const url = new URL(window.location.href);
  url.searchParams.delete('ref');
  window.history.replaceState(null, '', url.pathname + url.search + url.hash);
}

// A tapped alert on the canister address moves on to the main site. Read before
// the address bar is tidied below.
const forward = alertForward(window.location.hostname, window.location.search);

// The tournament an alert opened, read once and dropped for the same reason.
const tournament = readTournamentParam(window.location.search);
if (new URLSearchParams(window.location.search).has('tournament')) {
  const url = new URL(window.location.href);
  url.searchParams.delete('tournament');
  window.history.replaceState(null, '', url.pathname + url.search + url.hash);
}

captureInstallPrompt();

if (forward) {
  window.location.replace(forward);
} else {
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <App launchTournament={tournament} />
    </StrictMode>,
  );
}
