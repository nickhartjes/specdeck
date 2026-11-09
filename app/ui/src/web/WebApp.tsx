// The web viewer's page (`web-viewer` design decisions 4 and 6): a join screen, then the app's
// own viewer screen. Watching only; the page joins sessions on the service that served it.

import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { errorMessage, useBridge } from '../api/bridge';
import type { Profile } from '../api/types';
import { ShortcutsDialog } from '../components/ShortcutsDialog';
import { PLAIN_TEXT } from '../components/fields';
import { dispatchMenu } from '../menu/menu';
import { profileLayout } from '../model/layout';
import { ViewerScreen } from '../screens/ViewerScreen';
import { serviceName } from './bridge';

const PANELS: Record<string, string> = { Digit1: 'panel:files', Digit2: 'panel:terminal', Digit3: 'panel:timeline' };

/**
 * The web viewer's own shortcuts, where the app has a native menu: Alt+1–3 for the files, the
 * terminal and the timeline, as the app's Ctrl+1–3 (the browser keeps those) and ? for the list. Find (Ctrl+F or ⌘F) is the workspace's.
 */
function useWebKeys(onShortcuts: () => void) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typing = target?.closest?.('input, textarea, [contenteditable="true"], .xterm') != null;
      const panel = PANELS[event.code];
      if (panel && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault();
        dispatchMenu({ id: panel });
      } else if (event.key === '?' && !typing && !event.ctrlKey && !event.metaKey && !event.altKey) {
        event.preventDefault();
        onShortcuts();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onShortcuts]);
}

/**
 * The invite code in the page's address (`/join#CODE`), removed from the address bar and the
 * history once read, so it does not stay where others could see it. The server never sees it:
 * browsers do not send the part after `#`.
 */
export function takeHashCode(): string {
  const code = decodeURIComponent(location.hash.replace(/^#/, '')).trim();
  if (location.hash) history.replaceState(null, '', location.pathname + location.search);
  return code;
}

/** The web viewer's page on `service`, `wss://host/…/signal`, for another service's invite. */
export function webPage(service: string, code: string): string {
  const base = service.replace(/^ws(s?):\/\//, 'http$1://').replace(/\/[^/]*$/, '');
  return `${base}/join#${code}`;
}

/** Opens the session in the SpecDeck app, for people who have it. */
export function appLink(service: string, code: string): string {
  return `specdeck://join/${code}?server=${encodeURIComponent(service)}`;
}

type Screen =
  | { kind: 'join'; code: string; error?: string; elsewhere?: { code: string; server: string } }
  | { kind: 'viewer'; code: string };

interface Props {
  /** This page's sharing service. */
  service: string;
  /** From the address, or empty. */
  initialCode: string;
}

export function WebApp({ service, initialCode }: Props) {
  const bridge = useBridge();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [screen, setScreen] = useState<Screen>({ kind: 'join', code: initialCode });
  const [shortcuts, setShortcuts] = useState(false);
  const showShortcuts = useCallback(() => setShortcuts(true), []);
  const hideShortcuts = useCallback(() => setShortcuts(false), []);
  useWebKeys(showShortcuts);

  useEffect(() => {
    void bridge.getProfile().then(setProfile);
  }, [bridge]);
  // Leaving the page leaves the session, so the host's list is right.
  useEffect(() => {
    const leave = () => void bridge.leaveSession();
    window.addEventListener('pagehide', leave);
    return () => window.removeEventListener('pagehide', leave);
  }, [bridge]);

  if (!profile) return null;
  return (
    <>
      {screen.kind === 'viewer' ? (
        <ViewerScreen
          key={screen.code}
          code={screen.code}
          layout={profileLayout(profile)}
          onLayout={(layout) => void bridge.setLayout(layout).then(setProfile)}
          onLeave={(retry) => setScreen({ kind: 'join', code: retry?.code ?? '', error: retry?.message })}
        />
      ) : (
        <JoinPage service={service} profile={profile} screen={screen} onScreen={setScreen} onProfile={setProfile} />
      )}
      {shortcuts && <ShortcutsDialog web onClose={hideShortcuts} />}
    </>
  );
}

function JoinPage({
  service,
  profile,
  screen,
  onScreen,
  onProfile,
}: {
  service: string;
  profile: Profile;
  screen: Extract<Screen, { kind: 'join' }>;
  onScreen: (screen: Screen) => void;
  onProfile: (profile: Profile) => void;
}) {
  const bridge = useBridge();
  const [code, setCode] = useState(screen.code);
  const [name, setName] = useState(profile.username ?? '');
  const [error, setError] = useState(screen.error ?? null);
  const version = useServiceVersion();
  const elsewhere = screen.elsewhere;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!code.trim()) return setError('Enter the invite code the host shared.');
    if (!name.trim()) return setError('Enter the name others should see.');
    let joinCode = code.trim();
    // A pasted link: join it here only when it is for this service.
    if (joinCode.includes('://')) {
      const link = await bridge.readJoinLink(joinCode);
      if (link.error || !link.code) return setError(link.error ?? 'That link is not a SpecDeck invite.');
      if (link.server) return onScreen({ kind: 'join', code: '', elsewhere: { code: link.code, server: link.server } });
      joinCode = link.code;
    }
    try {
      onProfile(await bridge.setUsername(name));
      onScreen({ kind: 'viewer', code: joinCode });
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <main className="start">
      <form className="welcome" onSubmit={submit} aria-labelledby="join-title">
        <h1 id="join-title">Watch a SpecDeck session</h1>
        <p>Follow a coding agent’s session in your browser. Nothing to install, and nothing you can change in the session.</p>
        {elsewhere && (
          <p className="field-hint" role="alert" style={{ textAlign: 'left' }}>
            This invite is for {serviceName(elsewhere.server)}, another sharing service. This page only joins sessions on{' '}
            {serviceName(service)}. <a href={webPage(elsewhere.server, elsewhere.code)}>Open it on {serviceName(elsewhere.server)}</a>.
          </p>
        )}
        <label htmlFor="code" style={{ textAlign: 'left', fontWeight: 600 }}>
          Invite code
        </label>
        <input
          {...PLAIN_TEXT}
          id="code"
          className="field"
          autoFocus={!code}
          placeholder="7K2M-9QXH-4TPA"
          autoComplete="off"
          value={code}
          onChange={(event) => setCode(event.target.value)}
        />
        <label htmlFor="username" style={{ textAlign: 'left', fontWeight: 600 }}>
          Your name
        </label>
        <input
          {...PLAIN_TEXT}
          id="username"
          className="field"
          autoFocus={Boolean(code) && !name}
          maxLength={64}
          placeholder="Your name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          aria-describedby="username-hint"
        />
        <span id="username-hint" className="field-hint" style={{ textAlign: 'left' }}>
          Shown to everyone in the session. It is a label, not a login, and stays in this browser.
        </span>
        {error && (
          <span className="error-text" role="alert">
            {error}
          </span>
        )}
        <button className="btn primary large" type="submit">
          Watch in this browser
        </button>
        {code.trim() && !code.includes('://') && (
          <a className="btn ghost" href={appLink(service, code.trim())}>
            Open in the SpecDeck app
          </a>
        )}
        {version && <p className="field-hint">SpecDeck {version}</p>}
      </form>
    </main>
  );
}

/**
 * The release of the service that served this page, from its `web-viewer.json` (relative, so a
 * service under a path prefix works). `null` until it arrives, and for a service that states none
 * or a failed request; nothing waits for it.
 */
function useServiceVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    fetch('web-viewer.json')
      .then((response) => (response.ok ? response.json() : null))
      .then((body: unknown) => {
        const value = (body as { version?: unknown } | null)?.version;
        if (current && typeof value === 'string' && value) setVersion(value);
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, []);
  return version;
}
