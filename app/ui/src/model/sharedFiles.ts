// Shared files as the window handles them (`shared-files`): a quick check before sending, which
// the host repeats from the content and decides, and how a file's bytes are shown.

import { MAX_SHARED_FILE_BYTES, type SharedFileInfo, type TimelineEvent } from '../api/types';

/** What may be shared, for the dialog and refusals. */
export const SHAREABLE = 'Images (PNG, JPEG, GIF, WebP), Markdown and text files up to 10 MB';

const IMAGE_TYPES = new Set(['image/png', 'image/jpeg', 'image/gif', 'image/webp']);
const REFUSED_EXTENSIONS = /\.(svgz?|pdf|zip|gz|tgz|7z|rar|exe|dmg|pkg|msi|app|docx?|xlsx?|pptx?|mp[34]|mov|wav)$/i;

/**
 * Why `file` cannot be shared, or null when it may be sent. Only a first check: the host decides
 * from the content, so text with an unusual name still goes, and a renamed binary is refused there.
 */
export function refusal(file: { name: string; size: number; type: string }): string | null {
  if (file.size > MAX_SHARED_FILE_BYTES) return `${file.name} is larger than 10 MB.`;
  if (file.size === 0) return `${file.name} is empty.`;
  const type = file.type.toLowerCase();
  const refusedType =
    (type.startsWith('image/') && !IMAGE_TYPES.has(type)) ||
    type.startsWith('audio/') ||
    type.startsWith('video/') ||
    type === 'application/pdf' ||
    type === 'application/zip';
  if (refusedType || REFUSED_EXTENSIONS.test(file.name)) return `${file.name} can't be shared: only ${SHAREABLE.toLowerCase()}.`;
  return null;
}

/** A size as people read it: 18 KB, 2.4 MB. */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1).replace(/\.0$/, '')} MB`;
}

/** An image's bytes as a `data:` URL, which both the app's and the web viewer's pages allow. */
export function dataUrl(mediaType: string, bytes: ArrayBuffer): string {
  const view = new Uint8Array(bytes);
  let binary = '';
  // In pieces: one call with millions of arguments overflows the stack.
  for (let at = 0; at < view.length; at += 0x8000) binary += String.fromCharCode(...view.subarray(at, at + 0x8000));
  return `data:${mediaType};base64,${btoa(binary)}`;
}

/** Text from a shared file's bytes; the host only shares valid UTF-8 as text. */
export function textOf(bytes: ArrayBuffer): string {
  return new TextDecoder().decode(bytes);
}

/** Who shared a file, as this person reads it. */
export function sharedBy(file: Pick<SharedFileInfo, 'by' | 'byLabel'>, me: number | undefined): string {
  return file.by === me ? 'You' : file.byLabel || (file.by === 0 ? 'The host' : 'A viewer');
}

/** A share in the timeline: who shared which file, and when. */
export interface ShareStep {
  file: number;
  name: string;
  by: number;
  byLabel: string;
  at: number;
}

/** The timeline's shares, in order. */
export function shareSteps(events: TimelineEvent[]): ShareStep[] {
  return events.flatMap((event) =>
    event.kind === 'file-shared' ? [{ file: event.file, name: event.name, by: event.by, byLabel: event.byLabel, at: event.at }] : [],
  );
}

/** An item a giving of the braindump wrote, as the timeline step lists it. */
export interface GivenItem {
  file: number;
  /** The name it was written under in shared/braindump/, which may carry a number. */
  name: string;
  path: string;
  /** No longer in the braindump; its copy stays in the session folder. */
  removed: boolean;
}

/** A giving of the braindump in the timeline (`braindump`): when, from whom, and what. */
export interface GiveStep {
  round: number;
  at: number;
  from: string[];
  items: GivenItem[];
}

/** The timeline's givings of the braindump, in order; items not in `present` are marked removed. */
export function giveSteps(events: TimelineEvent[], present: ReadonlySet<number>): GiveStep[] {
  return events.flatMap((event) =>
    event.kind === 'braindump-given'
      ? [
          {
            round: event.round,
            at: event.at,
            from: event.from,
            items: event.files.map((file, i) => {
              const path = event.paths[i] ?? '';
              return { file, name: path.split('/').at(-1) ?? path, path, removed: !present.has(file) };
            }),
          },
        ]
      : [],
  );
}

/** Names as a list people read: "Sanne", "Sanne and Mo", "Sanne, Mo and Nick". */
export function people(names: string[]): string {
  if (names.length <= 1) return names[0] ?? 'nobody';
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
}

/** "3 items from Sanne and Mo", for a giving and the button that starts one. */
export function itemsFrom(count: number, from: string[]): string {
  return `${count} ${count === 1 ? 'item' : 'items'} from ${people(from)}`;
}

/** The prompt the host starts from when giving the braindump; the paths follow it. */
export const DEFAULT_GIVE_PROMPT =
  'Read the files below: notes and files from the people in this session. Turn them into a short list of what they need, and ask me about anything that is unclear or contradicts the plan.';

/** A note's size in bytes of UTF-8, which the host limits to `MAX_NOTE_BYTES`. */
export function noteBytes(text: string): number {
  return new TextEncoder().encode(text.replace(/\r\n/g, '\n')).byteLength;
}

/** For each braindump note that was edited, its newest version, from the timeline's `replaces`. */
export function newestVersions(events: TimelineEvent[]): Map<number, number> {
  const newer = new Map<number, number>();
  for (const event of events) if (event.kind === 'braindump-item' && event.replaces !== undefined) newer.set(event.replaces, event.file);
  const newest = new Map<number, number>();
  for (const old of newer.keys()) {
    let file = old;
    while (newer.has(file)) file = newer.get(file)!;
    newest.set(old, file);
  }
  return newest;
}
