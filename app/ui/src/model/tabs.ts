// The tabs in the middle of the session window (`session-window` spec: Tabs in the middle,
// Preview tabs): the terminal, and the documents, diffs, screenshots and shared files a
// participant opens, and a plan the agent presented for approval (`notification-open` design
// decision 5), with at most one preview tab that the next file opened replaces.

export type TabKind = 'terminal' | 'document' | 'diff' | 'image' | 'shared' | 'plan';

export interface OpenTab {
  id: string;
  kind: TabKind;
  /** The file, for a document or diff tab; the tool call, for a screenshot or a plan; the file's id, for a shared file. */
  path?: string;
  /** What the tab is called when that is not the file's name, such as a screenshot's page. */
  title?: string;
  /** A preview: replaced by the next file opened, until kept. */
  preview: boolean;
  /** The tool call whose change a diff tab scrolls to, and a counter so the same one scrolls again. */
  focus?: { toolUseId: string; n: number };
}

export interface Tabs {
  list: OpenTab[];
  active: string | null;
}

export const TERMINAL = 'terminal';

export function tabId(kind: TabKind, path?: string): string {
  return kind === 'terminal' ? TERMINAL : `${kind}:${path}`;
}

export function noTabs(terminal: boolean): Tabs {
  return terminal ? { list: [{ id: TERMINAL, kind: 'terminal', preview: false }], active: TERMINAL } : { list: [], active: null };
}

/**
 * Opens a file: shows its tab if it is open, else opens it in the preview tab, or as an ordinary
 * tab when `keep`. Opening an open preview with `keep` keeps it.
 */
export function openTab(tabs: Tabs, kind: Exclude<TabKind, 'terminal'>, path: string, options: { keep?: boolean; focus?: string; title?: string } = {}): Tabs {
  const id = tabId(kind, path);
  const existing = tabs.list.find((tab) => tab.id === id);
  const focus = options.focus ? { toolUseId: options.focus, n: (existing?.focus?.n ?? 0) + 1 } : existing?.focus;
  if (existing) {
    const updated = { ...existing, focus, preview: existing.preview && !options.keep };
    return { list: tabs.list.map((tab) => (tab.id === id ? updated : tab)), active: id };
  }
  const tab: OpenTab = { id, kind, path, preview: !options.keep, focus, ...(options.title && { title: options.title }) };
  const preview = tabs.list.findIndex((t) => t.preview);
  const list = preview >= 0 ? tabs.list.map((t, i) => (i === preview ? tab : t)) : [...tabs.list, tab];
  return { list, active: id };
}

/** A preview becomes an ordinary tab. */
export function keepTab(tabs: Tabs, id: string): Tabs {
  if (!tabs.list.some((tab) => tab.id === id && tab.preview)) return tabs;
  return { ...tabs, list: tabs.list.map((tab) => (tab.id === id ? { ...tab, preview: false } : tab)) };
}

/** Closes a tab and shows its neighbour; the terminal tab is never closed this way. */
export function closeTab(tabs: Tabs, id: string): Tabs {
  const index = tabs.list.findIndex((tab) => tab.id === id);
  if (index < 0 || id === TERMINAL) return tabs;
  const list = tabs.list.filter((tab) => tab.id !== id);
  const active = tabs.active === id ? (list[Math.min(index, list.length - 1)]?.id ?? null) : tabs.active;
  return { list, active };
}

export function selectTab(tabs: Tabs, id: string): Tabs {
  return tabs.list.some((tab) => tab.id === id) ? { ...tabs, active: id } : tabs;
}

/** Adds or takes away the terminal tab, which is always first. */
export function withTerminal(tabs: Tabs, terminal: boolean): Tabs {
  const has = tabs.list.some((tab) => tab.id === TERMINAL);
  if (has === terminal) return tabs;
  if (terminal) return { list: [{ id: TERMINAL, kind: 'terminal', preview: false }, ...tabs.list], active: tabs.active ?? TERMINAL };
  const list = tabs.list.filter((tab) => tab.id !== TERMINAL);
  return { list, active: tabs.active === TERMINAL ? (list[0]?.id ?? null) : tabs.active };
}
