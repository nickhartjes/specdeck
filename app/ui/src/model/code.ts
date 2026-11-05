// Invite codes as people type them: 12 Crockford base32 characters, shown as 7K2M-9QXH-4TPA.
// Mirrors crates/protocol/src/invite.rs, which has the final say.

const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const LENGTH = 12;

/** The code characters in what someone typed or pasted: O→0, I/L→1, anything else dropped. */
export function codeCharacters(input: string): string {
  const fromLink = input.match(/specdeck:\/\/join\/([^\s/?]+)/i)?.[1] ?? input.match(/\/join\/?#([^\s]+)/i)?.[1] ?? input;
  let out = '';
  for (const raw of fromLink.toUpperCase()) {
    const c = raw === 'O' ? '0' : raw === 'I' || raw === 'L' ? '1' : raw;
    if (ALPHABET.includes(c)) out += c;
    if (out.length === LENGTH) break;
  }
  return out;
}

/** Formats as the user types: groups of four with dashes. */
export function formatCode(input: string): string {
  return codeCharacters(input).match(/.{1,4}/g)?.join('-') ?? '';
}

export function isCompleteCode(input: string): boolean {
  return codeCharacters(input).length === LENGTH;
}

/**
 * A join link naming the host's sharing service, so a viewer on another service can join. When
 * the service serves the web viewer, a web link that opens in any browser and that the app also
 * reads, `https://host/join#CODE`; otherwise `specdeck://` (`web-viewer` design decision 6).
 * Mirrors `InviteCode::web_link` and `join_link` in crates/protocol/src/invite.rs.
 */
export function joinLink(code: string, service: string, webViewer = false): string {
  if (webViewer) {
    const base = service.replace(/^ws(s?):\/\//, 'http$1://').replace(/\/[^/]*$/, '');
    return `${base}/join#${formatCode(code)}`;
  }
  return `specdeck://join/${formatCode(code)}?server=${encodeURIComponent(service)}`;
}

/** Whether pasted text is a join link, which may name a sharing service. */
export function isJoinLink(input: string): boolean {
  return /^\s*specdeck:\/\/join\//i.test(input) || /^\s*https?:\/\/\S+\/join\/?#/i.test(input);
}

/** What to show for a sharing service URL: its host, with the port if there is one. */
export function serviceName(url: string): string {
  return url.replace(/^[a-z]+:\/\//i, '').split('/')[0] ?? url;
}
