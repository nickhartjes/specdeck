# Proposal

## Why

Pressing Shift+Enter in the host's terminal sends the prompt to the agent instead of starting a new line. xterm.js 6.0.0 sends `\r` for Enter and for Shift+Enter alike (`Keyboard.ts`: only Alt changes it), and it does not support the kitty keyboard protocol that agents use to tell the two apart. So every agent receives a plain Enter. Shift+Enter is the newline key hosts expect from Claude Code and the other agents in their own terminals, and a prompt sent too early cannot be taken back.

## What Changes

- In the host's terminal, Shift+Enter sends a line feed (`\n`, the same byte as Ctrl+J) instead of `\r`. Tested on 2026-11-03 in legacy keyboard mode, as SpecDeck runs them: Claude Code, Codex, pi, omp, OpenCode and Grok all insert a new line on `\n` without sending the prompt. The alternatives were ruled out: `ESC CR` sends the prompt in pi, and `CSI 13;2u` shows up as text in OpenCode.
- Enter, Alt/Option+Enter and Ctrl+J behave as before. Shift+Enter while an input method is still composing text is left to the input method.
- In a plain shell, Shift+Enter still runs the command, as it does today: the shell treats `\n` like Enter.
- The keyboard shortcuts dialog and the Keyboard and screen readers page say that Shift+Enter starts a new line in the host's own terminal.
- Not in this change: the kitty keyboard protocol, and a choice of newline key per agent.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-hosting`: "Host owns input" gains a scenario that Shift+Enter in the host's terminal starts a new line in the agent's prompt instead of sending it.

## Impact

- `app/ui/src/terminal/TerminalView.tsx`: the custom key handler sends `\n` for Shift+Enter in host mode.
- `app/ui/src/components/ShortcutsDialog.tsx` and `docs/site/src/content/docs/guide/keyboard.md`: one new row each.
- No change to the protocol, the Rust side, settings or viewers: a viewer's terminal sends nothing to the session.
- Windows has not been tested yet: input goes through ConPTY there, so it needs a manual check.
