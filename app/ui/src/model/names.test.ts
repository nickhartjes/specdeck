import { describe, expect, it } from 'vitest';
import type { ViewEvent } from '../api/types';
import vectorJson from '../../../../crates/protocol/tests/fixtures/shown-names.json?raw';
import { invisible, shownName, shownNames } from './names';

/** The cases `control::shown_name` in crates/protocol is tested against too. */
const vector = JSON.parse(vectorJson) as { hidden: [string, string][]; shown: string[]; names: { name: string; shown: string | null }[] };

describe('the shared cases', () => {
  it('leave out exactly the listed characters', () => {
    const ranges = vector.hidden.map(([first, last]) => [parseInt(first, 16), parseInt(last, 16)] as const);
    for (let code = 0; code <= 0x10ffff; code++) {
      if (code >= 0xd800 && code <= 0xdfff) continue;
      const listed = ranges.some(([first, last]) => code >= first && code <= last);
      if (invisible(String.fromCodePoint(code)) !== listed) expect.fail(`U+${code.toString(16).toUpperCase()}`);
    }
    for (const text of vector.shown) expect([...text].some(invisible)).toBe(false);
  });

  it('give the same names as the host', () => {
    for (const { name, shown } of vector.names) expect(shownName(name), JSON.stringify(name)).toBe(shown);
  });
});

describe('shown names', () => {
  it('drop zero-width and text-direction characters', () => {
    expect(shownName('nick')).toBe('nick');
    expect(shownName('ni\u200Bck')).toBe('nick');
    expect(shownName('nick\u200B')).toBe('nick');
    expect(shownName('\u202Ekcin')).toBe('kcin');
    for (const c of ['\u2066', '\u2067', '\u2068', '\u2069', '\uFEFF']) expect(shownName(`${c}nick${c}`)).toBe('nick');
    expect(shownName(' nick ')).toBe('nick');
  });

  it('are nothing when only such characters are left', () => {
    expect(shownName('\u200B\uFEFF\u2066')).toBeNull();
    expect(shownName('\u200B \u200B')).toBeNull();
  });

  it('are applied to what a host of an older release sends', () => {
    const event: ViewEvent = {
      type: 'participants',
      participants: [
        { id: 0, username: 'nick\u2066', role: 'Host', label: 'nick\u2066' },
        { id: 1, username: '\u202Eanna', role: 'Viewer', label: '\u202Eanna' },
      ],
    };
    const filtered = shownNames(event);
    expect(filtered.type === 'participants' && filtered.participants.map((p) => [p.username, p.label])).toEqual([
      ['nick', 'nick'],
      ['anna', 'anna'],
    ]);
    const link = shownNames({ type: 'timeline', event: { outputSeq: 0, at: 0, kind: 'link', id: 1, url: null, by: 'an\u200Bna' } });
    expect(link.type === 'timeline' && link.event.kind === 'link' && link.event.by).toBe('anna');
  });
});
