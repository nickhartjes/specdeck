import { useEffect, useRef } from 'react';
import { isMac } from '../menu/menu';

const MAC: [string, string][] = [
  ['New window', '⇧⌘N'],
  ['Host a session', '⌘N'],
  ['Join a session', '⌘J'],
  ['Copy the invite code', '⇧⌘C'],
  ['Find in the terminal and timeline', '⌘F'],
  ['Next or previous match', 'Return / ⇧Return'],
  ['Timeline, Files, People', '⌘1 ⌘2 ⌘3'],
  ['Jump to live', '⌘↓'],
  ['Zoom in, zoom out, actual size', '⌘+ ⌘− ⌘0'],
  ["New line in your terminal's prompt", '⇧Return'],
  ['Move between toolbar, terminal and side panel', 'F6 / ⇧F6'],
  ['Settings', '⌘,'],
  ['Close the window (a hosted session keeps running)', '⌘W'],
  ['Quit', '⌘Q'],
  ['Connection status', '⇧⌘D'],
  ['Keyboard shortcuts', '⌘/'],
];

const OTHERS: [string, string][] = [
  ['Host a session', 'Ctrl+Shift+N'],
  ['Join a session', 'Ctrl+Shift+J'],
  ['Find in the terminal and timeline', 'Ctrl+F (Ctrl+Shift+F in your own terminal)'],
  ['Next or previous match', 'Enter / Shift+Enter'],
  ['Copy the selection', 'Ctrl+C or Ctrl+Shift+C'],
  ['Paste into your terminal', 'Ctrl+V or Ctrl+Shift+V'],
  ["New line in your terminal's prompt", 'Shift+Enter'],
  ['Timeline, Files, People', 'Ctrl+1 Ctrl+2 Ctrl+3'],
  ['Zoom in, zoom out, actual size', 'Ctrl++ Ctrl+− Ctrl+0'],
  ['Move between toolbar, terminal and side panel', 'F6 / Shift+F6'],
  ['Settings', 'Ctrl+,'],
  ['Connection status', 'Ctrl+Shift+D'],
  ['Keyboard shortcuts', 'F1'],
];

/**
 * The web viewer's: the browser keeps Ctrl+1–3 for its tabs and does its own zooming
 * (`web-viewer` design decision 4).
 */
const WEB: [string, string][] = [
  ['Find in the terminal and timeline', isMac ? '⌘F' : 'Ctrl+F'],
  ['Next or previous match', isMac ? 'Return / ⇧Return' : 'Enter / Shift+Enter'],
  ['Timeline, Files, People', isMac ? '⌥1 ⌥2 ⌥3' : 'Alt+1 Alt+2 Alt+3'],
  ['Move between toolbar, terminal and side panel', isMac ? 'F6 / ⇧F6' : 'F6 / Shift+F6'],
  ['Zoom in, zoom out, actual size', 'Your browser’s zoom'],
  ['Keyboard shortcuts', '?'],
];

/** The app's keyboard shortcuts for this platform, or the web viewer's. */
export function ShortcutsDialog({ onClose, web = false }: { onClose: () => void; web?: boolean }) {
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    close.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);
  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog shortcuts" role="dialog" aria-modal="true" aria-labelledby="shortcuts-title">
        <h2 id="shortcuts-title">Keyboard shortcuts</h2>
        <dl>
          {(web ? WEB : isMac ? MAC : OTHERS).map(([what, keys]) => (
            <div key={what}>
              <dt>{what}</dt>
              <dd>
                <kbd>{keys}</kbd>
              </dd>
            </div>
          ))}
        </dl>
        {!isMac && !web && <p>In your own terminal, Ctrl+C without a selection interrupts the agent, as in any terminal.</p>}
        <div className="actions">
          <button ref={close} className="btn primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
