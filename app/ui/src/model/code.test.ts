import { describe, expect, it } from 'vitest';
import { codeCharacters, formatCode, isCompleteCode, isJoinLink, joinLink, serviceName } from './code';

describe('invite codes', () => {
  it('formats in groups of four as the user types', () => {
    expect(formatCode('7k2m')).toBe('7K2M');
    expect(formatCode('7k2m9')).toBe('7K2M-9');
    expect(formatCode('7k2m9qxh4tpa')).toBe('7K2M-9QXH-4TPA');
  });

  it('ignores dashes, spaces and characters that cannot be in a code', () => {
    expect(formatCode(' 7K2M - 9QXH - 4TPA!! ')).toBe('7K2M-9QXH-4TPA');
    expect(codeCharacters('u-u-u')).toBe('');
  });

  it('reads look-alike letters as digits', () => {
    expect(codeCharacters('OIL0')).toBe('0110');
  });

  it('takes the code from a pasted join link', () => {
    expect(formatCode('specdeck://join/7K2M-9QXH-4TPA')).toBe('7K2M-9QXH-4TPA');
    expect(isCompleteCode('Join me: specdeck://join/7k2m9qxh4tpa')).toBe(true);
    // The sharing service after the code is not part of it.
    expect(formatCode('specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fexample.com%2Fsignal')).toBe('7K2M-9QXH-4TPA');
    expect(isJoinLink(' specdeck://join/7K2M-9QXH-4TPA')).toBe(true);
    expect(isJoinLink('7K2M-9QXH-4TPA')).toBe(false);
  });

  it('names a sharing service by its host', () => {
    expect(serviceName('wss://specdeck.example.com/signal')).toBe('specdeck.example.com');
    expect(serviceName('ws://127.0.0.1:18081/signal')).toBe('127.0.0.1:18081');
  });

  it('knows when a code is complete', () => {
    expect(isCompleteCode('7K2M-9QXH-4TP')).toBe(false);
    expect(isCompleteCode('7K2M-9QXH-4TPA')).toBe(true);
  });

  it('makes join links', () => {
    expect(joinLink('7k2m9qxh4tpa', 'wss://specdeck.example.com/signal')).toBe(
      'specdeck://join/7K2M-9QXH-4TPA?server=wss%3A%2F%2Fspecdeck.example.com%2Fsignal',
    );
  });
});

describe('web join links', () => {
  it('are made when the service serves the web viewer, next to its signalling path', () => {
    expect(joinLink('7k2m9qxh4tpa', 'wss://specdeck.example.com/signal', true)).toBe('https://specdeck.example.com/join#7K2M-9QXH-4TPA');
    expect(joinLink('7K2M9QXH4TPA', 'wss://example.com/x/signal', true)).toBe('https://example.com/x/join#7K2M-9QXH-4TPA');
    expect(joinLink('7K2M9QXH4TPA', 'ws://127.0.0.1:18081/signal', true)).toBe('http://127.0.0.1:18081/join#7K2M-9QXH-4TPA');
  });

  it('are read when pasted, code and all', () => {
    expect(isJoinLink('https://specdeck.example.com/join#7K2M-9QXH-4TPA')).toBe(true);
    expect(isJoinLink('https://specdeck.example.com/other')).toBe(false);
    expect(codeCharacters('https://specdeck.example.com/join#7K2M-9QXH-4TPA')).toBe('7K2M9QXH4TPA');
  });
});
