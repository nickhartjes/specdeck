import { useEffect, useRef, useState, type FormEvent } from 'react';
import {
  Bot,
  Braces,
  CodeXml,
  ExternalLink,
  Folder,
  FolderOpen,
  LogIn,
  Pi,
  Radio,
  Sparkles,
  SquareTerminal,
  X,
  Zap,
  type LucideIcon,
} from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import type { AgentId, AgentInfo, Profile, RunningSession } from '../api/types';
import { ConfirmDialog } from '../components/ConfirmDialog';
import { initials } from '../components/PeoplePanel';
import { PLAIN_TEXT } from '../components/fields';
import { formatCode, isCompleteCode, isJoinLink } from '../model/code';
import type { MenuCommand } from '../menu/menu';
import { AGENT_NAMES, endingText } from './HostScreen';

interface Props {
  profile: Profile;
  onProfile: (profile: Profile) => void;
  onHosting: () => void;
  /** With the pasted join link, which may name another sharing service. */
  onJoin: (code: string, link?: string) => void;
  /** A code to show in the join field, for example after a failed join. */
  initialCode?: string;
  /** A message from a join that did not work. */
  joinError?: string;
  /** A host, join or recent-folder command from the menu, until `onCommandDone`. */
  command?: MenuCommand | null;
  onCommandDone?: () => void;
}

/** Choose to host or to join, and set your name the first time. */
export function StartScreen({ profile, onProfile, onHosting, onJoin, initialCode = '', joinError, command = null, onCommandDone = () => {} }: Props) {
  const bridge = useBridge();
  useEffect(() => void bridge.startScreenShown().catch(() => {}), [bridge]);
  // Without a name there is nothing to host or join yet.
  useEffect(() => {
    if (command && !profile.username) onCommandDone();
  }, [command, profile.username, onCommandDone]);
  if (!profile.username) return <Welcome onProfile={onProfile} />;
  return (
    <main className="start">
      <div className="start-inner">
        <header className="brand">
          <div>
            <h1>SpecDeck</h1>
            <p>Run a coding agent with your team watching along.</p>
          </div>
          <NameBadge profile={profile} onProfile={onProfile} />
        </header>
        <RunningSessions onOpened={onHosting} />
        <div className="cards">
          <HostCard profile={profile} onProfile={onProfile} onHosting={onHosting} command={command} onCommandDone={onCommandDone} />
          <JoinCard onJoin={onJoin} initialCode={initialCode} error={joinError} command={command} onCommandDone={onCommandDone} />
        </div>
      </div>
    </main>
  );
}

function viewersText(viewers: number): string {
  return viewers === 0 ? 'No viewers yet' : viewers === 1 ? '1 viewer' : `${viewers} viewers`;
}

/** Hosted sessions that run without a window (`running-sessions-on-start`); nothing when there are none. */
function RunningSessions({ onOpened }: { onOpened: () => void }) {
  const bridge = useBridge();
  const [sessions, setSessions] = useState<RunningSession[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [ending, setEnding] = useState<RunningSession | null>(null);
  useEffect(() => {
    let live = true;
    // Listening first, so a change while the list loads is not missed.
    const stop = bridge.onRunningSessions((list) => live && setSessions(list));
    void bridge
      .runningSessions()
      .then((list) => live && setSessions(list))
      .catch(() => {});
    return () => {
      live = false;
      stop();
    };
  }, [bridge]);

  const open = async (session: RunningSession) => {
    setError(null);
    try {
      await bridge.openRunningSession(session.id);
      onOpened();
    } catch (e) {
      // It got a window or ended meanwhile; the list refreshes by itself.
      setError(errorMessage(e));
    }
  };
  const end = async () => {
    const session = ending;
    setEnding(null);
    if (!session) return;
    setError(null);
    try {
      await bridge.endRunningSession(session.id);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  if (sessions.length === 0 && !error) return null;
  return (
    <section className="card running" aria-labelledby="running-title">
      <div>
        <h2 id="running-title">Running sessions</h2>
        <p className="lead">These keep running without a window; viewers are still connected.</p>
      </div>
      {sessions.length > 0 && (
        <ul className="recent">
          {sessions.map((session) => (
            <li key={session.id}>
              <div className="open">
                <Radio aria-hidden />
                <span className="name">{session.folder}</span>
                <span className="path">
                  {session.controllable ? session.state : 'Started by an older SpecDeck'} · {viewersText(session.viewers)}
                </span>
              </div>
              {session.controllable ? (
                <button className="btn small" aria-label={`Open ${session.folder}`} onClick={() => void open(session)}>
                  Open
                </button>
              ) : (
                <button className="btn small danger" aria-label={`End ${session.folder}`} onClick={() => setEnding(session)}>
                  End…
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
      {error && (
        <span className="error-text" role="alert">
          {error}
        </span>
      )}
      {ending && (
        <ConfirmDialog title="End the session for everyone?" confirm="End session" danger onConfirm={() => void end()} onCancel={() => setEnding(null)}>
          <p>{endingText(ending.agent === 'terminal' ? 'The shell' : AGENT_NAMES[ending.agent], ending.viewers)}</p>
        </ConfirmDialog>
      )}
    </section>
  );
}

function Welcome({ onProfile }: { onProfile: (profile: Profile) => void }) {
  const bridge = useBridge();
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!name.trim()) return setError('Enter the name others should see.');
    try {
      onProfile(await bridge.setUsername(name));
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <main className="start">
      <form className="welcome" onSubmit={submit}>
        <Logo />
        <h1>Welcome to SpecDeck</h1>
        <p>Run a coding agent on your computer while your team follows along, or join someone else's session.</p>
        <label htmlFor="username" style={{ textAlign: 'left', fontWeight: 600 }}>
          What should others call you?
        </label>
        <input
          {...PLAIN_TEXT}
          id="username"
          className="field"
          autoFocus
          maxLength={64}
          placeholder="Your name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          aria-describedby="username-hint"
        />
        <span id="username-hint" className="field-hint" style={{ textAlign: 'left' }}>
          Shown to everyone in a session. It is a label, not a login.
        </span>
        {error && <span className="error-text" role="alert">{error}</span>}
        <button className="btn primary large" type="submit">
          Continue
        </button>
      </form>
    </main>
  );
}

function NameBadge({ profile, onProfile }: { profile: Profile; onProfile: (profile: Profile) => void }) {
  const bridge = useBridge();
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(profile.username ?? '');
  if (editing) {
    return (
      <form
        className="me"
        onSubmit={async (event) => {
          event.preventDefault();
          if (!name.trim()) return;
          onProfile(await bridge.setUsername(name));
          setEditing(false);
        }}
      >
        <input {...PLAIN_TEXT} className="field" style={{ width: 180, height: 30 }} autoFocus maxLength={64} aria-label="Your name" value={name} onChange={(e) => setName(e.target.value)} />
        <button className="btn small primary" type="submit">
          Save
        </button>
        <button className="btn small ghost" type="button" onClick={() => setEditing(false)}>
          Cancel
        </button>
      </form>
    );
  }
  return (
    <div className="me">
      <span className="avatar" aria-hidden>
        {initials(profile.username ?? '')}
      </span>
      <span>
        You appear as <strong style={{ color: 'var(--text)' }}>{profile.username}</strong>
      </span>
      <button className="btn small ghost" onClick={() => setEditing(true)}>
        Change
      </button>
    </div>
  );
}

function defaultAgent(profile: Profile): AgentId | null {
  const installed = profile.agents.filter((agent) => agent.installed);
  if (profile.lastAgent && installed.some((agent) => agent.id === profile.lastAgent)) return profile.lastAgent;
  return installed[0]?.id ?? null;
}

interface CommandProps {
  command: MenuCommand | null;
  onCommandDone: () => void;
}

function HostCard({
  profile,
  onProfile,
  onHosting,
  command,
  onCommandDone,
}: { profile: Profile; onProfile: (p: Profile) => void; onHosting: () => void } & CommandProps) {
  const bridge = useBridge();
  const [agent, setAgent] = useState<AgentId | null>(() => defaultAgent(profile));
  const [starting, setStarting] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Asked for once the screen shows: reading them can take seconds the first time.
  const [versions, setVersions] = useState<Partial<Record<AgentId, string>>>({});
  useEffect(() => {
    let current = true;
    bridge
      .agentVersions()
      .then((found) => {
        if (!current) return;
        setVersions(Object.fromEntries(found.flatMap(({ agent, version }) => (version ? [[agent, version]] : []))));
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, [bridge]);
  const installed = profile.agents.filter((info) => info.installed);
  // Only those SpecDeck can point to an install page for; Terminal needs nothing.
  const missing = profile.agents.filter((info) => !info.installed && info.installPage);
  // Terminal is always there; the coding agents may all be missing.
  const noCodingAgents = profile.agents.every((a) => a.id === 'terminal' || !a.installed);

  const start = async (folder: string | null) => {
    if (!folder || !agent) return;
    setError(null);
    setStarting(folder);
    try {
      // A first guess at the terminal size; the host's terminal fits its pane right after.
      const cols = Math.max(80, Math.floor((window.innerWidth - 420) / 7.8));
      const rows = Math.max(24, Math.floor((window.innerHeight - 120) / 17));
      await bridge.startHosting(agent, folder, cols, rows);
      onHosting();
    } catch (e) {
      setError(errorMessage(e));
      setStarting(null);
    }
  };

  // File > Host a Session… and File > Open Recent.
  useEffect(() => {
    if (command?.id === 'host') {
      onCommandDone();
      void bridge.chooseFolder().then(start);
    } else if (command?.id === 'recent' && command.path) {
      const path = command.path;
      onCommandDone();
      // Started after this effect, like a click would be.
      void Promise.resolve().then(() => start(path));
    }
    // `start` changes every render; the command is what matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [command]);

  return (
    <section className="card" aria-labelledby="host-title">
      <div>
        <h2 id="host-title">Host a session</h2>
        <p className="lead">Start a coding agent or a terminal in a project folder. You type; people you invite watch.</p>
      </div>
      <div style={{ display: 'grid', gap: 8 }}>
        <span className="section-label" id="agent-label">
          Agent
        </span>
        <div className="agents" role="radiogroup" aria-labelledby="agent-label">
          {installed.map((info) => (
            <AgentChoice key={info.id} info={info} version={versions[info.id]} selected={agent === info.id} onSelect={() => setAgent(info.id)} />
          ))}
        </div>
        {missing.length > 0 && (
          <div className="agents-missing" role="group" aria-labelledby="agents-missing-label">
            <span id="agents-missing-label">Not installed</span>
            {missing.map((info) => (
              <button
                key={info.id}
                className="chip"
                onClick={() => void bridge.openInstallPage(info.id).catch((e) => setError(errorMessage(e)))}
                aria-label={`Install page of ${info.name}`}
                title={`How to install ${info.name}, in your browser`}
              >
                {info.name}
                <ExternalLink aria-hidden />
              </button>
            ))}
          </div>
        )}
        {agent === 'terminal' && <span className="field-hint">Everyone you invite sees all output, including any secrets a command prints.</span>}
        {noCodingAgents && (
          <span className="field-hint">No coding agent is installed. Install Claude Code, OpenCode, omp, Codex, Grok or pi to share an agent; you can still share a terminal.</span>
        )}
      </div>
      {profile.recentFolders.length > 0 && (
        <div style={{ display: 'grid', gap: 6 }}>
          <span className="section-label">Recent folders</span>
          <ul className="recent">
            {profile.recentFolders.map((folder) => (
              <li key={folder.path}>
                <button
                  className="open"
                  onClick={() => start(folder.path)}
                  disabled={!agent || starting !== null}
                  title={`Start in ${folder.path}`}
                  aria-label={`Start a session in ${folder.name}`}
                >
                  {starting === folder.path ? <span className="spinner" aria-label="Starting" /> : <Folder aria-hidden />}
                  <span className="name">{folder.name}</span>
                  <span className="path">{folder.path}</span>
                </button>
                <button
                  className="btn ghost icon small remove"
                  aria-label={`Remove ${folder.name} from recent folders`}
                  title="Remove from this list (the folder is not touched)"
                  onClick={async () => onProfile(await bridge.removeRecentFolder(folder.path))}
                >
                  <X aria-hidden />
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {error && (
        <span className="error-text" role="alert">
          {error}
        </span>
      )}
      <button className="btn primary large" disabled={!agent || starting !== null} onClick={async () => start(await bridge.chooseFolder())}>
        {starting && !profile.recentFolders.some((f) => f.path === starting) ? <span className="spinner" /> : <FolderOpen aria-hidden />}
        {profile.recentFolders.length > 0 ? 'Choose another folder…' : 'Choose a folder and start…'}
      </button>
    </section>
  );
}

const AGENT_ICONS: Partial<Record<AgentId, LucideIcon>> = {
  'claude-code': Sparkles,
  'open-code': Braces,
  omp: Bot,
  codex: CodeXml,
  grok: Zap,
  pi: Pi,
  terminal: SquareTerminal,
};

/** What viewers get with an agent, in a few words. */
function agentOffers(info: AgentInfo): string {
  if (info.id === 'terminal') return 'Your shell';
  const offers = [info.hasTimeline && 'Timeline, diffs and plans', info.reportsStatus && 'Status'].filter(Boolean);
  if (offers.length === 0) return 'Terminal only';
  return (info.hasTimeline ? offers : [...offers, 'Terminal']).join(' · ');
}

/** A path with the home folder as `~`. */
function shortPath(path: string): string {
  return path.replace(/^(\/Users\/[^/]+|\/home\/[^/]+|[A-Za-z]:\\Users\\[^\\]+)(?=[/\\]|$)/, '~');
}

function AgentChoice({ info, version, selected, onSelect }: { info: AgentInfo; version?: string; selected: boolean; onSelect: () => void }) {
  const Icon = AGENT_ICONS[info.id] ?? Bot;
  return (
    <button className="agent" role="radio" aria-checked={selected} onClick={onSelect} title={info.path ?? undefined}>
      <span className="agent-radio" aria-hidden />
      <Icon className="agent-icon" aria-hidden />
      <span className="agent-name">
        <strong>{info.name}</strong>
        {version && <span className="agent-version">{version}</span>}
      </span>
      <span className="agent-offers">{agentOffers(info)}</span>
      {/* bdi keeps a leading / or ~ in place while the line is cut from the start. */}
      {info.path && (
        <span className="agent-path">
          <bdi>{shortPath(info.path)}</bdi>
        </span>
      )}
    </button>
  );
}

function JoinCard({
  onJoin,
  initialCode,
  error,
  command,
  onCommandDone,
}: { onJoin: (code: string, link?: string) => void; initialCode: string; error?: string } & CommandProps) {
  const [code, setCode] = useState(formatCode(initialCode));
  // The field shows only the code; the link it came from is kept for its sharing service.
  const [link, setLink] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  // File > Join a Session…
  useEffect(() => {
    if (command?.id !== 'join') return;
    onCommandDone();
    input.current?.focus();
    input.current?.select();
  }, [command, onCommandDone]);
  const complete = isCompleteCode(code);
  return (
    <section className="card" aria-labelledby="join-title">
      <div>
        <h2 id="join-title">Join a session</h2>
        <p className="lead">Follow someone else's agent. You see everything; only the host can type.</p>
      </div>
      <form
        style={{ display: 'grid', gap: 10 }}
        onSubmit={(event) => {
          event.preventDefault();
          if (!complete) return;
          if (link && formatCode(link) === code) onJoin(code, link.trim());
          else onJoin(code);
        }}
      >
        <label htmlFor="invite-code" className="section-label">
          Invite code
        </label>
        <input
          {...PLAIN_TEXT}
          ref={input}
          id="invite-code"
          className="field code"
          placeholder="XXXX-XXXX-XXXX"
          autoComplete="off"
          value={code}
          onChange={(event) => setCode(formatCode(event.target.value))}
          onPaste={(event) => {
            event.preventDefault();
            const text = event.clipboardData.getData('text');
            setCode(formatCode(text));
            setLink(isJoinLink(text) ? text : null);
          }}
          aria-describedby="code-hint"
          aria-invalid={error ? true : undefined}
        />
        <span id="code-hint" className="field-hint">
          Paste the code or a specdeck:// link from the host. Upper or lower case does not matter.
        </span>
        {error && (
          <span className="error-text" role="alert">
            {error}
          </span>
        )}
        <button className="btn primary large" type="submit" disabled={!complete}>
          <LogIn aria-hidden /> Join
        </button>
      </form>
    </section>
  );
}

export function Logo() {
  return (
    <svg className="logo" viewBox="0 0 1024 1024" aria-hidden>
      <rect width="1024" height="1024" rx="224" fill="#1f2a44" />
      <rect x="300" y="250" width="470" height="560" rx="56" fill="#3d5a99" transform="rotate(-8 535 530)" />
      <rect x="254" y="214" width="470" height="560" rx="56" fill="#f4f1ea" />
      <path d="M348 380 l90 80 -90 80" fill="none" stroke="#1f2a44" strokeWidth="44" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M478 560 h140" stroke="#1f2a44" strokeWidth="44" strokeLinecap="round" />
    </svg>
  );
}
