import { useEffect, useRef, useState } from 'react';
import { ChevronDown, LayoutTemplate, PanelLeft, PanelRight } from 'lucide-react';
import { useBridge } from '../api/bridge';
import type { Appearance } from '../api/types';
import { APPEARANCES } from '../appearance';
import { choose, isAdjusted, PRESET_NAMES, type Layout, type Preset } from '../model/layout';

const isMac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);

export function inWebViewer(): boolean {
  return typeof document !== 'undefined' && document.documentElement.dataset.platform === 'web';
}

/** The shortcut for panel `n` as this platform writes it; the web viewer uses Alt, as the browser keeps Ctrl. */
export function panelKey(n: number): string {
  if (inWebViewer()) return `Alt+${n}`;
  return isMac ? `⌘${n}` : `Ctrl+${n}`;
}

const DESCRIPTIONS: Record<Preset, string> = {
  overview: 'Documents and the timeline in plain words',
  developer: 'Code, the terminal and every step',
  present: 'The middle only, for sharing your screen',
};

interface Props {
  layout: Layout;
  onLayout: (layout: Layout) => void;
  /** The host's terminal tab is always there, so it never counts as a change. */
  host?: boolean;
  /** Viewers: show the first-join question again. */
  onAskAgain?: () => void;
}

/** The toolbar's layout controls: the side panels, and the preset (`session-window` spec: Layout presets). */
export function LayoutControls({ layout, onLayout, host = false, onAskAgain }: Props) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const adjusted = isAdjusted(layout, host);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      button.current?.focus();
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!panel.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousedown', onDown);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousedown', onDown);
    };
  }, [open]);

  const pick = (preset: Preset) => {
    setOpen(false);
    onLayout(choose(preset, layout));
  };

  return (
    <div className="layout-controls" role="group" aria-label="Layout">
      <button
        className="btn ghost icon"
        aria-pressed={layout.files}
        aria-label="Files"
        title={`${layout.files ? 'Hide' : 'Show'} files (${panelKey(1)})`}
        onClick={() => onLayout({ ...layout, files: !layout.files })}
      >
        <PanelLeft aria-hidden />
      </button>
      <div className="layout-menu">
        <button ref={button} className="btn ghost" aria-expanded={open} aria-controls="layout-menu-panel" onClick={() => setOpen((o) => !o)}>
          <LayoutTemplate aria-hidden />
          <span>
            {PRESET_NAMES[layout.preset]}
            {adjusted && <span className="muted"> · adjusted</span>}
          </span>
          <ChevronDown aria-hidden className="chevron" />
        </button>
        {open && (
          <div ref={panel} id="layout-menu-panel" className="menu-panel">
            <fieldset>
              <legend>Layout</legend>
              {(Object.keys(PRESET_NAMES) as Preset[]).map((preset) => (
                <label key={preset} className="check">
                  <input type="radio" name="layout-preset" checked={layout.preset === preset && !adjusted} onChange={() => pick(preset)} />
                  <span>
                    {PRESET_NAMES[preset]} <span className="muted">· {DESCRIPTIONS[preset]}</span>
                  </span>
                </label>
              ))}
            </fieldset>
            {adjusted && (
              <button className="btn small" onClick={() => pick(layout.preset)}>
                Back to {PRESET_NAMES[layout.preset]}
              </button>
            )}
            <p className="muted">
              Your changes are remembered for every session.
              {onAskAgain && (
                <>
                  {' '}
                  <button
                    className="link"
                    onClick={() => {
                      setOpen(false);
                      onAskAgain();
                    }}
                  >
                    Show the first-join question again
                  </button>
                </>
              )}
            </p>
            {inWebViewer() && <AppearanceChoice />}
          </div>
        )}
      </div>
      <button
        className="btn ghost icon"
        aria-pressed={layout.timeline}
        aria-label="Timeline"
        title={`${layout.timeline ? 'Hide' : 'Show'} the timeline (${panelKey(3)})`}
        onClick={() => onLayout({ ...layout, timeline: !layout.timeline })}
      >
        <PanelRight aria-hidden />
      </button>
    </div>
  );
}

/**
 * The web viewer's appearance, kept in the browser (`web-viewer` spec). The app has it in
 * Settings and the View menu instead.
 */
function AppearanceChoice() {
  const bridge = useBridge();
  const [appearance, setAppearance] = useState<Appearance | null>(null);
  useEffect(() => {
    void bridge.getProfile().then((profile) => setAppearance(profile.appearance));
    return bridge.onAppearance(setAppearance);
  }, [bridge]);
  return (
    <fieldset>
      <legend>Appearance</legend>
      {APPEARANCES.map(({ value, label }) => (
        <label key={value} className="check">
          <input type="radio" name="appearance" checked={appearance === value} onChange={() => void bridge.setAppearance(value)} />
          <span>
            {label}
            {value === 'system' && <span className="muted"> · as your browser</span>}
          </span>
        </label>
      ))}
    </fieldset>
  );
}
