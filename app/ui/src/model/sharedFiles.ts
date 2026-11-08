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
