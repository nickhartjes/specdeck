import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { ArrowDown } from 'lucide-react';
import type { Participant, TimelineEvent, Steps, TerminalSnapshot } from '../api/types';
import { FilesPanel } from '../components/FilesPanel';
import { PeoplePanel } from '../components/PeoplePanel';
import { SearchBar } from '../components/SearchBar';
import { SidePanel } from '../components/SidePanel';
import { Splitter, useSideWidth } from '../components/Splitter';
import { TimelinePanel } from '../components/TimelinePanel';
import { buildDocuments, type Plan } from '../model/documents';
import { PlanCard, planReaderDocuments, planTitle } from '../components/PlanCard';
import { DocumentReader, type ReaderDocument } from '../components/DocumentReader';
import { buildTimeline, turnText } from '../model/timeline';
import { decodeTerminalStream } from '../terminal/stream';
import { TerminalView, type TerminalHandle } from '../terminal/TerminalView';
import { useMenu } from '../menu/menu';

export interface WorkspaceHandle {
  /** Terminal data from the session, as received over the bridge. */
  terminalData(data: ArrayBuffer): void;
  resetTerminal(): void;
}

interface Props {
  mode: 'host' | 'viewer';
  agentName: string;
  /** A terminal session: texts talk about the shell instead of an agent. */
  terminal?: boolean;
  /** The viewer chose not to see the terminal. It keeps running out of sight, so it comes back at
   * once with everything it received. */
  terminalHidden?: boolean;
  /** Screen readers read the terminal. */
  screenReader?: boolean;
  /** Steps in plain words, or every tool call in detail. */
  steps?: Steps;
  timelineSupported: boolean;
  timeline: TimelineEvent[];
  participants: Participant[];
  me?: number;
  onInput?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  onThemeColors?: (foreground: string, background: string) => void;
  /** The terminal's state for the Connection status panel. */
  onTerminalState?: (snapshot: TerminalSnapshot) => void;
  onRemove?: (participant: Participant) => void;
  viewerLimit?: number;
  onViewerLimit?: (limit: number) => void;
}

const isMac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);

/** Terminal, search and side panel: the part of the window both the host and viewers share. */
export const Workspace = forwardRef<WorkspaceHandle, Props>(function Workspace(props, ref) {
  const terminal = useRef<TerminalHandle>(null);
  const [following, setFollowing] = useState(true);
  const [newOutput, setNewOutput] = useState(false);
  const [searching, setSearching] = useState(false);
  const [term, setTerm] = useState('');
  const [terminalResult, setTerminalResult] = useState({ index: -1, count: 0 });
  const [timelineIndex, setTimelineIndex] = useState(-1);
  // The timeline first, unless this agent has none; until the reader picks a tab themselves.
  const [picked, setPicked] = useState<string | null>(null);
  const tab = picked ?? (props.timelineSupported ? 'timeline' : 'people');
  const setTab = setPicked;
  const [sideWidth, setSideWidth] = useSideWidth();

  const model = useMemo(() => buildTimeline(props.timeline), [props.timeline]);
  const written = useMemo(() => buildDocuments(props.timeline), [props.timeline]);
  // Which version of each document this participant last read, for the whole session.
  const lastRead = useRef(new Map<string, number>());
  const [reading, setReading] = useState<{ heading: string; docs: ReaderDocument[]; initial: string } | null>(null);
  const readDocument = useCallback((path: string) => {
    setReading({ heading: 'Document', docs: [{ title: path.split('/').at(-1) ?? path, path }], initial: path });
  }, []);
  const readPlan = useCallback(
    (plan: Plan, path: string) => {
      setReading({ heading: `Plan · ${planTitle(plan, written.documents)}`, docs: planReaderDocuments(plan), initial: path });
    },
    [written],
  );
  // From a step: the first file that step changed, in the plan's order.
  const readStepPlan = useCallback(
    (change: string, paths: string[]) => {
      const plan = written.plans.find((p) => p.change === change);
      const docs = plan ? planReaderDocuments(plan) : paths.map((path) => ({ path, title: path.split('/').at(-1) ?? path }));
      const initial = docs.find((doc) => paths.includes(doc.path))?.path ?? paths[0];
      if (!initial) return;
      setReading({ heading: plan ? `Plan · ${planTitle(plan, written.documents)}` : 'Plan', docs, initial });
    },
    [written],
  );
  const onRead = useCallback((path: string, version: number) => {
    lastRead.current.set(path, version);
  }, []);
  const timelineMatches = useMemo(() => {
    const needle = term.trim().toLowerCase();
    if (!needle) return [];
    return model.turns.filter((turn) => turnText(turn).toLowerCase().includes(needle)).map((turn) => turn.promptId);
  }, [model, term]);

  useImperativeHandle(ref, () => ({
    terminalData(data) {
      terminal.current?.write(decodeTerminalStream(data));
    },
    resetTerminal() {
      terminal.current?.reset();
    },
  }));

  const jump = useCallback((outputSeq: number) => {
    terminal.current?.scrollToSeq(outputSeq);
  }, []);

  // F6 and Shift+F6 move between the toolbar, the terminal and the side panel, also out of the
  // host's terminal, where Tab belongs to the agent.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'F6') return;
      event.preventDefault();
      event.stopPropagation();
      const parts = [
        () => document.querySelector<HTMLElement>('.toolbar button, .toolbar [tabindex="0"]'),
        () => (terminal.current?.focus(), null),
        () => document.querySelector<HTMLElement>('.side [role="tab"][aria-selected="true"]'),
      ];
      const active = document.activeElement;
      const current = terminal.current?.hasFocus() ? 1 : active?.closest('.toolbar') ? 0 : active?.closest('.side') ? 2 : -1;
      // From elsewhere, start at the toolbar.
      const next = current < 0 ? 0 : (current + (event.shiftKey ? parts.length - 1 : 1)) % parts.length;
      parts[next]?.()?.focus();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  // Find: Cmd+F on macOS; Ctrl+F elsewhere, except in the host's terminal, where Ctrl+F is a
  // terminal key and Ctrl+Shift+F opens search instead.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== 'f') return;
      const inTerminal = (event.target as HTMLElement | null)?.closest?.('.xterm') != null;
      const find = isMac
        ? event.metaKey && !event.ctrlKey
        : event.ctrlKey && (event.shiftKey || !(inTerminal && props.mode === 'host'));
      if (!find) return;
      event.preventDefault();
      setSearching(true);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [props.mode]);

  useMenu((command) => {
    const term = terminal.current;
    switch (command.id) {
      case 'find':
        setSearching(true);
        break;
      case 'tab:timeline':
      case 'tab:files':
      case 'tab:people':
        setTab(command.id.slice(4));
        break;
      case 'jump-to-live':
        term?.scrollToBottom();
        setNewOutput(false);
        break;
      // Edit commands from the Windows and Linux menu, which leaves those keys to the terminal.
      case 'copy': {
        const selected = term?.selection();
        if (selected) void navigator.clipboard?.writeText(selected);
        else document.execCommand('copy');
        break;
      }
      case 'paste':
        void navigator.clipboard?.readText().then((text) => {
          if (term?.hasFocus()) term.paste(text);
          else document.execCommand('insertText', false, text);
        });
        break;
      case 'select-all':
        if (term?.hasFocus()) term.selectAll();
        else document.execCommand('selectAll');
        break;
    }
  });

  const step = (forward: boolean) => {
    if (!term) return;
    if (terminalResult.count > 0 || timelineMatches.length === 0) {
      if (forward) terminal.current?.findNext(term);
      else terminal.current?.findPrevious(term);
    }
    if (timelineMatches.length > 0) {
      setTab('timeline');
      setTimelineIndex((i) => (forward ? i + 1 : i - 1 + timelineMatches.length) % timelineMatches.length);
    }
  };

  const onTerm = (value: string) => {
    setTerm(value);
    setTimelineIndex(-1);
    if (value) terminal.current?.findNext(value);
    else {
      terminal.current?.clearSearch();
      setTerminalResult({ index: -1, count: 0 });
    }
  };

  const closeSearch = () => {
    setSearching(false);
    setTerm('');
    setTimelineIndex(-1);
    terminal.current?.clearSearch();
    terminal.current?.focus();
  };

  const viewers = props.participants.filter((p) => p.role === 'Viewer').length;
  const tabs = [
    {
      id: 'timeline',
      label: 'Timeline',
      badge: props.timelineSupported ? model.turns.length : undefined,
      content: (
        <>
          {written.plans.map((plan) => (
            <PlanCard key={plan.change} plan={plan} versions={written.documents} lastRead={lastRead.current} onOpen={(path) => readPlan(plan, path)} />
          ))}
          <TimelinePanel
            model={model}
            timelineSupported={props.timelineSupported}
            agentName={props.agentName}
            steps={props.steps}
            live={props.terminalHidden}
            onReadPlan={readStepPlan}
            terminal={props.terminal}
            matches={new Set(timelineMatches)}
            current={timelineIndex >= 0 ? (timelineMatches[timelineIndex] ?? null) : null}
            onJump={jump}
          />
        </>
      ),
    },
    {
      id: 'files',
      label: 'Files',
      badge: props.timelineSupported ? model.files.length : undefined,
      content: (
        <FilesPanel
          files={model.files}
          timelineSupported={props.timelineSupported}
          agentName={props.agentName}
          readable={new Set(written.documents.keys())}
          onRead={readDocument}
          terminal={props.terminal}
          onJump={jump}
        />
      ),
    },
    {
      id: 'people',
      label: 'People',
      badge: viewers + 1,
      content: (
        <PeoplePanel
          participants={props.participants}
          me={props.me}
          onRemove={props.onRemove}
          viewerLimit={props.viewerLimit}
          onViewerLimit={props.onViewerLimit}
        />
      ),
    },
  ];

  return (
    <main className={props.terminalHidden ? 'workspace terminal-hidden' : 'workspace'} style={{ ['--side' as string]: `${sideWidth}px` }}>
      <section
        className="terminal-pane"
        aria-label={props.mode === 'host' ? 'Terminal' : 'Terminal, read-only'}
        aria-hidden={props.terminalHidden || undefined}
        inert={props.terminalHidden || undefined}
      >
        <TerminalView
          ref={terminal}
          mode={props.mode}
          screenReader={props.screenReader}
          onData={props.onInput}
          onResize={props.onResize}
          onThemeColors={props.onThemeColors}
          onFollowChange={(now) => {
            setFollowing(now);
            if (now) setNewOutput(false);
          }}
          onNewOutput={() => setNewOutput(true)}
          onSearchResults={setTerminalResult}
          onDiagnostics={props.onTerminalState}
        />
        {searching && !props.terminalHidden && (
          <SearchBar
            term={term}
            onTerm={onTerm}
            onNext={() => step(true)}
            onPrevious={() => step(false)}
            onClose={closeSearch}
            terminal={terminalResult}
            timeline={timelineMatches.length}
          />
        )}
        {!following && (
          <button
            className="btn jump-live"
            onClick={() => {
              terminal.current?.scrollToBottom();
              setNewOutput(false);
            }}
          >
            <ArrowDown aria-hidden />
            {newOutput ? 'New output · jump to live' : 'Jump to live'}
          </button>
        )}
      </section>
      {!props.terminalHidden && <Splitter width={sideWidth} onWidth={setSideWidth} />}
      {searching && props.terminalHidden && (
        <div className="search-over-side">
          <SearchBar
            term={term}
            onTerm={onTerm}
            onNext={() => step(true)}
            onPrevious={() => step(false)}
            onClose={closeSearch}
            terminal={terminalResult}
            timeline={timelineMatches.length}
          />
        </div>
      )}
      <SidePanel tabs={tabs} selected={tab} onSelect={setTab} />
      {reading && (
        <DocumentReader
          heading={reading.heading}
          docs={reading.docs}
          initial={reading.initial}
          versions={written.documents}
          lastRead={lastRead.current}
          onRead={onRead}
          changes={model.files.flatMap((file) => file.changes)}
          onClose={() => setReading(null)}
        />
      )}
    </main>
  );
});
