# Tasks

## 1. Shift+Enter in the host's terminal

- [x] 1.1 Add `app/ui/src/terminal/keys.ts` with a pure function that tells whether a keyboard event is a newline key: `key === 'Enter'`, Shift held, no Ctrl, Alt or Meta, and not `isComposing`. Verify with `app/ui/src/terminal/keys.test.ts` that Shift+Enter matches and that Enter, Alt+Enter, Ctrl+Enter, Ctrl+Shift+Enter, Meta+Shift+Enter and a composing Shift+Enter do not.
- [x] 1.2 In `attachCustomKeyEventHandler` in `app/ui/src/terminal/TerminalView.tsx`, in host mode only: on a newline key's keydown call `event.preventDefault()` and `term.input('\n')`, and return `false` for that key's keydown and keypress, as design decision 2 describes. Extend the comment above the handler with one line saying why. Verify `pnpm typecheck` and `pnpm test` pass in `app/ui`.
- [x] 1.3 Add a row "New line in your terminal's prompt", `⇧Return` (macOS) and `Shift+Enter` (others), to `MAC` and `OTHERS` in `app/ui/src/components/ShortcutsDialog.tsx`. Add a row to the table in `docs/site/src/content/docs/guide/keyboard.md`: "Start a new line in the agent's prompt, in your own terminal | **Shift+Enter**". Verify `pnpm test` passes and `just docs-build` succeeds.

## 2. Check with the agents

- [x] 2.1 In `just dev` on macOS, host a session with each of Claude Code, Codex, pi, omp, OpenCode and Grok. Type `alpha`, press Shift+Enter, type `beta`, then Enter. Verify that `beta` lands on a new line in the prompt and that Enter sends both lines as one prompt. In a Terminal session, verify that Shift+Enter still runs the command. Record the results in the PR.
- [x] 2.2 Moved to `release-and-verification` 5.13: Do the same check on Windows with at least Claude Code and one other agent. If Shift+Enter sends the prompt there, stop and revise the design before merging. Record the result in the PR.
- [x] 2.3 Run `just check` and verify it passes.
