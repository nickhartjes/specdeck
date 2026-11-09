# Design

## Context

- The docs site (`docs/site`, Astro Starlight) is built by `just docs-build` (`pnpm install --frozen-lockfile && pnpm build`) and by `.github/workflows/docs.yml`, on pull requests and pushes that touch `docs/site/**`. It is not part of `just check` or `just check-web`: `check-web` covers the web viewer's WebAssembly, page and service, not the docs.
- `astro build` does not fail on a broken internal link or anchor. The sidebar is listed by hand in `astro.config.mjs`, so a new page that is not added there builds but cannot be reached.
- `openspec/config.yaml` has no rules yet; the project uses the built-in `spec-driven` schema of OpenSpec 1.15.
- `.github/` has no PR template. There is no CONTRIBUTING.md.
- Recent changes already end their tasks with a Documentation group (for example `2026-11-09-viewer-diagnostics`). The rule makes that habit required.

## Audit

Done during this proposal, spec by spec against `README.md` (the overview paragraph and *Install*), the docs site and `docs/video`. "Mentioned" means the README names the feature; "linked" means it also links to its docs.

| Spec | README | Docs | Video |
|---|---|---|---|
| agent-browser | Mentioned, not linked | `guide/browser`, `security/claude-browser` | — |
| agent-status | **Missing** | `guide/host` (While you host), `guide/follow` (What the agent is doing), `guide/first-steps` | — |
| app-updates | **Missing** (only the maintainers' *Releases* section) | `install#update`, `guide/settings` | — |
| braindump | Mentioned, not linked | `guide/host#the-braindump`, `guide/follow#share-a-file-or-add-to-the-braindump` | — |
| diagnostics | Mentioned, linked | `guide/troubleshooting#see-what-the-connection-is-doing` | ✅ diagnostics |
| host-requests | Mentioned ("confirm they've read", "estimate"), not linked | `guide/read-and-estimate` | — |
| native-experience | **Missing** ("native app" only; tray, windows, appearance, zoom, keyboard not named) | `guide/settings#the-window`, `guide/keyboard`, `guide/host` (tray, running sessions, several windows) | — |
| plan-documents | **Missing** | `guide/documents` | — |
| session-hosting | Mentioned, not linked | `guide/host`, `guide/first-steps` | ✅ getting-started |
| session-sharing | Mentioned (invite code), not linked | `guide/host`, `guide/join` | ✅ getting-started |
| session-streaming | Mentioned ("viewers see the terminal"), not linked | `guide/follow#joining-late`, `guide/join#when-the-connection-drops` | — |
| session-timeline | Mentioned, not linked | `guide/follow` | ✅ following |
| session-window | **Missing** (layouts, panels, tabs) | `guide/follow#choose-a-layout`, `guide/keyboard` | ✅ following |
| shared-files | Mentioned, not linked | `guide/host#share-files`, `guide/follow` | — |
| shared-links | Mentioned, not linked | `guide/host#share-a-link`, `guide/follow#links` | — |
| viewer-experience | Mentioned in part (scroll and search are not), not linked | `guide/follow` | ✅ following |
| viewer-notifications | **Missing** | `guide/settings#notifications`, `guide/follow#get-called-back` | — |
| viewer-requests | Mentioned ("suggest prompts"), not linked | **Gap:** `guide/host` has only the on/off switch; nothing says how a viewer suggests a prompt or how the host sends, edits or declines one | — |
| web-viewer | Mentioned (*Can't install?*), no docs link | `guide/join#watch-in-a-browser` | — |
| self-hosting (sharing-service-limits, sharing-service-logging, signalling-image) | Mentioned, linked | `self-host/*` (limits and logs in `configuration`, the image in `docker-compose` and `kubernetes`) | — |
| security-documentation | Not user-facing (it specifies the security pages; README *Security and privacy* links them) | `security/*` | n/a |

The issue's table marks session-sharing and viewer-experience with ✅, and the audit agrees: getting-started shows sharing the code, and following shows the viewer's view.

**Gaps:** 6 specs missing from the README, about 12 mentioned without a link, and 1 docs gap (prompt suggestions). All are filled in this change.

### Videos to make (follow-up issues, in priority order)

1. **Taking part as a viewer**: suggesting a prompt and seeing what became of it, sharing a file and a link, adding to the braindump, and the host giving it to the agent. These are the most-used viewer features and have no video.
2. **Joining from the web viewer**: open the host's web link, *Watch in this browser*, the keys that differ. This is the easiest way in for viewers who can't install the app.
3. **Reading together and estimating**: the host asks to read, viewers answer, then a vote.
4. **Plan documents**: the plan card, reading a document formatted, *New* and *Changed* marks, the task count.
5. **Claude's browser (macOS)**: Claude opens the app it builds, with screenshots in the timeline.
6. **Running your own sharing service (Docker Compose)**: lowest, because it is mostly a terminal and a config file, which the docs page already shows step by step.

No video needed: agent-status, app-updates, native-experience, session-streaming, viewer-notifications (each is a status, a setting or a table that the docs show well; notifications can appear briefly in video 1), and the self-hosting specs beyond video 6.

## Goals / Non-Goals

**Goals:**
- The rule is asked for at the two points where work is planned and reviewed: OpenSpec's artifact instructions and the PR description.
- The docs build catches the mistakes the rule cannot: dead links, missing anchors, pages left out of the sidebar, README links that drifted.

**Non-Goals:**
- CI that fails a PR for lacking documentation. Whether a change is user-facing is a judgment for the author and reviewer.
- Checking external links (GitHub, agent websites). They need the network and would make the build flaky.
- Rewriting the docs pages beyond prompt suggestions.

## Decisions

### 1. `openspec/config.yaml` rules, not a project schema

Add `rules.proposal` and `rules.tasks` to `openspec/config.yaml`. OpenSpec passes them to the agent in `openspec instructions proposal|tasks` for every change.

Draft:

```yaml
rules:
  proposal:
    - "Impact names the documentation: the README overview line (linked to its docs page), the docs/site page or section, and the video in docs/video or why none is needed. A change that is not user-facing says so and why."
  tasks:
    - "A feature is not done until it is documented. In the task group of each user-facing feature, add tasks for its README overview line with a link, its docs/site page or section (what it does; who can use it: host, viewer, web viewer; how, with shortcuts and settings; added to the sidebar), and its video or why none is needed. Verify them with `just docs-build`."
```

Alternative: fork `spec-driven` into `openspec/schemas/` with a Documentation section in `templates/tasks.md`. Rejected for now. A fork freezes the schema's instructions at OpenSpec 1.15, so every upgrade would need a merge by hand. The built-in tasks instruction also already says that docs belong in each group, not a final one, and a template section would push the other way. Rules add to the built-in schema without replacing it. If the rules prove too weak, a fork can come later.

### 2. The rule goes in the README's Development section

A short paragraph *Documentation*, after the recipes. It states the rule, points to the PR template and `openspec/config.yaml`, and names `just docs-build`. A CONTRIBUTING.md would be a fourth place to keep in step, for a project whose development guide is already the README.

### 3. The README overview becomes a feature list

Keep the first paragraph as an introduction (host, viewers, invite code, only the host types). Replace the long sentence of features with a short list, one line per feature, each with its docs link:

- Host Claude Code, OpenCode, omp, Codex, Grok, pi or a shell, and see whether the agent is working or waiting (session-hosting, agent-status)
- Invite with a code or a link; viewers join from anywhere, also late, and catch up (session-sharing, session-streaming)
- Follow at your own pace: timeline, search, Overview, Developer and Present layouts (session-timeline, viewer-experience, session-window)
- Read plans and documents formatted (plan-documents)
- Take part: suggest prompts, share files and links, add to the braindump (viewer-requests, shared-files, shared-links, braindump)
- Read together and estimate (host-requests)
- Get called back by notifications (viewer-notifications)
- Claude's browser on macOS (agent-browser)
- Watch in a browser without the app (web-viewer)
- Native on macOS, Windows and Linux: menus, tray, several windows, keyboard and screen readers; updates itself (native-experience, app-updates)
- Run your own sharing service (self-hosting)

The wording is settled during implementation; the grouping is the decision. Grouping keeps the list at about 11 lines instead of 20.

### 4. Prompt suggestions in the docs

- `guide/follow.md`: a new section *Suggest a prompt*, next to *Share a file or add to the braindump*. It covers where to write one, that only the host and you see it, 4 KB, 3 at once and then one every 10 seconds, the states (waiting, sent, sent after the host edited it, declined), and what you see when the host turned suggestions off. It also says whether the web viewer can suggest; it can, because the web bridge implements `suggest`.
- `guide/host.md`: a new section *Prompt suggestions* after *While you host*. It covers where suggestions wait (oldest first, with the name), *Send*, *Edit* then send, *Decline*, that sending types the text as if you typed it (several lines stay one prompt), that suggestions wait while the window is closed, that they last for the session, and that they are off in terminal sessions. Link it from the existing row in the *While you host* table.
- Labels come from `SuggestPanel.tsx`, `SuggestionsInbox.tsx` and where `Workspace.tsx` places them, checked in the demo (`?demo=host`, `?demo=viewer`).

### 5. Link check: `docs/site/scripts/check-links.mjs`, run after `astro build`

A Node script with no dependencies beyond Node's standard library:

- Walk `dist/**/*.html`. For each `<a href>` that starts with `/specdeck/` (or is relative), resolve it to a file in `dist`, either `path/index.html` or a file such as `video/x.mp4`. Check a `#fragment` against the `id` attributes of the target page. Report each failure with source page and href, and exit non-zero.
- Sidebar: move the sidebar array from `astro.config.mjs` into `docs/site/sidebar.mjs`, imported by both the config and the script. Every content page except `index` and `404` must appear in it. This is simpler and less fragile than parsing the rendered sidebar.
- README: collect `https://nickhartjes.github.io/specdeck/…` links from `../../README.md` and check them as above.
- Simple regexes over Starlight's generated HTML are enough here; the output is regular. A parser would be a new dependency.

Wiring: `package.json` gets `"check-links": "node scripts/check-links.mjs"`. `just docs-build` runs `pnpm build && pnpm check-links`. `docs.yml` runs `pnpm check-links` after `pnpm build`, and its `paths` add `README.md`, so a README-only PR is checked.

Alternative: `starlight-links-validator`. Rejected: it is a new npm dependency that cannot be installed here (the public registry is blocked), and it does not check the sidebar or the README.

### 6. PR template

`.github/pull_request_template.md`: *What and why*, then *Documentation*:

```markdown
## Documentation

A feature is not done until it is documented. Tick what applies, or say why it doesn't.

- [ ] README: the overview mentions it, with a link to its docs page
- [ ] Docs: a page or section in `docs/site` says what it does, who can use it (host, viewer, web viewer) and how, with shortcuts and settings
- [ ] Video: added or updated in `docs/video`, embedded on the docs page and in the README's Videos table, or no video needed because: …
- [ ] `just docs-build` passes
- [ ] Not user-facing (internal, CI, refactor): …
```

## Risks / Trade-offs

- [Rules are advice to an agent, not enforced] → The PR checklist and the reviewer back them up. Revisit with a schema fork if changes keep missing docs.
- [The regex link check could miss links Starlight writes differently, or misread one] → Test the script against the current site, which should pass. Then break a link, an anchor, the sidebar and a README link on purpose to see each fail, as in task 1.
- [Moving the sidebar into its own file] → `astro build` output must be unchanged; compare the sidebar HTML of one page before and after.
- [README list drifts again] → The link check catches dead links but not missing features. The PR checklist is the guard for that.

## Migration Plan

None. The changes are documentation and tooling only. Rollback is reverting the commit.
