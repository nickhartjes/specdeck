// Syntax highlighting as data: a file's text split into lines of coloured spans, which React
// renders as text (`files-documents-and-code` design decision 8). Nothing here touches the DOM.

import type { Token } from 'prismjs/components/prism-core';
import Prism from '../highlight/languages';

/** The colour of a span, as a short class suffix (`tok-…`); null for plain text. */
export type Kind = 'kw' | 'str' | 'com' | 'num' | 'fn' | 'type' | 'tag' | 'attr' | 'punct' | 'op' | 'const' | 'prop' | 'title' | 'var' | 'ins' | 'del';

export interface Span {
  text: string;
  kind: Kind | null;
}

/** Longer files are shown plain, to keep the window responsive. */
export const MAX_HIGHLIGHTED_LINES = 50_000;

const BY_EXTENSION: Record<string, string> = {
  rs: 'rust',
  ts: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  jsx: 'jsx',
  json: 'json',
  toml: 'toml',
  yaml: 'yaml',
  yml: 'yaml',
  py: 'python',
  go: 'go',
  css: 'css',
  html: 'markup',
  htm: 'markup',
  xml: 'markup',
  svg: 'markup',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  sql: 'sql',
  md: 'markdown',
  markdown: 'markdown',
  diff: 'diff',
  patch: 'diff',
};

/** The language a file is highlighted as, by its extension; null for any other file. */
export function languageOf(path: string): string | null {
  const name = path.split(/[\\/]/).at(-1) ?? path;
  const dot = name.lastIndexOf('.');
  if (dot <= 0) return null;
  return BY_EXTENSION[name.slice(dot + 1).toLowerCase()] ?? null;
}

const KINDS: Record<string, Kind> = {
  keyword: 'kw',
  'control-flow': 'kw',
  important: 'kw',
  atrule: 'kw',
  string: 'str',
  char: 'str',
  'template-string': 'str',
  'attr-value': 'str',
  regex: 'str',
  url: 'str',
  comment: 'com',
  prolog: 'com',
  doctype: 'com',
  cdata: 'com',
  number: 'num',
  function: 'fn',
  'function-definition': 'fn',
  macro: 'fn',
  'class-name': 'type',
  builtin: 'type',
  namespace: 'type',
  'type-definition': 'type',
  tag: 'tag',
  selector: 'tag',
  'attr-name': 'attr',
  attribute: 'attr',
  decorator: 'attr',
  'lifetime-annotation': 'attr',
  punctuation: 'punct',
  operator: 'op',
  boolean: 'const',
  constant: 'const',
  symbol: 'const',
  property: 'prop',
  key: 'prop',
  title: 'title',
  variable: 'var',
  parameter: 'var',
  inserted: 'ins',
  'inserted-sign': 'ins',
  deleted: 'del',
  'deleted-sign': 'del',
};

function kindOf(token: Token): Kind | null {
  const aliases = token.alias === undefined ? [] : Array.isArray(token.alias) ? token.alias : [token.alias];
  for (const name of [token.type, ...aliases]) {
    const kind = KINDS[name];
    if (kind) return kind;
  }
  return null;
}

/** Flattens Prism's token tree into spans; an inner token's colour wins over its outer one's. */
function flatten(content: string | Token | (string | Token)[], outer: Kind | null, out: Span[]) {
  if (typeof content === 'string') {
    if (content) out.push({ text: content, kind: outer });
  } else if (Array.isArray(content)) {
    for (const part of content) flatten(part, outer, out);
  } else {
    flatten(content.content, kindOf(content) ?? outer, out);
  }
}

/** Splits spans at line breaks into lines of spans. */
function intoLines(spans: Span[]): Span[][] {
  const lines: Span[][] = [[]];
  for (const span of spans) {
    const parts = span.text.split('\n');
    parts.forEach((part, i) => {
      if (i > 0) lines.push([]);
      if (part) lines.at(-1)!.push({ text: part, kind: span.kind });
    });
  }
  return lines;
}

/**
 * `text` as lines of spans, highlighted when `language` is known; plain, one span per line,
 * otherwise or past {@link MAX_HIGHLIGHTED_LINES}. Joining every line's spans with `\n` gives
 * `text` back, without its carriage returns.
 */
export function tokenizeLines(text: string, language: string | null): Span[][] {
  const normalized = text.replace(/\r\n?/g, '\n');
  const grammar = language ? Prism.languages[language] : undefined;
  const plain = () => normalized.split('\n').map((line) => (line ? [{ text: line, kind: null }] : []));
  if (!grammar) return plain();
  if (normalized.split('\n', MAX_HIGHLIGHTED_LINES + 1).length > MAX_HIGHLIGHTED_LINES) return plain();
  const spans: Span[] = [];
  flatten(Prism.tokenize(normalized, grammar), null, spans);
  return intoLines(spans);
}

/**
 * Each line of a diff hunk (`+`, `-` or ` ` first), highlighted. The new side (context and
 * additions) and the old side (context and removals) are each highlighted as one text, so a
 * construct spanning lines is mostly right; one that starts before the hunk may be coloured
 * wrongly, never shown wrongly.
 */
export function highlightHunk(lines: string[], language: string | null): Span[][] {
  const sign = (line: string) => line[0] ?? ' ';
  const after = tokenizeLines(lines.filter((l) => sign(l) === ' ' || sign(l) === '+').map((l) => l.slice(1)).join('\n'), language);
  const before = tokenizeLines(lines.filter((l) => sign(l) === ' ' || sign(l) === '-').map((l) => l.slice(1)).join('\n'), language);
  let a = 0;
  let b = 0;
  return lines.map((line) => {
    switch (sign(line)) {
      case '+':
        return after[a++] ?? [];
      case '-':
        return before[b++] ?? [];
      case ' ':
        b++;
        return after[a++] ?? [];
      default:
        return [{ text: line.slice(1), kind: null }];
    }
  });
}
