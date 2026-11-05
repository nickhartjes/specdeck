# Proposal

## Why

The sharing service (signalling and TURN) is baked into the app at build time, and can otherwise only be changed with the `SPECDECK_SIGNALLING_URL` environment variable at every launch. A team that runs its own server cannot point the released installers at it, so "host your own server" is not something the documentation can honestly offer. Making the service a setting, and carrying it in join links, lets any team run its own server with the normal app.

## What Changes

- A **Sharing service** setting in Settings. Empty means the built-in service; a value overrides it. It accepts a domain (`specdeck.example.com`) or a full URL, takes effect for the next session started or joined, and cannot be changed during a session. On save the app checks whether the service is reachable, and saves either way.
- The `SPECDECK_SIGNALLING_URL` environment variable keeps working and takes precedence over the setting (for development and testing).
- Join links carry the host's sharing service: `specdeck://join/<code>?server=<url>`. Copying the code alone is unchanged, and a bare code uses the viewer's own setting.
- Opening a join link whose service differs from the viewer's asks first: join through that service for this session only, with an unchecked option to use it from now on. Links without a service keep working and use the viewer's own setting.
- A viewer's reconnects stay on the service they joined through.

Not a breaking change for 1.x hosts or 1.x links already shared. A 1.0 app that opens a new link with `?server=` cannot read it (see design.md).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-sharing`: where the app finds the sharing service (setting, environment, built-in default), and join links that carry the host's service, with a confirmation when it differs from the viewer's.

## Impact

- `app/src-tauri`: settings gain an optional sharing service; the service is chosen when a session starts instead of once at app start; join link handling reads the service and asks before using another one; a reachability check for the Settings screen.
- `app/ui`: Settings screen field with hint and check result; join prompt for a different service.
- `crates/protocol` (`invite.rs`): join links with an optional, URL-encoded `server` parameter.
- Docs: `docs/user-guide.md` (Settings, joining through another service); enables a "run your own server" guide in the documentation site.
