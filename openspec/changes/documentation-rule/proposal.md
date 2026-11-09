# Proposal

## Why

SpecDeck keeps gaining features, but whether one gets explained to the people who use it depends on whoever builds it remembering to (#102). An audit of `openspec/specs` against the README and `docs/site` shows that the docs site is nearly complete, but the README links to the docs for only one feature and does not mention six, and the docs never say how a viewer suggests a prompt or what the host does with a suggestion. Nothing in the PR or OpenSpec workflow asks for documentation, so the gaps will grow again.

## What Changes

- **The rule:** a feature is not done until it is documented. Every user-facing feature gets a README mention with a link to its docs page, a docs page or section that says what it does, who can use it (host, viewer, web viewer) and how, and either a video or a stated reason why none is needed.
  - A PR template (`.github/pull_request_template.md`) with a *Documentation* checklist.
  - `openspec/config.yaml` rules, so that every change's proposal names its documentation and its tasks plan it from the start.
  - The rule in the README's *Development* section.
- **The gaps the audit found**, filled in this change:
  - The README overview becomes a short feature list. Each item links to its docs page and covers the features not mentioned today: agent status, app updates, plan documents, window layouts, viewer notifications and the native app (tray, windows, keyboard).
  - Prompt suggestions in the docs: how a viewer suggests a prompt, and how the host sends, edits or declines one.
- **A docs link check:** a dependency-free script over the built site. It fails on a broken internal link or anchor, on a page missing from the sidebar, and on a README link into the docs site that does not resolve. `just docs-build` and the Docs workflow run it.
- **Out of scope:** the videos themselves. The change lists which videos to make, in priority order, as proposed follow-up issues. GitHub issues are not edited by this change.

## Capabilities

### New Capabilities
- `user-documentation`: what SpecDeck's user documentation (the README overview, the docs site and the videos) must cover for each user-facing feature, and how the project keeps it from falling behind: the PR checklist, the OpenSpec rules and the docs link check.

### Modified Capabilities
<!-- None: viewer-requests and the other feature specs keep their requirements; only their documentation changes. -->

## Impact

- New: `.github/pull_request_template.md`, `docs/site/scripts/check-links.mjs` (name to confirm in design), `openspec/specs/user-documentation` (on archive).
- Changed: `openspec/config.yaml` (rules for proposal and tasks), `README.md` (overview and Development), `docs/site/src/content/docs/guide/follow.md` and `guide/host.md` (prompt suggestions), `docs/site/package.json` and `justfile` (`docs-build` runs the check), `.github/workflows/docs.yml` (runs the check, and also runs on README changes).
- No app, server or protocol code changes. No new npm dependencies.
- Video: none in this change (out of scope); see design for the proposed list.
