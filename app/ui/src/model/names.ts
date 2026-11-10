// Names as others see them (`confine-window-navigation` design decision 5): without zero-width
// and text-direction characters, which could hide or reorder a name. The same characters as
// `control::invisible` in crates/protocol. A host of this release already filters the names it
// sends; this covers hosts of older releases, and the name a user sets.

import type { SharedFileInfo, ViewEvent } from '../api/types';

const INVISIBLE = /[\u200B-\u200F\u202A-\u202E\u2060-\u2064\u2066-\u2069\uFEFF]/g;

/** `name` without invisible and text-direction characters, trimmed; null when nothing is left. */
export function shownName(name: string): string | null {
  const shown = name.replace(INVISIBLE, '').trim();
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
