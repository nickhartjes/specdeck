// Which links may be shared, as `check_link` in crates/protocol/src/links.rs decides it, for
// the share dialog's messages. Both parse addresses by the WHATWG URL standard, so they agree;
// the Rust side checks again and decides.

/** Longest link, in bytes of its normalized form; `MAX_LINK_BYTES`. */
export const MAX_LINK_BYTES = 2048;
/** Longest title, in characters; `MAX_LINK_TITLE_CHARS`. */
export const MAX_LINK_TITLE_CHARS = 120;

export type LinkRefusal = 'too-long' | 'not-web' | 'own-machine' | 'credentials' | 'malformed';

const bytes = (text: string) => new TextEncoder().encode(text).length;

/** Whether an IP address, as `URL` writes it, is on this machine: loopback or unspecified. */
function ownAddress(host: string): boolean {
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host)) return host.startsWith('127.') || host === '0.0.0.0';
  if (!host.startsWith('[')) return false;
  const v6 = host.slice(1, -1);
  if (v6 === '::1' || v6 === '::') return true;
  // IPv4-mapped, which `URL` writes in hex: `::ffff:7f00:1` is 127.0.0.1.
  const mapped = /^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/.exec(v6);
  if (!mapped) return false;
  const high = parseInt(mapped[1]!, 16);
  return high >> 8 === 127 || (high === 0 && parseInt(mapped[2]!, 16) === 0);
}

/** The link's normalized form, which is what is sent, or why it can't be shared. */
export function checkLink(input: string): { url: string } | { refusal: LinkRefusal } {
  const text = input.trim();
  if (bytes(text) > MAX_LINK_BYTES) return { refusal: 'too-long' };
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    return { refusal: 'malformed' };
  }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return { refusal: 'not-web' };
  if (url.username || url.password) return { refusal: 'credentials' };
  const name = url.hostname.replace(/\.$/, '');
  if (name === 'localhost' || name.endsWith('.localhost') || ownAddress(name)) return { refusal: 'own-machine' };
  if (bytes(url.href) > MAX_LINK_BYTES) return { refusal: 'too-long' };
  return { url: url.href };
}

/** What the share dialog says for a refusal; `host` changes who can't reach what. */
export function refusalText(refusal: LinkRefusal, host: boolean): string {
  switch (refusal) {
    case 'own-machine':
      return host
        ? 'Viewers can’t reach your computer, so links to localhost or 127.0.0.1 can’t be shared. Opening your app to viewers is not available yet.'
        : 'That address is on your own computer, so nobody else can reach it.';
    case 'not-web':
      return 'Only web addresses starting with https:// can be shared.';
    case 'credentials':
      return 'Addresses with a user name or password can’t be shared.';
    case 'too-long':
      return `That address is too long to share: at most ${MAX_LINK_BYTES} bytes.`;
    case 'malformed':
      return 'Paste a whole address, starting with https://.';
  }
}

/** Whether a title may be sent: blank counts as none; otherwise one line of at most 120 characters. */
export function validTitle(title: string): boolean {
  const text = title.trim();
  // Control characters, as Rust's `char::is_control` counts them.
  // eslint-disable-next-line no-control-regex
  return text === '' || ([...text].length <= MAX_LINK_TITLE_CHARS && !/[\u0000-\u001f\u007f-\u009f]/.test(text));
}

/** Whether the link has a key-like `#` part, which makes *for this session only* start on. */
export function hasFragment(url: string): boolean {
  try {
    return new URL(url).hash.length > 1;
  } catch {
    return false;
  }
}

/** The link's site in ASCII form, for the card; null for what is not an address. */
export function site(url: string): string | null {
  try {
    return new URL(url).hostname || null;
  } catch {
    return null;
  }
}

/** The address up to its `#`, with the rest hidden, for a link for this session only. */
export function maskAfterHash(url: string): string {
  const at = url.indexOf('#');
  return at < 0 ? url : `${url.slice(0, at)}#…`;
}
