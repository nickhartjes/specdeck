import { describe, expect, it } from 'vitest';
import { remeasureOnSymbolFont, type FontLoadEvents } from './symbolFont';

type Listener = Parameters<FontLoadEvents['addEventListener']>[1];

function fakeFonts() {
  const listeners = new Set<Listener>();
  const fonts: FontLoadEvents = {
    addEventListener: (_type, listener) => listeners.add(listener),
    removeEventListener: (_type, listener) => listeners.delete(listener),
  };
  const loaded = (...families: string[]) => listeners.forEach((l) => l({ fontfaces: families.map((family) => ({ family })) }));
  return { fonts, loaded, listeners };
}

const STACK = 'ui-monospace, "DejaVu Sans Mono", "Symbols Nerd Font Mono", monospace';

describe('measuring again when the symbol font loads', () => {
  it('changes the family when the symbol font has loaded', () => {
    const { fonts, loaded } = fakeFonts();
    const options = { fontFamily: STACK };
    remeasureOnSymbolFont(fonts, options);
    loaded('"Symbols Nerd Font Mono"');
    expect(options.fontFamily).not.toBe(STACK);
    expect(options.fontFamily.trim()).toBe(STACK);
  });

  it('ignores other fonts', () => {
    const { fonts, loaded } = fakeFonts();
    const options = { fontFamily: STACK };
    remeasureOnSymbolFont(fonts, options);
    loaded('Inter', 'Symbols Nerd Font');
    expect(options.fontFamily).toBe(STACK);
  });

  it('changes the family again on every load, between two spellings of the same stack', () => {
    const { fonts, loaded } = fakeFonts();
    const options = { fontFamily: STACK };
    remeasureOnSymbolFont(fonts, options);
    loaded('Symbols Nerd Font Mono');
    const first = options.fontFamily;
    loaded('Symbols Nerd Font Mono');
    expect(options.fontFamily).not.toBe(first);
    expect(options.fontFamily).toBe(STACK);
  });

  it('stops listening when disposed', () => {
    const { fonts, loaded, listeners } = fakeFonts();
    const options = { fontFamily: STACK };
    const dispose = remeasureOnSymbolFont(fonts, options);
    dispose();
    loaded('Symbols Nerd Font Mono');
    expect(options.fontFamily).toBe(STACK);
    expect(listeners.size).toBe(0);
  });

  it('does nothing without a font set', () => {
    const options = { fontFamily: STACK };
    expect(() => remeasureOnSymbolFont(undefined, options)()).not.toThrow();
    expect(options.fontFamily).toBe(STACK);
  });
});
