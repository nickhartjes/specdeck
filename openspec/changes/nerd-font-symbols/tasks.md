# Tasks

## 1. The font asset

- [x] 1.1 Get `SymbolsNerdFontMono-Regular.ttf` from the Nerd Fonts `NerdFontsSymbolsOnly` release (design decision 6; nixpkgs `nerd-fonts.symbols-only` 3.5.0 is a pinned source). Convert it to WOFF2 (`fonttools ttLib.woff2 compress` or `woff2_compress`) and save it as `app/ui/src/assets/fonts/SymbolsNerdFontMono-Regular.woff2`. Verify the file opens as a font (`fc-scan` on the TTF lists family "Symbols Nerd Font Mono") and record its size. (Done: v3.5.0, TTF identical to nixpkgs; WOFF2 1,202,596 bytes.)
- [x] 1.2 Add the release's license text as `app/ui/src/assets/fonts/LICENSE` with a first line naming the source and version (e.g. "Symbols Nerd Font Mono, Nerd Fonts v3.5.0"). Read it and confirm that it allows bundling in an app. If it does not, stop and raise it before continuing.

## 2. Use it in the UI

- [x] 2.1 In `app/ui/src/styles.css`, add the `@font-face` from design decisions 2–4 (family "Symbols Nerd Font Mono", `local()` then a relative `url()` to the asset, `format("woff2")`, `font-display: block`, `unicode-range: U+E000-F8FF, U+F0000-FFFFD`), with a short comment saying why it exists and that the WebGL renderer, if it is ever enabled, would need to wait for the font (design decision 5). Append `"Symbols Nerd Font Mono"` to `--mono` just before `monospace`. Verify that `pnpm typecheck`, `pnpm lint` and `pnpm test` pass.
- [x] 2.2 Build both bundles (`pnpm --dir app/ui build` and `just check-web`) and verify that `app/ui/dist/assets` and `app/ui/dist-web/assets` each contain one hashed `SymbolsNerdFontMono-*.woff2`, that the built CSS references it by a relative path in `dist-web`, and that it is not inlined as a `data:` URL.
- [x] 2.3 Make the terminal measure again when the symbol font loads (design decision 5). Add `app/ui/src/terminal/symbolFont.ts` with a function that takes a `FontFaceSet`-like event target and the terminal's options. On `loadingdone` with a "Symbols Nerd Font Mono" face, it switches `fontFamily` between the stack and the stack plus a trailing space, and it returns a disposer. Call it from `TerminalView`'s setup and dispose it on cleanup; it does nothing where `document.fonts` is missing (jsdom). Verify with `symbolFont.test.ts`: the family changes on a matching `loadingdone`, not on other faces, alternates on repeated loads, and stops after dispose. Then run `pnpm typecheck`, `pnpm lint` and `pnpm test`.
- [x] 2.4 Add `size-adjust: 60%` to the `@font-face` (design decision 7) and say why in its comment. Verify with the headless check from 3.1's setup (real xterm, the built CSS, fonts limited to DejaVu): each icon is at most one cell wide, and `b`, `c` and `d` in `a\ue0a0b \uf07c c \ue0b0 d` sit in columns 2, 6 and 10 to within 0.5 px.

## 3. Check it end to end

- [x] 3.1 Browser viewer, on a machine or browser profile with no Nerd Font installed: host a session with `just dev`, join through `just local-dev`, and in the host's terminal run `printf 'a\ue0a0b \uf07c c \ue0b0 d\n'` (bash). Verify in the browser (Playwright is fine) that:
  - all three symbols are drawn, not as boxes;
  - `b`, `c` and `d` stay in their columns, so each symbol takes one cell;
  - the font request goes to the sharing service only, and passes its CSP with no console violation.
  - Done 2026-11-06 against a real session (starship prompt, fastfetch), headless Chromium with DejaVu only: 15 rows with 30+ icons incl. U+F0054, worst column offset 0.00 px; font only from the service, no CSP violations.
- [x] 3.2 Repeat 3.1 in a fresh browser page for a session that prints no symbols, and verify that the network log shows no request for the woff2 (spec scenario "No symbols, no font download").
  - Done 2026-11-06: host with a plain bash prompt, browser viewer joined with a fresh profile and DejaVu only: 0 font requests and the face "unloaded" while no symbol was shown. The host then printed the test line: the font loaded from the sharing service only, after the terminal had measured, and `b`, `c` and `d` sat in columns 2, 6 and 10 with a worst offset of 0.00 px (the re-measure case of design decision 5). No CSP violations.
- [x] 3.3 In the desktop app (host and app viewer), print the same line and verify the symbols draw and keep the grid. Verify that ordinary text and box-drawing characters look as before, using a screenshot before and after (spec scenario "Ordinary text keeps the platform font"). Check on Linux here and on macOS and Windows when the release verification runs.
  - 2026-11-06, Linux (WebKitGTK), host window with starship prompt and fastfetch: icons draw; with `main`'s styles.css swapped in through Vite they were empty boxes even with FiraCode Nerd Font installed. Rows without icons were pixel-identical between main's CSS and the new CSS (control: 0 differing pixels); text after icons kept its columns. The desktop viewer window, joined to the same session, shows the same icons in the same columns as the host (prompt, fastfetch info column, identical wrapping). macOS and Windows moved to `release-and-verification` task 5.17, which checks the packaged app per platform.
- [x] 3.4 Note the installer size change from 1.1 against the 25 MB limit in the PR description.
  - Done in PR #35: +1,202,596 bytes.

## Workflow follow-up

- Archive the change once it is merged, so the `native-experience` spec gains the new requirement and the clarified appearance requirement.
