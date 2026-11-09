# Spec Delta

## Purpose

Defines what SpecDeck's user documentation (the README overview, the documentation site and the how-to videos) must cover for each user-facing feature, and how the project keeps it from falling behind as features are added.

## ADDED Requirements

### Requirement: User-facing features
A feature SHALL count as user-facing when a host, a viewer in the app, a viewer in the web viewer, or someone who runs their own sharing service can see or use it. A spec that only describes internal behaviour or the documentation itself, such as `security-documentation`, SHALL NOT need a README mention or a docs page of its own. The specs of the sharing service (`sharing-service-limits`, `sharing-service-logging`, `signalling-image`) SHALL be covered together by the self-hosting pages.

#### Scenario: Internal spec
- **WHEN** a change only touches `security-documentation`
- **THEN** its PR says it is not user-facing, and no README line or docs page is required for it

#### Scenario: Self-hosting spec
- **WHEN** a change adds a sharing-service limit
- **THEN** the self-hosting pages document it, and the README's single line on running your own sharing service stays enough

### Requirement: README mentions every user-facing feature
The README's overview SHALL mention every user-facing feature in at most one line each, and each mention SHALL link to the docs page or section that explains it.

#### Scenario: Reader scans the README
- **WHEN** someone reads the README overview
- **THEN** they see each feature, such as plan documents, viewer notifications and app updates, with a link to its documentation

#### Scenario: Link into the docs
- **WHEN** a README mention links to the documentation site
- **THEN** the page and anchor it names exist in the built site

### Requirement: Docs explain every user-facing feature
The documentation site SHALL have a page or section for every user-facing feature that says what it does, who can use it (host, viewer, web viewer, or someone running the service), and how, including its shortcuts and settings. Every page SHALL be listed in the sidebar.

#### Scenario: Viewer looks up prompt suggestions
- **WHEN** a viewer wants to suggest a prompt
- **THEN** the docs say where to write it, that only the host and the viewer see it, the length and rate limits, and how the viewer learns whether it was sent, edited or declined

#### Scenario: Host looks up prompt suggestions
- **WHEN** the host wants to act on a suggestion
- **THEN** the docs say where suggestions wait, how to send, edit or decline one, and that nothing reaches the agent until the host sends it

#### Scenario: Feature differs in the web viewer
- **WHEN** a feature works differently or not at all in the web viewer, for example a shortcut
- **THEN** its docs section says so

### Requirement: Videos are listed and embedded
Every how-to video in `docs/video` SHALL be embedded on the docs page of the feature it shows and listed in the README's Videos table. A user-facing change without a new or updated video SHALL say in its PR why none is needed.

#### Scenario: New video
- **WHEN** a video is added to `docs/video`
- **THEN** it is embedded on its docs page and has a row in the README's Videos table

#### Scenario: No video needed
- **WHEN** a user-facing change adds no video
- **THEN** its PR states why, such as "a setting, shown well by the docs table"

### Requirement: Pull requests ask for documentation
Every pull request SHALL be opened with a *Documentation* checklist covering the README mention, the docs page or section, the video or the reason for none, and a passing docs build, or a statement that the change is not user-facing.

#### Scenario: Contributor opens a PR
- **WHEN** someone opens a pull request on GitHub
- **THEN** its description starts from a template with the Documentation checklist

### Requirement: OpenSpec changes plan documentation
Every OpenSpec change SHALL plan its documentation from the start: its proposal's Impact SHALL name the README lines, docs pages and video it adds or changes, or say why it is not user-facing. Its tasks SHALL include the documentation in the task group of the feature it documents, each with a way to verify it.

#### Scenario: New change
- **WHEN** an agent or a person creates the proposal and tasks of a new OpenSpec change
- **THEN** the instructions OpenSpec gives for those artifacts ask for the documentation

### Requirement: The docs build checks links
Building the documentation with the project's docs recipe and in CI SHALL fail when a page links to a page or anchor of the site that does not exist, when a page is not in the sidebar, or when a README link into the documentation site does not resolve. The check SHALL run offline, without dependencies beyond the site's own.

#### Scenario: Broken anchor
- **WHEN** a docs page links to `/specdeck/guide/follow/#no-such-section`
- **THEN** the docs build fails and names the page and the link

#### Scenario: Page not in the sidebar
- **WHEN** a new page is added under `docs/site/src/content/docs` but not to the sidebar
- **THEN** the docs build fails and names the page

#### Scenario: README link
- **WHEN** the README links to `https://nickhartjes.github.io/specdeck/guide/plans/` and no such page exists
- **THEN** the docs build fails and names the README link

#### Scenario: Pull request changes only the README
- **WHEN** a pull request changes `README.md` and nothing under `docs/site`
- **THEN** the docs workflow still runs the check
