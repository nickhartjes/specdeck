// Names as others see them (`confine-window-navigation` design decision 5): without characters
// that show as nothing or change the direction of text, which could hide part of a name or reorder
// it. The same characters as `control::invisible` in crates/protocol: Unicode's
// Default_Ignorable_Code_Point and Bidi_Control, with the Hangul fillers. Both are tested against
// crates/protocol/tests/fixtures/shown-names.json. A host of this release already filters the names it
// sends; this covers hosts of older releases, and the name a user sets.

import type { SharedFileInfo, ViewEvent } from '../api/types';

/** The ranges of characters left out, first and last code point. */
const HIDDEN: readonly (readonly [number, number])[] = [
  [0x00ad, 0x00ad], // soft hyphen
  [0x034f, 0x034f], // combining grapheme joiner
  [0x061c, 0x061c], // Arabic letter mark
  [0x115f, 0x1160], // Hangul fillers
  [0x17b4, 0x17b5], // Khmer inherent vowels
  [0x180b, 0x180f], // Mongolian selectors and vowel separator
  [0x200b, 0x200f], // zero-width spaces, joiners, direction marks
  [0x202a, 0x202e], // direction embeddings and overrides
  [0x2060, 0x206f], // word joiner, invisible operators, isolates
  [0x3164, 0x3164], // Hangul filler
  [0xfe00, 0xfe0f], // variation selectors
  [0xfeff, 0xfeff], // zero-width no-break space
  [0xffa0, 0xffa0], // halfwidth Hangul filler
  [0x1bca0, 0x1bca3], // shorthand format controls
  [0x1d173, 0x1d17a], // musical format controls
  [0xe0000, 0xe0fff], // tags and variation selectors supplement
];

/** Whether `c`, one character, is left out of names. */
export function invisible(c: string): boolean {
  const code = c.codePointAt(0) ?? 0;
  return HIDDEN.some(([first, last]) => code >= first && code <= last);
}

/** `name` without invisible and text-direction characters, trimmed; null when nothing is left. */
export function shownName(name: string): string | null {
  const shown = [...name].filter((c) => !invisible(c)).join('').trim();
  return shown === '' ? null : shown;
}

/** A name received from the host as it is shown; one with nothing left shows as `fallback`. */
const shown = (name: string, fallback = '?') => shownName(name) ?? fallback;

const file = (info: SharedFileInfo): SharedFileInfo => ({ ...info, byLabel: shown(info.byLabel) });

/** `event` from the host, with every name in it as it is shown. */
export function shownNames(event: ViewEvent): ViewEvent {
  switch (event.type) {
    case 'participants':
      return { ...event, participants: event.participants.map((p) => ({ ...p, username: shown(p.username), label: shown(p.label) })) };
    case 'shared-files':
      return { ...event, files: event.files.map(file) };
    case 'braindump':
      return { ...event, items: event.items.map((item) => ({ ...item, info: file(item.info) })) };
    case 'timeline': {
      const entry = event.event;
      switch (entry.kind) {
        case 'suggested':
        case 'link':
          return { ...event, event: { ...entry, by: shown(entry.by) } };
        case 'file-shared':
        case 'braindump-item':
          return { ...event, event: { ...entry, byLabel: shown(entry.byLabel) } };
        case 'braindump-given':
          return { ...event, event: { ...entry, from: entry.from.map((name) => shown(name)) } };
        default:
          return event;
      }
    }
    default:
      return event;
  }
}
