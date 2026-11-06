import { promptInstall, type InstallOffer } from '../pwa';

/** Offers to put the game on the home screen, where the browser allows it. */
export function InstallBanner({ offer, onClose }: { offer: InstallOffer; onClose: () => void }) {
  if (offer === 'none') return null;
  return (
    <div className="install-banner" role="region" aria-label="Install Bullets Poker">
      <img src="/favicon.svg" alt="" width="32" height="32" />
      {offer === 'prompt' ? (
        <>
          <span className="grow">Install Bullets Poker to open it in one tap, without the browser around it.</span>
          <button className="btn small" onClick={() => void promptInstall()}>
            Install
          </button>
          <button className="btn ghost small" onClick={onClose}>
            Not now
          </button>
        </>
      ) : (
        <>
          <span className="grow">
            To open Bullets Poker in one tap, tap <b>Share</b> in your browser, then <b>Add to Home Screen</b>.
          </span>
          <button className="btn ghost small" onClick={onClose}>
            Got it
          </button>
        </>
      )}
    </div>
  );
}
