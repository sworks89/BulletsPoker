import { describe, expect, it } from 'vitest';
import { installOffer, isIos } from './pwa';

const IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Version/18.0 Mobile/15E148 Safari/604.1';
const ANDROID = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 Chrome/130.0 Mobile Safari/537.36';
const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15';

describe('installing the game', () => {
  it('recognises iPhones, and iPads that report themselves as Macs', () => {
    expect(isIos(IPHONE, 'iPhone', 5)).toBe(true);
    expect(isIos(MAC, 'MacIntel', 5)).toBe(true);
  });

  it('leaves Android and real Macs to the browser', () => {
    expect(isIos(ANDROID, 'Linux armv8l', 5)).toBe(false);
    expect(isIos(MAC, 'MacIntel', 0)).toBe(false);
  });

  it('offers nothing once installed, whatever the browser would allow', () => {
    expect(installOffer(true, true, false)).toBe('none');
    expect(installOffer(true, false, true)).toBe('none');
  });

  it('uses the browser dialog where there is one, and instructions on iPhone', () => {
    expect(installOffer(false, true, false)).toBe('prompt');
    expect(installOffer(false, false, true)).toBe('ios');
  });

  it('offers nothing where the browser has no way to install', () => {
    expect(installOffer(false, false, false)).toBe('none');
  });
});
