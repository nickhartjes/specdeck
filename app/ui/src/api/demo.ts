// A fake bridge with a realistic session, for looking at the UI in a browser (`pnpm dev`, then
// open /?demo=start, /?demo=host or /?demo=viewer) and for component tests. Never used by the app.

import type { Bridge, TerminalSink } from './bridge';
import type { DiagnosticsStatus, HostEvent, HostSuggestion, Participant, Profile, Suggestion, TimelineEvent, ViewEvent } from './types';
import { formatCode } from '../model/code';

const t0 = Date.UTC(2026, 9, 31, 13, 2);

export const demoProfile: Profile = {
  username: 'Nick',
  recentFolders: [
    { path: '/Users/nick/projects/webshop', name: 'webshop' },
    { path: '/Users/nick/projects/specshare', name: 'specshare' },
    { path: '/Users/nick/work/billing-api', name: 'billing-api' },
  ],
  lastAgent: 'claude-code',
  notifications: true,
  view: null,
  layout: null,
  translucent: false,
  agents: [
    {
      id: 'claude-code',
      name: 'Claude Code',
      installed: true,
      hasTimeline: true,
      reportsStatus: true,
      path: '/Users/nick/.local/bin/claude',
      installPage: 'https://code.claude.com/docs/en/setup',
    },
    {
      id: 'open-code',
      name: 'OpenCode',
      installed: true,
      hasTimeline: false,
      reportsStatus: false,
      path: '/opt/homebrew/bin/opencode',
      installPage: 'https://opencode.ai/docs/',
    },
    { id: 'omp', name: 'omp', installed: false, hasTimeline: false, reportsStatus: true, path: null, installPage: 'https://omp.sh' },
    { id: 'terminal', name: 'Terminal', installed: true, hasTimeline: false, reportsStatus: false, path: '/bin/zsh', installPage: null },
  ],
  hosting: false,
  hostedSessions: 0,
  watchedSessions: 0,
  platform: 'macos',
  sharingService: null,
  effectiveService: 'wss://specdeck.hartj.es/signal',
  builtInService: 'wss://specdeck.hartj.es/signal',
  serviceOverridden: false,
  inSession: false,
  updatesItself: true,
  checkUpdates: true,
  screenReader: false,
  verboseLogging: false,
  browser: true,
  screenshots: true,
};

export const demoParticipants: Participant[] = [
  { id: 0, username: 'Nick', role: 'Host', label: 'Nick' },
  { id: 1, username: 'Anna de Vries', role: 'Viewer', label: 'Anna de Vries' },
  { id: 2, username: 'Bram', role: 'Viewer', label: 'Bram' },
  { id: 3, username: 'Anna de Vries', role: 'Viewer', label: 'Anna de Vries (2)' },
];

/** Suggestions waiting for the host in the demo, and one already sent. */
export const demoSuggestions: HostSuggestion[] = [
  { viewer: 1, id: 1, from: 'Anna de Vries', text: 'Can the health check also report the database latency?', state: { Sent: { edited: false } } },
  { viewer: 2, id: 1, from: 'Bram', text: 'Add a test for the 503 when the database is down.', state: 'Waiting' },
  { viewer: 1, id: 2, from: 'Anna de Vries', text: 'Please explain the retry policy in the README,\nwith an example.', state: 'Waiting' },
];

let seq = 0;
const at = (minutes: number) => ({ outputSeq: seq++, at: t0 + minutes * 60_000 });

export const demoTimeline: TimelineEvent[] = [
  { ...at(0), kind: 'session-started' },
  { ...at(0), kind: 'git', folder: 'webshop', branch: 'feature/SHOP-412_health-endpoint', dirty: true, ahead: 2, behind: 0 },
  {
    ...at(4),
    kind: 'status-line',
    model: 'Opus 5.5',
    contextWindow: 1_000_000,
    contextTokens: 178_200,
    effort: 'medium',
    fast: false,
    fiveHour: { usedPercent: 23, resetsAt: Math.floor(Date.now() / 1000) + 2 * 3600 + 14 * 60 },
    sevenDay: { usedPercent: 41, resetsAt: Math.floor(Date.now() / 1000) + 3 * 86_400 + 2 * 3600 },
  },
  { ...at(1), kind: 'prompt', promptId: 'p1', text: 'Add a /health endpoint to the API that reports the database status.' },
  { ...at(1), kind: 'message', promptId: 'p1', messageId: 'm1', text: "I'll look at how the routes are set up first.", final: true },
  { ...at(1), kind: 'tool-call', promptId: 'p1', toolUseId: 't1', toolName: 'Read', input: { file_path: 'src/routes/index.ts' } },
  { ...at(1), kind: 'tool-result', promptId: 'p1', toolUseId: 't1', status: 'succeeded' },
  {
    ...at(1),
    kind: 'tool-call',
    promptId: 'p1',
    toolUseId: 'q1',
    toolName: 'AskUserQuestion',
    input: {
      questions: [
        {
          question: 'What should /health return when the database is down?',
          header: 'Status',
          options: [
            { label: '503 Service Unavailable', description: 'Load balancers take the node out of rotation' },
            { label: '200 with database: "down"', description: 'Always up; the body says what is wrong' },
          ],
          multiSelect: false,
        },
      ],
    },
  },
  {
    ...at(1),
    kind: 'tool-result',
    promptId: 'p1',
    toolUseId: 'q1',
    status: 'succeeded',
    outcome: { kind: 'answers', answers: [{ question: 'What should /health return when the database is down?', selected: ['503 Service Unavailable'] }] },
  },
  { ...at(1), kind: 'file-read', promptId: 'p1', toolUseId: 't1', path: 'src/routes/index.ts' },
  { ...at(2), kind: 'tool-call', promptId: 'p1', toolUseId: 't2', toolName: 'Write', input: { file_path: 'src/routes/health.ts' } },
  { ...at(2), kind: 'tool-result', promptId: 'p1', toolUseId: 't2', status: 'succeeded' },
  {
    ...at(2),
    kind: 'file-change',
    promptId: 'p1',
    toolUseId: 't2',
    path: 'src/routes/health.ts',
    change: 'created',
    hunks: [
      {
        oldStart: 0,
        oldLines: 0,
        newStart: 1,
        newLines: 9,
        lines: [
          "+import { Router } from 'express';",
          "+import { db } from '../db';",
          '+',
          '+export const health = Router();',
          '+',
          "+health.get('/health', async (_req, res) => {",
          '+  const database = await db.ping().then(() => "up", () => "down");',
          '+  res.status(database === "up" ? 200 : 503).json({ status: "ok", database });',
          '+});',
        ],
      },
    ],
  },
  { ...at(2), kind: 'tool-call', promptId: 'p1', toolUseId: 't3', toolName: 'Edit', input: { file_path: 'src/routes/index.ts' } },
  { ...at(2), kind: 'tool-result', promptId: 'p1', toolUseId: 't3', status: 'succeeded' },
  {
    ...at(2),
    kind: 'file-change',
    promptId: 'p1',
    toolUseId: 't3',
    path: 'src/routes/index.ts',
    change: 'edited',
    hunks: [
      {
        oldStart: 3,
        oldLines: 4,
        newStart: 3,
        newLines: 6,
        lines: [" import { orders } from './orders';", "+import { health } from './health';", ' ', ' export const routes = Router();', "+routes.use(health);", " routes.use('/orders', orders);"],
      },
    ],
  },
  { ...at(3), kind: 'tool-call', promptId: 'p1', toolUseId: 't4', toolName: 'Bash', input: { command: 'npm test -- health', description: 'Run the health tests' } },
  { ...at(3), kind: 'tool-result', promptId: 'p1', toolUseId: 't4', status: 'failed', error: 'Exit code 1: no test files match "health"', permission: 'allowed' },
  {
    ...at(3),
    kind: 'response',
    promptId: 'p1',
    text: 'Added **`GET /health`** in `src/routes/health.ts` and registered it in the router.\n\n- Returns `200` with `{ status: "ok", database: "up" }` when the database answers\n- Returns `503` when it does not\n\nThere are no tests for it yet; want me to add one?',
  },
  { ...at(3), kind: 'usage', tokens: { input: 14, output: 2310, cacheRead: 96400, cacheWrite: 21800 }, contextTokens: 24180, contextWindow: 200_000 },
  { ...at(4), kind: 'prompt', promptId: 'p2', text: 'Yes, add a test with a stubbed database.' },
  { ...at(4), kind: 'message', promptId: 'p2', messageId: 'm2', text: 'Writing a test that stubs `db.ping` for both cases.', final: false },
  { ...at(4), kind: 'tool-call', promptId: 'p2', toolUseId: 't5', toolName: 'Bash', input: { command: 'npm test -- health', description: 'Run the health tests again' } },
  { ...at(4), kind: 'usage', tokens: { input: 19, output: 2890, cacheRead: 148200, cacheWrite: 26500 }, contextTokens: 151320, contextWindow: 200_000 },
];

const encoder = new TextEncoder();
const esc = '\x1b[';
const terminalText = [
  `${esc}38;5;245m╭──────────────────────────────────────────────────╮${esc}0m`,
  `${esc}38;5;245m│${esc}0m ${esc}38;2;217;119;87m✻${esc}0m Welcome to ${esc}1mClaude Code${esc}0m!                       ${esc}38;5;245m│${esc}0m`,
  `${esc}38;5;245m│${esc}0m   ${esc}2mcwd: /Users/nick/projects/webshop${esc}0m              ${esc}38;5;245m│${esc}0m`,
  `${esc}38;5;245m╰──────────────────────────────────────────────────╯${esc}0m`,
  '',
  `${esc}2m>${esc}0m Add a /health endpoint to the API that reports the database status.`,
  '',
  `${esc}38;2;217;119;87m●${esc}0m I'll look at how the routes are set up first.`,
  '',
  `${esc}32m●${esc}0m ${esc}1mRead${esc}0m(src/routes/index.ts)`,
  `  ⎿  Read 24 lines`,
  `${esc}32m●${esc}0m ${esc}1mWrite${esc}0m(src/routes/health.ts)`,
  `  ⎿  Wrote 9 lines to src/routes/health.ts`,
  `${esc}32m●${esc}0m ${esc}1mUpdate${esc}0m(src/routes/index.ts)`,
  `  ⎿  Updated src/routes/index.ts with 2 additions`,
  `${esc}31m●${esc}0m ${esc}1mBash${esc}0m(npm test -- health)`,
  `  ⎿  ${esc}31mError: no test files match "health"${esc}0m`,
  '',
  `${esc}38;2;217;119;87m●${esc}0m Added ${esc}1mGET /health${esc}0m in src/routes/health.ts and registered it.`,
  '  There are no tests for it yet; want me to add one?',
  '',
  `${esc}2m>${esc}0m Yes, add a test with a stubbed database.`,
  '',
  `${esc}38;2;217;119;87m✻${esc}0m ${esc}38;2;217;119;87mWriting tests…${esc}0m ${esc}2m(12s · esc to interrupt)${esc}0m`,
].join('\r\n');

function terminalStream(): ArrayBuffer {
  const bytes = encoder.encode(terminalText);
  const out = new Uint8Array(5 + bytes.length);
  out[0] = 1;
  new DataView(out.buffer).setUint32(1, bytes.length);
  out.set(bytes, 5);
  return out.buffer;
}

/** A terminal stream record that resizes the terminal. */
function resizeRecord(cols: number, rows: number): ArrayBuffer {
  const record = new Uint8Array(5);
  record[0] = 2;
  new DataView(record.buffer).setUint16(1, cols);
  new DataView(record.buffer).setUint16(3, rows);
  return record.buffer;
}

/** The Connection status panel's view of the demo session. */
export function demoDiagnostics(screen: string): DiagnosticsStatus {
  const now = Date.now();
  const at = (secondsAgo: number) => new Date(now - secondsAgo * 1000).toISOString();
  const terminal: DiagnosticsStatus['terminal'] = {
    screen: 'alternate',
    mouseTracking: 'any',
    scrollbackLines: 0,
    scrollbackLimit: 100_000,
    linesFromBottom: 0,
    following: true,
    fontSize: 13,
    recordsWritten: 1_204,
  };
  if (screen === 'viewer') {
    return {
      session: {
        role: 'viewer',
        viewer: {
          phase: 'connected',
          attempt: 2,
          retryAt: null,
          lastAttempt: { attempt: 1, reason: 'peer_failed', at: now - 95_000, connectedMs: 3_600_000 },
          signallingConnected: true,
          peer: 2,
          state: 'connected',
          relayed: true,
          recordsApplied: 1_204,
          eventsApplied: 87,
          bytesReceived: 2_400_000,
        },
      },
      window: { timelineEvents: 87, elements: 4_210 },
      terminal,
      events: [
        { ts: at(90), level: 'INFO', event: 'peer.path', peer: 2, local: 'relay', remote: 'srflx', relayed: true },
        { ts: at(92), level: 'INFO', event: 'viewer.attempt_started', attempt: 2 },
        { ts: at(95), level: 'WARN', event: 'viewer.attempt_ended', attempt: 1, reason: 'peer_failed', connected_ms: 3_600_000 },
        { ts: at(95), level: 'WARN', event: 'peer.state', peer: 1, state: 'disconnected' },
      ],
      logFileError: null,
      verbose: false,
      now,
    };
  }
  return {
    session: {
      role: 'host',
      sharing: {
        service: 'connected',
        retryAt: null,
        turnExpiresAt: now + 11 * 3_600_000,
        viewers: [
          { peer: 1, viewer: 1, state: 'connected', relayed: false, recordsSent: 1_204, eventsSent: 87, bytesSent: 2_400_000, queued: 0 },
          { peer: 3, viewer: 2, state: 'connected', relayed: true, recordsSent: 1_190, eventsSent: 87, bytesSent: 2_380_000, queued: 4_096 },
        ],
        lastDeparture: null,
      },
    },
    window: { timelineEvents: 87, elements: 4_480 },
    terminal,
    events: [
      { ts: at(30), level: 'INFO', event: 'host.viewer_admitted', viewer: 2, peer: 3, returning: false },
      { ts: at(31), level: 'INFO', event: 'peer.path', peer: 3, local: 'host', remote: 'relay', relayed: true },
      { ts: at(600), level: 'INFO', event: 'host.turn_credentials', expires_in_s: 43_200 },
      { ts: at(600), level: 'INFO', event: 'host.signalling_up' },
    ],
    logFileError: null,
    verbose: false,
    now,
  };
}

export function demoBridge(screen: string): Bridge {
  const profile = { ...demoProfile, hosting: screen === 'host', hostedSessions: screen === 'host' ? 1 : 0 };
  // Like the engine, the demo sends each resize of the host's terminal back as a record.
  let hostTerminal: TerminalSink | null = null;
  // Suggestions: the host's side and one viewer's side of the same demo.
  let hostEvents: ((event: HostEvent) => void) | null = null;
  let hostSuggestions: HostSuggestion[] = demoSuggestions.map((suggestion) => ({ ...suggestion }));
  let viewerEvents: ((event: ViewEvent) => void) | null = null;
  const mine: Suggestion[] = [];
  let nextRequest = 1;
  return {
    getProfile: async () => profile,
    agentVersions: async () => [
      { agent: 'claude-code', version: '2.1.309' },
      { agent: 'open-code', version: '2.2.5' },
    ],
    openInstallPage: async () => {},
    setUsername: async (username) => ({ ...profile, username }),
    setNotifications: async (notifications) => ({ ...profile, notifications }),
    setCheckUpdates: async (checkUpdates) => ({ ...profile, checkUpdates }),
    setScreenReader: async (screenReader) => ({ ...profile, screenReader }),
    setVerboseLogging: async (verboseLogging) => ({ ...profile, verboseLogging }),
    setBrowser: async (browser) => ({ ...profile, browser }),
    setScreenshots: async (screenshots) => ({ ...profile, screenshots }),
    // The start screen shows an update that is ready to install.
    updateStatus: async () => (screen === 'start' ? { state: 'ready', version: '1.2.1' } : { state: 'idle' }),
    onUpdate: () => () => {},
    restartToUpdate: async () => {},
    setLayout: async (layout) => ({ ...profile, layout }),
    setSharingService: async (service) => {
      const url = service.trim() ? `wss://${service.trim()}/signal` : null;
      return { ...profile, sharingService: url, effectiveService: url ?? profile.builtInService };
    },
    checkSharingService: async () => ({ reachable: true }),
    removeRecentFolder: async (path) => ({ ...profile, recentFolders: profile.recentFolders.filter((f) => f.path !== path) }),
    chooseFolder: async () => '/Users/nick/projects/webshop',
    startHosting: async () => {},
    hostAttach: async (terminal: TerminalSink, events: (event: HostEvent) => void) => {
      hostTerminal = terminal;
      events({
        type: 'snapshot',
        agent: 'claude-code',
        folder: '/Users/nick/projects/webshop',
        timelineSupported: true,
        sharing: 'ready',
        code: '7K2M9QXH4TPA',
        service: 'wss://specdeck.hartj.es/signal',
        webViewer: false,
        participants: demoParticipants,
        status: 'working',
        viewerLimit: 20,
        exited: null,
        timeline: demoTimeline,
        suggestionsPossible: true,
        suggestionsAllowed: true,
        suggestions: hostSuggestions,
        wholeFilesPossible: true,
        wholeFiles: false,
      });
      hostEvents = events;
      setTimeout(() => terminal(terminalStream()), 50);
    },
    hostInput: async () => {},
    hostResize: async (cols, rows) => {
      setTimeout(() => hostTerminal?.(resizeRecord(cols, rows)), 40);
    },
    hostSetColors: async () => {},
    hostNewCode: async () => {},
    hostRemoveViewer: async () => {},
    hostSetViewerLimit: async () => {},
    hostAllowSuggestions: async (on) => {
      hostEvents?.({ type: 'allowed', suggestions: on });
    },
    hostShareCode: async (wholeFiles) => {
      hostEvents?.({ type: 'shared', wholeFiles });
    },
    hostAnswerSuggestion: async (viewerId, id, text) => {
      hostSuggestions = hostSuggestions.map((suggestion) =>
        suggestion.viewer === viewerId && suggestion.id === id
          ? { ...suggestion, state: text === null ? 'Declined' : { Sent: { edited: text !== suggestion.text } } }
          : suggestion,
      );
      hostEvents?.({ type: 'suggestions', suggestions: hostSuggestions });
    },
    hostClose: async () => {},
    joinSession: async (_code, _server, terminal: TerminalSink, events: (event: ViewEvent) => void) => {
      setTimeout(() => {
        // ?cols=&rows= sets the host's terminal size, for trying how a viewer scales it. Read
        // through globalThis: in tests the page can be gone by the time this timer fires.
        const params = new URLSearchParams(globalThis.location?.search ?? '');
        const cols = Number(params.get('cols')) || 100;
        const rows = Number(params.get('rows')) || 32;
        // ?agent=omp shows an agent that reports a request; ?agent=terminal a terminal session
        // with a program that reports it is working.
        const agent = params.get('agent');
        if (agent === 'omp' || agent === 'terminal') {
          events({ type: 'joined', viewerId: 2, agent, timelineSupported: false, cols, rows });
          events({ type: 'participants', participants: demoParticipants });
          if (agent === 'omp') {
            events({ type: 'timeline', event: { ...at(1), kind: 'attention', message: 'omp needs permission: approve Write: src/routes/health.ts' } });
            events({ type: 'status', status: 'waiting' });
          } else {
            events({ type: 'status', status: 'working' });
          }
        } else {
          events({ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols, rows });
          events({ type: 'participants', participants: demoParticipants });
          events({ type: 'status', status: 'working' });
          for (const event of demoTimeline) events({ type: 'timeline', event });
        }
        viewerEvents = events;
        events({ type: 'allowed', suggestions: params.get('suggestions') !== 'off' });
        events({ type: 'shared', wholeFiles: params.get('whole') === 'on' });
        events({ type: 'suggestions', mine: [] });
        // A viewer takes the host's size from the stream's resize records, as from the engine.
        terminal(resizeRecord(cols, rows));
        terminal(terminalStream());
      }, 300);
    },
    leaveSession: async () => {},
    suggestPrompt: async (text) => {
      const id = nextRequest++;
      viewerEvents?.({ type: 'request-sent', id });
      mine.push({ id, text, state: 'Waiting' });
      viewerEvents?.({ type: 'reply', id, outcome: 'Received' });
      viewerEvents?.({ type: 'suggestions', mine: [...mine] });
      // The demo host declines every other suggestion after a while.
      setTimeout(() => {
        const suggestion = mine.find((s) => s.id === id);
        if (!suggestion) return;
        suggestion.state = id % 2 ? { Sent: { edited: false } } : 'Declined';
        viewerEvents?.({ type: 'suggestions', mine: mine.map((s) => ({ ...s })) });
      }, 4000);
    },
    takeJoinLink: async () => null,
    takeStartCommand: async () => null,
    setSessionTitle: async () => {},
    readJoinLink: async (link) => ({ code: formatCode(link) }),
    copyText: async () => {},
    runningSessions: async () => [],
    openRunningSession: async () => {},
    endRunningSession: async () => {},
    onRunningSessions: () => () => {},
    startScreenShown: async () => {},
    reportTerminal: async () => {},
    diagnosticsStatus: async () => demoDiagnostics(screen),
    reconnectViewer: async () => {},
    heartbeat: async () => {},
    exportDiagnostics: async () => true,
    // The demo has no native menu; window.demoMenu({ id: 'connection-status' }) stands in for it.
    onMenu: (handler) => {
      (window as { demoMenu?: typeof handler }).demoMenu = handler;
      return () => {};
    },
  };
}
