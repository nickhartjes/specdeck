# Tasks

## 1. Set-up

- [x] 1.1 Install the packages of `app/ui` (root workspace) and `docs/video` after the CodeArtifact login, and the video venv (`docs/video/.venv`, `requirements.txt`). Verify that the installs succeed and that `pnpm lint` passes.
- [x] 1.2 `lib/capture.mjs`: a `server` option (`app` on :5173, `web` on :5174 with `vite.web.config.ts`), and a `page(url)` for local files (the terminal page). Verify that the remade captures of group 2 run with it.
- [x] 1.3 Extend `docs/site/scripts/check-links.mjs` to `src` and `poster` attributes of `<video>` and `<img>` (design decision 6). Verify that `just docs-build` passes, and fails for a poster that does not exist.

## 2. The existing three videos (design decision 7)

- [x] 2.1 `getting-started`: update `capture.mjs`, `SCRIPT.json` (only lines no longer true) and `build.mjs` to today's window. Verify with `just video getting-started`, and by watching the render in full.
- [x] 2.2 `following`: the same. Verify with `just video following`, and by watching the render in full.
- [x] 2.3 `diagnostics`: the same. Verify with `just video diagnostics`, and by watching the render in full.
- [x] 2.4 Check that the docs pages and README still describe each video correctly (the `aria-label`s and the Videos table's lines), and `docs/video/README.md`'s *Shows* column. Verify with `just docs-build`.

## 3. taking-part

- [x] 3.1 `docs/video/taking-part/`: `SCRIPT.json`, `capture.mjs` and `build.mjs` (design decisions 4–5). Cover the viewer's **Prompts** tab (suggest, *Your suggestions* and their states), the host's inbox (*Send*, *Edit*, *Decline*), sharing a file and a link, and the braindump with the host's *Give*. Verify with `just video taking-part`, and by watching the render in full (decision 6).
- [x] 3.2 Embed it in `guide/follow.md` at *Suggest a prompt*, and add it to the README's Videos table and `docs/video/README.md`. Verify with `just docs-build`.

- [x] 3.3 Pilot the smoother look of design decision 8 on `taking-part`: spotlight, blur-swap cuts, pointer, motion blur and word-group captions in `lib/film.mjs` as options, plus a transcription step in the `video` recipe. Verify with `just video taking-part`, `hyperframes check` with no new errors, and a side-by-side of old and new frames shown to the user.
- [x] 3.4 After the user approves the pilot, make the look the default and render the other three made so far again. Verify by rendering each, and by a contact sheet of each.

## 4. web-viewer

- [x] 4.1 The development-only web demo, `web-demo.html` and `src/web/demo.tsx` (design decision 2). Verify that `pnpm --dir app/ui build:web` output does not contain the demo (`dist-web` holds only `web.html` and its assets, and no demo-only string), that `/web-demo.html` on the web dev server shows the join page and then the demo session, and that `pnpm test` passes.
- [x] 4.2 `docs/video/web-viewer/` with `server: 'web'`: the join page, the name, *Watch in this browser*, the session, the layout menu (appearance), and the keys that differ. Verify with `just video web-viewer`, and by watching the render.
- [x] 4.3 Embed it in `guide/join.md` at *Watch in a browser*, README and `docs/video/README.md`. Verify with `just docs-build`.

## 5. read-and-estimate

- [x] 5.1 `docs/video/read-and-estimate/`: the host asks to read, viewers answer *Ready* or *I have a question*, the host sees who has read, then an estimate, votes and the result. Use the demo's existing timed answers (`demo.ts`, "the demo viewers answer a read request and vote"). Verify with `just video read-and-estimate`, and by watching the render.
- [x] 5.2 Embed it at the top of `guide/read-and-estimate.md`, README and `docs/video/README.md`. Verify with `just docs-build`.

## 6. plans

- [x] 6.1 The `scene=plans` demo content (design decision 1). Verify that `?demo=viewer&scene=plans` shows the plan card with its documents and task count, that a document tab shows *New* and *Changed* marks and switches to *Source*, that `?demo=viewer` without a scene is unchanged (the remade `following` capture still runs and shows the same), and that `pnpm test` passes.
- [x] 6.2 `docs/video/plans/`. Verify with `just video plans`, and by watching the render.
- [x] 6.3 Embed it at the top of `guide/documents.md`, README and `docs/video/README.md`. Verify with `just docs-build`.

## 7. claude-browser

- [x] 7.1 The `scene=browser` demo content with `demo-browser.png` (design decision 1), using the tool names in `app/src-tauri/src/browser`. Verify that `?demo=viewer&scene=browser` and `?demo=host&scene=browser` show the browser steps and the screenshot in the timeline, that the default demo is unchanged, and that `pnpm test` passes.
- [x] 7.2 `docs/video/claude-browser/`: Claude opens the app, the screenshot in the timeline, the setting *Give Claude a browser*, and that it is only on macOS. Verify with `just video claude-browser`, and by watching the render.
- [x] 7.3 Embed it at the top of `guide/browser.md`, README and `docs/video/README.md`. Verify with `just docs-build`.

## 8. self-hosting

- [x] 8.1 `docs/video/self-hosting/record.sh` and `terminal.html` (design decision 3). Run it once against Colima's Docker. Verify that `run/` holds each step's real command and output, that `grep` finds no TURN secret and no address other than 127.0.0.1 or the container network in `run/`, and that `docker compose ps` in the recording shows the three services running.
- [x] 8.2 `docs/video/self-hosting/`: DNS and firewall from the docs page, `.env`, `docker compose up -d`, the check, the logs, and the app's Settings with the sharing service. Verify with `just video self-hosting` (without Docker), and by watching the render.
- [x] 8.3 Embed it at the top of `self-host/docker-compose.md`, README and `docs/video/README.md`, and add all nine names to the `video` recipe's default list in the `justfile`. Verify with `just docs-build` and `just --dry-run video`.

## 9. Checks

- [x] 9.1 Run `just check`, `pnpm --dir app/ui build:web`, `just docs-build` and `mise exec node@24.22.2 -- openspec validate how-to-videos --strict`. Verify that all pass, and that `docs/site/public/video` holds all nine videos and posters.
- [ ] 9.2 Fill in the PR's Documentation checklist: the docs pages and README rows for each video. Verify by reading the PR description.

## Workflow follow-up

- Merge after `documentation-rule`, or rebase on it: this change edits the README Videos table and the docs sections that change adds.
- Archive after review. #102's "Done when" is then met; propose closing it to the user, who decides.
