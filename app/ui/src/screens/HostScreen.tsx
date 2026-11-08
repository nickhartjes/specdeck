import { useCallback, useEffect, useMemo, useReducer, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, BookOpen, Copy, Gauge, Link2, Power, RefreshCw } from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import type { AgentId, AgentStatus, BraindumpItemInfo, HostAsks, HostEvent, HostLink, HostSuggestion, Participant, SharedFileInfo, SharingState, TerminalSnapshot, TimelineEvent } from '../api/types';
import { AskReadDialog } from '../components/AskReadDialog';
import { EstimateDialog } from '../components/EstimateDialog';
import { ReadBanner } from '../components/ReadBanner';
import { VoteBanner } from '../components/VoteBanner';
import { shareEach, type SharedFiles } from '../components/SharedFiles';
import { useSharedBytes } from './useSharedBytes';
import { SuggestionsInbox } from '../components/SuggestionsInbox';
import { LinksBar } from '../components/LinksBar';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { StatusPill } from '../components/StatusPill';
import { StatusBar } from '../components/StatusBar';
import { LayoutControls } from '../components/LayoutMenu';
import { PeopleButton } from '../components/PeopleButton';
import { SharingSettings } from '../components/SharingSettings';
import { choose, type Layout } from '../model/layout';
import { useLayout } from './useLayout';
import { useMenu } from '../menu/menu';
import { statusBarInfo } from '../model/statusbar';
import { nextRequest, requestIn } from '../model/request';
import { Toast, useToast } from '../components/Toast';
import { formatCode, joinLink } from '../model/code';
import { Workspace, type WorkspaceHandle } from './Workspace';
import { setTimelineEvents } from '../model/heartbeat';

export const AGENT_NAMES: Record<AgentId, string> = {
  'claude-code': 'Claude Code',
  'open-code': 'OpenCode',
  omp: 'omp',
  codex: 'Codex',
  grok: 'Grok',
  pi: 'pi',
  terminal: 'Terminal',
};

interface State {
  agent: AgentId | null;
  folder: string;
  timelineSupported: boolean;
  sharing: SharingState;
  code: string | null;
  /** The sharing service, which join links name. */
  service: string;
  /** The service serves the web viewer, so join links open in a browser. */
  webViewer: boolean;
  participants: Participant[];
  status: AgentStatus;
  viewerLimit: number;
  exited: number | null;
  timeline: TimelineEvent[];
  /** What the agent waits on, if it does. */
  request: string | null;
  /** False for terminal sessions and sessions of the previous release, which take no suggestions. */
  suggestionsPossible: boolean;
  suggestionsAllowed: boolean;
  suggestions: HostSuggestion[];
  /** The latest suggestion to arrive while the window is open, to announce. */
  arrived: HostSuggestion | null;
  /** False for sessions without a timeline and sessions of the previous release. */
  wholeFilesPossible: boolean;
  /** Whether viewers can open whole code files. */
  wholeFiles: boolean;
  /** False for sessions of the previous release, which take no shared files. */
  sharedFilesPossible: boolean;
  /** Whether viewers may share files. */
  filesAllowed: boolean;
  sharedFiles: SharedFileInfo[];
  /** The braindump; null for sessions of the previous release, which have none. */
  braindump: BraindumpItemInfo[] | null;
  /** False for terminal sessions, where giving the braindump types nothing. */
  braindumpPromptPossible: boolean;
  /** False for sessions of the previous release, which can't share links. */
  linksPossible: boolean;
  /** Whether viewers may send links. */
  linksAllowed: boolean;
  /** Links viewers sent, oldest first. */
  viewerLinks: HostLink[];
  /** The latest link from a viewer to arrive while the window is open, to announce. */
  linkArrived: HostLink | null;
  /** False for sessions of the previous release, which can't ask to read or estimate. */
  asksPossible: boolean;
  /** The open read request and vote. */
  asks: HostAsks;
  /** When `asks` arrived, to count the time left down from. */
  asksAt: number;
}

const initial: State = {
  agent: null,
  folder: '',
  timelineSupported: false,
  sharing: 'connecting',
  code: null,
  service: '',
  webViewer: false,
  participants: [],
  status: 'starting',
  viewerLimit: 20,
  exited: null,
  timeline: [],
  request: null,
  suggestionsPossible: false,
  suggestionsAllowed: false,
  suggestions: [],
  arrived: null,
  wholeFilesPossible: false,
  wholeFiles: false,
  sharedFilesPossible: false,
  filesAllowed: false,
  sharedFiles: [],
  braindump: null,
  braindumpPromptPossible: false,
  linksPossible: false,
  linksAllowed: false,
  viewerLinks: [],
  linkArrived: null,
  asksPossible: false,
  asks: { read: null, vote: null },
  asksAt: 0,
};

export function hostReducer(state: State, event: HostEvent | { type: 'limit'; limit: number }): State {
  switch (event.type) {
    case 'snapshot':
      return {
        ...state,
        ...event,
        timeline: event.timeline,
        request: requestIn(event.timeline, event.status),
        // A session of the previous release says nothing of suggestions: it takes none.
        suggestionsPossible: event.suggestionsPossible ?? false,
        suggestionsAllowed: event.suggestionsAllowed ?? false,
        suggestions: event.suggestions ?? [],
        wholeFilesPossible: event.wholeFilesPossible ?? false,
        wholeFiles: event.wholeFiles ?? false,
        sharedFilesPossible: event.sharedFilesPossible ?? false,
        filesAllowed: event.filesAllowed ?? false,
        sharedFiles: event.sharedFiles ?? [],
        // A session of the previous release has no braindump.
        braindump: event.braindump ?? null,
        braindumpPromptPossible: event.braindumpPromptPossible ?? false,
        // A session of the previous release says nothing of links: it can't share them.
        linksPossible: event.linksPossible ?? false,
        linksAllowed: event.linksAllowed ?? false,
        viewerLinks: event.viewerLinks ?? [],
        // A session of the previous release says nothing of asks: it takes none.
        asksPossible: event.asksPossible ?? false,
        asks: event.asks ?? { read: null, vote: null },
        asksAt: Date.now(),
      };
    case 'asks':
      return { ...state, asks: { read: event.read, vote: event.vote }, asksAt: Date.now() };
    case 'shared-files':
      return { ...state, sharedFiles: event.files, braindump: event.braindump ?? state.braindump, filesAllowed: event.filesAllowed };
    case 'allowed':
      return { ...state, suggestionsAllowed: event.suggestions };
    case 'shared':
      return { ...state, wholeFiles: event.wholeFiles };
    case 'links-allowed':
      return { ...state, linksAllowed: event.links };
    case 'viewer-links': {
      const newest = event.links.at(-1);
      const linkArrived = event.links.length > state.viewerLinks.length && newest?.state === 'Waiting' ? newest : state.linkArrived;
      return { ...state, viewerLinks: event.links, linkArrived };
    }
    case 'suggestions': {
      const newest = event.suggestions.at(-1);
      const arrived = event.suggestions.length > state.suggestions.length && newest?.state === 'Waiting' ? newest : state.arrived;
      return { ...state, suggestions: event.suggestions, arrived };
    }
    case 'sharing':
      return { ...state, sharing: event.state, code: event.code ?? (event.state === 'connecting' ? null : state.code) };
    case 'web-viewer':
      return { ...state, webViewer: event.available };
    case 'participants':
      return { ...state, participants: event.participants };
    case 'status':
      return { ...state, status: event.status, request: nextRequest(state.request, event) };
    case 'timeline':
      return { ...state, timeline: [...state.timeline, event.event], request: nextRequest(state.request, event) };
    case 'exited':
      return { ...state, exited: event.code, status: 'exited' };
    case 'limit':
      return { ...state, viewerLimit: event.limit };
  }
}

/** What ending the session does; `running` is the agent or shell still to stop, if any. */
export function endingText(running: string | null, viewers: number): string {
  const effects = [
    running && `${running} stops`,
    viewers > 0 && `${viewers === 1 ? '1 viewer is' : `${viewers} viewers are`} disconnected`,
  ].filter(Boolean);
  const first = effects.join(', and ');
  return `${first ? `${first.charAt(0).toUpperCase()}${first.slice(1)}. ` : ''}The invite code stops working.`;
}

/** The host's session: the agent's terminal, the invite code and everything viewers see. */
interface Props {
  /** How the host arranges the window; the host's terminal tab is always there. */
  layout: Layout | null;
  onLayout: (layout: Layout) => void;
  /** Screen readers read the terminal; the setting in Settings. */
  screenReader?: boolean;
  onEnded: () => void;
  /** Shown in the toolbar when a new SpecDeck is ready to install. */
  update?: ReactNode;
}

export function HostScreen({ layout: stored, onLayout, screenReader = false, onEnded, update }: Props) {
  const bridge = useBridge();
  const workspace = useRef<WorkspaceHandle>(null);
  // The host is never asked the first-join question; they start as a developer.
  const [layout, setLayout] = useLayout(stored, onLayout);
  const shown = layout ?? choose('developer');
  const [state, dispatch] = useReducer(hostReducer, initial);
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, showToast] = useToast();
  const [dialog, setDialog] = useState<'read' | 'estimate' | null>(null);

  const attached = useRef(false);
  useEffect(() => {
    // Attach once, also when React mounts the screen twice in development. The window gets the
    // whole session from the start, also after it was closed and reopened.
    if (attached.current) return;
    attached.current = true;
    workspace.current?.resetTerminal();
    bridge.hostAttach((data) => workspace.current?.terminalData(data), dispatch).catch((e) => setError(errorMessage(e)));
  }, [bridge]);

  const onInput = useCallback((data: string) => void bridge.hostInput(data).catch(() => {}), [bridge]);
  const onResize = useCallback((cols: number, rows: number) => void bridge.hostResize(cols, rows).catch(() => {}), [bridge]);
  const onThemeColors = useCallback((fg: string, bg: string) => void bridge.hostSetColors(fg, bg).catch(() => {}), [bridge]);
  useEffect(() => setTimelineEvents(state.timeline.length), [state.timeline.length]);
  const onTerminalState = useCallback((snapshot: TerminalSnapshot) => void bridge.reportTerminal(snapshot).catch(() => {}), [bridge]);

  const agentName = state.agent ? AGENT_NAMES[state.agent] : 'Agent';
  const waiting = state.suggestions.filter((s) => s.state === 'Waiting').length;
  // A new suggestion is announced without taking focus from the terminal.
  useEffect(() => {
    if (state.arrived) showToast(`${state.arrived.from} suggests a prompt. Open Prompts to send, edit or decline it.`);
  }, [state.arrived, showToast]);
  useEffect(() => {
    if (state.linkArrived) showToast(`${state.linkArrived.from} wants to share a link. It waits above the timeline.`);
  }, [state.linkArrived, showToast]);
  const onRemoveLink = useCallback((id: number) => void bridge.hostRemoveLink(id).catch((e) => showToast(errorMessage(e))), [bridge, showToast]);
  const linksBar = state.linksPossible ? (
    <LinksBar
      host
      viewerLinks={state.viewerLinks}
      onShare={(url, title, sessionOnly) => bridge.hostShareLink(url, title, sessionOnly)}
      onAnswer={(link, share) => void bridge.hostAnswerLink(link.viewer, link.id, share).catch((e) => showToast(errorMessage(e)))}
    />
  ) : undefined;
  const terminal = state.agent === 'terminal';
  const statusBar = useMemo(() => statusBarInfo(state.timeline, state.timelineSupported), [state.timeline, state.timelineSupported]);
  const folderName = state.folder.split(/[\\/]/).filter(Boolean).at(-1) ?? state.folder;
  const viewers = state.participants.filter((p) => p.role === 'Viewer').length;
  const held = useMemo(() => [...state.sharedFiles, ...(state.braindump ?? []).map((item) => item.info)], [state.sharedFiles, state.braindump]);
  const open = useSharedBytes(held, bridge.hostReadSharedFile);
  const hostName = state.participants.find((p) => p.role === 'Host')?.label ?? 'You';
  const shared = useMemo<SharedFiles | undefined>(
    () =>
      state.sharedFilesPossible
        ? {
            files: state.sharedFiles,
            me: 0,
            host: true,
            canShare: true,
            hostName,
            agentName: terminal ? 'The shell' : agentName,
            share: (files) => void shareEach(files, bridge.hostShareFile, showToast),
            open,
            remove: (file) => void bridge.hostRemoveSharedFile(file).catch((e) => showToast(errorMessage(e))),
            put: (file) => bridge.hostPutSharedFile(file),
            save: bridge.saveSharedFile,
            say: showToast,
            braindump: state.braindump
              ? {
                  items: state.braindump,
                  add: (files) => void shareEach(files, bridge.hostAddToBraindump, showToast, (name) => `${name} is in the braindump.`),
                  note: async (text, file) => void (await (file === undefined ? bridge.hostAddNote(text) : bridge.hostReplaceNote(file, text))),
                  give: bridge.hostGiveBraindump,
                  promptPossible: state.braindumpPromptPossible,
                }
              : undefined,
          }
        : undefined,
    [state.sharedFilesPossible, state.sharedFiles, state.braindump, state.braindumpPromptPossible, hostName, terminal, agentName, bridge, showToast, open],
  );

  // The host's asks: a failure says why in a toast.
  const ask = (call: () => Promise<void>) => void call().catch((e) => showToast(errorMessage(e)));
  const { read, vote } = state.asks;
  const requests = (read || vote) && (
    <>
      {read && (
        <ReadBanner
          view={read}
          receivedAt={state.asksAt}
          participants={state.participants}
          onOpen={() => workspace.current?.openTarget(read.target, read.label)}
          onEnd={() => ask(() => bridge.hostEndRead())}
        />
      )}
      {vote && (
        <VoteBanner
          key={`${vote.id}:${vote.round}`}
          view={vote}
          participants={state.participants}
          me={0}
          onVote={(card) => ask(() => bridge.hostCastVote(card))}
          host={{
            onReveal: () => ask(() => bridge.hostRevealVotes()),
            onVoteAgain: () => ask(() => bridge.hostVoteAgain()),
            onStop: () => ask(() => bridge.hostStopVote()),
            onKeep: (card) => ask(() => bridge.hostKeepEstimate(card)),
          }}
        />
      )}
    </>
  );

  const copy = async (text: string, message: string) => {
    try {
      await bridge.copyText(text);
      showToast(message);
    } catch {
      showToast('Could not copy. Select the code and copy it yourself.');
    }
  };

  useMenu((command) => {
    // Ended from the menu-bar or tray icon.
    if (command.id === 'session-ended') return onEnded();
    if (command.id === 'copy-code') {
      if (state.code) void copy(formatCode(state.code), 'Invite code copied');
      else showToast('The invite code is not ready yet.');
    }
  });

  // The window's title names the folder, so the platform's window list tells sessions apart.
  useEffect(() => {
    if (folderName) void bridge.setSessionTitle(folderName).catch(() => {});
  }, [bridge, folderName]);
  useEffect(() => () => void bridge.setSessionTitle(null).catch(() => {}), [bridge]);

  const end = async () => {
    setConfirmEnd(false);
    await bridge.hostClose();
    onEnded();
  };

  return (
    <div className="session">
      <header className="toolbar" data-tauri-drag-region>
        <div className="title" data-tauri-drag-region>
          <strong title={state.folder}>{folderName || 'Starting…'}</strong>
          <span>
            {agentName} · {state.folder}
          </span>
        </div>
        <StatusPill status={state.status} agentName={agentName} terminal={terminal} request={state.timelineSupported ? null : state.request} />
        <div className="spacer" />
        {update}
        <PeopleButton
          participants={state.participants}
          me={0}
          onRemove={(p) => bridge.hostRemoveViewer(p.id)}
          viewerLimit={state.viewerLimit}
          onViewerLimit={(limit) => {
            dispatch({ type: 'limit', limit });
            void bridge.hostSetViewerLimit(limit);
          }}
          agentName={agentName}
        />
        <SharingSettings
          wholeFiles={
            state.wholeFilesPossible
              ? { on: state.wholeFiles, onChange: (on) => void bridge.hostShareCode(on).catch((e) => showToast(errorMessage(e))) }
              : undefined
          }
          suggestions={
            state.suggestionsPossible
              ? { allowed: state.suggestionsAllowed, onChange: (on) => void bridge.hostAllowSuggestions(on).catch((e) => showToast(errorMessage(e))) }
              : undefined
          }
          files={
            state.sharedFilesPossible
              ? { allowed: state.filesAllowed, onChange: (on) => void bridge.hostAllowFiles(on).catch((e) => showToast(errorMessage(e))), terminal }
              : undefined
          }
          links={
            state.linksPossible
              ? { allowed: state.linksAllowed, onChange: (on) => void bridge.hostAllowLinks(on).catch((e) => showToast(errorMessage(e))) }
              : undefined
          }
          agentName={agentName}
        />
        {state.asksPossible && (
          <>
            <button className="btn" onClick={() => setDialog('read')} disabled={read !== null} title={read ? 'A read request is open. Go on first.' : undefined}>
              <BookOpen aria-hidden /> Ask to read
            </button>
            <button className="btn" onClick={() => setDialog('estimate')} disabled={vote !== null} title={vote ? 'A vote is open. Close it first.' : undefined}>
              <Gauge aria-hidden /> Estimate
            </button>
          </>
        )}
        <LayoutControls host layout={shown} onLayout={setLayout} />
        <InviteCode
          sharing={state.sharing}
          code={state.code}
          onCopyCode={() => state.code && copy(formatCode(state.code), 'Invite code copied')}
          onCopyLink={() => state.code && copy(joinLink(state.code, state.service, state.webViewer), 'Join link copied')}
          onNewCode={() => bridge.hostNewCode()}
        />
        <button className="btn danger" onClick={() => setConfirmEnd(true)}>
          <Power aria-hidden /> End session
        </button>
      </header>
      <div>
        {error && (
          <div className="banner danger" role="alert">
            <AlertTriangle aria-hidden /> {error}
          </div>
        )}
        {state.sharing === 'unavailable' && (
          <div className="banner warning" role="status">
            <AlertTriangle aria-hidden />
            Sharing is unavailable: SpecDeck's sharing service cannot be reached. Your session keeps running here, and SpecDeck keeps
            trying; the invite code appears once it can be shared.
          </div>
        )}
        {state.exited !== null && (
          <div className="banner info" role="status">
            {terminal ? 'The shell' : agentName} has exited. Viewers can still read the session; end it when you are done.
          </div>
        )}
      </div>
      <Workspace
        ref={workspace}
        mode="host"
        agentName={agentName}
        terminal={terminal}
        screenReader={screenReader}
        layout={shown}
        onLayout={setLayout}
        timelineSupported={state.timelineSupported}
        timeline={state.timeline}
        onInput={onInput}
        onResize={onResize}
        onThemeColors={onThemeColors}
        onTerminalState={onTerminalState}
        wholeFiles={state.wholeFiles}
        shared={shared}
        linksBar={linksBar}
        onRemoveLink={state.linksPossible ? onRemoveLink : undefined}
        requests={requests}
        prompts={
          state.suggestionsPossible
            ? {
                badge: waiting,
                content: (
                  <SuggestionsInbox
                    agentName={agentName}
                    suggestions={state.suggestions}
                    allowed={state.suggestionsAllowed}
                    onAnswer={(suggestion, text) =>
                      void bridge.hostAnswerSuggestion(suggestion.viewer, suggestion.id, text).catch((e) => showToast(errorMessage(e)))
                    }
                  />
                ),
              }
            : undefined
        }
      />
      <StatusBar info={statusBar} folder={folderName || undefined} />
      {confirmEnd && (
        <ConfirmDialog title="End the session for everyone?" confirm="End session" danger onConfirm={end} onCancel={() => setConfirmEnd(false)}>
          <p>{endingText(state.exited === null ? (terminal ? 'The shell' : agentName) : null, viewers)}</p>
        </ConfirmDialog>
      )}
      {dialog === 'read' && (
        <AskReadDialog
          timeline={state.timeline}
          timelineSupported={state.timelineSupported}
          shared={state.sharedFiles}
          onAsk={(target, label, limitS) => bridge.hostAskToRead(target, label, limitS)}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === 'estimate' && <EstimateDialog timeline={state.timeline} onStart={(task, cards) => bridge.hostStartVote(task, cards)} onClose={() => setDialog(null)} />}
      <Toast message={toast} />
    </div>
  );
}

function InviteCode(props: {
  sharing: SharingState;
  code: string | null;
  onCopyCode: () => void;
  onCopyLink: () => void;
  onNewCode: () => void;
}) {
  const [confirmNew, setConfirmNew] = useState(false);
  if (props.sharing !== 'ready' || !props.code) {
    return (
      <div className="invite" aria-live="polite">
        <span className="pending">{props.sharing === 'unavailable' ? 'Sharing unavailable' : 'Getting an invite code…'}</span>
        {props.sharing === 'connecting' && <span className="spinner" style={{ margin: '0 6px' }} />}
      </div>
    );
  }
  return (
    <div className="invite-wrap">
      <div className="invite" aria-label="Invite code" aria-describedby="invite-note">
        <span className="label">Invite code</span>
        <code className="selectable">{formatCode(props.code)}</code>
        <button className="btn ghost small" onClick={props.onCopyCode} title="Copy the code">
          <Copy aria-hidden /> Copy
        </button>
        <button className="btn ghost small icon" onClick={props.onCopyLink} aria-label="Copy join link" title="Copy a join link">
          <Link2 aria-hidden />
        </button>
        <button className="btn ghost small icon" onClick={() => setConfirmNew(true)} aria-label="Make a new code" title="New code">
          <RefreshCw aria-hidden />
        </button>
        {confirmNew && (
          <ConfirmDialog
            title="Make a new invite code?"
            confirm="New code"
            onCancel={() => setConfirmNew(false)}
            onConfirm={() => {
              setConfirmNew(false);
              props.onNewCode();
            }}
          >
            <p>The current code stops working. People already watching stay; share the new code with anyone still to join.</p>
          </ConfirmDialog>
        )}
      </div>
      {/* Late joiners get the whole history, terminal and timeline (session-streaming spec). */}
      <span className="invite-note" id="invite-note">
        Anyone who joins sees the whole session so far.
      </span>
    </div>
  );
}
