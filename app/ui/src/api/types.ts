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

/** A kind of notification a viewer can switch on or off; `reads` and `estimates` are the host's asks. */
export type ViewerNotificationKind = 'plans' | 'documents' | 'finished' | 'reads' | 'estimates';

export type ViewerNotifications = Record<ViewerNotificationKind, boolean>;

/** What a click on a viewer's notification opens: a document, or a plan the agent presented in plan mode. */
export type NoticeOpen = { kind: 'document'; path: string } | { kind: 'plan-mode'; toolUseId: string };

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
  /** Which kinds of notification a viewer gets about a session they watch (`viewer-notifications`). */
  viewerNotifications: ViewerNotifications;
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
  /** New sessions show screenshots in the timeline. */
  screenshots: boolean;
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
  | { kind: 'report'; text: string; truncated: boolean }
  | Screenshot;

/** An image an MCP tool returned, such as a browser's screenshot; without `image` when it was left out. */
export interface Screenshot {
  kind: 'screenshot';
  image?: ScreenshotImage;
  /** The page shown, without its scheme; only from SpecDeck's own browser. */
  address?: string;
  leftOut?: LeftOut;
}

/** Base64 data of `mediaType`, `width` by `height` pixels. */
export interface ScreenshotImage {
  mediaType: string;
  data: string;
  width: number;
  height: number;
}

/** Why a screenshot's image was left out: the host turned screenshots off, or it could not be sent. */
export type LeftOut = 'turned-off' | 'too-large' | 'unreadable';

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
    /** A code file's whole text after its change, while the host shares whole code files; null when too large. */
    | { kind: 'file-text'; promptId: string; toolUseId: string; path: string; text: string | null }
    /** Someone shared a file with everyone (`shared-files`); `by` is 0 for the host. */
    | {
        kind: 'file-shared';
        file: number;
        name: string;
        fileKind: FileKind;
        mediaType: string;
        size: number;
        sha256: string;
        by: number;
        byLabel: string;
      }
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
    /** A link shared with everyone; `url` is null once the host removed it. `by` is the sharer's name. */
    | { kind: 'link'; id: number; url: string | null; title?: string; by: string; sessionOnly?: boolean }
    /** The host removed link `id`. */
    | { kind: 'link-removed'; id: number }
    /** A read request closed: how many of those who could answer were ready or had a question. No names or question texts. */
    | { kind: 'read-done'; id: number; target: ReadTarget; label: string; ready: number; questions: number; of: number; timeUp: boolean }
    /** The host kept an estimate: the lowest and highest value card voted (null when every vote was ? or ☕). No names. */
    | { kind: 'estimated'; id: number; task: string; cards: CardSet; kept: Card; low: Card | null; high: Card | null; votes: number }
  );

/** What the host asks everyone to read; `ReadTarget` in crates/protocol. `changes` are the timeline events after `fromSeq` (a prompt) up to `toSeq`. */
export type ReadTarget =
  | { document: { path: string } }
  | { code: { path: string } }
  | { shared: { file: number } }
  | { changes: { fromSeq: number; toSeq: number } };

/** The cards of a vote: story points (1, 2, 3, 5, 8, 13) or T-shirt sizes (XS to XL), both with ? and ☕. */
export type CardSet = 'story-points' | 't-shirt';

/** A card: `value` indexes into its set's value cards. */
export type Card = { value: number } | 'unsure' | 'break';

/** A viewer's answer to a read request. */
export type ReadAnswer = 'reading' | 'ready' | { question: { text: string | null } };

/** How everyone sees a participant during a read request; `older-app` can't answer. */
export type ReadState = 'reading' | 'ready' | 'question' | 'older-app';

/** The open read request as one participant sees it. */
export interface ReadView {
  id: number;
  target: ReadTarget;
  label: string;
  /** 0 for no limit. */
  limitS: number;
  /** Time left when it was sent; null without a limit. */
  leftMs: number | null;
  /** Each participant asked, by id, the host left out. */
  people: [number, ReadState][];
  /** This participant's own answer, with their own question's text. */
  mine: ReadAnswer;
  /** Every question's text by participant id: only in the host's view. */
  questions: [number, string][];
}

/** The open vote as one participant sees it: no vote's value but their own before the reveal. */
export interface VoteView {
  id: number;
  round: number;
  task: string;
  cards: CardSet;
  /** Who has voted this round, by participant id, 0 for the host. */
  voted: number[];
  mine: Card | null;
  /** Every vote by participant id, once revealed. */
  revealed: [number, Card][] | null;
  /** Participants on an older app, which can't vote. */
  older: number[];
}

/** The host's asks as the host's window shows them. */
export interface HostAsks {
  read: ReadView | null;
  vote: VoteView | null;
}

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

/** What became of a viewer's link; `SentLinkState` in crates/protocol. */
export type SentLinkState = 'Waiting' | 'Shared' | 'Declined';

/** A viewer's own link, as that viewer sees it. */
export interface SentLink {
  id: number;
  url: string;
  title: string | null;
  sessionOnly: boolean;
  state: SentLinkState;
}

/** A viewer's link as the host sees it. */
export interface HostLink extends SentLink {
  /** The viewer's number; with `id` it names the link. */
  viewer: number;
  /** The viewer's name as the participant list showed it when the link arrived. */
  from: string;
}

/** Whether a request got through to the host; `Outcome` in crates/protocol. */
export type RequestOutcome = 'Received' | 'Off' | 'RateLimited' | 'TooLarge' | 'Full' | 'NotAllowedType' | 'Gone' | { Ready: { upload: number } };

/** What a shared file is, decided by the host from its content. */
export type FileKind = 'image' | 'markdown' | 'text';

/** A file shared with everyone in the session; `SharedFileInfo` in crates/protocol. */
export interface SharedFileInfo {
  file: number;
  name: string;
  kind: FileKind;
  mediaType: string;
  size: number;
  sha256: number[];
  /** Who shared it: 0 for the host, else the viewer's number. */
  by: number;
  /** Their name when they shared it. */
  byLabel: string;
  /** When, in milliseconds since the epoch. */
  atMs: number;
}

/** Largest shared file, in bytes; `MAX_SHARED_FILE_BYTES` in crates/protocol. */
export const MAX_SHARED_FILE_BYTES = 10 * 1024 * 1024;

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
      /** False for sessions without a timeline; absent from sessions of the previous release, which never share whole code files. */
      wholeFilesPossible?: boolean;
      wholeFiles?: boolean;
      /** Absent from sessions of the previous release, which take no shared files. */
      sharedFilesPossible?: boolean;
      filesAllowed?: boolean;
      sharedFiles?: SharedFileInfo[];
      /** Absent from sessions of the previous release, which can't share links. */
      linksPossible?: boolean;
      linksAllowed?: boolean;
      /** Every link viewers sent, oldest first. */
      viewerLinks?: HostLink[];
      /** Absent from sessions of the previous release, which can't ask to read or estimate. */
      asksPossible?: boolean;
      asks?: HostAsks;
    }
  | { type: 'sharing'; state: SharingState; code: string | null }
  /** What viewers may send, after the host changed it. */
  | { type: 'allowed'; suggestions: boolean }
  /** Every suggestion in the session, oldest first, on every change. */
  | { type: 'suggestions'; suggestions: HostSuggestion[] }
  /** Whether viewers can open whole code files, after the host changed it. */
  | { type: 'shared'; wholeFiles: boolean }
  /** The shared files, or whether viewers may share them, changed. */
  | { type: 'shared-files'; files: SharedFileInfo[]; filesAllowed: boolean }
  /** Whether viewers may send links, after the host changed it. */
  | { type: 'links-allowed'; links: boolean }
  /** Every link viewers sent, oldest first, on every change. */
  | { type: 'viewer-links'; links: HostLink[] }
  /** The open read request and vote, on every change. */
  | ({ type: 'asks' } & HostAsks)
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
  /** The sharing service is at a capacity limit. */
  | 'service-busy'
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
  /** Whether the host shares whole code files; never sent by a host of the previous release. */
  | { type: 'shared'; wholeFiles: boolean }
  /** Whether the host lets viewers share files; never sent by a host of the previous release. */
  | { type: 'files-allowed'; files: boolean }
  /** The files shared in the session now. */
  | { type: 'shared-files'; files: SharedFileInfo[] }
  /** Bytes of a shared file this viewer is opening. */
  | { type: 'file-progress'; file: number; received: number; size: number }
  /** Whether the host lets viewers send links; never sent by a host of the previous release. */
  | { type: 'links-allowed'; links: boolean }
  /** This viewer's links and what became of each. */
  | { type: 'links'; mine: SentLink[] }
  /** The host's open read request; null once it closed. Never sent by a host of the previous release. */
  | { type: 'read-request'; open: ReadView | null }
  /** The host's open vote; null once it closed. */
  | { type: 'vote'; open: VoteView | null }
  /** An answer or vote was numbered; its reply carries this id. */
  | { type: 'answer-sent'; id: number }
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
