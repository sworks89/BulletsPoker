import { describe, expect, it } from 'vitest';
import { LOCAL_CONNECT_SOURCES, withLocalConnectSources } from './localCsp';
import headers from '../public/_headers?raw';
const connectSrc = (h: string) => h.match(/connect-src ([^;]+);/)![1].split(' ');

describe('content security policy', () => {
  it('lets the mainnet page call only IC boundary nodes and GitHub', () => {
    const sources = connectSrc(headers);
    expect(sources.filter((s) => s.includes('localhost') || s.includes('127.0.0.1'))).toEqual([]);
    expect(sources.every((s) => s === "'self'" || s.startsWith('https://'))).toBe(true);
  });

  it('adds the local network for a local build, and nothing else changes', () => {
    const local = withLocalConnectSources(headers);
    expect(connectSrc(local)).toEqual(["'self'", ...LOCAL_CONNECT_SOURCES, ...connectSrc(headers).slice(1)]);
    expect(local.replace(` ${LOCAL_CONNECT_SOURCES.join(' ')}`, '')).toBe(headers);
  });
});
