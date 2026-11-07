# Spec Delta

## ADDED Requirements

### Requirement: Screenshots in the steps
A step with a screenshot SHALL show a thumbnail of it, with the page's address when known. In plain words, a run of browser calls SHALL show the thumbnail of its latest screenshot. With every step, each tool call SHALL show its own. A screenshot that was left out SHALL show why in place of the thumbnail.

#### Scenario: Plain words
- **WHEN** Claude opens a page, clicks a button and takes a screenshot in its browser
- **THEN** the step "Used its browser: opened localhost:5173/login, clicked on the page, took a screenshot" shows a thumbnail of the screenshot and the address

#### Scenario: Two screenshots in one run
- **WHEN** a run of browser calls takes two screenshots
- **THEN** the step in plain words shows the second one, and with every step each screenshot call shows its own

#### Scenario: Screenshots turned off
- **WHEN** Claude takes a screenshot in a session where the host turned screenshots off
- **THEN** the step says that a screenshot was taken and that screenshots aren't shown in this session

#### Scenario: Web viewer
- **WHEN** a viewer in the web viewer follows a session in which Claude takes a screenshot
- **THEN** they see the same thumbnail as viewers in the app
