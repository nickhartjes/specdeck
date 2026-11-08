import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { ArrowDown, FileDiff, FileText, Files, Image, Share2, SquareTerminal, X } from "lucide-react";
import type { ReadTarget, TimelineEvent, TerminalSnapshot } from "../api/types";
import { ChangesTab } from "../components/ChangesTab";
import { changesTabPath } from "../model/asks";
import { Divider } from "../components/Divider";
import { DiffTab, type CodeView } from "../components/DiffTab";
import { DocumentTab } from "../components/DocumentTab";
import { ScreenshotTab, screenshotTitle } from "../components/Screenshot";
import { FilesPanel, isDocument } from "../components/FilesPanel";
import { SearchBar } from "../components/SearchBar";
import { SharedFileTab, type SharedFiles } from "../components/SharedFiles";
import { shareSteps } from "../model/sharedFiles";
import { TimelinePanel } from "../components/TimelinePanel";
import { buildDocuments, type Plan } from "../model/documents";
import {
  PlanCard,
  planReaderDocuments,
  planTitle,
} from "../components/PlanCard";
import {
  panelWidth,
  setWidth,
  usualWidth,
  type Layout,
  type SidePanel,
} from "../model/layout";
import {
  closeTab,
  keepTab,
  noTabs,
  openTab,
  selectTab,
  TERMINAL,
  withTerminal,
  type OpenTab,
  type Tabs,
} from "../model/tabs";
import {
  buildTimeline,
  turnText,
  type TimelineModel,
  type ToolItem,
  type TouchedFile,
} from "../model/timeline";
import { screenshotOf } from "../model/steps";
import { decodeTerminalStream } from "../terminal/stream";
import { TerminalView, type TerminalHandle } from "../terminal/TerminalView";
import { useMenu } from "../menu/menu";

export interface WorkspaceHandle {
  /** Terminal data from the session, as received over the bridge. */
  terminalData(data: ArrayBuffer): void;
  resetTerminal(): void;
  /** Opens what the host asked to read, as a kept tab. */
  openTarget(target: ReadTarget, label: string): void;
}

interface Props {
  mode: "host" | "viewer";
  agentName: string;
  /** A terminal session: texts talk about the shell instead of an agent. */
  terminal?: boolean;
  /** Screen readers read the terminal. */
  screenReader?: boolean;
  /** How this person arranges the window; changes go back through `onLayout`. */
  layout: Layout;
  onLayout: (layout: Layout) => void;
  timelineSupported: boolean;
  timeline: TimelineEvent[];
  onInput?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
  onThemeColors?: (foreground: string, background: string) => void;
  /** The terminal's state for the Connection status panel. */
  onTerminalState?: (snapshot: TerminalSnapshot) => void;
  /** The Prompts tab beside the timeline: the viewer's suggestions or the host's inbox; absent where there are none. */
  prompts?: { badge?: number; content: ReactNode };
  /** Whether the host shares whole code files now. */
  wholeFiles?: boolean;
  /** Files shared with everyone; absent where the session takes none. */
  shared?: SharedFiles;
  /** Bytes received so far of shared files being fetched, by file. */
  sharedProgress?: ReadonlyMap<number, number>;
  /** Above the timeline: sharing links; absent where links can't be shared. */
  linksBar?: ReactNode;
  /** The host's: removes a shared link. */
  onRemoveLink?: (linkId: number) => void;
  /** The request row above the panes: an open read request and vote; nothing takes no space. */
  requests?: ReactNode;
}

type Pane = "files" | "session" | "timeline";

const isMac =
  typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
const NARROW = "(max-width: 760px)";

function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(
    () => window.matchMedia?.(NARROW).matches ?? false,
  );
  useEffect(() => {
    const media = window.matchMedia?.(NARROW);
    if (!media) return;
    const onChange = () => setNarrow(media.matches);
    media.addEventListener?.("change", onChange);
    return () => media.removeEventListener?.("change", onChange);
  }, []);
  return narrow;
}

/** Keys that scroll a tab: using them means reading it, which keeps a preview tab. */
const SCROLL_KEYS = new Set([
  "ArrowUp",
  "ArrowDown",
  "PageUp",
  "PageDown",
  "Home",
  "End",
  " ",
]);

const fileName = (path: string) => path.split("/").at(-1) ?? path;

/**
 * The session window below the toolbar (`session-window` spec): files on the left, the terminal
 * and opened files as tabs in the middle, the timeline on the right, as this person's layout
 * arranges them; one at a time on a narrow window.
 */
export const Workspace = forwardRef<WorkspaceHandle, Props>(
  function Workspace(props, ref) {
    const { layout, onLayout } = props;
    const host = props.mode === "host";
    const terminal = useRef<TerminalHandle>(null);
    const [following, setFollowing] = useState(true);
    const [newOutput, setNewOutput] = useState(false);
    const [searching, setSearching] = useState(false);
    const [term, setTerm] = useState("");
    const [terminalResult, setTerminalResult] = useState({
      index: -1,
      count: 0,
    });
    const [timelineIndex, setTimelineIndex] = useState(-1);
    const [dragging, setDragging] = useState(false);
    const narrow = useNarrow();
    const [pane, setPane] = useState<Pane>("session");
    // The timeline panel's tabs: the timeline, and the prompts viewers suggest.
    const [cuesTab, setCuesTab] = useState<"timeline" | "prompts">("timeline");
    const showPrompts = cuesTab === "prompts" && props.prompts !== undefined;

    // The host types into the terminal, and a session without a timeline can only be followed there.
    const hasTerminal = host || layout.terminalTab || !props.timelineSupported;
    const [kept, setTabs] = useState<Tabs>(() => noTabs(hasTerminal));

    const model = useMemo(
      () => buildTimeline(props.timeline),
      [props.timeline],
    );
    const written = useMemo(
      () => buildDocuments(props.timeline),
      [props.timeline],
    );
    const shares = useMemo(() => shareSteps(props.timeline), [props.timeline]);
    const sharedNow = useMemo(
      () => new Set(props.shared?.files.map((file) => file.file)),
      [props.shared?.files],
    );
    // Which version of each document this participant last read, for the whole session; changed in place.
    const [lastRead] = useState(() => new Map<string, number>());
    const onRead = useCallback(
      (path: string, version: number) => void lastRead.set(path, version),
      [lastRead],
    );
    // The view each code file was left on, for the session; a file not opened yet opens on its changes.
    const [codeViews] = useState(() => new Map<string, CodeView>());

    // Overview puts a document in front: the newest plan's first document, else the newest document.
    const newestDocument = useMemo(() => {
      const plan = written.plans[0];
      const fromPlan =
        plan &&
        planReaderDocuments(plan).find((doc) => written.documents.has(doc.path))
          ?.path;
      return fromPlan ?? [...written.documents.keys()].at(-1) ?? null;
    }, [written]);
    // The document last put in front, so one someone closed is not opened again.
    const [fronted, setFronted] = useState<string | null>(null);
    // Tabs follow the layout and the documents as they come, adjusted while rendering.
    let shownTabs = withTerminal(kept, hasTerminal);
    const wantsDocument =
      layout.preset === "overview" || shownTabs.active === null;
    if (
      newestDocument &&
      newestDocument !== fronted &&
      wantsDocument &&
      !shownTabs.list.some((tab) => tab.kind !== "terminal")
    ) {
      shownTabs = openTab(shownTabs, "document", newestDocument);
      setFronted(newestDocument);
    }
    if (shownTabs !== kept) setTabs(shownTabs);
    const tabs = shownTabs;
    const terminalShown = tabs.active === TERMINAL;
    const timelineMatches = useMemo(() => {
      const needle = term.trim().toLowerCase();
      if (!needle) return [];
      return model.turns
        .filter((turn) => turnText(turn).toLowerCase().includes(needle))
        .map((turn) => turn.promptId);
    }, [model, term]);

    // Opening something on a narrow window shows it.
    const show = useCallback(
      (next: Tabs) => {
        setTabs(next);
        if (narrow) setPane("session");
      },
      [narrow],
    );

    /** The terminal tab, at a record's line if given; added for a viewer who had none. */
    const showTerminal = useCallback(
      (seq?: number) => {
        if (!host && !layout.terminalTab && props.timelineSupported)
          onLayout({ ...layout, terminalTab: true });
        setTabs((tabs) => selectTab(withTerminal(tabs, true), TERMINAL));
        if (narrow) setPane("session");
        if (seq !== undefined) terminal.current?.scrollToSeq(seq, true);
      },
      [host, layout, narrow, onLayout, props.timelineSupported],
    );

    // A prompt in the timeline: scrolls the terminal there, if this person has one.
    const jumpToPrompt = useCallback(
      (seq: number) => {
        if (!tabs.list.some((tab) => tab.id === TERMINAL)) return;
        setTabs((current) => selectTab(current, TERMINAL));
        terminal.current?.scrollToSeq(seq);
      },
      [tabs.list],
    );

    const openDocument = useCallback(
      (path: string, keep = false) =>
        show(openTab(tabs, "document", path, { keep })),
      [show, tabs],
    );
    const openChange = useCallback(
      (path: string, toolUseId: string) =>
        show(openTab(tabs, "diff", path, { focus: toolUseId })),
      [show, tabs],
    );
    const openScreenshot = useCallback(
      (item: ToolItem, keep: boolean) => {
        const screenshot = screenshotOf(item);
        if (!screenshot) return;
        const title = screenshotTitle(screenshot);
        show(openTab(tabs, "image", item.id, { keep, title }));
      },
      [show, tabs],
    );

    const openFile = (file: TouchedFile, keep: boolean) => {
      if (file.readOnly) return showTerminal(file.readSeq);
      if (isDocument(file.path) && written.documents.has(file.path))
        return openDocument(file.path, keep);
      show(openTab(tabs, "diff", file.path, { keep }));
    };

    const openShared = (file: number, name: string, keep = false) =>
      show(openTab(tabs, "shared", String(file), { keep, title: name }));

    useImperativeHandle(ref, () => ({
      terminalData(data) {
        terminal.current?.write(decodeTerminalStream(data));
      },
      resetTerminal() {
        terminal.current?.reset();
      },
      openTarget(target, label) {
        if ("document" in target) {
          // A document the agent wrote opens as one; a code file shows its changes.
          const path = target.document.path;
          show(openTab(tabs, written.documents.has(path) ? "document" : "diff", path, { keep: true }));
        } else if ("code" in target) {
          show(openTab(tabs, "diff", target.code.path, { keep: true }));
        } else if ("shared" in target) {
          show(openTab(tabs, "shared", String(target.shared.file), { keep: true, title: label }));
        } else {
          const { fromSeq, toSeq } = target.changes;
          // The label reads in a sentence; a tab starts with a capital.
          const title = label.charAt(0).toUpperCase() + label.slice(1);
          show(openTab(tabs, "changes", changesTabPath(fromSeq, toSeq), { keep: true, title }));
        }
      },
    }));

    const readPlan = useCallback(
      (_plan: Plan, path: string) => openDocument(path),
      [openDocument],
    );
    // From a step: the first document that step changed, in the plan's order.
    const readStepPlan = useCallback(
      (change: string, paths: string[]) => {
        const plan = written.plans.find((p) => p.change === change);
        const docs = plan
          ? planReaderDocuments(plan)
          : paths.map((path) => ({ path, title: fileName(path) }));
        const first =
          docs.find((doc) => paths.includes(doc.path))?.path ?? paths[0];
        if (first) openDocument(first);
      },
      [written, openDocument],
    );

    const panelOpen = (panel: SidePanel) =>
      panel === "files" ? layout.files : layout.timeline;
    const togglePanel = (panel: SidePanel) => {
      if (narrow) return setPane((p) => (p === panel ? "session" : panel));
      onLayout({ ...layout, [panel]: !panelOpen(panel) });
    };
    const closePanel = (panel: SidePanel) => {
      if (narrow) return setPane("session");
      onLayout({ ...layout, [panel]: false });
    };

    // F6 and Shift+F6 move between the toolbar, the open panels and the tab area, also out of the
    // host's terminal, where Tab belongs to the agent.
    useEffect(() => {
      const onKey = (event: KeyboardEvent) => {
        if (event.key !== "F6") return;
        event.preventDefault();
        event.stopPropagation();
        const regions = [
          ...document.querySelectorAll<HTMLElement>(
            ".toolbar, .workspace .pane:not([hidden]), .workspace .tab-area",
          ),
        ];
        const active = document.activeElement;
        const current = terminal.current?.hasFocus()
          ? regions.findIndex((region) => region.classList.contains("tab-area"))
          : regions.findIndex((region) => region.contains(active));
        const next =
          regions[
            current < 0
              ? 0
              : (current + (event.shiftKey ? regions.length - 1 : 1)) %
                regions.length
          ];
        if (!next) return;
        if (next.classList.contains("tab-area") && terminalShown)
          return terminal.current?.focus();
        const target = next.matches(".toolbar")
          ? next.querySelector<HTMLElement>('button, [tabindex="0"]')
          : (next.querySelector<HTMLElement>(
              '[role="tab"][aria-selected="true"]',
            ) ?? next.querySelector<HTMLElement>('button, [tabindex="0"]'));
        target?.focus();
      };
      window.addEventListener("keydown", onKey, true);
      return () => window.removeEventListener("keydown", onKey, true);
    }, [terminalShown]);

    // Find: Cmd+F on macOS; Ctrl+F elsewhere, except in the host's terminal, where Ctrl+F is a
    // terminal key and Ctrl+Shift+F opens search instead.
    useEffect(() => {
      const onKey = (event: KeyboardEvent) => {
        if (event.key.toLowerCase() !== "f") return;
        const inTerminal =
          (event.target as HTMLElement | null)?.closest?.(".xterm") != null;
        const find = isMac
          ? event.metaKey && !event.ctrlKey
          : event.ctrlKey && (event.shiftKey || !(inTerminal && host));
        if (!find) return;
        event.preventDefault();
        setSearching(true);
      };
      window.addEventListener("keydown", onKey, true);
      return () => window.removeEventListener("keydown", onKey, true);
    }, [host]);

    useMenu((command) => {
      const term = terminal.current;
      switch (command.id) {
        case "find":
          setSearching(true);
          break;
        case "panel:files":
          togglePanel("files");
          break;
        case "panel:timeline":
          togglePanel("timeline");
          break;
        case "panel:terminal":
          showTerminal();
          // After the terminal tab is shown and its pane is visible.
          requestAnimationFrame(() => terminal.current?.focus());
          break;
        case "jump-to-live":
          term?.scrollToBottom();
          setNewOutput(false);
          break;
        // Edit commands from the Windows and Linux menu, which leaves those keys to the terminal.
        case "copy": {
          const selected = term?.selection();
          if (selected) void navigator.clipboard?.writeText(selected);
          else document.execCommand("copy");
          break;
        }
        case "paste":
          void navigator.clipboard?.readText().then((text) => {
            if (term?.hasFocus()) term.paste(text);
            else document.execCommand("insertText", false, text);
          });
          break;
        case "select-all":
          if (term?.hasFocus()) term.selectAll();
          else document.execCommand("selectAll");
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
        setCuesTab("timeline");
        if (narrow) setPane("timeline");
        else if (!layout.timeline) onLayout({ ...layout, timeline: true });
        setTimelineIndex(
          (i) =>
            (forward ? i + 1 : i - 1 + timelineMatches.length) %
            timelineMatches.length,
        );
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
      setTerm("");
      setTimelineIndex(-1);
      terminal.current?.clearSearch();
      if (terminalShown) terminal.current?.focus();
    };

    const keep = (id: string) => setTabs((current) => keepTab(current, id));

    // Which panels show: as the layout says, or the one chosen on a narrow window.
    const filesShown = narrow ? pane === "files" : layout.files;
    const timelineShown = narrow ? pane === "timeline" : layout.timeline;
    const sessionShown = !narrow || pane === "session";
    const width = (panel: SidePanel) => panelWidth(layout, panel);
    const columns = narrow
      ? "1fr"
      : [
          layout.files && `minmax(0, ${width("files")}px) 5px`,
          "minmax(240px, 1fr)",
          layout.timeline && `5px minmax(0, ${width("timeline")}px)`,
        ]
          .filter(Boolean)
          .join(" ");
    const resize = (panel: SidePanel) => (w: number) =>
      onLayout(setWidth(layout, panel, w));
    const reset = (panel: SidePanel) => () =>
      onLayout(setWidth(layout, panel, usualWidth(layout.preset, panel)));
    const activeTab = tabs.list.find((tab) => tab.id === tabs.active);
    const activeShared =
      activeTab?.kind === "shared" ? Number(activeTab.path) : null;
    const activePath =
      activeTab?.kind === "shared" ? null : (activeTab?.path ?? null);

    return (
      <main className="workspace">
        {/* The host's read request and vote (#53, #54); the row takes no space while neither is open. */}
        <div className="requests">{props.requests}</div>
        {narrow && (
          <nav className="pane-switch" aria-label="Panels">
            {(
              [
                ["files", "Files"],
                ["session", "Session"],
                ["timeline", "Timeline"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                aria-pressed={pane === id}
                onClick={() => setPane(id)}
              >
                {label}
              </button>
            ))}
          </nav>
        )}
        <div
          className={`panes${dragging ? " dragging" : ""}`}
          style={{ gridTemplateColumns: columns }}
        >
          <aside
            className="pane files-pane"
            aria-label="Files"
            hidden={!filesShown}
          >
            <FilesPanel
              files={model.files}
              timelineSupported={props.timelineSupported}
              agentName={props.agentName}
              terminal={props.terminal}
              tab={layout.filesTab}
              onTab={(filesTab) => onLayout({ ...layout, filesTab })}
              active={activePath}
              onOpen={openFile}
              onClose={() => closePanel("files")}
              shared={props.shared}
              activeShared={activeShared}
              onOpenShared={(file, keep) => openShared(file.file, file.name, keep)}
            />
          </aside>
          {!narrow && layout.files && (
            <Divider
              name="files"
              side={1}
              width={width("files")}
              onWidth={resize("files")}
              onReset={reset("files")}
              onDragging={setDragging}
            />
          )}
          <section
            className="tab-area"
            aria-label="Open"
            hidden={!sessionShown}
          >
            <TabBar
              tabs={tabs}
              host={host}
              onSelect={(id) => setTabs((current) => selectTab(current, id))}
              onKeep={keep}
              onClose={(id) => setTabs((current) => closeTab(current, id))}
            />
            <div className="tab-panels">
              {searching && (
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
              {/* Always there, so it keeps its size, its scrollback and its search while another tab is shown. */}
              <section
                className={`terminal-pane${terminalShown ? "" : " out-of-sight"}`}
                role="tabpanel"
                aria-label={host ? "Terminal" : "Terminal, read-only"}
                aria-hidden={!terminalShown || undefined}
                inert={!terminalShown || undefined}
              >
                <TerminalView
                  ref={terminal}
                  mode={props.mode}
                  screenReader={props.screenReader}
                  onData={props.onInput}
                  onResize={props.onResize}
                  hold={host && (!terminalShown || !sessionShown || dragging)}
                  onThemeColors={props.onThemeColors}
                  onFollowChange={(now) => {
                    setFollowing(now);
                    if (now) setNewOutput(false);
                  }}
                  onNewOutput={() => setNewOutput(true)}
                  onSearchResults={setTerminalResult}
                  onDiagnostics={props.onTerminalState}
                />
                {!following && (
                  <button
                    className="btn jump-live"
                    onClick={() => {
                      terminal.current?.scrollToBottom();
                      setNewOutput(false);
                    }}
                  >
                    <ArrowDown aria-hidden />
                    {newOutput ? "New output · jump to live" : "Jump to live"}
                  </button>
                )}
              </section>
              {tabs.list
                .filter((tab) => tab.kind !== "terminal")
                .map((tab) => (
                  <div
                    key={tab.id}
                    className="tab-panel"
                    role="tabpanel"
                    aria-label={tab.title ?? fileName(tab.path ?? "")}
                    hidden={tab.id !== tabs.active}
                    onWheel={() => keep(tab.id)}
                    onTouchMove={() => keep(tab.id)}
                    onKeyDown={(event) =>
                      SCROLL_KEYS.has(event.key) && keep(tab.id)
                    }
                    onMouseUp={() =>
                      (window.getSelection()?.toString() ?? "") !== "" &&
                      keep(tab.id)
                    }
                  >
                    {tab.kind === "shared" && props.shared ? (
                      <SharedFileTab
                        file={
                          props.shared.files.find(
                            (file) => String(file.file) === tab.path,
                          ) ?? null
                        }
                        shared={props.shared}
                        progress={props.sharedProgress?.get(Number(tab.path))}
                        onKeep={() => keep(tab.id)}
                      />
                    ) : tab.kind === "changes" ? (
                      <ChangesTab
                        timeline={props.timeline}
                        fromSeq={Number(tab.path!.split("-")[0])}
                        toSeq={Number(tab.path!.split("-")[1])}
                      />
                    ) : tab.kind === "image" ? (
                      <ScreenshotTab
                        screenshot={screenshotIn(model, tab.path!)}
                      />
                    ) : tab.kind === "document" ? (
                      <DocumentTab
                        path={tab.path!}
                        plan={planOf(
                          written.plans,
                          tab.path!,
                          written.documents,
                        )}
                        versions={written.documents}
                        lastRead={lastRead}
                        onRead={onRead}
                        changes={model.files.flatMap((file) => file.changes)}
                        onOpen={(path) => openDocument(path)}
                        onKeep={() => keep(tab.id)}
                      />
                    ) : (
                      <DiffTab
                        file={
                          model.files.find(
                            (file) => file.path === tab.path,
                          ) ?? { path: tab.path!, readOnly: false, changes: [] }
                        }
                        focus={tab.focus}
                        onJump={showTerminal}
                        texts={written.codeTexts.get(tab.path!)}
                        wholeFiles={props.wholeFiles}
                        host={host}
                        view={codeViews.get(tab.path!) ?? "changes"}
                        onView={(view) => {
                          codeViews.set(tab.path!, view);
                          keep(tab.id);
                        }}
                      />
                    )}
                  </div>
                ))}
              {tabs.active === null && (
                <div className="empty tabs-empty">
                  <FileText aria-hidden />
                  <strong>Nothing open</strong>
                  <span>
                    Plans and documents the agent writes open here. Pick one in
                    Files, or a step in the timeline.
                  </span>
                </div>
              )}
            </div>
          </section>
          {!narrow && layout.timeline && (
            <Divider
              name="timeline"
              side={-1}
              width={width("timeline")}
              onWidth={resize("timeline")}
              onReset={reset("timeline")}
              onDragging={setDragging}
            />
          )}
          <aside
            className="pane cues"
            aria-label="Timeline"
            hidden={!timelineShown}
          >
            <div className="pane-head">
              {props.prompts ? (
                <div
                  role="tablist"
                  aria-label="Timeline and prompts"
                  className="pane-tabs"
                >
                  {(
                    [
                      ["timeline", "Timeline"],
                      ["prompts", "Prompts"],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      role="tab"
                      className="pane-tab"
                      aria-selected={cuesTab === id}
                      tabIndex={cuesTab === id ? 0 : -1}
                      onClick={() => setCuesTab(id)}
                      onKeyDown={(event) =>
                        (event.key === "ArrowLeft" ||
                          event.key === "ArrowRight") &&
                        setCuesTab(id === "timeline" ? "prompts" : "timeline")
                      }
                    >
                      {label}
                      {id === "prompts" && !!props.prompts?.badge && (
                        <span className="badge">{props.prompts.badge}</span>
                      )}
                    </button>
                  ))}
                </div>
              ) : (
                <h2 className="pane-title">Timeline</h2>
              )}
              <span className="spacer" />
              {!showPrompts && (
                <div className="toggle" role="group" aria-label="Steps">
                  <button
                    aria-pressed={layout.steps === "summary"}
                    onClick={() => onLayout({ ...layout, steps: "summary" })}
                  >
                    Plain words
                  </button>
                  <button
                    aria-pressed={layout.steps === "details"}
                    onClick={() => onLayout({ ...layout, steps: "details" })}
                  >
                    Every step
                  </button>
                </div>
              )}
              <button
                className="btn ghost small icon"
                aria-label="Hide the timeline"
                title="Hide the timeline"
                onClick={() => closePanel("timeline")}
              >
                <X aria-hidden />
              </button>
            </div>
            {showPrompts && (
              <div className="pane-body prompts-body">
                {props.prompts!.content}
              </div>
            )}
            <div className="pane-body" hidden={showPrompts}>
              {written.plans.map((plan) => (
                <PlanCard
                  key={plan.change}
                  plan={plan}
                  versions={written.documents}
                  lastRead={lastRead}
                  onOpen={(path) => readPlan(plan, path)}
                />
              ))}
              <TimelinePanel
                model={model}
                timelineSupported={props.timelineSupported}
                agentName={props.agentName}
                steps={layout.steps}
                live={!terminalShown}
                onReadPlan={readStepPlan}
                terminal={props.terminal}
                matches={new Set(timelineMatches)}
                current={
                  timelineIndex >= 0
                    ? (timelineMatches[timelineIndex] ?? null)
                    : null
                }
                onJump={jumpToPrompt}
                onStep={showTerminal}
                onOpenChange={openChange}
                onOpenScreenshot={openScreenshot}
                shares={shares}
                sharedNow={sharedNow}
                onOpenShared={(file, name) => openShared(file, name)}
                linksBar={props.linksBar}
                onRemoveLink={props.onRemoveLink}
              />
            </div>
          </aside>
        </div>
      </main>
    );
  },
);

/** The plan a document belongs to, with its documents for the tab's links. */
function planOf(
  plans: Plan[],
  path: string,
  versions: Parameters<typeof planTitle>[1],
) {
  const plan = plans.find((p) =>
    p.documents.some((doc) => doc.paths.includes(path)),
  );
  return plan
    ? { title: planTitle(plan, versions), docs: planReaderDocuments(plan) }
    : undefined;
}

const TAB_ICONS = {
  terminal: SquareTerminal,
  document: FileText,
  diff: FileDiff,
  image: Image,
  shared: Share2,
  changes: Files,
};

/** The screenshot of tool call `toolUseId`, if the timeline has one. */
function screenshotIn(model: TimelineModel, toolUseId: string) {
  for (const turn of model.turns) {
    const item = turn.items.find(
      (entry) => entry.kind === "tool" && entry.id === toolUseId,
    );
    if (item?.kind === "tool") return screenshotOf(item);
  }
  return null;
}

/** The tabs in the middle, like an editor's: a preview in italics, the terminal tab never closed. */
function TabBar({
  tabs,
  host,
  onSelect,
  onKeep,
  onClose,
}: {
  tabs: Tabs;
  host: boolean;
  onSelect: (id: string) => void;
  onKeep: (id: string) => void;
  onClose: (id: string) => void;
}) {
  const refs = useRef(new Map<string, HTMLButtonElement | null>());
  const label = (tab: OpenTab) =>
    tab.kind === "terminal"
      ? host
        ? "Terminal"
        : "Terminal, read-only"
      : (tab.title ?? fileName(tab.path ?? ""));
  const onKeyDown = (event: ReactKeyboardEvent, tab: OpenTab) => {
    const index = tabs.list.indexOf(tab);
    if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
      event.preventDefault();
      const next =
        tabs.list[
          (index + (event.key === "ArrowRight" ? 1 : tabs.list.length - 1)) %
            tabs.list.length
        ]!;
      onSelect(next.id);
      refs.current.get(next.id)?.focus();
    } else if (event.key === "Delete") {
      onClose(tab.id);
    }
  };
  return (
    <div className="tabs-bar" role="tablist" aria-label="Open">
      {tabs.list.map((tab) => {
        const Icon = TAB_ICONS[tab.kind];
        const selected = tab.id === tabs.active;
        return (
          <div
            key={tab.id}
            className={`tab-item${selected ? " on" : ""}${tab.preview ? " preview" : ""}${tab.kind === "terminal" ? " term" : ""}`}
          >
            <button
              ref={(element) => {
                refs.current.set(tab.id, element);
              }}
              role="tab"
              aria-selected={selected}
              tabIndex={selected ? 0 : -1}
              title={
                tab.kind === "terminal"
                  ? host
                    ? "Keeps its size while another tab is shown"
                    : undefined
                  : tab.preview
                    ? `${tab.title ?? tab.path}. Preview: replaced by the next file you open. Double-click to keep it.`
                    : (tab.title ?? tab.path)
              }
              onClick={() => onSelect(tab.id)}
              onDoubleClick={() => onKeep(tab.id)}
              onAuxClick={(event) => event.button === 1 && onClose(tab.id)}
              onKeyDown={(event) => onKeyDown(event, tab)}
            >
              <Icon aria-hidden />
              <span className="label">{label(tab)}</span>
              {tab.preview && (
                <span className="visually-hidden">, preview</span>
              )}
            </button>
            {tab.kind !== "terminal" && (
              <button
                className="close"
                aria-label={`Close ${label(tab)}`}
                tabIndex={-1}
                onClick={() => onClose(tab.id)}
              >
                <X aria-hidden />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
