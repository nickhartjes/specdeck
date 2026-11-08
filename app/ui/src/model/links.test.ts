import { describe, expect, it } from 'vitest';
import { MAX_LINK_BYTES, MAX_LINK_TITLE_CHARS, checkLink, hasFragment, maskAfterHash, refusalText, site, validTitle } from './links';

// The same cases as `links.rs`, which decides on the Rust side.
describe('checkLink', () => {
  it('accepts web addresses others can reach, normalized', () => {
    expect(checkLink('https://example.com/')).toEqual({ url: 'https://example.com/' });
    expect(checkLink('http://192.168.1.20:8080/')).toEqual({ url: 'http://192.168.1.20:8080/' });
    expect(checkLink('  https://Example.COM  ')).toEqual({ url: 'https://example.com/' });
    const room = 'https://excalidraw.com/#room=5c1e2a9b,Q3h9vX2m';
    expect(checkLink(room)).toEqual({ url: room });
  });

  it("refuses the sharer's own machine", () => {
    for (const link of [
      'http://localhost:5173/',
      'https://localhost/',
      'http://LOCALHOST./',
      'https://app.localhost/',
      'https://127.0.0.1/',
      'http://127.1.2.3:8443/',
      'http://2130706433/',
      'http://0x7f.1/',
      'http://[::1]/',
      'http://[::ffff:127.0.0.1]/',
      'http://0.0.0.0/',
      'http://[::]/',
    ]) {
      expect(checkLink(link), link).toEqual({ refusal: 'own-machine' });
    }
  });

  it('refuses other schemes, credentials and what is not an address', () => {
    for (const link of ['javascript:alert(1)', 'file:///etc/hosts', 'ftp://example.com/', 'localhost:5173']) {
      expect(checkLink(link), link).toEqual({ refusal: 'not-web' });
    }
    expect(checkLink('https://user:pw@example.com/')).toEqual({ refusal: 'credentials' });
    expect(checkLink('https://google.com@evil.example/')).toEqual({ refusal: 'credentials' });
    expect(checkLink('example.com')).toEqual({ refusal: 'malformed' });
    expect(checkLink('')).toEqual({ refusal: 'malformed' });
  });

  it('refuses links longer than the limit, also once normalized', () => {
    const base = 'https://example.com/';
    const fits = base + 'a'.repeat(MAX_LINK_BYTES - base.length);
    expect(checkLink(fits)).toEqual({ url: fits });
    expect(checkLink(fits + 'a')).toEqual({ refusal: 'too-long' });
    expect(checkLink(`${base}${' '.repeat(MAX_LINK_BYTES - base.length - 1)}x`)).toEqual({ refusal: 'too-long' });
  });

  it('says to the host that viewers cannot reach their computer, and to a viewer that it is theirs', () => {
    expect(refusalText('own-machine', true)).toMatch(/Viewers can’t reach your computer/);
    expect(refusalText('own-machine', false)).toMatch(/on your own computer/);
  });
});

describe('the card', () => {
  it('shows a look-alike site in ASCII form', () => {
    expect(site('https://exаmple.com/')).toBe('xn--exmple-4nf.com');
    expect(site('not a link')).toBeNull();
  });

  it('masks the part after # and knows when there is one', () => {
    expect(maskAfterHash('https://excalidraw.com/#room=a,b')).toBe('https://excalidraw.com/#…');
    expect(maskAfterHash('https://example.com/')).toBe('https://example.com/');
    expect(hasFragment('https://excalidraw.com/#room=a,b')).toBe(true);
    expect(hasFragment('https://example.com/#')).toBe(false);
    expect(hasFragment('https://example.com/')).toBe(false);
  });

  it('takes a title of one line of at most 120 characters, or none', () => {
    expect(validTitle('Staging: the new pill')).toBe(true);
    expect(validTitle('')).toBe(true);
    expect(validTitle('é'.repeat(MAX_LINK_TITLE_CHARS))).toBe(true);
    expect(validTitle('a'.repeat(MAX_LINK_TITLE_CHARS + 1))).toBe(false);
    expect(validTitle('two\nlines')).toBe(false);
    expect(validTitle('bell\u0007')).toBe(false);
  });
});
