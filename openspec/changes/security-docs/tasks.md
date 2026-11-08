# Tasks

Run pnpm through `mise exec node@24.22.2 -- …`. Install with `pnpm install --offline` in `docs/site`, because the public registry is blocked. Change only docs, `README.md` and `SECURITY.md`: no app, server or deployment code.

## 1. Site setup and diagrams

- [x] 1.1 Install the docs site's dependencies offline and check that the site builds before any change. Verify that `pnpm build` in `docs/site` succeeds. If the offline install fails, stop and tell the maintainer.
- [x] 1.2 Add the **Security and privacy** sidebar group to `docs/site/astro.config.mjs`, between *Use SpecDeck* and *Run your own server*. List the six pages in the order of design decision 1. Create each page with its frontmatter and a one-line placeholder. Verify that `pnpm build` succeeds and the group shows in `astro preview`.
- [x] 1.3 Draw the data-flow diagram (design decisions 2 and 3) as `docs/site/src/assets/security/data-flow.svg`. Use `currentColor` and Starlight colour variables with fallbacks, and give it a `<title>` and `role="img"`. Show it on a test `.mdx` page through an Astro import, and verify that `pnpm build` succeeds. Verify in `astro preview` that it is readable with the theme set to Light and to Dark. If the import does not work, use the `[data-theme]` `<img>` pair fallback and record that in design.md.
- [x] 1.4 Draw the Claude's-browser diagram the same way, as `docs/site/src/assets/security/claude-browser.svg`, and verify it in both themes as in 1.3.
- [x] 1.5 Export the data-flow diagram with fixed colours as `docs/diagrams/data-flow-light.svg` and `docs/diagrams/data-flow-dark.svg`, each with a comment naming its source. Verify that both open in a browser with readable text against a white and a `#0d1117` background.

## 2. Security pages

Each page states only facts from design.md's table or facts checked in the code while writing. Re-check a fact against the code if a merge since 2026-11-08 may have changed it. Write in plain words, as in the user guide.

- [x] 2.1 Write `security/overview.md`, made `.mdx` for the diagram:
  - a few bullets on what SpecDeck does with your data;
  - the data-flow diagram with a text alternative;
  - who runs the built-in service and where: the project's maintainer, Nick Hartjes, best effort, with no uptime or support promise, on a Hetzner server in Falkenstein, Germany, with the server reached through a Cloudflare Tunnel, and what Cloudflare can see (design, Open Questions);
  - when to self-host;
  - links to every security page and to `SECURITY.md`.

  Verify against the spec requirements *A security section in the docs* and *The sharing service described* (Operator named), and that `pnpm build` passes.
- [x] 2.2 Write `security/data.md`:
  - session content over DTLS, never through the service;
  - the files kept on the host while a session runs, and their deletion;
  - the rule *everything in the terminal is shared; the timeline adds nothing from outside the session folder*;
  - late joiners;
  - *Code viewers can open*;
  - shared files and links;
  - what reaches the agent.

  Verify that the page never says files outside the session folder stay private, that it meets *Where session data lives and travels* and *What viewers can see*, and that `pnpm build` passes.
- [x] 2.3 Ask the maintainer how long the built-in service keeps coturn's logs (design, Open Questions). Then write `security/sharing-service.md`:
  - join codes and their weakness, worded as in design decision 4;
  - what the service and the relay see and log;
  - relay credentials, limits and the busy messages;
  - the web viewer's trust in the operator, linking to *The web viewer's code*;
  - self-hosting, linking to the `self-host/` pages without repeating their setup.

  Verify that frame signatures are not presented as a separate layer, that the PAKE is described as planned with no date, that the page meets *Join codes and their weakness* and *The sharing service described*, and that `pnpm build` passes.
- [x] 2.4 Write `security/claude-browser.mdx` with the second diagram:
  - the loopback endpoint and its `/hooks`, `/status` and `/mcp` routes behind the per-session token;
  - `SPECDECK_HOOK_TOKEN`, and the 0600 MCP config holding `${SPECDECK_HOOK_TOKEN}`, not the token;
  - the hidden incognito WebKit window with no IPC;
  - nothing kept after the session or a restart;
  - Claude Code's permission mode;
  - screenshots and *Show screenshots in the timeline*.

  Link to `guide/browser.md` for use. Verify against *Claude's browser described*, and that `pnpm build` passes.
- [x] 2.5 Write `security/your-computer.md`:
  - `settings.json`'s location per OS, its fields and its permissions, and the other files from the table;
  - logs and Export diagnostics;
  - no telemetry;
  - what notifications can show;
  - updates (minisign, GitHub only, what a check sends, the off switch, `.deb` and Nix);
  - release signing today, linking to `install.md` and the cosign check.

  Verify against *The app on your computer*, and that `pnpm build` passes.
- [x] 2.6 Write `security/known-limits.md` with the list from design decision 5. Each item gets what it means and a workaround where one exists, and the page says it reflects the latest release. Verify that every item has a source in design.md's table or in a file named in the item, that it meets *Known limits listed*, and that `pnpm build` passes.

## 3. Existing pages and SECURITY.md

- [x] 3.1 Correct `self-host/overview.md` as in design decision 6:
  - drop "never sees invite codes", "cannot put itself in between" and "every frame is signed";
  - add connection setup and network addresses to what the service sees;
  - link to `security/sharing-service.md`.

  Verify with a search of `docs/site/src/content/docs` for those phrases, which finds none, and that `pnpm build` passes.
- [x] 3.2 Add the links from design decision 6 to `guide/host.md`, `guide/join.md` and `guide/browser.md`. Verify that `pnpm build` passes and that each link resolves in `astro preview`.
- [x] 3.3 Write `SECURITY.md` as in design decision 7, with GitHub private vulnerability reporting first and `security@hartj.es` second. Verify that its docs links point at the published paths (`https://nickhartjes.github.io/specdeck/security/…`).
- [x] 3.4 Add the *Security and privacy* line and the data-flow `<picture>` (`prefers-color-scheme`, with the light file as the `<img>` fallback, plus alt text) to `README.md`. Verify by previewing the README on GitHub, or with `gh markdown-preview` if available, in light and dark.

## 4. Integration checks

- [x] 4.1 Run `pnpm build` in `docs/site`. Check every internal link in the new and changed pages in `astro preview`, and switch the theme to check both diagrams. Verify that the build succeeds with no broken links and both diagrams are readable in both themes.
- [x] 4.2 Read the whole section against the `security-documentation` delta, scenario by scenario, and record any gap. Verify with `openspec validate security-docs --strict`.
- [x] 4.3 Show the maintainer the pages and the known-limits list, and wait for their review. Do not commit or push until they ask.

## Workflow follow-up

- Before merging, the maintainer enables private vulnerability reporting on `nickhartjes/specdeck`. `security@hartj.es` already works through the catch-all.
- The changes that fix a known limit, such as `harden-session-protocol` and the app hardening, update `security/known-limits.md` and the affected page.
- Archive after merge.
