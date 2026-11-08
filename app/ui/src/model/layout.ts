// How a participant's session window is arranged (`session-window-layout` design decision 1):
// a preset, what they changed by hand since, and the side panels' widths per preset. Stored as
// is in the app's settings or the browser, and remembered for every session.

import type { Profile, Steps, View } from '../api/types';

export type Preset = 'overview' | 'developer' | 'present';
export type FilesTab = 'documents' | 'code' | 'shared';
export type SidePanel = 'files' | 'timeline';

export interface PanelWidths {
  files: number;
  timeline: number;
}

export interface Layout {
  preset: Preset;
  /** The side panels are open. */
  files: boolean;
  timeline: boolean;
  filesTab: FilesTab;
  steps: Steps;
  /** A viewer has a terminal tab; the host always has one, since they type into it. */
  terminalTab: boolean;
  widths: Record<Preset, PanelWidths>;
}

type Arrangement = Omit<Layout, 'preset' | 'widths'>;

export const PRESETS: Record<Preset, Arrangement> = {
  overview: { files: true, timeline: true, filesTab: 'documents', steps: 'summary', terminalTab: false },
  developer: { files: true, timeline: true, filesTab: 'code', steps: 'details', terminalTab: true },
  present: { files: false, timeline: false, filesTab: 'code', steps: 'details', terminalTab: true },
};

export const PRESET_NAMES: Record<Preset, string> = { overview: 'Overview', developer: 'Developer', present: 'Present' };

/** A side panel's usual width in a preset; Overview's timeline is wider, for reading. */
export function usualWidth(preset: Preset, panel: SidePanel): number {
  if (panel === 'files') return 230;
  return preset === 'overview' ? 400 : 340;
}

export const MIN_PANEL = 180;
export const MAX_PANEL = 620;
/** The tab area never gets narrower than this; side panels give way first. */
export const MIN_TABS = 240;

const PRESET_IDS = Object.keys(PRESETS) as Preset[];

function usualWidths(): Record<Preset, PanelWidths> {
  return Object.fromEntries(PRESET_IDS.map((p) => [p, { files: usualWidth(p, 'files'), timeline: usualWidth(p, 'timeline') }])) as Record<
    Preset,
    PanelWidths
  >;
}

export function clampWidth(width: number): number {
  return Math.round(Math.min(MAX_PANEL, Math.max(MIN_PANEL, width)));
}

/** The layout of a preset, keeping the widths someone gave its panels. */
export function choose(preset: Preset, from?: Layout): Layout {
  return { preset, ...PRESETS[preset], widths: from?.widths ?? usualWidths() };
}

/** A change by hand; the preset stays, and shows as adjusted. */
export function adjust(layout: Layout, change: Partial<Arrangement>): Layout {
  return { ...layout, ...change };
}

/** Whether the window differs from its preset; for the host, the terminal tab does not count. */
export function isAdjusted(layout: Layout, host = false): boolean {
  const preset = PRESETS[layout.preset];
  return (Object.keys(preset) as (keyof Arrangement)[]).some((key) => (key !== 'terminalTab' || !host) && layout[key] !== preset[key]);
}

export function panelWidth(layout: Layout, panel: SidePanel): number {
  return layout.widths[layout.preset][panel];
}

/** Sets a side panel's width in the current preset only. */
export function setWidth(layout: Layout, panel: SidePanel, width: number): Layout {
  const widths = { ...layout.widths, [layout.preset]: { ...layout.widths[layout.preset], [panel]: clampWidth(width) } };
  return { ...layout, widths };
}

/** The layout that the view settings of earlier versions stood for (design decision 2). */
export function fromView(view: View): Layout {
  return adjust(choose(view.terminal ? 'developer' : 'overview'), { steps: view.steps });
}

const isBool = (value: unknown): value is boolean => typeof value === 'boolean';

/** A stored layout, or null when there is none or it cannot be read; missing widths are usual. */
export function parseLayout(value: unknown): Layout | null {
  if (!value || typeof value !== 'object') return null;
  const v = value as Record<string, unknown>;
  if (!PRESET_IDS.includes(v.preset as Preset)) return null;
  if (!isBool(v.files) || !isBool(v.timeline) || !isBool(v.terminalTab)) return null;
  if (v.filesTab !== 'documents' && v.filesTab !== 'code' && v.filesTab !== 'shared') return null;
  if (v.steps !== 'summary' && v.steps !== 'details') return null;
  const stored = (v.widths && typeof v.widths === 'object' ? v.widths : {}) as Record<string, Partial<PanelWidths> | undefined>;
  const widths = usualWidths();
  for (const preset of PRESET_IDS) {
    for (const panel of ['files', 'timeline'] as const) {
      const width = stored[preset]?.[panel];
      if (typeof width === 'number' && Number.isFinite(width)) widths[preset][panel] = clampWidth(width);
    }
  }
  return {
    preset: v.preset as Preset,
    files: v.files,
    timeline: v.timeline,
    filesTab: v.filesTab,
    steps: v.steps,
    terminalTab: v.terminalTab,
    widths,
  };
}

/** Where earlier versions kept the side panel's width. */
export const SIDE_WIDTH_KEY = 'specdeck.sideWidth';

/**
 * Someone's layout from what is stored: the layout itself, else the view settings of earlier
 * versions, with the side panel's old width as the timeline's. Null until they chose one.
 */
export function storedLayout(layout: unknown, view: View | null, sideWidth: string | null): Layout | null {
  const parsed = parseLayout(layout);
  if (parsed) return parsed;
  if (!view) return null;
  let migrated = fromView(view);
  const width = Number(sideWidth);
  if (sideWidth && Number.isFinite(width) && width > 0) {
    for (const preset of PRESET_IDS) migrated = setWidth({ ...migrated, preset }, 'timeline', width);
    migrated = { ...migrated, preset: view.terminal ? 'developer' : 'overview' };
  }
  return migrated;
}

/** The side panel's width as earlier versions stored it in this window's storage. */
export function oldSideWidth(): string | null {
  try {
    return localStorage.getItem(SIDE_WIDTH_KEY);
  } catch {
    return null;
  }
}

/** Someone's layout from their profile; null until they answered the first-join question. */
export function profileLayout(profile: Pick<Profile, 'layout' | 'view'>): Layout | null {
  return storedLayout(profile.layout, profile.view, oldSideWidth());
}
