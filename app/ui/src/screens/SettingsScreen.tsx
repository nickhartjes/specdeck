import { useEffect, useState, type FormEvent } from 'react';
import { errorMessage, useBridge } from '../api/bridge';
import type { Profile, ServiceCheck } from '../api/types';
import { APPEARANCES } from '../appearance';
import { serviceName } from '../model/code';

/** The settings window: changes apply at once, as is usual for app settings. */
export function SettingsScreen() {
  const bridge = useBridge();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    void bridge.getProfile().then((p) => {
      setProfile(p);
      setName(p.username ?? '');
    });
  }, [bridge]);
  // A session started or ended, which decides whether the sharing service can change.
  useEffect(
    () => bridge.onMenu((command) => command.id === 'profile-changed' && void bridge.getProfile().then(setProfile)),
    [bridge],
  );

  if (!profile) return <div className="settings" aria-busy="true" />;

  const saveName = async (event?: FormEvent) => {
    event?.preventDefault();
    if (name.trim() === (profile.username ?? '')) return;
    if (!name.trim()) return setError('Enter the name others should see.');
    try {
      setProfile(await bridge.setUsername(name));
      setError(null);
      setSaved(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <main className="settings">
      <form className="row" onSubmit={saveName}>
        <label htmlFor="settings-name">Your name</label>
        <div>
          <input
            id="settings-name"
            className="field"
            maxLength={64}
            value={name}
            onChange={(event) => {
              setName(event.target.value);
              setSaved(false);
            }}
            onBlur={() => void saveName()}
            aria-describedby="settings-name-hint"
            aria-invalid={error ? true : undefined}
          />
          <span id="settings-name-hint" className="field-hint">
            {error ? (
              <span className="error-text" role="alert">
                {error}
              </span>
            ) : saved ? (
              'Saved. Sessions you start or join from now on use this name.'
            ) : (
              'Shown to everyone in a session. It is a label, not a login.'
            )}
          </span>
        </div>
      </form>
      <div className="row">
        <span id="settings-appearance">Appearance</span>
        <div>
          <div className="segmented" role="radiogroup" aria-labelledby="settings-appearance" aria-describedby="settings-appearance-hint">
            {APPEARANCES.map(({ value, label }) => (
              <label key={value}>
                <input
                  type="radio"
                  name="appearance"
                  value={value}
                  checked={profile.appearance === value}
                  onChange={async () => setProfile(await bridge.setAppearance(value))}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <span id="settings-appearance-hint" className="field-hint">
            System follows your computer. The terminal stays dark, so everyone sees the agent&rsquo;s colours the same way.
          </span>
        </div>
      </div>
      <div className="row">
        <span id="settings-notifications">Notifications</span>
        <div>
          <label className="check">
            <input
              type="checkbox"
              checked={profile.notifications}
              aria-labelledby="settings-notifications"
              aria-describedby="settings-notifications-hint"
              onChange={async (event) => setProfile(await bridge.setNotifications(event.target.checked))}
            />
            Show notifications
          </label>
          <span id="settings-notifications-hint" className="field-hint">
            When someone joins or leaves your session, when the agent waits for you, and when a session you watch ends. Only
            while SpecDeck is not in front.
          </span>
        </div>
      </div>
      <div className="row">
        <span id="settings-screen-reader">Screen reader</span>
        <div>
          <label className="check">
            <input
              type="checkbox"
              checked={profile.screenReader}
              aria-labelledby="settings-screen-reader"
              aria-describedby="settings-screen-reader-hint"
              onChange={async (event) => setProfile(await bridge.setScreenReader(event.target.checked))}
            />
            Screen reader support
          </label>
          <span id="settings-screen-reader-hint" className="field-hint">
            Lets VoiceOver, Narrator and Orca read the terminal. While it is on, typing in the terminal of a session you host can
            repeat text.
          </span>
        </div>
      </div>
      {profile.platform === 'macos' && (
        <div className="row">
          <span id="settings-browser">Browser</span>
          <div>
            <label className="check">
              <input
                type="checkbox"
                checked={profile.browser}
                aria-labelledby="settings-browser"
                aria-describedby="settings-browser-hint"
                onChange={async (event) => setProfile(await bridge.setBrowser(event.target.checked))}
              />
              Give Claude a browser
            </label>
            <span id="settings-browser-hint" className="field-hint">
              Claude Code sessions you start from now on can open, read and screenshot web pages in a browser of their own,
              separate from yours and empty at the start. Claude asks before using it.
            </span>
          </div>
        </div>
      )}
      <div className="row">
        <span id="settings-verbose-logging">Diagnostics</span>
        <div>
          <label className="check">
            <input
              type="checkbox"
              checked={profile.verboseLogging}
              aria-labelledby="settings-verbose-logging"
              aria-describedby="settings-verbose-logging-hint"
              onChange={async (event) => setProfile(await bridge.setVerboseLogging(event.target.checked))}
            />
            Verbose logging
          </label>
          <span id="settings-verbose-logging-hint" className="field-hint">
            Also records how much each connection sends, summed per second. Never what is sent. Logs are kept for 7 days.
          </span>
        </div>
      </div>
      {profile.updatesItself && (
        <div className="row">
          <span id="settings-updates">Updates</span>
          <div>
            <label className="check">
              <input
                type="checkbox"
                checked={profile.checkUpdates}
                aria-labelledby="settings-updates"
                aria-describedby="settings-updates-hint"
                onChange={async (event) => setProfile(await bridge.setCheckUpdates(event.target.checked))}
              />
              Check for updates automatically
            </label>
            <span id="settings-updates-hint" className="field-hint">
              SpecDeck downloads new versions in the background and installs them when you choose Restart to update. Check for
              Updates… in the menu works either way.
            </span>
          </div>
        </div>
      )}
      <SharingService profile={profile} onProfile={setProfile} />
    </main>
  );
}

type Status = { kind: 'idle' } | { kind: 'error'; message: string } | { kind: 'checking' } | { kind: 'checked'; check: ServiceCheck };

/**
 * The sharing service sessions use (`configurable-sharing-service`): empty for the built-in one.
 * Saving checks whether it answers, and saves either way.
 */
function SharingService({ profile, onProfile }: { profile: Profile; onProfile: (profile: Profile) => void }) {
  const bridge = useBridge();
  const [value, setValue] = useState(profile.sharingService ?? '');
  const [status, setStatus] = useState<Status>({ kind: 'idle' });
  const locked = profile.inSession || profile.serviceOverridden;

  const save = async (event?: FormEvent) => {
    event?.preventDefault();
    if (locked || value.trim() === (profile.sharingService ?? '')) return;
    let saved: Profile;
    try {
      saved = await bridge.setSharingService(value);
    } catch (e) {
      return setStatus({ kind: 'error', message: errorMessage(e) });
    }
    onProfile(saved);
    setValue(saved.sharingService ?? '');
    setStatus({ kind: 'checking' });
    setStatus({ kind: 'checked', check: await bridge.checkSharingService(saved.effectiveService) });
  };

  const name = serviceName(profile.effectiveService);
  const unencrypted = profile.effectiveService.startsWith('ws://') ? ' Not encrypted.' : '';
  const hint = profile.serviceOverridden
    ? `Set to ${name} by SPECDECK_SIGNALLING_URL when SpecDeck started.`
    : profile.inSession
      ? 'Applies to the next session you start or join.'
      : status.kind === 'checking'
        ? `Checking ${name}…`
        : status.kind === 'checked' && status.check.reachable
          ? `Saved. ${name} can be reached.${unencrypted}`
          : status.kind === 'checked'
            ? `Saved, but ${name} cannot be reached. ${status.check.problem ?? ''}`
            : `Leave empty for the built-in service. If your team runs its own, enter its domain, such as specdeck.example.com.${unencrypted}`;

  return (
    <form className="row" onSubmit={save}>
      <label htmlFor="settings-service">Sharing service</label>
      <div>
        <input
          id="settings-service"
          className="field"
          placeholder={serviceName(profile.builtInService)}
          autoComplete="off"
          spellCheck={false}
          disabled={locked}
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
            setStatus({ kind: 'idle' });
          }}
          onBlur={() => void save()}
          aria-describedby="settings-service-hint"
          aria-invalid={status.kind === 'error' ? true : undefined}
        />
        <span id="settings-service-hint" className="field-hint" aria-live="polite">
          {status.kind === 'error' ? (
            <span className="error-text" role="alert">
              {status.message}
            </span>
          ) : (
            hint
          )}
        </span>
      </div>
    </form>
  );
}
