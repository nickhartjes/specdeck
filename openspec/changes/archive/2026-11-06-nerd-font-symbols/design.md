# Design

## Context

See proposal.md for why. Facts that shape the approach:

- One UI bundle, two builds. `app/ui/src/styles.css` is imported by both `src/main.tsx` (Tauri app, `vite.config.ts` → `dist`) and `src/web/main.tsx` (web viewer, `vite.web.config.ts` → `dist-web`, `base: './'`, embedded into the sharing service by `crates/signalling/src/web.rs`). A CSS-referenced asset ends up in both.
- The terminal takes its font from the `--mono` variable (`TerminalView.tsx` reads it into xterm's `fontFamily`). It uses xterm's default DOM renderer: `@xterm/addon-webgl` is a dependency but is not loaded anywhere. Text in each row is ordinary HTML, so the browser's per-glyph font fallback applies.
- Content security: the Tauri CSP has no `font-src`, so `default-src 'self'` covers fonts; the web viewer sends `font-src 'self'`. The `web-viewer` spec forbids requests to any other address.

## Goals / Non-Goals

**Goals:**
- Same symbols on every platform and in every browser, from one bundled file.
- No visible change to ordinary text, and no download when no symbol is shown.

**Non-Goals:**
- A full Nerd-Font-patched monospace font for all terminal text.
- A font setting, or using whichever Nerd Font the user has installed in preference to the bundled one.
- Changing the terminal renderer. Whether to remove the unused `@xterm/addon-webgl` dependency is a separate question.

## Decisions

**1. Bundle "Symbols Nerd Font Mono", not a patched monospace font.**
The symbols-only font adds icons and nothing else, so the platform's monospace font still draws all ordinary text (`native-experience`: system fonts are the platform's own). In the *Mono* variant every icon has the same advance, one em; the non-Mono variant's advances vary. One em is still about 1.66 cells of a typical monospace font (see decision 7). Alternative: bundle e.g. JetBrainsMono Nerd Font as the terminal font. Its metrics would match exactly, but it would replace the native look, add several weights, and contradict the spec.

**2. Fallback position plus `unicode-range`.**
The family goes last in `--mono`, before the generic `monospace`, and its `@font-face` declares `unicode-range: U+E000-F8FF, U+F0000-FFFFD`. The browser reaches it only for code points that earlier fonts lack, and fetches the file only when such a code point is on the page. Alternative: no `unicode-range`. That is simpler, but every page would download about 1 MB on load.

**3. `src: local(...), url(...)`.**
`local("Symbols Nerd Font Mono")` first lets a machine that has the font installed skip the download. The bundled `url()` is the guarantee. The file sits in `app/ui/src/assets/fonts/` and is referenced from CSS with a relative URL, so Vite hashes it and emits it into both `dist` and `dist-web`. It is larger than Vite's inline limit, so it is never turned into a `data:` URL. A `public/` directory was not chosen: the project has none, and hashed names cache better.

**4. `font-display: block`.**
While the font loads, symbols stay invisible for a moment instead of flashing as boxes. Only symbol characters are affected, because ordinary text uses another face.

**5. The terminal measures again when the symbol font arrives.**
xterm's DOM renderer keeps each character on the grid by measuring how wide the font draws it and giving its span a `letter-spacing` of the cell width minus that width. It caches each measurement, and clears the cache only when the font settings change. Because of `unicode-range`, the symbol font loads only after the first symbol is written, so xterm has already measured that symbol with the fallback font. With the stale width, every icon pushed the rest of its line about two-thirds of a cell to the right; three icons put the last character two columns late (measured in Chromium with xterm 6.0.0). So `TerminalView` listens for `document.fonts` `loadingdone`, and when the symbol face is among the loaded faces it re-applies `fontFamily`, which makes xterm measure again. The terminal switches between two equivalent spellings of the `--mono` stack, one with a trailing space, because re-applying an identical string does nothing. Switching to another font and back would also measure again, but could briefly change the cell size. Alternative: wait for `document.fonts.load(...)` before opening every terminal. That works, but every page would download the font even when no symbol is ever shown. The WebGL renderer, if it is ever enabled, also caches glyph images, and would need `clearTextureAtlas()` at the same moment.

**6. Source and license.**
Use `SymbolsNerdFontMono-Regular.ttf` from the Nerd Fonts `NerdFontsSymbolsOnly` release, converted to WOFF2. Nixpkgs has `nerd-fonts.symbols-only` at 3.5.0, a convenient pinned source. Commit the release's license text next to the font, and record the release version.

**7. Icons scaled to one cell with `size-adjust: 60%`.**
At 1 em an icon is about 1.66 cells wide. After decision 5 it no longer moves later characters, but it is drawn over its right-hand neighbour. `size-adjust: 60%` on the `@font-face` scales the symbol font so an icon is about one cell wide. 0.6 em is the cell of most monospace fonts (DejaVu Sans Mono and Menlo 0.602, SF Mono about 0.6). The rest of any difference is taken up by xterm's `letter-spacing` (decision 5), so the grid stays exact even where the cell is narrower, e.g. Ubuntu Mono at 0.5 em. This matches how terminals size icons when they fall back to "Symbols Nerd Font Mono". Alternative: keep 1 em icons. They are larger and match the non-Mono Nerd Fonts most terminal users have, but they overlap the next character. Chosen by the user after comparing screenshots of both.

## Risks / Trade-offs

- [The icons' look does not match every platform font exactly (baseline, weight)] → Accepted. Alignment comes from xterm's measurement (decision 5), not from the font matching.
- [Scaled to 60%, the icons are small, and Powerline separators (U+E0B0–E0B3) cover only part of the row's height instead of all of it, as in terminals that draw them specially] → Accepted for now. Prompts that use separators show small triangles. Drawing separators ourselves would need the WebGL renderer's custom glyphs, which the DOM renderer lacks.
- [Re-applying `fontFamily` depends on xterm clearing its width cache when the font settings change, which is internal behaviour] → A test pins the event handling. If an xterm upgrade changes this, the visible symptom is icons pushing text right, and the check in the tasks catches it.
- [The bundle grows by about 1 MB (installer, sharing-service binary)] → Within the 25 MB installer limit. Measure the size in the tasks.
- [Glyph licensing: Nerd Fonts gathers icons from several sets] → Ship the license file from the release unchanged, and check that it covers redistribution in an app before merging.
- [A user's older installed copy wins through `local()` and lacks newer icons] → Rare and harmless (the same boxes as today). If it causes trouble, drop `local()`.
- [Code points of the deprecated Material Design icon range (U+F500–FD46) were reassigned in Nerd Fonts 3] → Programs written for v2 may show different icons. That is outside our control, and terminals using v3 fonts behave the same way.
