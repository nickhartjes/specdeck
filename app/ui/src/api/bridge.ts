// Everything the UI asks of the Rust side. Components get it from BridgeContext, so tests can
// pass a fake.

import type { MenuCommand } from '../menu/menu';
import type { Heartbeat } from '../model/heartbeat';
import type { Layout } from '../model/layout';
import { createContext, useContext } from 'react';
import type {
  AgentId,
  Appearance,
  AgentVersion,
  DiagnosticsStatus,
  HostEvent,
  JoinLink,
  Profile,
  RunningSession,
  ServiceCheck,
  SharedFileInfo,
  TerminalSnapshot,
  UpdateState,
  ViewEvent,
} from './types';

/** Receives terminal data as the binary stream described in src/terminal/stream.ts. */
export type TerminalSink = (data: ArrayBuffer) => void;

export interface Bridge {
  getProfile(): Promise<Profile>;
  /** The installed agents' versions; slow the first time, so asked for after the start screen shows. */
  agentVersions(): Promise<AgentVersion[]>;
  openInstallPage(agent: AgentId): Promise<void>;
  setUsername(username: string): Promise<Profile>;
  setNotifications(enabled: boolean): Promise<Profile>;
  setCheckUpdates(enabled: boolean): Promise<Profile>;
  setScreenReader(enabled: boolean): Promise<Profile>;
  setVerboseLogging(enabled: boolean): Promise<Profile>;
  setBrowser(enabled: boolean): Promise<Profile>;
  setAppearance(appearance: Appearance): Promise<Profile>;
  /** Calls `handler` when the appearance changes, from any window; returns a function that stops listening. */
  onAppearance(handler: (appearance: Appearance) => void): () => void;
  setScreenshots(enabled: boolean): Promise<Profile>;
  updateStatus(): Promise<UpdateState>;
  /** Calls `handler` whenever the update state changes; returns a function that stops listening. */
  onUpdate(handler: (state: UpdateState) => void): () => void;
  /** Ends or leaves the session, installs the ready update and restarts; fails if it cannot. */
  restartToUpdate(): Promise<void>;
  /** Remembers how the user arranges the session window. */
  setLayout(layout: Layout): Promise<Profile>;
  /** A domain or URL; empty for the built-in service. Fails for what is not a service. */
  setSharingService(service: string): Promise<Profile>;
  checkSharingService(service: string): Promise<ServiceCheck>;
  removeRecentFolder(path: string): Promise<Profile>;
  /** Opens the system folder picker; null when cancelled. */
  chooseFolder(): Promise<string | null>;
  startHosting(agent: AgentId, folder: string, cols: number, rows: number): Promise<void>;
  hostAttach(terminal: TerminalSink, events: (event: HostEvent) => void): Promise<void>;
  hostInput(data: string): Promise<void>;
  hostResize(cols: number, rows: number): Promise<void>;
  hostSetColors(foreground: string, background: string): Promise<void>;
  hostNewCode(): Promise<void>;
  hostRemoveViewer(viewerId: number): Promise<void>;
  hostSetViewerLimit(limit: number): Promise<void>;
  /** Lets viewers suggest prompts, or stops them. */
  hostAllowSuggestions(on: boolean): Promise<void>;
  /** Lets viewers open whole code files the agent changes from now on, or stops it. */
  hostShareCode(wholeFiles: boolean): Promise<void>;
  /** Types `text` (the suggestion, maybe edited) into the agent's prompt and submits it, or declines the suggestion when `text` is null. */
  hostAnswerSuggestion(viewerId: number, id: number, text: string | null): Promise<void>;
  /** Lets viewers share files, or stops them; files already shared stay. */
  hostAllowFiles(on: boolean): Promise<void>;
  /** Shares a file the host chose; fails with why it was refused. */
  hostShareFile(name: string, bytes: ArrayBuffer): Promise<SharedFileInfo>;
  hostRemoveSharedFile(file: number): Promise<void>;
  hostReadSharedFile(file: number): Promise<ArrayBuffer>;
  /** Puts a shared file into shared/ in the session folder; the path it was written to. */
  hostPutSharedFile(file: number): Promise<string>;
  hostClose(): Promise<void>;
  /** Joins through `server`, from a link the user agreed to, or their own service when null. */
  joinSession(code: string, server: string | null, terminal: TerminalSink, events: (event: ViewEvent) => void): Promise<void>;
  leaveSession(): Promise<void>;
  /** Suggests a prompt to the host of the watched session; fails for text the host would refuse. */
  suggestPrompt(text: string): Promise<void>;
  /** Shares a file with everyone in the watched session; fails with why the host refused it. */
  viewerShareFile(name: string, bytes: ArrayBuffer): Promise<void>;
  /** A shared file's bytes, fetched from the host and checked. */
  viewerOpenFile(file: number): Promise<ArrayBuffer>;
  /** Removes a file this viewer shared. */
  viewerRemoveFile(file: number): Promise<void>;
  /** Saves a copy of a shared file where the person chooses; false when they cancelled. */
  saveSharedFile(name: string, bytes: ArrayBuffer): Promise<boolean>;
  /** The join link for this window that opened the app or came in since, once; null when there is none. */
  takeJoinLink(): Promise<JoinLink | null>;
  /** The host, join or recent-folder command a new window opened with, once; null when there is none. */
  takeStartCommand(): Promise<MenuCommand | null>;
  /** Names this window's session in its title, or clears it with null. */
  setSessionTitle(name: string | null): Promise<void>;
  /** Reads a link pasted into the start screen. */
  readJoinLink(link: string): Promise<JoinLink>;
  copyText(text: string): Promise<void>;
  /** The hosted sessions that run without a window. */
  runningSessions(): Promise<RunningSession[]>;
  /** Shows listed session `id` in this window; fails with a message when it got a window or ended meanwhile. */
  openRunningSession(id: number): Promise<void>;
  /** Ends listed session `id` for everyone: one an older SpecDeck started. */
  endRunningSession(id: number): Promise<void>;
  /** Calls `handler` with the whole list whenever it may have changed; returns a function that stops listening. */
  onRunningSessions(handler: (sessions: RunningSession[]) => void): () => void;
  /** Tells the app the start screen can be used; scripts/measure-footprint times this. */
  startScreenShown(): Promise<void>;
  /** The terminal's state, for diagnostics. */
  reportTerminal(snapshot: TerminalSnapshot): Promise<void>;
  diagnosticsStatus(): Promise<DiagnosticsStatus>;
  /** Drops the viewer's connection and reconnects at once, keeping its place. */
  reconnectViewer(): Promise<void>;
  /** Saves the diagnostics where the user chooses; false when they cancel. */
  exportDiagnostics(): Promise<boolean>;
  /** This window is alive; see model/heartbeat.ts. */
  heartbeat(beat: Heartbeat): Promise<void>;
  /** Calls `handler` for each native menu command sent to this window; returns a function that stops listening. */
  onMenu(handler: (command: MenuCommand) => void): () => void;
}

export const BridgeContext = createContext<Bridge | null>(null);

export function useBridge(): Bridge {
  const bridge = useContext(BridgeContext);
  if (!bridge) throw new Error('BridgeContext is missing');
  return bridge;
}

/** Tauri's answer to a command the app doesn't have: the window is newer than the app running it. */
const UNKNOWN_COMMAND = /^command \S+ not found$/;

/** Error text from a failed command, for showing to the user. */
export function errorMessage(error: unknown): string {
  const text = typeof error === 'string' ? error : error instanceof Error ? error.message : null;
  if (text === null) return 'Something went wrong.';
  if (UNKNOWN_COMMAND.test(text)) {
    console.warn(text);
    return 'This window is newer than the SpecDeck app running it. Quit SpecDeck and open it again.';
  }
  return text;
}
