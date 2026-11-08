# Proposal

## Why

Step 5 of the security plan agreed on 2026-11-03, and todo.txt's "detailed security information in documentation". People who want to use SpecDeck at work, and the security officers who must approve it, have no single place that says what SpecDeck does with their data, what the sharing service can see, and what the risks are. What exists is spread over the user guide and the self-host pages, and part of it claims more than the code delivers:

- `self-host/overview.md` says the server "never sees invite codes" and "cannot put itself in between", and that "every frame is signed by the host". The room id and the key that authenticates the connection setup are both derived from the 60-bit code with HKDF and a fixed salt. So whoever runs the service could try every code offline against a room id. Once they found the code, they could sit in the middle. Frame signatures add nothing beyond DTLS, because the key comes over the same channel.
- The same page lists what the service sees but leaves out the connection setup itself (SDP and ICE candidates), which from the app can include LAN addresses.

The fixes the plan put first (limit-viewer-data, server-limits) are merged. The PAKE (`harden-session-protocol`) and the app hardening are not. The docs now describe what is true today, and list what is not done yet.

## What Changes

- **A new *Security and privacy* section in the docs site**, written for a developer and their security officer, in plain words:
  - **Overview**: what SpecDeck is from a security point of view, the data-flow diagram, who runs the built-in service (the project's maintainer, Nick Hartjes, best effort, no promises), and when to self-host.
  - **Your data**:
    - where session content lives and how it travels (host to viewer over WebRTC/DTLS, never through the service);
    - what is kept on the host's computer while a session runs, and what is deleted after it;
    - what viewers can see, as the rule *everything in the terminal is shared; the timeline adds nothing from outside the session folder*;
    - late joiners get the whole history;
    - whole code files appear only under *Code viewers can open*;
    - shared files and links;
    - what reaches the agent.
  - **The sharing service**:
    - how join codes work, and the offline-guessing weakness, stated honestly;
    - a PAKE is planned, with no date;
    - what the service and the relay see and log;
    - who gets relay credentials, the quotas, the addresses the relay never reaches, and what apps show when the service is full;
    - the web viewer's trust in the operator;
    - why self-hosting gives full control, linking to the `self-host/` pages instead of repeating them.
  - **Claude's browser** (macOS):
    - the loopback-only hook endpoint and its `/mcp` route behind the per-session bearer token;
    - the 0600 MCP config, which holds `${SPECDECK_HOOK_TOKEN}`, not the token;
    - the hidden, incognito WebKit window with no IPC, which keeps nothing;
    - Claude Code's own permission mode;
    - screenshots in the timeline and the setting that turns them off.
  - **On your computer**:
    - `settings.json` and the other files the app writes;
    - logs and Export diagnostics;
    - notifications and what they can show;
    - no telemetry;
    - updates (signed, verified, what a check sends);
    - release signing as it is today: no OS code signing yet, a cosign-signed server image, no checksums.
  - **Known limits**: one honest list of everything found, each with what it means for you and a workaround where one exists. See design.md, decision 5.
- **Two hand-made SVG diagrams**, readable in light and dark: the data flow, and Claude's browser. The data-flow diagram also goes in the README.
- **`SECURITY.md`** at the repository root. It links to the docs section and says how to report a vulnerability: GitHub private vulnerability reporting first, `security@hartj.es` as the fallback.
- **Corrections and links in existing pages:**
  - `self-host/overview.md`: drop the overclaims above and add what the service sees, linking to the security pages;
  - `README.md`, `guide/join.md`, `guide/host.md`: link to the section where it fits.
- **Not in this change**:
  - fixing any of the known limits;
  - the PAKE (`harden-session-protocol`, parked);
  - turning on GitHub private vulnerability reporting, which the maintainer does (`security@hartj.es` already reaches the maintainer through the domain's catch-all);
  - app code of any kind.

## Capabilities

### New Capabilities

- `security-documentation`: what the published security and privacy documentation must state, so that it stays true as the app changes:
  - where data lives and travels;
  - what viewers see;
  - the join code and its weakness;
  - the sharing service and its limits;
  - Claude's browser;
  - local files, updates and signing;
  - the known-limits list;
  - how to report a vulnerability;
  - diagrams that work in both themes.

### Modified Capabilities

None. `session-sharing` keeps its *End-to-end encryption* requirement as the target that `harden-session-protocol` will fully meet. The docs say where today's code falls short of it.

## Impact

- `docs/site/src/content/docs/security/`: new pages.
- `docs/site/astro.config.mjs`: a sidebar group.
- `docs/site/src/assets/` and `docs/` (or `docs/site/public/`): the SVG diagrams.
- `docs/site/src/content/docs/self-host/overview.md`, `guide/join.md`, `guide/host.md`: corrections and links.
- `README.md`: a link and the diagram.
- `SECURITY.md`: new.
- No app, server or deployment code changes. The other workspace (`read-and-estimate`) is not touched.
- Outside the repo, for the maintainer: enable private vulnerability reporting on `nickhartjes/specdeck` before the docs are published.
