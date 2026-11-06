// The bundled Nerd Font symbols (styles.css) load only once a symbol is on screen, so xterm has
// already measured that symbol with a fallback font. Its DOM renderer keeps characters on the grid
// with letter-spacing from those measurements and caches them until the font settings change, so
// with the stale width every icon pushes the rest of its line out of its columns.

export const SYMBOL_FONT = 'Symbols Nerd Font Mono';

/** The part of a `FontFaceSet` this needs: its `loadingdone` event. */
export interface FontLoadEvents {
  addEventListener(type: 'loadingdone', listener: (event: { fontfaces: readonly { family: string }[] }) => void): void;
  removeEventListener(type: 'loadingdone', listener: (event: { fontfaces: readonly { family: string }[] }) => void): void;
}

/**
 * Makes the terminal measure its characters again when the symbol font finishes loading. Setting
 * the same family again does nothing in xterm, so the family switches between two spellings of the
 * same stack, the second with a trailing space. Returns a function that stops listening.
 */
export function remeasureOnSymbolFont(fonts: FontLoadEvents | undefined, options: { fontFamily?: string }): () => void {
  if (!fonts) return () => {};
  const listener = (event: { fontfaces: readonly { family: string }[] }) => {
    if (!event.fontfaces.some((face) => face.family.replace(/["']/g, '').trim() === SYMBOL_FONT)) return;
    const family = options.fontFamily ?? '';
    options.fontFamily = family.endsWith(' ') ? family.trimEnd() : `${family} `;
  };
  fonts.addEventListener('loadingdone', listener);
  return () => fonts.removeEventListener('loadingdone', listener);
}
