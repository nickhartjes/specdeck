# Proposal

## Why

A sharing service now states its release in `/web-viewer.json`, but only someone who knows that address sees it. A browser viewer, or an operator checking the page after an update, has nothing on the page itself that says which release the service runs.

## What Changes

- The web viewer's join page shows the sharing service's release under the form, small and muted: "SpecDeck 1.7.0".
- The page asks its own service for the version (`/web-viewer.json`), so the text always matches the server, the one the page was built into.
- When the service gives no version (a release before 1.7, or the request fails), the line is left out; joining is never delayed or blocked by it.

Out of scope: showing the version inside a running session, and showing the desktop app's own version.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `web-viewer`: the join page shows the service's release.

## Impact

- `app/ui/src/web/WebApp.tsx` (the join page) and its tests.
- No change to the server, the protocol or the desktop app. The page's security policy already allows requests to its own service.
