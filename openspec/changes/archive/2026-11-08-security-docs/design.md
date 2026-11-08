# Design

## Context

See proposal.md for the motivation, and the `security-documentation` delta for what the docs must state.

The docs site is Starlight (`docs/site`, Astro 7, Starlight 0.42), published to GitHub Pages under `/specdeck`. Its pages are Markdown, except `index.mdx`. The sidebar is listed by hand in `astro.config.mjs`. `custom.css` holds one rule for videos. No page has a diagram yet: `self-host/overview.md` draws the data flow in ASCII. The other docs already cover security in a few places, which the new section must agree with:

- `guide/host.md`, *What viewers see*: the right rule, already phrased as "what the agent prints is still shared".
- `guide/join.md`: the web viewer, and joining through another service.
- `guide/browser.md`: Claude's browser, including the loopback endpoint and the screenshots setting.
- `guide/settings.md` and `install.md`: says releases are not signed yet.
- `self-host/overview.md`: what the service sees, overclaimed (see proposal), and the cosign check.

### The facts the pages rest on

Checked in the code on 2026-11-08 (app 1.6.0 in this tree). Each page states only these, or facts checked the same way while writing.

| Topic | Fact | Where |
|---|---|---|
| Join code | 12 Crockford base32 characters, 60 bits, from the OS random source | `crates/protocol/src/invite.rs` |
| | HKDF-SHA256, fixed salt `specdeck invite v1`: `room id` (16 bytes, sent to the server) and `auth key` (32 bytes, never sent) | `invite.rs:67-74` |
| | Offer and answer SDP carry an HMAC-SHA256 under the auth key; ICE candidates do not (DTLS fingerprint in the SDP covers them) | `signal_auth.rs`, `signalling.rs:68-73` |
| | No expiry: the code works until a new code or the end of the session. Removing a viewer makes a new code | `sharing.rs:1416-1439` |
| | No host approval: anyone with the code joins, up to the viewer limit | `sharing.rs:240-243` |
| | Server: 10 failed joins per network (IPv4 address or IPv6 /64) per minute, 60 joins per room per minute | `crates/server/src/lib.rs:93-95,574-586` |
| | Returning viewers use a random 32-byte resume token and a random rejoin room | `sharing.rs:612-621,856-860` |
| Frames | Ed25519 per session, public key sent over the same DTLS channel. Adds nothing against the service | `frames.rs`, `control.rs:102-106` |
| Server | Memory only, one instance. Sees client IPs, room ids, SDP and ICE candidates (from the app, LAN addresses too). Logs only startup and "limit reached", without addresses | `lib.rs:1-3,459-471` |
| TURN | coturn REST credentials, 12 h, only for room members, one per connection, not bound to a room. coturn logs to stdout, including client and peer addresses | `turn.rs`, `lib.rs:163-171`, `compose.yaml:51` |
| | Quotas, 20 Mbit/s per allocation, denied ranges: the `sharing-service-limits` spec | `compose.yaml`, `coturn.yaml` |
| STUN | None, and no third-party servers. Only TURN from the service | `crates/session/src/main.rs:38` |
| Web viewer | Served by the server, strict CSP, code in the `#` and never sent | `crates/server/src/web.rs`, `web-viewer` spec |
| Host disk | Full terminal output in `<cache>/sessions/<id>/session-*.log` (0600), deleted when the session ends. Git snapshots with file contents in a 0700 temp folder. Hook and MCP configs (0600, no token). Shared files in memory only | `output_log.rs`, `shell_changes.rs:183-189`, `hooks.rs` |
| Settings | `settings.json` in the app config folder: username, recent folders, preferences, sharing service. No secrets. Default umask, not 0600 | `settings.rs:22-53,176-184` |
| Logs | Folder 0700, files default umask. Enums and numbers only, 7 days, 50 MB. Export only to a place the user picks, never uploaded | `crates/diag`, `diagnostics` spec |
| Telemetry | None | `diagnostics` design, non-goal |
| Notifications | Host: viewer display names, and "waiting" with Bash command, path or URL (up to 200 characters). Viewer: plan and file names | `timeline.rs:304-391`, `viewerNotices.ts` |
| Hook endpoint | `127.0.0.1`, random port, 32-byte token in `SPECDECK_HOOK_TOKEN`, checked on `/hooks`, `/status`, `/mcp` | `hooks.rs:154,183-188,272-281` |
| Claude's browser | Hidden, `incognito` (non-persistent WebKit data store), label `browser-*` named by no capability, so no IPC. Closed with the session | `browser/webkit.rs:126-161`, `browser/mod.rs:71-88` |
| Updates | minisign signature checked before install, GitHub only, every 6 h, off switch. Not for `.deb` or Nix | `updates.rs`, `app-updates` spec |
| Signing | macOS ad-hoc, Windows and Linux installers unsigned (certificates not set). Server image: keyless cosign. No checksums, SBOM or provenance. Actions pinned by tag | `release.yml`, `release-and-verification` 1.3 |
| App window | Markdown links navigate the window itself (no `on_navigation` guard). Tauri CSP `default-src 'self'`. No shell or fs plugin | `Markdown.tsx:20-26`, `lib.rs:1750-1771`, `tauri.conf.json:15` |
| | Export diagnostics writes to any path the window passes | `lib.rs:435-441` |
| | A `ws://` sharing service is accepted and marked not encrypted | `session-sharing` spec, *Sharing service check* |
| Session files | `sessions/<id>` is created with default permissions and never cleaned up after a killed session process | `client.rs:481-482`, `output_log.rs`, `shell_changes.rs` (removed only on drop) |
| Shared links | `https`, or `http` not to the sharer's own machine. No userinfo. Private LAN ranges allowed | `crates/protocol/src/links.rs:48-87` |
| Compose relay | coturn runs with host networking and without `--no-cli`; k8s has `--no-cli` | `deploy/compose/compose.yaml:44-62`, `deploy/k8s/base/coturn.yaml:33` |
| CI | Third-party actions pinned by tag, not commit SHA | `.github/workflows/release.yml` |
| Built-in service | Hetzner, Falkenstein. Server and web viewer through a Cloudflare Tunnel; TURN direct | Maintainer, 2026-11-08; DNS |

## Goals / Non-Goals

**Goals:**

- A security officer can answer "where does our code go, who can see it, what can go wrong" from the section alone, and knows what to check (signatures, self-hosting).
- Every claim can be traced to code or a spec (the table above), so later changes can find what to update.

**Non-Goals:**

- A formal threat model, certification language, or a comparison with other tools.
- Fixing anything on the known-limits list.
- Repeating the self-host setup or the user guide's how-to steps. The security pages link to them.

## Decisions

### 1. Six pages under `security/`

| Page | Holds |
|---|---|
| `security/overview.md` | Summary in a few bullets, the data-flow diagram, who runs the built-in service, when to self-host, links to the rest and to `SECURITY.md` |
| `security/data.md` | Where data lives and travels, what is kept on the host while a session runs, what viewers see, late joiners, *Code viewers can open*, shared files and links, what reaches the agent |
| `security/sharing-service.md` | Join codes and their weakness, what the service and relay see and log, relay credentials, limits, busy messages, the web viewer's trust, self-hosting |
| `security/claude-browser.md` | The endpoint, token, MCP config, hidden private window, permissions, screenshots. Includes the second diagram |
| `security/your-computer.md` | Files on disk, logs and diagnostics, no telemetry, notifications, updates, release signing |
| `security/known-limits.md` | The list (decision 5) |

The sidebar gets a group, **Security and privacy**, between *Use SpecDeck* and *Run your own server*, in the order above.

- **Alternative: one long page.** Easier to keep in sync, but a security officer needs to point to "the sharing service" on its own, and a long page buries the limits.
- **Alternative: the four pages the brief suggested.** Local storage, updates and signing then has no home.

### 2. Diagrams: one SVG per diagram for the site, `<picture>` for the README

- **On the site**, each diagram is a hand-written SVG in `docs/site/src/assets/security/`, drawn with `currentColor` for lines and text, and with Starlight's colour variables (`var(--sl-color-accent, …)`, with fallbacks) for highlights. The page that shows it is `.mdx` and imports the SVG as an Astro component. That inlines it, so it follows the site's own light/dark toggle, not only the OS setting. Each diagram has a `<title>` and `role="img"`, and the paragraph after it says the same in words.
  - **Alternative: `<img>` pairs toggled by `[data-theme]` in `custom.css`.** This works without the import, but means two files per diagram. It is the fallback if the build cannot import SVGs.
  - **Alternative: a `prefers-color-scheme` media query inside the SVG.** Rejected: it ignores Starlight's theme toggle.
- **In the README**, GitHub shows SVGs as `<img>`, where `currentColor` is black. The data-flow diagram gets two exported files, `docs/diagrams/data-flow-light.svg` and `-dark.svg`, under a `<picture>` with `prefers-color-scheme`, as GitHub recommends. Both are the site SVG with the colours written in. A comment in each names the source file to copy from.

### 3. What the diagrams show

- **Data flow.**
  - The host app and the viewer apps and browsers.
  - The session content path, direct or through the TURN relay, marked *encrypted (DTLS): the relay cannot read it*.
  - The sharing service, with only "who is in which room, connection setup" going through it, marked *sees IP addresses, room ids, connection setup*.
  - On the host side, a box for the agent, the session folder, and the temporary files that exist only while the session runs.
- **Claude's browser.**
  - Claude Code on the host.
  - The `127.0.0.1` endpoint with `/hooks` and `/mcp`, and *token required*.
  - SpecDeck's session process.
  - A hidden, private WebKit window, marked *no app functions, nothing kept*.
  - The web pages it opens.
  - Screenshots going into the shared timeline.

### 4. How the join-code weakness is written

Plainly, with the reasoning a security officer needs, and no hard time estimate, because cost depends on hardware:

- The service sees a room id derived from the code. Someone who controls the service can test guesses against it offline, against every room at once, and the server's join limits do not apply to that. The code has 60 bits: a large search, but a fast hash, so it is within reach of an operator with serious computing power, not of a casual attacker.
- What a found code gives: while the session runs and the code is still valid, they can put themselves between the host and **new** viewers who join through that service, and read what those viewers get. It does not decrypt traffic of viewers already connected, nor any recorded traffic, because DTLS keys are made fresh per connection.
- What a team can do today:
  - self-host, so the operator is the team;
  - make a new code once everyone has joined;
  - end sessions when done.
- What is planned: `harden-session-protocol` replaces the derivation with a PAKE, so a guess needs the host's participation and can be tried only once. Planned, no date.

### 5. The known-limits list

Each item has a short title, what it means for the reader, and a workaround if one exists. Grouped, most important first:

- **Join codes**
  - The offline guessing above.
  - No host approval of viewers.
  - The code lives until changed or the session ends.
- **The sharing service**
  - It sees IP addresses, including LAN addresses in the app's connection setup.
  - coturn logs client and peer addresses.
  - Relay credentials last 12 h and are not tied to a room.
  - The web viewer trusts the operator for its code.
  - The Compose relay runs without `--no-cli`.
- **The app**
  - A Markdown link opens inside the app window instead of the browser.
  - `settings.json`, the session folder and log files use default permissions, not owner-only.
  - A session process that is killed (crash, power loss, `kill -9`) leaves its terminal output and snapshots in the cache folder's `sessions/`; nothing removes them later (`client.rs` creates `sessions/<id>`, and only the session's own drop deletes the files). Found while writing the pages.
  - The hook token is readable by every command the agent runs.
  - Notifications can show commands, paths and URLs, and display names chosen by viewers.
  - Shared links may point at LAN addresses and plain `http`.
  - The diagnostics export trusts the window for its save path.
  - A sharing service set to `ws://` is allowed, and only marked as not encrypted.
- **Releases**
  - Installers are not OS-signed.
  - No checksums or provenance.
  - CI actions are pinned by tag.
- **By design**
  - Everything in the terminal is shared.
  - Late joiners see everything.
  - Screenshots can show more than the terminal.

The list says it reflects the latest release, and that fixes remove items (spec, *Known limits listed*).

### 6. Corrections to existing pages

- `self-host/overview.md`, *What the service can see*: replace the overclaiming paragraph with two sentences. Content is encrypted between the apps and the service cannot read it. What the service sees and its one known weakness go to a link to `security/sharing-service.md`. Add "connection setup, which includes network addresses" to the *They do see* list. Keep *The web viewer's code* and the cosign check where they are, and link to them from the security page instead of moving them.
- `guide/host.md`, *What viewers see*: one link line to `security/data.md`.
- `guide/join.md`, *Through another sharing service* and *Watch in a browser*: one link each.
- `guide/browser.md`, the *Safety* bullets: one link to `security/claude-browser.md`.
- `README.md`: a *Security and privacy* line linking to the section and `SECURITY.md`, and the data-flow `<picture>`.

### 7. `SECURITY.md`

A short file:

- what it covers (the app, the server, the web viewer, the deployments);
- the latest release only, as supported versions;
- how to report: GitHub's private vulnerability reporting link (`/security/advisories/new`), or `security@hartj.es`;
- not to open public issues;
- what to include;
- that it is a one-person project, answered best effort;
- a link to the docs section.

## Risks / Trade-offs

- **[Docs drift as the code changes]** → Requirement *Claims match the code* plus the facts table. The changes that fix a limit (app hardening, `harden-session-protocol`) get a task to update the list. This change adds no tooling for it.
- **[Publishing unfixed weaknesses]** → The maintainer chose to list everything plainly. The repository is public, so the facts are findable anyway. The list gives workarounds where they exist.
- **[The public service's real configuration differs from `deploy/`]**, for example coturn log retention on `specdeck.hartj.es` → The pages describe what the provided deployments do, and say that the built-in service runs them. See Open Questions.
- **[Astro SVG import not available or misbehaving in this Starlight version]** → Fallback in decision 2. `pnpm build` in `docs/site` decides.
- **[GitHub private reporting is not on yet]** → The maintainer enables it before this is merged (tasks). `security@hartj.es` already works through the domain's catch-all.

## Migration Plan

Docs only. Merge after the maintainer has enabled GitHub private vulnerability reporting. The site deploys with `docs.yml`. Rollback is a revert.

## Open Questions

- **Where the built-in service runs.** Known so far:
  - The maintainer runs it on a Hetzner server in Falkenstein, Germany, managed with their NixOS configuration.
  - `turn.hartj.es` resolves to that server.
  - `specdeck.hartj.es` reaches the server through a Cloudflare Tunnel (confirmed by the maintainer). So Cloudflare terminates TLS for the signalling WebSocket and the web viewer page: it sees what the server sees (IP addresses, room ids, connection setup) and delivers the page's code. It never sees session content, which goes host-to-viewer over DTLS. The TURN relay is reached directly, not through Cloudflare.
- **How long the built-in service keeps logs that hold IP addresses**: coturn's stdout, Docker or journald, and Cloudflare's. The SpecDeck host is not in the published `nickhartjes/nixos-config` (any branch), so this needs the maintainer's configuration. Without an answer, the page says the relay logs addresses and that their retention on the built-in service is not stated.
