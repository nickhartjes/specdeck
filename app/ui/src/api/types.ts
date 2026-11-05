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

export interface Profile {
  username: string | null;
  recentFolders: RecentFolder[];
  lastAgent: AgentId | null;
  agents: AgentInfo[];
  hosting: boolean;
  /** Whether to show OS notifications while the window is not in front. */
  notifications: boolean;
  /** How the user follows sessions; null until they answered the first-join question. */
  view: View | null;
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
  /** Hosting or viewing a session; the service cannot change meanwhile. */
  inSession: boolean;
  /** This install updates itself (macOS app, Windows installer, AppImage). */
  updatesItself: boolean;
  /** Check for and download new releases in the background. */
  checkUpdates: boolean;
  /** Screen readers read the terminal. */
  screenReader: boolean;
}

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
    }
  | { type: 'sharing'; state: SharingState; code: string | null }
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
  | { type: 'ended'; reason: EndReason };
