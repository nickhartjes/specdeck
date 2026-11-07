// Types shared with the Rust side (app/src-tauri). Names follow its serde output.

export type AgentId = 'claude-code' | 'open-code' | 'omp' | 'codex' | 'grok' | 'pi' | 'terminal';
export type AgentStatus = 'starting' | 'working' | 'waiting' | 'exited' | 'unknown';

export interface AgentInfo {
  id: AgentId;
  name: string;
  installed: boolean;
  hasTimeline: boolean;
  /** Reports whether it is working or waiting. */
  reportsStatus: boolean;
  /** The executable, or for Terminal the shell it starts. */
  path: string | null;
  /** Where to read how to install it. */
  installPage: string | null;
}

export interface AgentVersion {
  agent: AgentId;
  /** Null when the agent printed none in time. */
  version: string | null;
}

export interface RecentFolder {
  path: string;
  name: string;
}

/** A `specdeck://join/…` link that opened the app: its code, or why it does not work. */
export interface JoinLink {
  code?: string;
  error?: string;
  /** The link's sharing service, only when it differs from the user's own. */
  server?: string;
  /** Its host name, for asking whether to join through it. */
  serverName?: string;
}

/** Whether a sharing service answered the check in Settings. */
export interface ServiceCheck {
  reachable: boolean;
  /** Why not, in plain words. */
  problem?: string;
}

export type Steps = 'summary' | 'details';

/** One document of an OpenSpec change, as its schema names it. */
export interface PlanDocument {
  id: string;
  name: string;
  /** Relative to the session folder; several for a document such as specs. */
  paths: string[];
  exists: boolean;
}

/** How someone follows sessions; the host's terminal is always shown. */
export interface View {
  terminal: boolean;
  steps: Steps;
}

/** A hosted session that runs without a window, as the start screen lists it (`running-sessions-on-start`). */
export interface RunningSession {
  id: number;
  folder: string;
  agent: AgentId;
  /** What the agent is doing, in the tray's words. */
  state: string;
  viewers: number;
  /** False for a session an older SpecDeck started, which can only be ended. */
  controllable: boolean;
}

export interface Profile {
  username: string | null;
  recentFolders: RecentFolder[];
  lastAgent: AgentId | null;
  agents: AgentInfo[];
  /** This window shows a hosted session. */
  hosting: boolean;
  /** The sessions hosted and watched in all windows, for the question before restarting. */
  hostedSessions: number;
  watchedSessions: number;
  /** Whether to show OS notifications while the window is not in front. */
  notifications: boolean;
  /** How the user followed sessions before layouts, to make their first layout from. */
  view: View | null;
  /** How the user arranges the session window, as stored (`model/layout.ts` reads it); null until they answered the first-join question. */
  layout: unknown;
  /** The window has the platform's material behind it (macOS vibrancy, Windows Mica). */
  translucent: boolean;
  platform: string;
  /** The user's sharing service setting as a URL; null for the built-in service. */
  sharingService: string | null;
  /** The service a session started now uses. */
  effectiveService: string;
  builtInService: string;
  /** SPECDECK_SIGNALLING_URL decides the service, so the setting has no effect. */
  serviceOverridden: boolean;
  /** Hosting or viewing a session in any window; the service cannot change meanwhile. */
  inSession: boolean;
  /** This install updates itself (macOS app, Windows installer, AppImage). */
  updatesItself: boolean;
  /** Check for and download new releases in the background. */
  checkUpdates: boolean;
  /** Screen readers read the terminal. */
  screenReader: boolean;
  /** The diagnostic log records traffic too. */
  verboseLogging: boolean;
  /** New Claude Code sessions get SpecDeck's own browser; only on macOS. */
  browser: boolean;
  /** Light, dark or the OS's appearance; the terminal stays dark in each. */
  appearance: Appearance;
}

/** The app's appearance; `system` follows the OS, or the browser in the web viewer. */
export type Appearance = 'system' | 'light' | 'dark';

/** Where a new release is: being looked for, downloaded, or ready to install on restart. */
export type UpdateState =
  | { state: 'disabled' }
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'downloading'; version: string }
  | { state: 'ready'; version: string };

export interface Participant {
  id: number;
  username: string;
  role: 'Host' | 'Viewer';
  label: string;
}

export interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  lines: string[];
}

/** `truncated`: cut by the host to fit in one message, so only part of it is shown. */
type Stamp = { outputSeq: number; at: number; truncated?: boolean };

/** Use of one of the host account's rate-limit windows. */
export interface RateLimit {
  usedPercent: number;
  /** Seconds since the Unix epoch. */
  resetsAt?: number;
}

/** Tokens used in a session, summed over every model call, subagents included. */
export interface TokenCounts {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}

/** The host's answer to one question; empty when they did not answer. */
export interface Answer {
  question: string;
  /** The labels of the options the host picked. */
  selected: string[];
  /** What the host typed instead of, or besides, picking options. */
  typed?: string;
  notes?: string;
}

/** What the host decided, or what a helper reported, for a call. */
export type ToolOutcome =
  | { kind: 'answers'; answers: Answer[] }
  | { kind: 'plan-decision'; approved: boolean }
  | { kind: 'report'; text: string; truncated: boolean };

/** Whether the host allowed a call Claude Code asked them about. */
export type Permission = 'allowed' | 'denied';

export type TimelineEvent = Stamp &
  (
    | { kind: 'session-started' }
    | { kind: 'session-ended' }
    | { kind: 'prompt'; promptId: string; text: string }
    | { kind: 'message'; promptId: string; messageId: string; text: string; final: boolean }
    | { kind: 'response'; promptId: string; text: string }
    | { kind: 'tool-call'; promptId: string; toolUseId: string; toolName: string; input: Record<string, unknown> }
    | {
        kind: 'tool-result';
        promptId: string;
        toolUseId: string;
        status: 'succeeded' | 'failed' | 'not-run';
        error?: string;
        outcome?: ToolOutcome;
        permission?: Permission;
      }
    | { kind: 'file-change'; promptId: string; toolUseId: string; path: string; change: 'created' | 'edited' | 'deleted'; hunks: DiffHunk[] }
    | { kind: 'file-read'; promptId: string; toolUseId: string; path: string }
    | { kind: 'attention'; message: string }
    | { kind: 'document'; promptId: string; toolUseId: string; path: string; text: string | null }
    | { kind: 'plan'; change: string; documents: PlanDocument[]; tasks: { done: number; total: number } }
    | { kind: 'timeline-unavailable'; reason: string }
    | { kind: 'timeline-available' }
    | { kind: 'usage'; tokens: TokenCounts; contextTokens?: number; contextWindow?: number }
    | { kind: 'usage-unavailable'; reason: string }
    | {
        kind: 'status-line';
        model?: string;
        contextWindow?: number;
        contextTokens?: number;
        effort?: string;
        fast: boolean;
        fiveHour?: RateLimit;
        sevenDay?: RateLimit;
      }
    | { kind: 'git'; folder: string; branch?: string; dirty: boolean; ahead: number; behind: number; worktree?: string }
  );

export type SharingState = 'connecting' | 'ready' | 'unavailable';

/** What became of a suggestion; `SuggestionState` in crates/protocol. */
export type SuggestionState = 'Waiting' | { Sent: { edited: boolean } } | 'Declined';

/** A viewer's own suggestion, as that viewer sees it. */
export interface Suggestion {
  id: number;
  text: string;
  state: SuggestionState;
}

/** A viewer's suggestion as the host sees it. */
export interface HostSuggestion extends Suggestion {
  /** The viewer's number; with `id` it names the suggestion. */
  viewer: number;
  /** The viewer's name as the participant list showed it when the suggestion arrived. */
  from: string;
}

/** Whether a request got through to the host; `Outcome` in crates/protocol. */
export type RequestOutcome = 'Received' | 'Off' | 'RateLimited';

/** The longest suggestion, in bytes of UTF-8; `MAX_SUGGESTION_BYTES` in crates/protocol. */
export const MAX_SUGGESTION_BYTES = 4096;

export type HostEvent =
  | {
      type: 'snapshot';
      agent: AgentId;
      folder: string;
      timelineSupported: boolean;
      sharing: SharingState;
      code: string | null;
      /** The sharing service the session uses, which join links name. */
      service: string;
      /** The service serves the web viewer, so join links open in a browser. */
      webViewer: boolean;
      participants: Participant[];
      status: AgentStatus;
      viewerLimit: number;
      exited: number | null;
      timeline: TimelineEvent[];
      /** False for terminal sessions; absent from sessions of the previous release, which take no suggestions. */
      suggestionsPossible?: boolean;
      suggestionsAllowed?: boolean;
      suggestions?: HostSuggestion[];
    }
  | { type: 'sharing'; state: SharingState; code: string | null }
  /** What viewers may send, after the host changed it. */
  | { type: 'allowed'; suggestions: boolean }
  /** Every suggestion in the session, oldest first, on every change. */
  | { type: 'suggestions'; suggestions: HostSuggestion[] }
  /** Whether the sharing service serves the web viewer, once checked. */
  | { type: 'web-viewer'; available: boolean }
  | { type: 'participants'; participants: Participant[] }
  | { type: 'status'; status: AgentStatus }
  | { type: 'timeline'; event: TimelineEvent }
  | { type: 'exited'; code: number };

export type EndReason =
  | 'closed'
  | 'removed'
  | 'full'
  | 'invalid-code'
  | 'too-many-attempts'
  | 'service-unavailable'
  | 'network-blocked'
  | 'update-app'
  | 'host-update'
  /** The web viewer is older than the host's app: its sharing service needs updating. */
  | 'service-outdated'
  /** The web viewer's browser has WebRTC turned off. */
  | 'webrtc-off';

export type ViewEvent =
  | { type: 'joined'; viewerId: number; agent: AgentId; timelineSupported: boolean; cols: number; rows: number }
  | { type: 'reconnecting' }
  | { type: 'reconnected' }
  | { type: 'timeline'; event: TimelineEvent }
  | { type: 'participants'; participants: Participant[] }
  | { type: 'status'; status: AgentStatus }
  /** What the host lets viewers send. */
  | { type: 'allowed'; suggestions: boolean }
  /** A request was numbered; its reply carries this id. */
  | { type: 'request-sent'; id: number }
  /** Whether a request got through to the host. */
  | { type: 'reply'; id: number; outcome: RequestOutcome }
  /** This viewer's suggestions and what became of each. */
  | { type: 'suggestions'; mine: Suggestion[] }
  | { type: 'ended'; reason: EndReason };

/** The terminal's state for diagnostics: enums and numbers only, never its content. */
export interface TerminalSnapshot {
  screen: 'normal' | 'alternate';
  /** Whether the program in the terminal takes the mouse, and so the wheel. */
  mouseTracking: 'none' | 'x10' | 'vt200' | 'drag' | 'any';
  /** Lines scrolled off the top of the normal screen, against the most it keeps. */
  scrollbackLines: number;
  scrollbackLimit: number;
  /** How far the view is scrolled up from the latest output, in lines. */
  linesFromBottom: number;
  following: boolean;
  fontSize: number;
  recordsWritten: number;
}

/** A peer connection's state. */
export type LinkState = 'new' | 'connecting' | 'connected' | 'disconnected' | 'failed' | 'closed';

/** The host's side of sharing. Times are Unix milliseconds. */
export interface SharingStatus {
  service: 'connecting' | 'connected' | 'unavailable';
  retryAt: number | null;
  turnExpiresAt: number | null;
  viewers: {
    peer: number;
    viewer: number | null;
    state: LinkState;
    relayed: boolean | null;
    recordsSent: number;
    eventsSent: number;
    bytesSent: number;
    queued: number;
  }[];
  lastDeparture: { viewer: number | null; peer: number; reason: string; at: number } | null;
}

/** A viewer's connection. Times are Unix milliseconds. */
export interface ViewerStatus {
  phase: 'joining' | 'connected' | 'reconnecting' | 'ended';
  attempt: number;
  retryAt: number | null;
  lastAttempt: { attempt: number; reason: string; at: number; connectedMs: number } | null;
  signallingConnected: boolean;
  peer: number | null;
  state: LinkState | null;
  relayed: boolean | null;
  recordsApplied: number;
  eventsApplied: number;
  bytesReceived: number;
}

/** One line of the diagnostic log. */
export type DiagnosticEvent = { ts: string; level: string; event: string } & Record<string, string | number | boolean>;

export interface DiagnosticsStatus {
  session: { role: 'host'; sharing: SharingStatus } | { role: 'viewer'; viewer: ViewerStatus } | { role: 'none' };
  /** How large the window's content is, as its heartbeat last said. */
  window: { timelineEvents: number; elements: number } | null;
  terminal: TerminalSnapshot | null;
  /** Newest first. */
  events: DiagnosticEvent[];
  /** Why the log file cannot be written; null when it is. */
  logFileError: string | null;
  verbose: boolean;
  /** The app's clock, Unix milliseconds, for the times above. */
  now: number;
}
