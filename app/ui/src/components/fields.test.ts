import { describe, expect, it } from 'vitest';

// Every component's source, to find the text fields in it.
const sources = import.meta.glob<string>(['../**/*.tsx', '!../**/*.test.tsx'], { query: '?raw', import: 'default', eager: true });

/** Input types that take no typed text. */
const NOT_TEXT = /\btype="(checkbox|radio|file|range|color|hidden)"/;

/** The opening tags of `<input>` and `<textarea>` elements in `source`, braces kept whole. */
function fields(source: string): string[] {
  const tags: string[] = [];
  for (const match of source.matchAll(/<(input|textarea)\b/g)) {
    let depth = 0;
    let end = match.index;
    for (; end < source.length; end++) {
      const char = source[end];
      if (char === '{') depth++;
      else if (char === '}') depth--;
      else if (char === '>' && depth === 0) break;
    }
    tags.push(source.slice(match.index, end + 1));
  }
  return tags;
}

describe('text fields', () => {
  it('are found in the components', () => {
    const text = Object.values(sources)
      .flatMap(fields)
      .filter((tag) => !NOT_TEXT.test(tag));
    expect(text.length).toBeGreaterThanOrEqual(17);
  });

  it('all turn off smart dashes and quotes (#85)', () => {
    const missing = Object.entries(sources).flatMap(([path, source]) =>
      fields(source)
        .filter((tag) => !NOT_TEXT.test(tag) && !tag.includes('{...PLAIN_TEXT}'))
        .map((tag) => `${path}: ${tag.split('\n').slice(0, 3).join(' ').replace(/\s+/g, ' ')}`),
    );
    expect(missing).toEqual([]);
  });
});
