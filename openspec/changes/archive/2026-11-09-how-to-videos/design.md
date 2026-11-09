# Design

## Context

- A video is three files in `docs/video/<name>/`. `capture.mjs` screenshots the UI's demo mode (`/?demo=start|host|viewer` on the Vite dev server, through `lib/capture.mjs`) and records where things are (`boxes.json`). `SCRIPT.json` holds the narration, one line per scene. `build.mjs` times the scenes from the spoken lines and writes the HyperFrames composition, with camera moves, a highlight ring, captions, a title card and a closing card (`lib/film.mjs`). `just video <names>` runs all three, renders the video, and writes a smaller copy and a poster to `docs/site/public/video/`.
- The demo (`app/ui/src/api/demo.ts`) is a fake bridge with one session about a `/health` endpoint. It already has suggestions, shared files, links, the braindump, read requests and votes. It has no OpenSpec plan, no steps of Claude's browser, and the web page (`web.html`, `src/web/main.tsx`) has no demo at all. Component tests also use the demo, and the existing videos depend on where things are in it.
- `deploy/compose` runs the released server image behind Caddy, with coturn 4.18, configured through `.env` (from `.env.example`). Colima's Docker runs here. Caddy gives `localhost` a certificate from its own local CA.

## Goals / Non-Goals

**Goals:**
- Six videos with the same look, voice and length as the existing three, each embedded on its docs page and listed in the README.
- Everything a video shows is true of the app today: real labels from the UI, and real output in the Compose video.
- `just video` can make every video again from the repository.

**Non-Goals:**
- Changing the look, voice or length of the existing three videos. They are made again for today's window (decision 7), not redesigned.
- Screen recordings of the real app. The film-from-screenshots approach stays.
- A demo mode for the host's side of the web viewer, which does not exist (hosting needs the app).

## Decisions

### 1. Demo scenes are opt-in: `?demo=viewer&scene=plans`

The new demo content (a plan, browser steps) is added only when the URL names a scene: `scene=plans` or `scene=browser`. Everything else is as today. Without that, a new prompt with its steps would push the existing timeline, which would move what the `following` video shows and could break component tests that read the demo.

Alternative: add the content to the default demo and remake the existing videos. Rejected: more work, for no gain to anyone watching.

- `plans`: an OpenSpec change `add-health-endpoint` in a `plan` timeline event with proposal, design and tasks (3 of 6 done). Its documents come as `document` events, with a revised design, so a tab shows *New* and *Changed* marks against an earlier version. It also has a summary step that offers *Read the plan*.
- `browser`: a prompt where Claude opens `http://localhost:3000/health` in its browser (the tool calls Claude's browser makes) and takes a screenshot. The screenshot is a small PNG of a plain JSON page, kept next to the demo (`app/ui/src/api/demo-browser.png`, imported by Vite) and shown in the timeline as MCP images are.
- The exact event shapes come from `api/types.ts` and from how the host builds these events. Wrong shapes show up as a missing card, which the capture catches (decision 6).

### 2. A development-only demo of the web page

A separate page, `web-demo.html` with `src/web/demo.tsx`, renders `WebApp` with `demoBridge('viewer')` and a made-up service (`wss://specdeck.example.com/signal`), with no WebAssembly. The build reads only `web.html`, so the demo page is served by the dev server and never built, and `src/web/main.tsx` is unchanged. The join page then shows as it does for real. Selecting *Watch in this browser* opens the demo session, with the web viewer's own layout menu and keys.

Alternative: a branch in `src/web/main.tsx` under `import.meta.env.DEV` and `?demo=viewer`. Not chosen: it puts demo code in the production entry and relies on the bundler to drop it.

How the capture reaches it: the page is `web-demo.html` with `vite.web.config.ts`. `lib/capture.mjs` gets a `server` option, `app` (today's, :5173) or `web` (the web config, on :5174). The option starts the matching dev server when none is running.

Alternative: film the real web viewer against `just local-dev`. Rejected: a real session has no realistic content without a scripted agent, and it would not be reproducible.

### 3. `self-hosting`: a real run, saved, then rendered

1. **Record once** with a script `docs/video/self-hosting/record.sh`, run by hand, not by `just video`. In a temporary copy of `deploy/compose`, it writes `.env` with the settings `compose.yaml` requires: `SPECDECK_DOMAIN=localhost`, `TURN_DOMAIN=127.0.0.1`, `TURN_EXTERNAL_IP=127.0.0.1`, a fresh `TURN_SECRET` and the current release as `SPECDECK_VERSION`. The temporary copy is under the video's folder, because Colima and Docker Desktop share only the home folder with their VM. The recording ran on a separate Colima profile (`colima start --profile compose`), because the default VM's k3s already holds UDP 3478 and the relay ports for the local-dev environments. It then runs the steps of the guide: `docker compose up -d`, `docker compose ps`, `curl https://localhost/health` and `/web-viewer.json` with Caddy's local CA, and `docker compose logs server --tail`. For each step it saves the command and its real output to `run/<n>-<step>.txt`. Then `docker compose down -v`.
2. **Redact** in the script, before saving: the TURN secret, and any address other than 127.0.0.1 and the container network, become placeholders such as `•••`. The narration says the run is local. The video never claims `localhost` is how to deploy; the DNS and firewall steps are shown from the docs page.
3. **Render**: `capture.mjs` opens a small terminal page, `self-hosting/terminal.html` (static, dark, the same font as the app's terminal). It feeds the page each saved step and screenshots it, and also screenshots the Docker Compose docs page (from `just docs` or the built site) for DNS, firewall and `.env`. Then it screenshots `?demo=start` Settings with a sharing service typed in, for connecting the apps.

Because the output is saved in the repository, `just video self-hosting` makes the same video without Docker. Recording again after a release is one command, and the diff of `run/` shows what changed.

Alternative: run Compose during `just video`. Rejected: it needs Docker and the network on every render, and the output would change from run to run.

### 4. Where each video goes

| Video | Docs page (embedded at) | README Videos table |
|---|---|---|
| taking-part | `guide/follow`, at *Suggest a prompt* | row 2 |
| web-viewer | `guide/join`, at *Watch in a browser* | row 2 |
| read-and-estimate | `guide/read-and-estimate`, at the top | row 2 |
| plans | `guide/documents`, at the top | row 3 |
| claude-browser | `guide/browser`, at the top | row 3 |
| self-hosting | `self-host/docker-compose`, at the top | row 3 |

The README table keeps three columns per row, as today, so its 9 videos fill three rows. The embed is the same `<video class="video" …>` tag the existing pages use, with an `aria-label` that names the video, its length, and that it is narrated with captions. `docs/video/README.md` lists all nine, and the `video` recipe's default names list all nine.

### 5. Narration

Each `SCRIPT.json` is written from the docs pages, so the video says what the docs say, in the same words for labels: **Prompts**, **Suggest to**, **Watch in this browser**, **Read the plan**, and so on. The lines are about 10–14 per video, like the existing ones, with the same Kokoro voice (the default in `speak.py`). A shortcut is spoken for both macOS and Windows/Linux, as in `following`.

### 6. Checking a video

- `capture.mjs` throws when a box it needs is missing (`lib/capture.mjs` already does), so a wrong label or a missing demo card fails the capture rather than filming the wrong thing.
- Look at each render in full once. Check the poster frame, that captions match the voice, and that every ring lands on what the narration names.
- `just docs-build` resolves each docs page's poster and video file through the link check. A `<video src>` is not an `<a href>`, so task 1.3 extends the check to `src` and `poster` attributes.

### 7. The existing three videos are made again first

They were captured on 2026-11-05. Since then the session window was redesigned: the viewer's side panel, the People tab, the View menu with *Show terminal*, and the file detail pane are gone or changed. Their captures fail, and the renders in the docs show the old window.

- Update each `capture.mjs` to today's window, using roles and names over class names where the UI offers them, so the next redesign breaks less.
- Read each `SCRIPT.json` against the docs and the UI. Change only lines that are no longer true, such as "Change this any time under View", and keep the rest word for word, so the voice and timing stay close.
- Adjust each `build.mjs` to the new boxes. Keep the scenes, the title and the closing card.
- They go first: a working remake proves the set-up, `lib/capture.mjs` and the render before the new videos depend on them.

### 8. A smoother look, piloted on `taking-part`

Optional in `lib/film.mjs`, so a video opts in and the others are unchanged until the pilot is approved. The techniques come from the HyperFrames registry (`yt-feather-highlight`, `oversized-cursor`) and the motion guidance bundled with the CLI (`dist/skills/hyperframes/references/motion.md`, `captions.md`), adapted to our screenshots rather than mounted as components, which bring their own stage.

- **Spotlight instead of the ring:** a feathered dim layer with a soft hole that glides from target to target, with a faint outline for precision. It replaces the hard yellow ring and flat dim.
- **Blur-swap cuts:** the next shot blurs in over the current one on the same stage, and the camera moves on from there. This replaces zooming out to the whole window, fading, and zooming back in.
- **A pointer:** a macOS arrow that glides to the control the narration names and presses it (scale 0.9 and back), with no click ring, which the guidance lists as a generated-looking default.
- **Motion blur** on camera moves: `data-motion-blur="on"` on the root.
- **Captions per word group:** `hyperframes transcribe` (local whisper, `small.en`) gives each spoken word's time. Captions then show 4–6 words at a time and follow the voice, instead of whole sentences timed by character count. The words shown are the script's, matched by position. If the counts differ, that line falls back to today's timing.

`hyperframes compare` (or a side-by-side of key frames) shows the old and new versions to the user. Only after their approval does the look become the default for all nine videos.

Not adopted: HyperFrames' agent skills are not installed into the user's tools (the user decides; they install globally and enable telemetry), and no music or sound effects in this pass.

## Risks / Trade-offs

- [The demo drifts from the app] → Scenes reuse the real components; only the data is fake. The capture fails on a missing label.
- [Six renders add about 25–30 MB of mp4 to the repository] → The same size and compression as the existing three. Revisit Git LFS or hosting the files elsewhere if the folder passes about 100 MB.
- [The Compose run needs pulling images (ghcr.io, Docker Hub)] → Only when recording. If the network blocks a pull, record on another network, or ask.
- [Caddy's local CA in the recording differs from a real deployment] → The narration says it is a local run, and the shown output (containers up, `/version`, logs) is the same as on a server.
- [Claude's browser tool names change] → The `browser` scene uses the names the host recognises today (`app/src-tauri/src/browser`). Remake the video if they change.

## Migration Plan

None. Documentation and development tooling only.

## Open Questions

- The voice: keep the default Kokoro voice of the existing videos, unless you want another. This can be settled when listening to the first render, and doesn't change the plan.
