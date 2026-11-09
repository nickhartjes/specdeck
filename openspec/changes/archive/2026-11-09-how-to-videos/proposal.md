# Proposal

## Why

#102 asks for a short narrated video for every feature worth showing in motion. `documentation-rule` audited the specs and listed six videos still to make, in priority order. Only three videos exist (getting started, following a session, diagnostics), so taking part, the web viewer, reading and estimating, plans, Claude's browser and self-hosting have none.

## What Changes

- **Six new how-to videos** in `docs/video`, made like the existing three with `just video`, each about 1 to 1½ minutes, narrated and captioned:
  1. `taking-part`: suggesting a prompt and what became of it, sharing a file and a link, the braindump, and the host giving it to the agent.
  2. `web-viewer`: opening the host's web link, *Watch in this browser*, the session in the page, and the keys that differ.
  3. `read-and-estimate`: the host asks everyone to read, viewers answer, then a vote and its result.
  4. `plans`: the plan card, a plan's documents formatted, *New* and *Changed* marks, Source, and the task count.
  5. `claude-browser`: Claude opens the app it builds in its own browser, with the screenshot in the timeline (macOS).
  6. `self-hosting`: running your own sharing service with Docker Compose, from a real local run, then pointing the app at it.
- **The existing three videos made again** (getting started, following, diagnostics). They show the window from before the session-window redesign, and their captures no longer run. Their captures and narration are brought up to date with today's window, and they are rendered again first, which also proves the pipeline.
- **Demo scenes** in the UI's demo mode, for what it cannot show today: a plan with its documents, steps of Claude's browser with a screenshot, and the web viewer's page. They are opt-in through a `scene` parameter, so the default demo and its component tests stay as they are, and so do the remade videos.
- **A terminal recording for `self-hosting`:** the commands and output of a real `docker compose` run, saved in the video's folder with secrets and addresses removed, and rendered as a terminal page that the capture screenshots like the app.
- **Embedded and listed:** each video on its docs page, in the README's Videos table, and in `docs/video/README.md`. `just video` makes all nine by default.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
<!-- None: the videos are documentation, and the demo scenes are development tooling (the demo is never used by the app). The rule that videos are embedded and listed is in documentation-rule's user-documentation spec. -->

This change sets `skip_specs: true`.

## Impact

- New: `docs/video/{taking-part,web-viewer,read-and-estimate,plans,claude-browser,self-hosting}/` (SCRIPT.json, capture.mjs, build.mjs, hyperframes.json; `self-hosting/run/` holds the recording), and `docs/site/public/video/<name>.{mp4,jpg}` (about 4–5 MB each, as today).
- Changed: `docs/video/{getting-started,following,diagnostics}/` (captures, narration and builds) and their renders, `app/ui/src/api/demo.ts` (the scenes), `app/ui/src/web/main.tsx` (a development-only demo of the web page), `docs/video/lib/capture.mjs` (the web page and the terminal page as sources), `justfile` (`video` default names), the docs pages that embed the videos, `README.md` (Videos table), `docs/video/README.md`.
- Documentation: this change is the documentation. Each video is embedded on its docs page and listed in the README. `just docs-build` checks the links and posters.
- Depends on `documentation-rule` (the README Videos table and the prompt-suggestions docs sections it links to). Needs the npm packages of `app/ui` and `docs/video`, so a CodeArtifact login, and Colima's Docker for the Compose run.
