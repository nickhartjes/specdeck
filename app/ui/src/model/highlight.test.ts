import { describe, expect, it } from 'vitest';
import { MAX_HIGHLIGHTED_LINES, languageOf, tokenizeLines, type Span } from './highlight';

const joined = (lines: Span[][]) => lines.map((line) => line.map((span) => span.text).join('')).join('\n');
const kinds = (lines: Span[][]) => new Set(lines.flat().map((span) => span.kind));

const SAMPLES: Record<string, string> = {
  rust: 'fn main() {\n    let x = "hi"; // note\n}',
  typescript: 'const a: number = 1; // n',
  tsx: 'export const A = () => <div className="a">{1}</div>;',
  javascript: 'function a() { return "b"; }',
  jsx: 'const a = <b c="d" />;',
  json: '{ "a": [1, true, null] }',
  toml: '[package]\nname = "specdeck" # n',
  yaml: 'name: specdeck\nlist:\n  - 1 # n',
  python: 'def a():\n    return "b"  # n',
  go: 'func main() { fmt.Println("a") }',
  css: '.a { color: red; }',
  markup: '<a href="b">c</a>',
  bash: 'echo "a" # n',
  sql: 'SELECT a FROM b WHERE c = 1;',
  markdown: '# Title\n\nSome `code` and **bold**.',
  diff: '+added\n-removed',
};

describe('languageOf', () => {
  it('picks the language by extension', () => {
    expect(languageOf('crates/core/src/session.rs')).toBe('rust');
    expect(languageOf('app/ui/src/App.TSX')).toBe('tsx');
    expect(languageOf('openspec/changes/x/design.md')).toBe('markdown');
    expect(languageOf('C:\\work\\a.py')).toBe('python');
  });

  it('knows no language for other files', () => {
    expect(languageOf('Justfile')).toBeNull();
    expect(languageOf('.gitignore')).toBeNull();
    expect(languageOf('a.unknown')).toBeNull();
  });
});

describe('tokenizeLines', () => {
  it.each(Object.entries(SAMPLES))('highlights %s and keeps the text', (language, text) => {
    const lines = tokenizeLines(text, language);
    expect(joined(lines)).toBe(text);
    expect([...kinds(lines)].some((kind) => kind !== null)).toBe(true);
  });

  it('colours both lines of a multi-line comment', () => {
    const lines = tokenizeLines('/* one\n   two */\nfn a() {}', 'rust');
    expect(lines[0]).toEqual([{ text: '/* one', kind: 'com' }]);
    expect(lines[1]).toEqual([{ text: '   two */', kind: 'com' }]);
    expect(lines[2]!.some((span) => span.kind === 'kw')).toBe(true);
  });

  it('shows an unknown language plain, one span per line', () => {
    expect(tokenizeLines('a\n\nb', null)).toEqual([[{ text: 'a', kind: null }], [], [{ text: 'b', kind: null }]]);
  });

  it('drops carriage returns', () => {
    expect(joined(tokenizeLines('a\r\nb', 'rust'))).toBe('a\nb');
  });

  it('keeps markup in a file as text', () => {
    const text = '<script>alert(1)</script>';
    expect(joined(tokenizeLines(text, 'markup'))).toBe(text);
  });

  it('shows a very long file plain', () => {
    const text = 'let a = 1;\n'.repeat(MAX_HIGHLIGHTED_LINES);
    const lines = tokenizeLines(text, 'rust');
    expect(lines.length).toBe(MAX_HIGHLIGHTED_LINES + 1);
    expect(kinds(lines)).toEqual(new Set([null]));
  });
});
