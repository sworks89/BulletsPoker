import { describe, expect, it } from 'vitest';
import { CUSTOM_DOMAINS } from './domains';
import app from './App.tsx?raw';
import home from '../index.html?raw';
import howToPlay from '../public/how-to-play/index.html?raw';
import tournaments from '../public/tournaments/index.html?raw';
import provablyFair from '../public/provably-fair/index.html?raw';
import sitemap from '../public/sitemap.xml?raw';
import robots from '../public/robots.txt?raw';
import logo from '../public/logo.svg?raw';

const SITE = 'https://www.bulletspoker.com';

/** Every public page: its address and its HTML. */
const PAGES: [string, string][] = [
  ['/', home],
  ['/how-to-play/', howToPlay],
  ['/tournaments/', tournaments],
  ['/provably-fair/', provablyFair],
];

const content = (html: string, attr: string, name: string) =>
  html.match(new RegExp(`<meta ${attr}="${name}" content="([^"]*)"`))?.[1];

describe('what search engines read', () => {
  it('the official address is one of the domains the site is served at', () => {
    expect(CUSTOM_DOMAINS).toContain(new URL(SITE).hostname);
  });

  it.each(PAGES)('%s has its own title, description and official address', (path, html) => {
    const title = html.match(/<title>([^<]*)<\/title>/)?.[1] ?? '';
    expect(title).toContain('Bullets Poker');
    expect(title.length).toBeLessThanOrEqual(65);
    const description = content(html, 'name', 'description') ?? '';
    expect(description.length).toBeGreaterThanOrEqual(70);
    expect(description.length).toBeLessThanOrEqual(165);
    expect(html).toContain(`<link rel="canonical" href="${SITE}${path}" />`);
    expect(content(html, 'property', 'og:url')).toBe(`${SITE}${path}`);
    expect(content(html, 'property', 'og:image')).toBe(`${SITE}/og.png`);
    expect(html.match(/<h1[ >]/g)).toHaveLength(1);
  });

  it('no two pages share a title or a description', () => {
    const pages = PAGES.map(([, html]) => html);
    expect(new Set(pages.map((h) => h.match(/<title>([^<]*)/)?.[1])).size).toBe(PAGES.length);
    expect(new Set(pages.map((h) => content(h, 'name', 'description'))).size).toBe(PAGES.length);
  });

  it('the home page text stays when the app loads, outside what React replaces', () => {
    const root = home.indexOf('<div id="root"></div>');
    expect(root).toBeGreaterThan(-1);
    expect(home.indexOf('class="site-about"')).toBeGreaterThan(root);
  });

  it('the home page says who it is, by both spellings of the name', () => {
    const data = JSON.parse(home.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)![1]);
    expect(data).toMatchObject({ '@type': 'WebSite', name: 'Bullets Poker', url: `${SITE}/` });
    expect(data.alternateName).toContain('BulletsPoker');
  });

  it('the sitemap lists every page and nothing else, and robots.txt points to it', () => {
    const urls = [...sitemap.matchAll(/<loc>([^<]*)<\/loc>/g)].map((m) => m[1]);
    expect(urls.sort()).toEqual(PAGES.map(([path]) => `${SITE}${path}`).sort());
    expect(robots).toContain(`Sitemap: ${SITE}/sitemap.xml`);
    expect(robots).not.toMatch(/^Disallow: \/\s*$/m);
  });

  it('the files pages refer to exist', () => {
    const files = Object.keys(import.meta.glob('../public/*', { query: '?url' }));
    for (const file of ['site.css', 'favicon.svg', 'logo.svg', 'og.png']) {
      expect(files).toContain(`../public/${file}`);
    }
  });

  it('the logo is on the game and on every page, and holds still on request', () => {
    expect(app).toContain('src="/logo.svg"');
    for (const [, page] of PAGES.slice(1)) {
      expect(page).toContain('src="/logo.svg"');
    }
    expect(logo).toContain('prefers-reduced-motion: reduce');
  });

  it('the home page invites people to play, and every other page sends them to it', () => {
    expect(home.match(/<title>([^<]*)/)?.[1]).toMatch(/^Bullets Poker \| Play /);
    expect(content(home, 'name', 'description')).toMatch(/^Play /);
    for (const [, page] of PAGES.slice(1)) {
      expect(page).toContain('class="site-cta" href="/"');
    }
  });

  it('no page promises winnings', () => {
    for (const [, html] of PAGES) {
      expect(html).not.toMatch(/win (real )?money|guaranteed win|get rich|risk[- ]free/i);
    }
  });

  it('every page carries the real-money notice', () => {
    // The home page's is in the app's own footer, just above the About text.
    for (const html of [app, ...PAGES.slice(1).map(([, page]) => page)]) {
      expect(html).toContain('Real-money play may be restricted where you live');
    }
  });
});
