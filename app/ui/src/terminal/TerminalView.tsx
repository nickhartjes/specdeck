import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react';
import { Terminal, type ITheme } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';
import type { TerminalSnapshot } from '../api/types';
import { TerminalReporter } from '../model/terminalState';
import type { TerminalStep } from './stream';
import { clampViewerFont, viewerFontSize } from './fontScale';
import { isNewlineKey } from './keys';
import { isMac } from '../menu/menu';

export interface SearchResult {
  index: number;
  count: number;
}

export interface TerminalHandle {
  /** Applies records in order; each step is one record of the session's output log. */
  write(steps: TerminalStep[]): void;
  /** Clears everything, before a replay from the start. */
  reset(): void;
  findNext(term: string): void;
  findPrevious(term: string): void;
  clearSearch(): void;
  /** Scrolls to where the output stood when record `seq` was the newest. */
  scrollToSeq(seq: number): void;
  scrollToBottom(): void;
  focus(): void;
  /** Whether the terminal has keyboard focus. */
  hasFocus(): boolean;
  /** The selected text, or an empty string. */
  selection(): string;
  selectAll(): void;
  /** Types `text` as a paste, in the host's terminal. */
  paste(text: string): void;
}

interface Props {
  /** The host's terminal fits its pane and takes input; a viewer's keeps the host's size, scales its text to the pane's width and keeps the cursor in sight. */
  mode: 'host' | 'viewer';
  /**
   * Lets VoiceOver, Narrator and Orca read the terminal's lines. Off unless the user turns it on:
   * in this mode xterm keeps typed text in its hidden text box and can send it again.
   */
  screenReader?: boolean;
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  /** Whether the view follows the latest output. */
  onFollowChange?: (following: boolean) => void;
  /** Output arrived while the viewer was scrolled away from the latest. */
  onNewOutput?: () => void;
  onSearchResults?: (result: SearchResult) => void;
  /** The terminal's colours, for answering colour queries while no window is attached. */
  onThemeColors?: (foreground: string, background: string) => void;
  /** The terminal's state for diagnostics, when it changes; never its content. */
  onDiagnostics?: (snapshot: TerminalSnapshot) => void;
}

/** ANSI palettes tuned for the app's light and dark surfaces. */
const PALETTE_LIGHT = {
  black: '#1b1f27', red: '#c0392b', green: '#1d7a55', yellow: '#9a6700', blue: '#3d5a99', magenta: '#8e44ad', cyan: '#0e7490', white: '#d0d4da',
  brightBlack: '#5d6573', brightRed: '#e0473a', brightGreen: '#2a9d6b', brightYellow: '#b7791f', brightBlue: '#4f6fb8', brightMagenta: '#a55bc4', brightCyan: '#1693ad', brightWhite: '#f4f5f7',
};
const PALETTE_DARK = {
  black: '#1d2025', red: '#ff7b6b', green: '#5cc595', yellow: '#f0b14f', blue: '#8ea6e0', magenta: '#c49be8', cyan: '#5fc6d8', white: '#c9ced6',
  brightBlack: '#6f7682', brightRed: '#ff9a8d', brightGreen: '#7fd8aa', brightYellow: '#f7c977', brightBlue: '#adc0ee', brightMagenta: '#d7b6f2', brightCyan: '#86d8e6', brightWhite: '#f2f4f7',
};

const FONT_SIZE = 13;
/** Room kept for the scrollbar beside a viewer's scaled terminal. */
const SCROLLBAR_ROOM = 12;

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function currentTheme(): ITheme {
  const dark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
  return {
    ...(dark ? PALETTE_DARK : PALETTE_LIGHT),
    background: cssVar('--terminal-bg') || (dark ? '#121417' : '#fbfbfc'),
    foreground: cssVar('--terminal-fg') || (dark ? '#dfe3e8' : '#1b1f27'),
    cursor: cssVar('--terminal-cursor') || '#3d5a99',
    cursorAccent: cssVar('--terminal-bg') || '#fbfbfc',
    selectionBackground: cssVar('--terminal-selection') || 'rgba(61, 90, 153, 0.25)',
  };
}

export const TerminalView = forwardRef<TerminalHandle, Props>(function TerminalView(props, ref) {
  const container = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const search = useRef<SearchAddon | null>(null);
  // lineAfter[seq]: the absolute line the cursor was on after record `seq` was written.
  const lineAfter = useRef<number[]>([]);
  const following = useRef(true);
  const recordsWritten = useRef(0);
  const reportState = useRef<() => void>(() => {});
  const callbacks = useRef(props);
  callbacks.current = props;

  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const term = new Terminal({
      allowProposedApi: true,
      cursorBlink: props.mode === 'host',
      disableStdin: props.mode === 'viewer',
      fontFamily: cssVar('--mono') || 'Menlo, monospace',
      fontSize: FONT_SIZE,
      lineHeight: 1.15,
      scrollback: 100_000,
      theme: currentTheme(),
      macOptionIsMeta: true,
      screenReaderMode: props.screenReader ?? false,
    });
    const fit = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(searchAddon);
    term.loadAddon(new WebLinksAddon());
    term.open(element);
    terminal.current = term;

    // Keys the terminal leaves to the app: F6 moves between the window's parts (handled by the
    // workspace), and in a viewer's read-only terminal Tab moves focus on, as anywhere else.
    // On Windows and Linux, Ctrl+C copies when text is selected and interrupts otherwise, as in
    // their terminals; Ctrl+Shift+C and Ctrl+Shift+V always copy and paste. macOS uses Cmd.
    // Shift+Enter in the host's terminal sends `\n`, which every supported agent takes as a new
    // line; xterm would send `\r`, as for Enter. Its keypress is stopped too, or xterm sends `\r`.
    term.attachCustomKeyEventHandler((event) => {
      if (event.key === 'F6') return false;
      if (event.key === 'Tab' && props.mode === 'viewer') return false;
      if (props.mode === 'host' && isNewlineKey(event)) {
        if (event.type === 'keydown') {
          event.preventDefault();
          term.input('\n');
        }
        return false;
      }
      if (!isMac) {
        if (event.type !== 'keydown' || !event.ctrlKey || event.altKey || event.metaKey) return true;
        const key = event.key.toLowerCase();
        if (key === 'c' && (event.shiftKey || term.hasSelection())) {
          event.preventDefault();
          if (term.hasSelection()) {
            void navigator.clipboard?.writeText(term.getSelection());
            term.clearSelection();
          }
          return false;
        }
        if (key === 'v' && event.shiftKey && props.mode === 'host') {
          event.preventDefault();
          void navigator.clipboard?.readText().then((text) => term.paste(text));
          return false;
        }
      }
      return true;
    });
    search.current = searchAddon;

    const reporter = new TerminalReporter((snapshot) => callbacks.current.onDiagnostics?.(snapshot));
    reportState.current = () => {
      const buffer = term.buffer.active;
      reporter.update({
        screen: buffer.type,
        mouseTracking: term.modes.mouseTrackingMode,
        scrollbackLines: term.buffer.normal.baseY,
        scrollbackLimit: term.options.scrollback ?? 0,
        linesFromBottom: buffer.baseY - buffer.viewportY,
        following: following.current,
        fontSize: term.options.fontSize ?? FONT_SIZE,
        recordsWritten: recordsWritten.current,
      });
    };
    const report = () => {
      const buffer = term.buffer.active;
      const nowFollowing = buffer.viewportY >= buffer.baseY;
      if (nowFollowing !== following.current) {
        following.current = nowFollowing;
        callbacks.current.onFollowChange?.(nowFollowing);
      }
      reportState.current();
    };
    const disposables = [
      term.onScroll(report),
      term.onWriteParsed(report),
      term.buffer.onBufferChange(report),
      searchAddon.onDidChangeResults(({ resultIndex, resultCount }) =>
        callbacks.current.onSearchResults?.({ index: resultIndex, count: resultCount }),
      ),
      // A viewer's terminal still answers queries locally; those answers go nowhere.
      term.onData((data) => {
        if (props.mode === 'host') callbacks.current.onData?.(data);
      }),
    ];
    element.addEventListener('wheel', () => requestAnimationFrame(report), { passive: true });

    let resizeObserver: ResizeObserver | undefined;
    if (props.mode === 'host') {
      let pending: number | undefined;
      const fitNow = () => {
        try {
          fit.fit();
        } catch {
          return;
        }
        callbacks.current.onResize?.(term.cols, term.rows);
      };
      resizeObserver = new ResizeObserver(() => {
        window.clearTimeout(pending);
        pending = window.setTimeout(fitNow, 60);
      });
      resizeObserver.observe(element);
      fitNow();
    } else {
      // A viewer's terminal keeps the host's columns and rows, so its text is scaled instead:
      // the font grows or shrinks until the host's columns fill the pane's width. Rows that do
      // not fit scroll, with the cursor kept in sight.
      const screen = element.querySelector<HTMLElement>('.xterm-screen');
      const pane = element.parentElement;
      // The pane width and host size the font was last chosen for. Cells are rounded to whole
      // pixels, so a font chosen from a measurement can come out a little off; only a new width
      // or host size may grow it again, or it could swing between two sizes for ever.
      let chosenFor = '';
      const scaleToPane = () => {
        if (!screen || !pane) return;
        // Measured on the pane, which a scrollbar in the terminal does not narrow.
        const style = getComputedStyle(element);
        const width = pane.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight) - SCROLLBAR_ROOM;
        const drawn = screen.getBoundingClientRect().width;
        if (width <= 0 || !drawn) return;
        const current = term.options.fontSize ?? FONT_SIZE;
        const key = `${width} ${term.cols}x${term.rows}`;
        let size: number;
        if (key !== chosenFor) {
          chosenFor = key;
          size = viewerFontSize(current, width, drawn);
        } else if (drawn > width) {
          size = clampViewerFont(current - 0.25);
        } else {
          return;
        }
        if (size !== current) {
          term.options.fontSize = size;
          reportState.current();
        }
      };
      // Scrolls the pane just enough to show the cursor's line, while the viewer follows the
      // live output; the conversation sits above the cursor in agents such as Claude Code.
      const showCursor = () => {
        if (!screen || !following.current) return;
        const box = element.getBoundingClientRect();
        const drawn = screen.getBoundingClientRect();
        const rowHeight = drawn.height / term.rows;
        const top = drawn.top - box.top + element.scrollTop + term.buffer.active.cursorY * rowHeight;
        if (top + rowHeight > element.scrollTop + element.clientHeight) element.scrollTop = top + rowHeight - element.clientHeight;
        else if (top < element.scrollTop) element.scrollTop = top;
      };
      disposables.push(term.onCursorMove(showCursor), term.onResize(() => requestAnimationFrame(showCursor)));
      let pending: number | undefined;
      resizeObserver = new ResizeObserver(() => {
        window.clearTimeout(pending);
        pending = window.setTimeout(() => {
          scaleToPane();
          requestAnimationFrame(showCursor);
        }, 60);
      });
      // The pane changes with the window; the drawn terminal changes with the host's size.
      if (pane) resizeObserver.observe(pane);
      if (screen) resizeObserver.observe(screen);
    }

    const media = window.matchMedia?.('(prefers-color-scheme: dark)');
    const applyTheme = () => {
      const theme = currentTheme();
      term.options.theme = theme;
      callbacks.current.onThemeColors?.(theme.foreground ?? '#000000', theme.background ?? '#ffffff');
    };
    applyTheme();
    media?.addEventListener('change', applyTheme);

    reportState.current();

    return () => {
      reporter.dispose();
      reportState.current = () => {};
      media?.removeEventListener('change', applyTheme);
      resizeObserver?.disconnect();
      disposables.forEach((d) => d.dispose());
      term.dispose();
      terminal.current = null;
    };
    // The mode is fixed for the life of a session screen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Changed in the settings, also while a session is open.
  useEffect(() => {
    if (terminal.current) terminal.current.options.screenReaderMode = props.screenReader ?? false;
  }, [props.screenReader]);

  useImperativeHandle(ref, () => ({
    write(steps) {
      const term = terminal.current;
      if (!term) return;
      for (const step of steps) {
        if ('resize' in step) {
          const [cols, rows] = step.resize;
          // The host's own pane decides its size; it follows records only during a replay.
          if (props.mode === 'viewer' || cols !== term.cols || rows !== term.rows) term.resize(cols, rows);
          lineAfter.current.push(lineAfter.current.at(-1) ?? 0);
        } else {
          const wasFollowing = following.current;
          recordsWritten.current += 1;
          term.write(step.output, () => {
            const buffer = term.buffer.active;
            lineAfter.current.push(buffer.baseY + buffer.cursorY);
          });
          if (!wasFollowing) callbacks.current.onNewOutput?.();
        }
      }
    },
    reset() {
      terminal.current?.reset();
      terminal.current?.clear();
      lineAfter.current = [];
    },
    findNext(term) {
      search.current?.findNext(term, searchOptions());
    },
    findPrevious(term) {
      search.current?.findPrevious(term, searchOptions());
    },
    clearSearch() {
      search.current?.clearDecorations();
    },
    scrollToSeq(seq) {
      const term = terminal.current;
      if (!term) return;
      const lines = lineAfter.current;
      const line = seq < 0 ? 0 : (lines[Math.min(seq, lines.length - 1)] ?? 0);
      term.scrollToLine(Math.max(0, line - 2));
    },
    scrollToBottom() {
      terminal.current?.scrollToBottom();
    },
    focus() {
      terminal.current?.focus();
    },
    hasFocus() {
      return container.current?.contains(document.activeElement) ?? false;
    },
    selection() {
      return terminal.current?.getSelection() ?? '';
    },
    selectAll() {
      terminal.current?.selectAll();
    },
    paste(text) {
      if (props.mode === 'host') terminal.current?.paste(text);
    },
  }));

  return <div ref={container} className={props.mode === 'host' ? 'terminal-host' : 'terminal-host fixed'} />;
});

function searchOptions() {
  return {
    caseSensitive: false,
    decorations: {
      matchBackground: '#f0b14f55',
      activeMatchBackground: '#f0b14f',
      matchOverviewRuler: '#f0b14f',
      activeMatchColorOverviewRuler: '#e67e22',
    },
  };
}
