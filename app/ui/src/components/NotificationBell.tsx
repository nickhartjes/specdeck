import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { Bell } from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import type { Profile, ViewerNotifications } from '../api/types';
import type { ListEntry, ListOpen, NoticeList } from '../model/noticeList';
import { VIEWER_NOTIFICATION_KINDS } from '../model/viewerNotices';
import { inWebViewer } from './LayoutMenu';

interface Props {
  list: NoticeList;
  /** Opens what an entry is about; the list has closed by then. */
  onOpen: (target: ListOpen) => void;
  /** The viewer changed which kinds pop up. */
  onSwitches: (switches: ViewerNotifications) => void;
}

/**
 * The viewer's bell (`notification-bell` design decision 5): a count of what is new, the list of
 * what happened while they watched, and below it which kinds also pop up.
 */
export function NotificationBell({ list, onOpen, onSwitches }: Props) {
  const { entries, count } = useSyncExternalStore(list.subscribe, list.snapshot);
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);

  const close = (focus = true) => {
    setOpen(false);
    list.closed();
    if (focus) button.current?.focus();
  };

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!panel.current?.contains(target) && !button.current?.contains(target)) close();
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousedown', onDown);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousedown', onDown);
    };
    // `close` only reads refs and the list.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const toggle = () => {
    if (open) return close();
    list.opened();
    setOpen(true);
  };

  const label = count ? `Notifications, ${count} new` : 'Notifications';
  return (
    <div className="bell-menu">
      <button ref={button} className="btn ghost icon bell" aria-expanded={open} aria-controls="bell-panel" aria-label={label} title={label} onClick={toggle}>
        <Bell aria-hidden />
        {count > 0 && (
          <span className="bell-count" aria-hidden>
            {count > 99 ? '99+' : count}
          </span>
        )}
      </button>
      {open && (
        <div ref={panel} id="bell-panel" className="menu-panel bell-panel" role="dialog" aria-label="Notifications">
          {entries.length ? (
            <ul className="bell-list" aria-label="What happened">
              {entries.map((entry) => (
                <li key={entry.id} className={entry.marked ? 'new' : undefined}>
                  <Entry
                    entry={entry}
                    onOpen={(target) => {
                      close(false);
                      onOpen(target);
                    }}
                  />
                </li>
              ))}
            </ul>
          ) : (
            <p className="bell-empty muted">Nothing yet. What happens in this session from now on shows up here.</p>
          )}
          <Switches onSwitches={onSwitches} />
        </div>
      )}
    </div>
  );
}

function Entry({ entry, onOpen }: { entry: ListEntry; onOpen: (target: ListOpen) => void }) {
  const time = new Date(entry.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const content = (
    <>
      <span className="bell-title">
        {entry.marked && <span className="visually-hidden">New: </span>}
        {entry.title}
      </span>
      {entry.body && <span className="bell-body">{entry.body}</span>}
      <time className="bell-time" dateTime={new Date(entry.at).toISOString()}>
        {time}
      </time>
    </>
  );
  const { open } = entry;
  if (!open) return <div className="bell-entry">{content}</div>;
  return (
    <button className="bell-entry" onClick={() => onOpen(open)}>
      {content}
    </button>
  );
}

/**
 * Which kinds also pop up: the same settings as Settings' group. In the web viewer, which has no
 * Settings, the main switch is here too and asks the browser's permission.
 */
function Switches({ onSwitches }: { onSwitches: (switches: ViewerNotifications) => void }) {
  const bridge = useBridge();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [blocked, setBlocked] = useState<string | null>(null);
  useEffect(() => void bridge.getProfile().then(setProfile), [bridge]);
  if (!profile) return null;
  const web = inWebViewer();

  const turn = async (enabled: boolean) => {
    try {
      setProfile(await bridge.setNotifications(enabled));
      setBlocked(null);
    } catch (error) {
      setBlocked(errorMessage(error));
    }
  };
  const switchKind = async (kinds: ViewerNotifications) => {
    const next = await bridge.setViewerNotifications(kinds);
    setProfile(next);
    onSwitches(next.viewerNotifications);
  };

  return (
    <>
      {web && (
        <fieldset>
          <legend>Notifications</legend>
          <label className="check">
            <input type="checkbox" checked={profile.notifications} onChange={(event) => void turn(event.target.checked)} />
            <span>While this tab is in the background</span>
          </label>
          {blocked && (
            <p className="muted" role="status">
              {blocked}
            </p>
          )}
        </fieldset>
      )}
      {(!web || profile.notifications) && (
        <fieldset aria-describedby={profile.notifications ? undefined : 'bell-switches-hint'}>
          <legend>Also pop up when</legend>
          {VIEWER_NOTIFICATION_KINDS.map(({ kind, label }) => (
            <label key={kind} className="check">
              <input
                type="checkbox"
                checked={profile.viewerNotifications[kind]}
                disabled={!profile.notifications}
                onChange={(event) => void switchKind({ ...profile.viewerNotifications, [kind]: event.target.checked })}
              />
              <span>{label}</span>
            </label>
          ))}
          {!profile.notifications && (
            <span id="bell-switches-hint" className="field-hint">
              Notifications are off in Settings.
            </span>
          )}
        </fieldset>
      )}
    </>
  );
}
