import { useCallback, useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, Eye, LogOut, WifiOff } from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import type { AgentId, AgentStatus, EndReason, Participant, TerminalSnapshot, TimelineEvent, ViewEvent } from '../api/types';
import { StatusPill } from '../components/StatusPill';
import { StatusBar } from '../components/StatusBar';
import { FirstJoinDialog } from '../components/FirstJoinDialog';
import { LayoutControls } from '../components/LayoutMenu';
import { PeopleButton } from '../components/PeopleButton';
import { choose, type Layout } from '../model/layout';
import { useLayout } from './useLayout';
import { statusBarInfo } from '../model/statusbar';
import { nextRequest } from '../model/request';
import { formatCode } from '../model/code';
import { AGENT_NAMES } from './HostScreen';
import { Workspace, type WorkspaceHandle } from './Workspace';
import { setTimelineEvents } from '../model/heartbeat';

/** What to tell the viewer when their session ends, and whether they can still read it. */
export const END_MESSAGES: Record<EndReason, { title: string; text: string; keepReading?: boolean; retry?: boolean }> = {
  closed: { title: 'The host ended the session', text: 'You can keep reading what was shared until you leave.', keepReading: true },
  removed: { title: 'The host removed you', text: 'You are no longer connected to this session.', keepReading: true },
  full: { title: 'This session is full', text: 'The host has reached their viewer limit. Ask them to raise it, then try again.', retry: true },
  'invalid-code': {
    title: 'That code does not work',
    text: 'It does not match an open session. Check it with the host: codes stop working when the host ends the session or makes a new one.',
    retry: true,
  },
  'too-many-attempts': { title: 'Too many attempts', text: 'Several wrong codes were tried from this network. Wait a minute, then try again.', retry: true },
  'service-unavailable': {
    title: 'SpecDeck cannot reach its sharing service',
    text: 'Check your internet connection, then try again.',
    retry: true,
  },
  'network-blocked': {
    title: 'Your network does not allow the connection',
    text: 'SpecDeck connects over UDP, which some company networks block. Try another network, such as your phone’s hotspot.',
    retry: true,
  },
  'update-app': { title: 'Update SpecDeck to join', text: 'The host uses a newer version of SpecDeck than you.', retry: false },
  'host-update': { title: 'The host needs to update', text: 'The host uses an older version of SpecDeck. Ask them to update, then try again.', retry: true },
  'service-outdated': {
    title: 'This web viewer needs to be updated',
    text: 'The host uses a newer SpecDeck than this sharing service’s web viewer. Ask whoever runs the service to update it, or watch in the SpecDeck app.',
    retry: false,
  },
  'webrtc-off': {
    title: 'This browser blocks the connection',
    text: 'Its settings turn off WebRTC, which watching a session needs. Allow WebRTC, use another browser, or watch in the SpecDeck app.',
    retry: false,
  },
};

interface State {
  phase: 'joining' | 'live' | 'reconnecting' | 'ended';
  ended: EndReason | null;
  viewerId: number | undefined;
  agent: AgentId | null;
  timelineSupported: boolean;
  participants: Participant[];
  status: AgentStatus;
  timeline: TimelineEvent[];
  /** What the agent waits on, if it does. */
  request: string | null;
}

const initial: State = {
  phase: 'joining',
  ended: null,
  viewerId: undefined,
  agent: null,
  timelineSupported: false,
  participants: [],
  status: 'unknown',
  timeline: [],
  request: null,
};

export function viewerReducer(state: State, event: ViewEvent): State {
  switch (event.type) {
    case 'joined':
      return { ...state, phase: 'live', viewerId: event.viewerId, agent: event.agent, timelineSupported: event.timelineSupported };
    case 'reconnecting':
      return state.phase === 'ended' ? state : { ...state, phase: 'reconnecting' };
    case 'reconnected':
      return { ...state, phase: 'live' };
    case 'timeline':
      return { ...state, timeline: [...state.timeline, event.event], request: nextRequest(state.request, event) };
    case 'participants':
      return { ...state, participants: event.participants };
    case 'status':
      return { ...state, status: event.status, request: nextRequest(state.request, event) };
    case 'ended':
      return { ...state, phase: 'ended', ended: event.reason };
  }
}

interface Props {
  code: string;
  /** How this person arranges the window; null until they answered the first-join question. */
  layout: Layout | null;
  onLayout: (layout: Layout) => void;
  /** Screen readers read the terminal; the setting in Settings. */
  screenReader?: boolean;
  /** Back to the start screen; with the code and a message when the join did not work. */
  onLeave: (retry?: { code: string; server?: string; message: string }) => void;
  /** The sharing service of a join link the user agreed to join through; their own when unset. */
  server?: string;
  /** Shown in the toolbar when a new SpecDeck is ready to install. */
  update?: ReactNode;
}

/** Watching someone else's session, read-only. */
export function ViewerScreen({ code, server, layout: stored, onLayout, screenReader = false, onLeave, update }: Props) {
  const bridge = useBridge();
  const onTerminalState = useCallback((snapshot: TerminalSnapshot) => void bridge.reportTerminal(snapshot).catch(() => {}), [bridge]);
  const [layout, setLayout] = useLayout(stored, onLayout);
  const [asking, setAsking] = useState(stored === null);
  const shown = layout ?? choose('developer');
  const workspace = useRef<WorkspaceHandle>(null);
  const [state, dispatch] = useReducer(viewerReducer, initial);
  useEffect(() => setTimelineEvents(state.timeline.length), [state.timeline.length]);

  const joined = useRef(false);
  useEffect(() => {
    // Join once, also when React mounts the screen twice in development.
    if (joined.current) return;
    joined.current = true;
    bridge
      .joinSession(code, server ?? null, (data) => workspace.current?.terminalData(data), dispatch)
      .catch((error) => onLeave({ code, server, message: errorMessage(error) }));
    // Joins once per screen; leaving is explicit or happens when the window closes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const leave = () => {
    void bridge.leaveSession();
    onLeave();
  };

  const host = state.participants.find((p) => p.role === 'Host');
  // The window's title names the host's session, as the toolbar does.
  const title = host ? `${host.label}'s session` : null;
  useEffect(() => {
    if (title) void bridge.setSessionTitle(title).catch(() => {});
  }, [bridge, title]);
  useEffect(() => () => void bridge.setSessionTitle(null).catch(() => {}), [bridge]);
  const agentName = state.agent ? AGENT_NAMES[state.agent] : 'the agent';
  const terminal = state.agent === 'terminal';
  const ending = state.ended ? END_MESSAGES[state.ended] : null;
  const statusBar = useMemo(() => statusBarInfo(state.timeline, state.timelineSupported), [state.timeline, state.timelineSupported]);

  // Ended before ever joining: explain, and offer to try again.
  if (ending && !ending.keepReading && state.viewerId === undefined) {
    return (
      <div className="center-state">
        <div className="box" role="alert">
          <AlertTriangle className="big" aria-hidden />
          <h2>{ending.title}</h2>
          <p>{ending.text}</p>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn" onClick={() => onLeave()}>
              Back
            </button>
            {ending.retry && (
              <button className="btn primary" onClick={() => onLeave({ code, server, message: ending.title })}>
                Try again
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="session">
      <header className="toolbar" data-tauri-drag-region>
        <div className="title" data-tauri-drag-region>
          <strong>{host ? `${host.label}'s session` : 'Joining session…'}</strong>
          <span>
            {state.agent ? AGENT_NAMES[state.agent] : 'Invite code'} · {formatCode(code)}
          </span>
        </div>
        <span className="pill readonly" title="Only the host can type in this session">
          <Eye size={13} aria-hidden /> Read-only
        </span>
        {state.phase !== 'joining' && state.phase !== 'ended' && <StatusPill status={state.status} agentName={AGENT_NAMES[state.agent ?? 'claude-code']} terminal={terminal} request={state.timelineSupported ? null : state.request} />}
        {state.phase === 'ended' && <span className="pill ended">Ended</span>}
        <div className="spacer" />
        {update}
        {state.participants.length > 0 && <PeopleButton participants={state.participants} me={state.viewerId} />}
        <LayoutControls layout={shown} onLayout={setLayout} onAskAgain={() => setAsking(true)} />
        <button className="btn" onClick={leave}>
          <LogOut aria-hidden /> Leave
        </button>
      </header>
      <div>
        {!shown.terminalTab && state.phase !== 'joining' && !state.timelineSupported && (
          <div className="banner info" role="status">
            This session can only be followed in the terminal: {terminal ? 'a terminal session has no steps to show.' : `${agentName} does not share its steps.`}
          </div>
        )}
        {state.phase === 'reconnecting' && (
          <div className="banner warning" role="status">
            <WifiOff aria-hidden /> Connection lost. Reconnecting… You will not miss anything; the session catches up when you are back.
          </div>
        )}
        {state.phase === 'ended' && ending && (
          <div className="banner info" role="status">
            {ending.title}. {ending.text}
          </div>
        )}
      </div>
      <div style={{ position: 'relative', minHeight: 0, display: 'grid' }}>
        <Workspace
          ref={workspace}
          mode="viewer"
          onTerminalState={onTerminalState}
          agentName={agentName}
          terminal={terminal}
          screenReader={screenReader}
          layout={shown}
          onLayout={setLayout}
          timelineSupported={state.timelineSupported}
          timeline={state.timeline}
        />
        {state.phase === 'joining' && (
          <div className="overlay" style={{ position: 'absolute', background: 'var(--window)' }}>
            <div className="box" style={{ display: 'grid', gap: 12, justifyItems: 'center', textAlign: 'center' }} role="status">
              <span className="spinner" />
              <strong>Joining the session…</strong>
              <span className="field-hint">Connecting to the host. This usually takes a few seconds.</span>
            </div>
          </div>
        )}
      </div>
      <StatusBar info={statusBar} />
      {asking && (
        <FirstJoinDialog
          onChoose={(preset) => {
            setAsking(false);
            setLayout(choose(preset, layout ?? undefined));
          }}
        />
      )}
    </div>
  );
}
