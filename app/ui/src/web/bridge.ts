// The web viewer's bridge (`web-viewer` design decision 4): watching only, through the sharing
// service that served the page. The viewer's name, layout and appearance stay in this browser's storage; host,
// folder, agent and update calls are not part of the web viewer and fail.

import type { Bridge } from '../api/bridge';
import type { Appearance, JoinLink, Profile, View, ViewEvent } from '../api/types';
import { asAppearance } from '../appearance';
import { Connection, type Platform, type ViewerCore } from './connection';

/** Keys in the browser's storage. */
export const STORAGE_KEYS = {
  username: 'specdeck.username',
  view: 'specdeck.view',
  layout: 'specdeck.layout',
  appearance: 'specdeck.appearance',
} as const;

export interface WebBridgeOptions {
  /** This page's sharing service, `wss://…/signal`; the only one it joins through. */
  service: string;
  /** The shared Rust core for a code; throws for what is not a code. */
  core: (code: string) => ViewerCore;
  /** Reads a join link of either form; undefined when it is not one. */
  parseJoinLink: (link: string) => { code: string; service?: string } | undefined;
  /** A shared link in the form the page may open, or undefined: `checkLink` of model/links.ts. */
  openableLink: (url: string) => string | undefined;
  /** Opens a page; `window.open` unless a test passes its own. */
  open?: (url: string, target: string, features: string) => void;
  /** `localStorage`, or null where the browser keeps nothing. */
  storage: Storage | null;
  platform?: Platform;
}

/** A new tab that can't reach this page and isn't told where it came from (`web-viewer` spec: Links in the browser). */
export const LINK_FEATURES = 'noopener,noreferrer';

/** Notes a shared link as it arrives, or forgets one the host removed. */
export function recordLink(links: Map<number, string>, event: ViewEvent) {
  if (event.type !== 'timeline') return;
  const entry = event.event;
  if (entry.kind === 'link' && entry.url) links.set(entry.id, entry.url);
  else if (entry.kind === 'link' || entry.kind === 'link-removed') links.delete(entry.id);
}

/** Opens link `id` in a new tab after the host's own check; fails for anything else. */
export function openSharedLink(
  links: Map<number, string>,
  id: number,
  openableLink: WebBridgeOptions['openableLink'],
  open: NonNullable<WebBridgeOptions['open']>,
) {
  const stored = links.get(id);
  if (stored === undefined) throw new Error('That link is not in this session.');
  // The same check the host made, whatever the host sent.
  const url = openableLink(stored);
  if (url === undefined) throw new Error('This link can’t be opened.');
  open(url, '_blank', LINK_FEATURES);
}

/** The host name of a `wss://host/…/signal` URL. */
export function serviceName(service: string): string {
  return service.replace(/^wss?:\/\//, '').replace(/\/.*$/, '');
}

function read(storage: Storage | null, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function write(storage: Storage | null, key: string, value: string) {
  try {
    storage?.setItem(key, value);
  } catch {
    // Private windows and blocked storage: the page works, and asks again next time.
  }
}

function readView(storage: Storage | null): View | null {
  const text = read(storage, STORAGE_KEYS.view);
  if (!text) return null;
  try {
    const view = JSON.parse(text) as Partial<View>;
    return typeof view.terminal === 'boolean' && (view.steps === 'summary' || view.steps === 'details')
      ? { terminal: view.terminal, steps: view.steps }
      : null;
  } catch {
    return null;
  }
}

/** The appearance kept in this browser; System when there is none (`web-viewer` spec). */
export function storedAppearance(storage: Storage | null): Appearance {
  return asAppearance(read(storage, STORAGE_KEYS.appearance));
}

/** The stored layout as is; `model/layout.ts` reads it. */
function readLayout(storage: Storage | null): unknown {
  const text = read(storage, STORAGE_KEYS.layout);
  if (!text) return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

const notInTheWebViewer = () => Promise.reject(new Error('The web viewer can only watch sessions.'));

export function webBridge(options: WebBridgeOptions): Bridge {
  const { storage, service } = options;
  let connection: Connection | null = null;
  // Kept here as well, so a browser that stores nothing still remembers them until reload.
  let username = read(storage, STORAGE_KEYS.username);
  // The view of earlier versions, to make the first layout from.
  const view = readView(storage);
  let layout = readLayout(storage);
  let appearance = storedAppearance(storage);
  const appearanceHandlers = new Set<(appearance: Appearance) => void>();
  // The session's links by id, as they arrived: the page opens only these.
  const links = new Map<number, string>();
  const open = options.open ?? ((url, target, features) => void window.open(url, target, features));

  const profile = (): Profile => ({
    username,
    recentFolders: [],
    lastAgent: null,
    agents: [],
    hosting: false,
    hostedSessions: 0,
    watchedSessions: connection !== null ? 1 : 0,
    notifications: false,
    view,
    layout,
    translucent: false,
    platform: 'web',
    sharingService: null,
    effectiveService: service,
    builtInService: service,
    serviceOverridden: true,
    inSession: connection !== null,
    updatesItself: false,
    checkUpdates: false,
    screenReader: false,
    verboseLogging: false,
    browser: false,
    appearance,
    screenshots: false,
  });

  const readJoinLink = (link: string): JoinLink => {
    const parsed = options.parseJoinLink(link);
    if (!parsed) return { error: 'That link is not a SpecDeck invite. Ask the host for a new one.' };
    if (parsed.service && parsed.service !== service) {
      return { code: parsed.code, server: parsed.service, serverName: serviceName(parsed.service) };
    }
    return { code: parsed.code };
  };

  return {
    getProfile: () => Promise.resolve(profile()),
    agentVersions: () => Promise.resolve([]),
    openInstallPage: notInTheWebViewer,
    setUsername: (name) => {
      username = name.trim();
      write(storage, STORAGE_KEYS.username, username);
      return Promise.resolve(profile());
    },
    setNotifications: () => Promise.resolve(profile()),
    setCheckUpdates: () => Promise.resolve(profile()),
    setScreenReader: () => Promise.resolve(profile()),
    setVerboseLogging: () => Promise.resolve(profile()),
    setBrowser: () => Promise.resolve(profile()),
    setAppearance: (next) => {
      // Kept for this page even when the browser keeps nothing.
      appearance = next;
      write(storage, STORAGE_KEYS.appearance, next);
      appearanceHandlers.forEach((handler) => handler(next));
      return Promise.resolve(profile());
    },
    onAppearance: (handler) => {
      appearanceHandlers.add(handler);
      return () => void appearanceHandlers.delete(handler);
    },
    setScreenshots: () => Promise.resolve(profile()),
    updateStatus: () => Promise.resolve({ state: 'disabled' }),
    onUpdate: () => () => {},
    restartToUpdate: notInTheWebViewer,
    setLayout: (next) => {
      layout = next;
      write(storage, STORAGE_KEYS.layout, JSON.stringify(next));
      return Promise.resolve(profile());
    },
    setSharingService: notInTheWebViewer,
    checkSharingService: notInTheWebViewer,
    removeRecentFolder: notInTheWebViewer,
    // It hosts nothing, so nothing runs without a window.
    runningSessions: () => Promise.resolve([]),
    openRunningSession: notInTheWebViewer,
    endRunningSession: notInTheWebViewer,
    onRunningSessions: () => () => {},
    chooseFolder: notInTheWebViewer,
    startHosting: notInTheWebViewer,
    hostAttach: notInTheWebViewer,
    hostInput: notInTheWebViewer,
    hostResize: notInTheWebViewer,
    hostSetColors: notInTheWebViewer,
    hostNewCode: notInTheWebViewer,
    hostRemoveViewer: notInTheWebViewer,
    hostSetViewerLimit: notInTheWebViewer,
    hostAllowSuggestions: notInTheWebViewer,
    hostShareCode: notInTheWebViewer,
    hostAnswerSuggestion: notInTheWebViewer,
    hostShareLink: notInTheWebViewer,
    hostAnswerLink: notInTheWebViewer,
    hostRemoveLink: notInTheWebViewer,
    hostAllowLinks: notInTheWebViewer,
    hostClose: notInTheWebViewer,
    joinSession: async (code, server, terminal, events) => {
      // Each page joins only through its own service (spec: Only the page's own service).
      if (server && server !== service) throw new Error(`This invite is for ${serviceName(server)}. Open it there.`);
      if (!username) throw new Error('Set your name first.');
      let core: ViewerCore;
      try {
        core = options.core(code);
      } catch {
        throw new Error('That is not an invite code. It looks like 7K2M-9QXH-4TPA.');
      }
      connection?.leave();
      links.clear();
      connection = new Connection({
        url: service,
        username,
        core,
        platform: options.platform,
        onRecords: (bytes) => terminal(bytes.slice().buffer),
        onEvent: (event) => {
          if (event.type === 'ended') connection = null;
          recordLink(links, event);
          events(event);
        },
      });
    },
    leaveSession: () => {
      connection?.leave();
      connection = null;
      return Promise.resolve();
    },
    suggestPrompt: async (text) => {
      if (!connection) throw new Error('You are not in a session.');
      connection.suggest(text);
    },
    sendLink: async (url, title, sessionOnly) => {
      if (!connection) throw new Error('You are not in a session.');
      connection.sendLink(url, title, sessionOnly);
    },
    openLink: async (id) => openSharedLink(links, id, options.openableLink, open),
    takeJoinLink: () => Promise.resolve(null),
    // One page, one session: no other windows to start or title.
    takeStartCommand: () => Promise.resolve(null),
    setSessionTitle: () => Promise.resolve(),
    readJoinLink: (link) => Promise.resolve(readJoinLink(link)),
    copyText: (text) => navigator.clipboard.writeText(text),
    startScreenShown: () => Promise.resolve(),
    // The browser's developer tools show the web viewer's state.
    reportTerminal: () => Promise.resolve(),
    diagnosticsStatus: () => Promise.reject(new Error('Not available in the browser.')),
    reconnectViewer: () => Promise.resolve(),
    heartbeat: () => Promise.resolve(),
    exportDiagnostics: () => Promise.resolve(false),
    onMenu: () => () => {},
  };
}
