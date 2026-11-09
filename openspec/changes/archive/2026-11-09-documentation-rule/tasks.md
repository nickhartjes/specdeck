# Tasks

## 1. Docs link check (`docs/site`)

- [x] 1.1 Move the sidebar array from `docs/site/astro.config.mjs` into `docs/site/sidebar.mjs` and import it in the config (design decision 5). Verify that `just docs-build` succeeds and that the sidebar HTML of `dist/guide/host/index.html` is the same before and after.
- [x] 1.2 Write `docs/site/scripts/check-links.mjs` (design decision 5). It checks internal `<a href>` targets and `#anchors` in `dist`, that every content page except `index` and `404` is in the sidebar, and that the README's `https://nickhartjes.github.io/specdeck/…` links resolve. Add the `check-links` script to `docs/site/package.json`. Verify that it passes on the current site, then fail it on purpose four times and restore after each: a broken anchor in a page, a link to a missing page, a page left out of `sidebar.mjs`, and a bad README link. Check that each failure names the source and the link.
- [x] 1.3 Run the check from `just docs-build` (`pnpm build && pnpm check-links`). In `.github/workflows/docs.yml`, add a `pnpm check-links` step after `pnpm build`, and add `README.md` to both `paths` lists. Verify with `just docs-build` (offline, after `pnpm install --offline` in `docs/site`), and by reading the workflow diff.

## 2. The rule

- [x] 2.1 Add `rules.proposal` and `rules.tasks` to `openspec/config.yaml`, as drafted in design decision 1, keeping the file's comments. Verify that `mise exec node@24.22.2 -- openspec instructions tasks --change documentation-rule --json` and the same for `proposal` return the rules.
- [x] 2.2 Add `.github/pull_request_template.md` with *What and why* and the *Documentation* checklist from design decision 6. Verify by reading the Markdown: every box names something a reviewer can check, and the wording matches the rule in 2.1.
- [x] 2.3 Add a short *Documentation* paragraph to the README's *Development* section (design decision 2). It states the rule and names the PR template, `openspec/config.yaml` and `just docs-build`, and adds `just docs-build` to the recipe list with a comment. Verify that the paragraph matches the PR template and the rules word for word where they overlap.

## 3. README overview

- [x] 3.1 Rewrite the README overview as the feature list in design decision 3. Each line links to its docs page or section from the audit table: the six missing features (agent status, app updates, plan documents, window layouts, viewer notifications, native app) and links for the ones already mentioned. Keep the introduction, the *only the host types* sentence, and the Videos, Security and Install sections as they are. Point *Can't install?* to `guide/join#watch-in-a-browser`. Verify with `just docs-build` (the README links resolve) and by checking each row of the design's audit table against the new list.

## 4. Prompt suggestions in the docs

- [x] 4.1 Check the labels and the flow in the demo (`pnpm --filter @specdeck/ui dev`, `?demo=viewer` and `?demo=host`), and in `SuggestPanel.tsx`, `SuggestionsInbox.tsx` and `Workspace.tsx`: where a viewer writes a suggestion, the states it shows, and where the host's inbox is. Check the same in the web viewer (`app/ui/src/web`). Verify by noting the labels to use in 4.2 and 4.3.
- [x] 4.2 `guide/follow.md`: a section *Suggest a prompt* (design decision 4), with whether the web viewer can suggest. Verify with `just docs-build`, and that every statement matches a requirement in `openspec/specs/viewer-requests/spec.md`.
- [x] 4.3 `guide/host.md`: a section *Prompt suggestions* (design decision 4), linked from the *Let viewers suggest prompts* row of the *While you host* table, and from the new viewer section. Verify with `just docs-build` (the link check covers the new anchors), and that every statement matches the spec.

## 5. Checks

- [x] 5.1 Run `just docs-build` and `mise exec node@24.22.2 -- openspec validate documentation-rule --strict`. Verify that both pass.
- [ ] 5.2 Fill in the PR's own Documentation checklist from the new template: README, docs, "no video needed: the videos are follow-up issues", `just docs-build`. Verify by reading the PR description.

## Workflow follow-up

- Propose to the user (do not edit GitHub issues): the filled-in audit table for #102 from design.md, and one follow-up issue per video in the design's priority order.
- Archive the change after review; this adds `openspec/specs/user-documentation`.
- The other parts of #102 (the videos) stay open after this change merges.
