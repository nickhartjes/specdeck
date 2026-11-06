import { useEffect, useState } from 'react';
import { errorMessage, useBridge } from './api/bridge';
import type { JoinLink, Profile, View } from './api/types';
import { ConfirmDialog } from './components/ConfirmDialog';
import { JoinThroughDialog } from './components/JoinThroughDialog';
import { ConnectionStatus } from './components/ConnectionStatus';
import { ShortcutsDialog } from './components/ShortcutsDialog';
import { UpdateButton, UpdateNotice, useReadyUpdate } from './components/UpdateNotice';
import { dispatchMenu, useMenu, type MenuCommand } from './menu/menu';
import { formatCode } from './model/code';
import { startHeartbeat } from './model/heartbeat';
import { restartQuestion, type RestartQuestion } from './model/restart';
import { HostScreen } from './screens/HostScreen';
import { StartScreen } from './screens/StartScreen';
import { ViewerScreen } from './screens/ViewerScreen';

// `server`: the sharing service of a join link the user agreed to join through.
type Screen =
  | { kind: 'start'; code?: string; server?: string; joinError?: string }
  | { kind: 'host' }
  | { kind: 'viewer'; code: string; server?: string };

export function App() {
  const bridge = useBridge();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [screen, setScreen] = useState<Screen>({ kind: 'start' });
  const [error, setError] = useState<string | null>(null);
  // A host or join command for the start screen. A window that shows a session gets none: the app
  // opens those in a new window.
  const [startCommand, setStartCommand] = useState<MenuCommand | null>(null);
  // A join link through another sharing service, waiting for the user to agree.
  const [joinThrough, setJoinThrough] = useState<JoinLink | null>(null);
  const [shortcuts, setShortcuts] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState(false);
  const refresh = () =>
    bridge.getProfile().then((p) => {
      setProfile(p);
      // Styles that depend on the platform and on the window's material.
      document.documentElement.dataset.platform = p.platform;
      document.documentElement.classList.toggle('translucent', p.translucent);
      return p;
    });
  // A new SpecDeck ready to install, the question before restarting during a session, and why
  // installing failed.
  const update = useReadyUpdate();
  const [askRestart, setAskRestart] = useState<RestartQuestion | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const installUpdate = () => {
    setAskRestart(null);
    bridge.restartToUpdate().catch((e) => setUpdateError(errorMessage(e)));
  };
  // Restarting ends every hosted session for everyone and leaves every watched one, in all
  // windows, so it asks first when there is any.
  const restartToUpdate = async () => {
    const p = await refresh().catch(() => profile);
    const hosted = Math.max(p?.hostedSessions ?? 0, screen.kind === 'host' ? 1 : 0);
    const watched = Math.max(p?.watchedSessions ?? 0, screen.kind === 'viewer' ? 1 : 0);
    const question = restartQuestion(update.version ?? '', hosted, watched);
    if (question) setAskRestart(question);
    else installUpdate();
  };
  const updateButton = update.version ? <UpdateButton version={update.version} onRestart={() => void restartToUpdate()} /> : null;

  const setView = (view: View) => void bridge.setView(view).then(setProfile);

  useEffect(() => bridge.onMenu(dispatchMenu), [bridge]);
  // Joins from a link, asking first when it names another sharing service.
  const joinFromLink = (link: JoinLink) => {
    if (!link.code) return;
    if (link.server) setJoinThrough(link);
    else setScreen({ kind: 'viewer', code: link.code });
  };
  // A join link that opened the app, or came in while it runs. The app gives one only to a window
  // on its start screen, and opens a new window for it otherwise.
  const takeJoinLink = async () => {
    const link = await bridge.takeJoinLink();
    if (!link || screen.kind !== 'start') return;
    if (link.error) setScreen({ kind: 'start', joinError: link.error });
    else if (link.code) joinFromLink(link);
  };
  // The join field on the start screen: a code, or a pasted link that may name a service.
  const join = (code: string, pasted?: string) => {
    if (pasted) {
      void bridge.readJoinLink(pasted).then((link) => (link.error ? setScreen({ kind: 'start', joinError: link.error }) : joinFromLink(link)));
    } else if (screen.kind === 'start' && screen.server && screen.code && formatCode(screen.code) === formatCode(code)) {
      // Trying again after a failed join keeps the service the user agreed to.
      setScreen({ kind: 'viewer', code, server: screen.server });
    } else {
      setScreen({ kind: 'viewer', code });
    }
  };

  useEffect(() => startHeartbeat((beat) => void bridge.heartbeat(beat).catch(() => {})), [bridge]);

  useMenu((command) => {
    if (command.id === 'shortcuts') return setShortcuts(true);
    if (command.id === 'connection-status') return setConnectionStatus(true);
    if (command.id === 'join-link') return void takeJoinLink();
    if (command.id === 'restart-to-update') return void restartToUpdate();
    // Changed in the settings window.
    if (command.id === 'profile-changed') return void refresh();
    if (START_COMMANDS.has(command.id) && screen.kind === 'start') setStartCommand(command);
  });

  useEffect(() => {
    refresh()
      // A window reopened while a session runs goes straight back to it; a new window may have
      // opened for a join link or a start command.
      .then((p) => {
        if (p.hosting) setScreen({ kind: 'host' });
        else {
          void takeJoinLink();
          void bridge.takeStartCommand().then((command) => command && setStartCommand(command));
        }
        const demo = import.meta.env.DEV ? new URLSearchParams(location.search).get('demo') : null;
        if (demo === 'viewer') setScreen({ kind: 'viewer', code: '7K2M9QXH4TPA' });
      })
      .catch((e) => setError(errorMessage(e)));
    // Loads once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (error) {
    return (
      <div className="center-state">
        <div className="box" role="alert">
          <h2>SpecDeck could not start</h2>
          <p>{error}</p>
        </div>
      </div>
    );
  }
  if (!profile) {
    return (
      <div className="center-state" aria-busy="true">
        <span className="spinner" aria-label="Loading" />
      </div>
    );
  }

  return (
    <>
      {screen.kind === 'host' && (
        <HostScreen
          view={profile.view}
          onView={setView}
          screenReader={profile.screenReader}
          onEnded={() => refresh().then(() => setScreen({ kind: 'start' }))}
          update={updateButton}
        />
      )}
      {screen.kind === 'viewer' && (
        <ViewerScreen
          key={`${screen.code} ${screen.server ?? ''}`}
          code={screen.code}
          server={screen.server}
          view={profile.view}
          onView={setView}
          screenReader={profile.screenReader}
          update={updateButton}
          onLeave={(retry) =>
            setScreen(retry ? { kind: 'start', code: retry.code, server: retry.server, joinError: retry.message } : { kind: 'start' })
          }
        />
      )}
      {/* Without a toolbar the top of the window is where it is dragged (macOS title bar). */}
      {screen.kind === 'start' && <div className="drag-strip" data-tauri-drag-region />}
      {screen.kind === 'start' && update.version && !updateError && (
        <UpdateNotice version={update.version} onRestart={() => void restartToUpdate()} onDismiss={update.dismiss} />
      )}
      {updateError && (
        <div className="update-notice" role="alert">
          <span>{updateError}</span>
          <button className="btn ghost icon" onClick={() => setUpdateError(null)} aria-label="Close">
            ×
          </button>
        </div>
      )}
      {screen.kind === 'start' && (
        <StartScreen
          profile={profile}
          onProfile={setProfile}
          onHosting={() => {
            setScreen({ kind: 'host' });
            void refresh();
          }}
          onJoin={join}
          initialCode={screen.code}
          joinError={screen.joinError}
          command={startCommand}
          onCommandDone={() => setStartCommand(null)}
        />
      )}
      {joinThrough?.code && joinThrough.server && (
        <JoinThroughDialog
          serverName={joinThrough.serverName ?? joinThrough.server}
          canKeep={!profile.serviceOverridden}
          onCancel={() => setJoinThrough(null)}
          onJoin={async (keep) => {
            const { code, server } = joinThrough as { code: string; server: string };
            setJoinThrough(null);
            if (keep) {
              // Joining does not depend on saving the setting.
              await bridge.setSharingService(server).then(setProfile, () => {});
            }
            setScreen({ kind: 'viewer', code, server });
          }}
        />
      )}
      {askRestart && update.version && (
        <ConfirmDialog
          title={askRestart.title}
          confirm={askRestart.confirm}
          danger={askRestart.danger}
          onCancel={() => setAskRestart(null)}
          onConfirm={installUpdate}
        >
          <p>{askRestart.text}</p>
        </ConfirmDialog>
      )}
      {shortcuts && <ShortcutsDialog onClose={() => setShortcuts(false)} />}
      {connectionStatus && <ConnectionStatus onClose={() => setConnectionStatus(false)} />}
    </>
  );
}

/** Menu commands that start something from the start screen. */
const START_COMMANDS = new Set(['host', 'join', 'recent']);
