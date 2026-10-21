import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
