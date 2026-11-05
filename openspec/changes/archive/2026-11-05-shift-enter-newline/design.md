# Design

## Context

See proposal.md for motivation, and the `session-hosting` delta for the behavior.

What is there today:

- `TerminalView.tsx` passes everything xterm emits through `term.onData` to the pty, in host mode only. Keys the app handles itself go through `term.attachCustomKeyEventHandler` (F6, Tab in a viewer, Ctrl+C / Ctrl+Shift+C / Ctrl+Shift+V off macOS).
- xterm.js 6.0.0 turns Enter into `\r` and Alt+Enter into `ESC CR`; Shift is ignored (`common/input/Keyboard.ts`). It has no kitty keyboard protocol: it ignores `CSI > flags u` and never answers `CSI ? u`. So agents that ask (Claude Code, Codex, pi, omp and Grok all send `CSI ? u`) fall back to legacy input.
- xterm calls the custom key handler for keydown and again for keypress (`CoreBrowserTerminal._keyPress`). A keypress the handler does not stop is turned into input from its `charCode`, which for Enter is `\r` again.
- What each agent does with each candidate byte sequence in legacy mode (tested on 2026-11-03, macOS, in a pty with network blocked; Grok checked by hand with Ctrl+J in SpecDeck):

  | Agent | `ESC CR` | `\n` | `CSI 13;2u` | `CSI 27;2;13~` |
  |---|---|---|---|---|
  | Claude Code 2.1.311 | newline | newline | newline | newline |
  | Codex 0.166.0 | newline | newline | newline | ignored |
  | pi 1.4.0 | sends | newline | newline | newline |
  | omp 18.15.0 | newline | newline | newline | newline |
  | OpenCode | newline | newline | shown as text | shown as text |
  | Grok 1.0.70 | not tested | newline | not tested | not tested |

## Goals / Non-Goals

**Goals:**

- Shift+Enter in the host's terminal reaches the agent as a newline, the same for every supported agent.

**Non-Goals:**

- The kitty keyboard protocol (answering `CSI ? u`, keeping the flag stack, encoding keys as `CSI u`). It would change how every key reaches agents that turn it on, and the table shows it gains nothing for Shift+Enter.
- A newline key per agent, or a setting. One byte works for all of them.
- Other modified Enter keys (Ctrl+Enter, Ctrl+Shift+Enter): they keep xterm's behavior.

## Decisions

### 1. Shift+Enter sends `\n`

It is the only sequence that every agent in the table treats as a newline. It is also the byte Ctrl+J already sends, so nothing new reaches the agents. In a shell, `\n` runs the command like `\r`, so Shift+Enter in a Terminal session keeps working as it does today.

- **Alternative: `ESC CR`** (what Alt/Option+Enter sends, and what Claude Code's `/terminal-setup` binds). pi sends the prompt on it.
- **Alternative: `CSI 13;2u`** (kitty's Shift+Enter). OpenCode shows it as text, and sending it without having answered the protocol query is not what the protocol describes.
- **Alternative: `CSI 27;2;13~`** (xterm's modifyOtherKeys). Codex ignores it, OpenCode shows it as text.

### 2. Handled in the custom key handler, through `term.input`

For a keydown with `key === 'Enter'`, Shift held, no Ctrl, Alt or Meta, not composing (`isComposing`), in host mode: the handler calls `preventDefault()` and `term.input('\n')`, and returns `false`. For the keypress of the same key it returns `false` as well, so xterm's keypress path cannot send `\r` after the newline. `term.input` with user input on goes through xterm's own input path, so `onData` fires as for any typed key, the view scrolls to the bottom and the selection clears. Calling `onData` directly would skip those.

The test (Shift, Enter, no other modifier, not composing) goes in a small pure function next to the terminal (`terminal/keys.ts`), so it can be unit tested without xterm, which needs a canvas.

## Risks / Trade-offs

- [An agent added later, or a new version of one, may treat `\n` as Enter] → The spec scenario names the agents, and the manual check in tasks covers each. Revisit the kitty protocol if an agent needs it.
- [Windows: input goes through ConPTY, which turns VT input into key events, so `\n` may reach the agent as Ctrl+J or as Enter] → A manual check on Windows in tasks. If it sends there, the fix needs a different path on Windows, and the change waits for that.
- [Hosts who used Shift+Enter in a shell to run a command] → Unchanged: the shell treats `\n` like Enter.

## Migration Plan

Nothing to migrate. Rolling back restores `\r` for Shift+Enter.
